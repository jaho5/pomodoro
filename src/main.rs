use std::sync::{Arc, Mutex};
use std::time::Duration as StdDuration;
use std::io::{self, Write};

use chrono::Duration;
use clap::Parser;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    style::{self, Color, Stylize},
    terminal::{self, Clear, ClearType},
};
use tokio::sync::mpsc;

mod cli;
mod db;
mod notification;
mod pomodoro;

use cli::{Args, Command};
use db::Database;
use notification::get_default_notifier;
use pomodoro::{Pomodoro, PomodoroCommand, PomodoroConfig, PomodoroState};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    
    // Initialize database
    let database = Arc::new(Database::new(args.database.to_str().unwrap_or("pomodoro.db"))?);
    
    // Initialize notifier
    let notifier = Arc::new(get_default_notifier());
    
    // Create Pomodoro config
    let config = PomodoroConfig {
        work_duration: Duration::minutes(args.pomodoro_minutes as i64),
        short_break_duration: Duration::minutes(args.short_break_minutes as i64),
        long_break_duration: Duration::minutes(args.long_break_minutes as i64),
        long_break_after: args.pomodoros_until_long_break,
    };
    
    // Create Pomodoro instance
    let pomodoro = Arc::new(Mutex::new(Pomodoro::new(
        config,
        database.clone(),
        notifier,
    )));
    
    // Check if a command was specified
    match args.command {
        Some(Command::Start) => {
            // Start the timer without interactive mode
            let mut pom = pomodoro.lock().unwrap();
            pom.start()?;
            drop(pom);
            
            run_interactive_mode(pomodoro).await?;
        }
        Some(Command::Stop) => {
            let mut pom = pomodoro.lock().unwrap();
            pom.stop()?;
            println!("Pomodoro timer stopped.");
        }
        Some(Command::Next) => {
            let mut pom = pomodoro.lock().unwrap();
            pom.next()?;
            println!("Moved to next Pomodoro/break interval.");
        }
        Some(Command::Stats { limit }) => {
            let sessions = database.get_session_stats(limit)?;
            
            println!("Recent Pomodoro Sessions:");
            println!("------------------------");
            
            if sessions.is_empty() {
                println!("No sessions recorded yet.");
            } else {
                for (i, session) in sessions.iter().enumerate() {
                    let status = if session.completed { "✅ Completed" } else { "❌ Cancelled" };
                    let duration_min = session.duration_seconds / 60;
                    let session_id = session.id.unwrap_or(0);  // Use the ID field
                    let end_time_str = match session.end_time {
                        Some(time) => time.format("%Y-%m-%d %H:%M").to_string(),
                        None => "In progress".to_string(),
                    };
                    
                    println!(
                        "{}. ID: {} - {} ({} min) - Started: {} - Ended: {} - {}",
                        i + 1,
                        session_id,
                        session.session_type,
                        duration_min,
                        session.start_time.format("%Y-%m-%d %H:%M"),
                        end_time_str,
                        status,
                    );
                }
            }
        }
        None => {
            // If no command specified, start the interactive mode
            run_interactive_mode(pomodoro).await?;
        }
    }
    
    Ok(())
}

async fn run_interactive_mode(
    pomodoro: Arc<Mutex<Pomodoro>>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Set up command channel
    let (cmd_tx, cmd_rx) = mpsc::channel(32);
    
    // Spawn the Pomodoro timer task
    let timer_pomodoro = pomodoro.clone();
    let timer_handle = tokio::spawn(async move {
        pomodoro::run_pomodoro_timer(timer_pomodoro, cmd_rx).await;
    });
    
    // Set up terminal
    terminal::enable_raw_mode()?;
    execute!(io::stdout(), cursor::Hide, Clear(ClearType::All))?;
    
    let mut last_state = None;
    let mut last_seconds = None;
    let mut redraw_counter = 0;
    
    // Main loop for interactive mode
    loop {
        // Only redraw at most once per 250ms (about 4 frames per second)
        // to reduce flickering
        let current_state;
        let current_seconds;
        
        {
            let pom = pomodoro.lock().unwrap();
            current_state = pom.get_state();
            current_seconds = pom.get_remaining_seconds();
        }
        
        // Check if we need to redraw - only redraw when:
        // 1. State changes
        // 2. Seconds change (but only if not paused)
        // 3. Or once every 4 cycles (for any other updates)
        // For paused or idle state, we only need to redraw much less frequently
        // since nothing is changing
        let is_static = current_state == PomodoroState::Paused || current_state == PomodoroState::Idle;
        let periodic_refresh = if is_static {
            redraw_counter % 20 == 0  // Much less frequent updates when paused/idle
        } else {
            redraw_counter % 4 == 0   // Regular updates when running
        };
        
        let should_redraw = last_state != Some(current_state) || 
                          (!is_static && last_seconds != Some(current_seconds)) || 
                          periodic_refresh;
                          
        if should_redraw {
            // Draw the UI
            draw_ui(&pomodoro)?;
            last_state = Some(current_state);
            last_seconds = Some(current_seconds);
        }
        
        redraw_counter += 1;
        
        // Poll for keyboard events with timeout (250ms normally, longer when static)
        let poll_timeout = if is_static {
            StdDuration::from_millis(500)  // Longer timeout when paused/idle
        } else {
            StdDuration::from_millis(250)  // Normal timeout when running
        };
        
        if event::poll(poll_timeout)? {
            if let Event::Key(KeyEvent {
                code, modifiers: _, kind, state: _,
            }) = event::read()?
            {
                if kind == event::KeyEventKind::Press {
                    match code {
                        KeyCode::Char('q') | KeyCode::Esc => {
                            // Quit
                            let _ = cmd_tx.send(PomodoroCommand::Shutdown).await;
                            break;
                        }
                        KeyCode::Char('s') => {
                            // Start/resume
                            let _ = cmd_tx.send(PomodoroCommand::Start).await;
                        }
                        KeyCode::Char('p') => {
                            // Pause
                            let _ = cmd_tx.send(PomodoroCommand::Stop).await;
                        }
                        KeyCode::Char('n') => {
                            // Next
                            let _ = cmd_tx.send(PomodoroCommand::Next).await;
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    
    // Wait for the timer task to finish
    let _ = timer_handle.await;
    
    // Clean up terminal
    terminal::disable_raw_mode()?;
    execute!(io::stdout(), cursor::Show)?;
    
    Ok(())
}

fn draw_ui(pomodoro: &Arc<Mutex<Pomodoro>>) -> io::Result<()> {
    let mut stdout = io::stdout();
    
    let (state, remaining_seconds, completed_pomodoros) = {
        let pom = pomodoro.lock().unwrap();
        (
            pom.get_state(),
            pom.get_remaining_seconds(),
            pom.get_completed_pomodoros(),
        )
    };
    
    // Format time
    let minutes = remaining_seconds / 60;
    let seconds = remaining_seconds % 60;
    
    // Get state information
    let (state_text, state_color) = match state {
        PomodoroState::Idle => ("Idle", Color::White),
        PomodoroState::Work => ("Working", Color::Red),
        PomodoroState::ShortBreak => ("Short Break", Color::Green),
        PomodoroState::LongBreak => ("Long Break", Color::Blue),
        PomodoroState::Paused => ("Paused", Color::Yellow),
    };
    
    // Only clear screen once at the beginning of the function
    // to reduce flickering
    execute!(
        stdout,
        cursor::MoveTo(0, 0),
        Clear(ClearType::All)
    )?;
    
    // Draw the header
    execute!(
        stdout,
        cursor::MoveTo(0, 0),
        style::PrintStyledContent(
            "🍅 Pomodoro Timer".bold().with(Color::White)
        )
    )?;
    
    // Draw the state
    execute!(
        stdout,
        cursor::MoveTo(0, 2),
        style::PrintStyledContent(
            format!("State: {}", state_text).with(state_color)
        )
    )?;
    
    // Draw the time remaining
    let time_display = format!("{:02}:{:02}", minutes, seconds);
    execute!(
        stdout,
        cursor::MoveTo(0, 4),
        style::PrintStyledContent(
            format!("Time Remaining: {}", time_display).bold().with(Color::White)
        )
    )?;
    
    // Draw completed pomodoros
    execute!(
        stdout,
        cursor::MoveTo(0, 6),
        style::PrintStyledContent(
            format!("Completed Pomodoros: {}", completed_pomodoros).with(Color::White)
        )
    )?;
    
    // Draw controls
    execute!(
        stdout,
        cursor::MoveTo(0, 8),
        style::PrintStyledContent(
            "Controls:".bold().with(Color::White)
        )
    )?;
    
    execute!(
        stdout,
        cursor::MoveTo(0, 9),
        style::PrintStyledContent(
            " s - Start/Resume".with(Color::White)
        )
    )?;
    
    execute!(
        stdout,
        cursor::MoveTo(0, 10),
        style::PrintStyledContent(
            " p - Pause".with(Color::White)
        )
    )?;
    
    execute!(
        stdout,
        cursor::MoveTo(0, 11),
        style::PrintStyledContent(
            " n - Next".with(Color::White)
        )
    )?;
    
    execute!(
        stdout,
        cursor::MoveTo(0, 12),
        style::PrintStyledContent(
            " q - Quit".with(Color::White)
        )
    )?;
    
    // Make sure to flush the output to display immediately
    stdout.flush()?;
    
    Ok(())
}
