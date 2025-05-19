use chrono::{DateTime, Duration, Local};
use std::sync::{Arc, Mutex};
use thiserror::Error;
use tokio::sync::mpsc;
use tokio::time;

use crate::db::{Database, DatabaseError};
use crate::notification::Notifier;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PomodoroState {
    Idle,
    Work,
    ShortBreak,
    LongBreak,
    Paused,
}

#[derive(Debug, Clone, Copy)]
pub struct PomodoroConfig {
    pub work_duration: Duration,
    pub short_break_duration: Duration,
    pub long_break_duration: Duration,
    pub long_break_after: usize,
}

impl Default for PomodoroConfig {
    fn default() -> Self {
        Self {
            work_duration: Duration::minutes(25),
            short_break_duration: Duration::minutes(5),
            long_break_duration: Duration::minutes(15),
            long_break_after: 4,
        }
    }
}

#[derive(Error, Debug)]
pub enum PomodoroError {
    #[error("Timer already running")]
    AlreadyRunning,
    
    #[error("Timer not running")]
    NotRunning,
    
    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),
}

pub enum PomodoroCommand {
    Start,
    Stop,
    Next,
    Shutdown,
}

pub struct Pomodoro {
    state: PomodoroState,
    prev_state: Option<PomodoroState>,  // To remember state before pausing
    config: PomodoroConfig,
    completed_pomodoros: usize,
    current_session_id: Option<i64>,
    start_time: Option<DateTime<Local>>,
    remaining_seconds: i64,
    database: Arc<Database>,
    notifier: Arc<dyn Notifier + Send + Sync>,
}

impl Pomodoro {
    pub fn new(
        config: PomodoroConfig, 
        database: Arc<Database>,
        notifier: Arc<dyn Notifier + Send + Sync>,
    ) -> Self {
        Self {
            state: PomodoroState::Idle,
            prev_state: None,
            config,
            completed_pomodoros: 0,
            current_session_id: None,
            start_time: None,
            remaining_seconds: 0,
            database,
            notifier,
        }
    }
    
    pub fn get_state(&self) -> PomodoroState {
        self.state
    }
    
    pub fn get_remaining_seconds(&self) -> i64 {
        self.remaining_seconds
    }
    
    pub fn get_completed_pomodoros(&self) -> usize {
        self.completed_pomodoros
    }
    
    pub fn start(&mut self) -> Result<(), PomodoroError> {
        match self.state {
            PomodoroState::Idle => {
                self.transition_to_work()
            },
            PomodoroState::Paused => {
                // Resume from paused state using the saved previous state
                if let Some(prev_state) = self.prev_state {
                    self.state = prev_state;
                    
                    // Calculate elapsed time based on the correct duration for the state we're resuming
                    let duration_seconds = match prev_state {
                        PomodoroState::Work => self.config.work_duration.num_seconds(),
                        PomodoroState::ShortBreak => self.config.short_break_duration.num_seconds(),
                        PomodoroState::LongBreak => self.config.long_break_duration.num_seconds(),
                        _ => 0, // Should never happen
                    };
                    
                    // Set start time to make remaining_seconds correct
                    let elapsed_seconds = duration_seconds - self.remaining_seconds;
                    self.start_time = Some(Local::now() - Duration::seconds(elapsed_seconds));
                    
                    // Clear the previous state
                    self.prev_state = None;
                    
                    Ok(())
                } else {
                    // If we don't have a previous state for some reason, start a work session
                    self.transition_to_work()
                }
            },
            _ => Err(PomodoroError::AlreadyRunning),
        }
    }
    
    fn transition_to_work(&mut self) -> Result<(), PomodoroError> {
        self.state = PomodoroState::Work;
        self.start_time = Some(Local::now());
        self.remaining_seconds = self.config.work_duration.num_seconds();
        
        let session_id = self.database.start_session(
            "work", 
            self.config.work_duration.num_seconds()
        )?;
        
        self.current_session_id = Some(session_id);
        Ok(())
    }
    
    pub fn stop(&mut self) -> Result<(), PomodoroError> {
        if self.state == PomodoroState::Idle {
            return Err(PomodoroError::NotRunning);
        }
        
        // Store the current state before pausing and calculate remaining time
        if self.state != PomodoroState::Paused {
            // Save the current state so we can resume to it later
            self.prev_state = Some(self.state);
            
            // Calculate the remaining time manually instead of calling update()
            let current_time = Local::now();
            let elapsed = match self.start_time {
                Some(start) => current_time.signed_duration_since(start).num_seconds(),
                None => 0,
            };
            
            let duration = match self.state {
                PomodoroState::Work => self.config.work_duration.num_seconds(),
                PomodoroState::ShortBreak => self.config.short_break_duration.num_seconds(),
                PomodoroState::LongBreak => self.config.long_break_duration.num_seconds(),
                _ => 0,
            };
            
            self.remaining_seconds = duration - elapsed;
            if self.remaining_seconds < 0 {
                self.remaining_seconds = 0;
            }
        }
        
        // When pausing, we don't cancel the database session anymore
        // This allows proper resuming
        
        self.state = PomodoroState::Paused;
        Ok(())
    }
    
    pub fn next(&mut self) -> Result<(), PomodoroError> {
        match self.state {
            PomodoroState::Work => {
                // Complete the current work session
                if let Some(session_id) = self.current_session_id.take() {
                    self.database.complete_session(session_id)?;
                }
                
                self.completed_pomodoros += 1;
                
                // Determine which break to take but don't start it automatically
                if self.completed_pomodoros % self.config.long_break_after == 0 {
                    self.state = PomodoroState::Paused;
                    self.prev_state = Some(PomodoroState::LongBreak);
                    self.remaining_seconds = self.config.long_break_duration.num_seconds();
                    self.notifier.notify("Long Break Ready", "Long break is ready!");
                } else {
                    self.state = PomodoroState::Paused;
                    self.prev_state = Some(PomodoroState::ShortBreak);
                    self.remaining_seconds = self.config.short_break_duration.num_seconds();
                    self.notifier.notify("Short Break Ready", "Short break is ready!");
                }
            },
            PomodoroState::ShortBreak | PomodoroState::LongBreak => {
                // Prepare for work session but don't start it automatically
                self.state = PomodoroState::Paused;
                self.prev_state = Some(PomodoroState::Work);
                self.remaining_seconds = self.config.work_duration.num_seconds();
                self.notifier.notify("Work Session Ready", "Work session is ready!");
            },
            PomodoroState::Paused => {
                // If paused, determine what the next state should be
                if let Some(prev_state) = self.prev_state {
                    match prev_state {
                        PomodoroState::Work => {
                            // We were paused in a work session, so next would be a break
                            if let Some(session_id) = self.current_session_id.take() {
                                self.database.complete_session(session_id)?;
                            }
                            
                            self.completed_pomodoros += 1;
                            
                            // Set up the break type but don't start it
                            if self.completed_pomodoros % self.config.long_break_after == 0 {
                                self.prev_state = Some(PomodoroState::LongBreak);
                                self.remaining_seconds = self.config.long_break_duration.num_seconds();
                                self.notifier.notify("Long Break Ready", "Long break is ready - press 's' to start!");
                            } else {
                                self.prev_state = Some(PomodoroState::ShortBreak);
                                self.remaining_seconds = self.config.short_break_duration.num_seconds();
                                self.notifier.notify("Short Break Ready", "Short break is ready - press 's' to start!");
                            }
                        },
                        PomodoroState::ShortBreak | PomodoroState::LongBreak => {
                            // We were paused in a break, so next would be work
                            self.prev_state = Some(PomodoroState::Work);
                            self.remaining_seconds = self.config.work_duration.num_seconds();
                            self.notifier.notify("Work Session Ready", "Work session is ready - press 's' to start!");
                        },
                        _ => {}
                    }
                } else {
                    // If we don't know what state we were in, set up for work session
                    self.prev_state = Some(PomodoroState::Work);
                    self.remaining_seconds = self.config.work_duration.num_seconds();
                    self.notifier.notify("Work Session Ready", "Work session is ready - press 's' to start!");
                }
            },
            PomodoroState::Idle => {
                // From idle, set up for work session but don't start it
                self.state = PomodoroState::Paused;
                self.prev_state = Some(PomodoroState::Work);
                self.remaining_seconds = self.config.work_duration.num_seconds();
                self.notifier.notify("Work Session Ready", "Work session is ready - press 's' to start!");
            },
        }
        
        // Don't set start_time as we're not starting automatically
        Ok(())
    }
    
    pub fn update(&mut self) {
        // Make sure we don't update if we're already paused
        if self.state == PomodoroState::Idle || self.state == PomodoroState::Paused {
            return;
        }
        
        let current_time = Local::now();
        let elapsed = match self.start_time {
            Some(start) => current_time.signed_duration_since(start).num_seconds(),
            None => 0,
        };
        
        let duration = match self.state {
            PomodoroState::Work => self.config.work_duration.num_seconds(),
            PomodoroState::ShortBreak => self.config.short_break_duration.num_seconds(),
            PomodoroState::LongBreak => self.config.long_break_duration.num_seconds(),
            _ => 0,
        };
        
        self.remaining_seconds = duration - elapsed;
        
        // Check if the timer has expired
        if self.remaining_seconds <= 0 {
            match self.state {
                PomodoroState::Work => {
                    // Complete the work session
                    if let Some(session_id) = self.current_session_id.take() {
                        let _ = self.database.complete_session(session_id);
                    }
                    
                    self.completed_pomodoros += 1;
                    
                    // Set up for a break but don't start it automatically
                    self.state = PomodoroState::Paused;
                    if self.completed_pomodoros % self.config.long_break_after == 0 {
                        self.prev_state = Some(PomodoroState::LongBreak);
                        self.remaining_seconds = self.config.long_break_duration.num_seconds();
                        self.notifier.notify("Long Break Ready", "Long break is ready - press 's' to start!");
                    } else {
                        self.prev_state = Some(PomodoroState::ShortBreak);
                        self.remaining_seconds = self.config.short_break_duration.num_seconds();
                        self.notifier.notify("Short Break Ready", "Short break is ready - press 's' to start!");
                    }
                },
                PomodoroState::ShortBreak | PomodoroState::LongBreak => {
                    // Set up for work session but don't start it automatically
                    self.state = PomodoroState::Paused;
                    self.prev_state = Some(PomodoroState::Work);
                    self.remaining_seconds = self.config.work_duration.num_seconds();
                    self.notifier.notify("Work Session Ready", "Work session is ready - press 's' to start!");
                },
                _ => {}
            }
            
            // Don't set start_time as we're not starting automatically
        }
    }
}

pub async fn run_pomodoro_timer(
    pomodoro: Arc<Mutex<Pomodoro>>,
    mut command_rx: mpsc::Receiver<PomodoroCommand>,
) {
    // Create timer intervals - regular and slow for static states
    let mut regular_interval = time::interval(time::Duration::from_secs(1));
    let mut slow_interval = time::interval(time::Duration::from_secs(2)); // Slower interval for static states
    
    // Track if we're in a static state
    let mut was_static = true; // Start assuming static state
    
    loop {
        // Determine which interval to use based on state
        let current_state = {
            let pomodoro_lock = pomodoro.lock().unwrap();
            pomodoro_lock.get_state()
        };
        
        let is_static = current_state == PomodoroState::Paused || current_state == PomodoroState::Idle;
        
        // If state changed between static/active, log it (useful for debugging)
        if is_static != was_static {
            was_static = is_static;
        }
        
        tokio::select! {
            _ = if is_static { slow_interval.tick() } else { regular_interval.tick() } => {
                // Only update the timer if it's not in a static state (paused or idle)
                let mut pomodoro_lock = pomodoro.lock().unwrap();
                
                if !is_static {
                    pomodoro_lock.update();
                }
            }
            
            cmd = command_rx.recv() => {
                match cmd {
                    Some(PomodoroCommand::Start) => {
                        let mut pomodoro = pomodoro.lock().unwrap();
                        let _ = pomodoro.start();
                    }
                    Some(PomodoroCommand::Stop) => {
                        let mut pomodoro = pomodoro.lock().unwrap();
                        let _ = pomodoro.stop();
                    }
                    Some(PomodoroCommand::Next) => {
                        let mut pomodoro = pomodoro.lock().unwrap();
                        let _ = pomodoro.next();
                    }
                    Some(PomodoroCommand::Shutdown) | None => {
                        break;
                    }
                }
            }
        }
    }
}
