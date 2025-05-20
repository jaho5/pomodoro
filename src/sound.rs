use rodio::{Decoder, OutputStream, Sink, Source};
use std::io::{self, Cursor};
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration as StdDuration;
use thiserror::Error;

// Included default sounds as bytes
const WORK_DONE_SOUND: &[u8] = include_bytes!("../sounds/work_done.wav");
const BREAK_DONE_SOUND: &[u8] = include_bytes!("../sounds/break_done.wav");
const START_SOUND: &[u8] = include_bytes!("../sounds/start.wav");

// Minimum sound duration in seconds
const MIN_SOUND_DURATION: u64 = 3;

#[derive(Error, Debug)]
pub enum SoundError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
}

pub struct SoundPlayer {
    enabled: bool,
}

impl SoundPlayer {
    /// Create a new sound player with sounds optionally enabled
    pub fn with_enabled(enabled: bool) -> Self {
        Self { enabled }
    }
    
    /// Check if sounds are enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
    
    /// Play a sound from memory (embedded resources) in a separate thread
    pub fn play_bytes(&self, data: &'static [u8]) -> Result<(), SoundError> {
        if !self.enabled {
            return Ok(());
        }
        
        // Spawn a new thread to play the sound
        thread::spawn(move || {
            // This is done in a separate thread to avoid blocking the main thread
            // and to handle the non-Send OutputStream
            match OutputStream::try_default() {
                Ok((stream, handle)) => {
                    let cursor = Cursor::new(data);
                        if let Ok(source) = Decoder::new(cursor) {
                            let source = source.convert_samples::<f32>();
                            if let Ok(sink) = Sink::try_new(&handle) {
                                // Get sound duration
                                let duration_hint = source.total_duration();
                                
                                // Play the sound
                                sink.append(source);
                                
                                // Calculate how long to wait
                                let min_duration = StdDuration::from_secs(MIN_SOUND_DURATION);
                                
                                // Sleep until the sound ends or minimum duration is reached
                                if let Some(duration) = duration_hint {
                                    if duration < min_duration {
                                        // If sound is shorter than minimum, sleep for minimum
                                        sink.sleep_until_end();
                                        // Sleep additional time to meet minimum duration
                                        let extra_sleep = min_duration.checked_sub(duration).unwrap_or_default();
                                        thread::sleep(extra_sleep);
                                    } else {
                                        // Sound is longer than minimum, just wait for it to finish
                                        sink.sleep_until_end();
                                    }
                                } else {
                                    // Duration unknown, play for at least minimum duration
                                    sink.play();
                                    thread::sleep(min_duration);
                                    sink.stop();
                                }
                            }
                        }
                    // stream is dropped here, releasing the audio device
                    drop(stream);
                }
                Err(_) => {
                    // Failed to get an output stream
                }
            }
        });
        
        Ok(())
    }
    
    /// Play the work done notification sound
    pub fn play_work_done(&self) -> Result<(), SoundError> {
        self.play_bytes(WORK_DONE_SOUND)
    }
    
    /// Play the break done notification sound
    pub fn play_break_done(&self) -> Result<(), SoundError> {
        self.play_bytes(BREAK_DONE_SOUND)
    }
    
    /// Play the start notification sound
    pub fn play_start(&self) -> Result<(), SoundError> {
        self.play_bytes(START_SOUND)
    }
}

/// Get a default sound player based on configuration
pub fn get_default_sound_player(enabled: bool) -> Arc<Mutex<SoundPlayer>> {
    Arc::new(Mutex::new(SoundPlayer::with_enabled(enabled)))
}
