use notify_rust::Notification;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::ops::Deref;

use crate::sound::SoundPlayer;

pub trait Notifier {
    fn notify(&self, title: &str, message: &str);
    
    // Default implementation for notification with sound type
    fn notify_with_sound(&self, title: &str, message: &str, _sound_type: NotificationSound) {
        // Default just calls the regular notify method
        self.notify(title, message);
    }
}

// Types of sounds that can be played with notifications
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NotificationSound {
    WorkDone,
    BreakDone,
    Start,
}

// Implement Notifier for Arc<Notifier> to allow Arc wrapping
impl<T: Notifier + ?Sized> Notifier for Arc<T> {
    fn notify(&self, title: &str, message: &str) {
        self.deref().notify(title, message)
    }
    
    fn notify_with_sound(&self, title: &str, message: &str, sound_type: NotificationSound) {
        self.deref().notify_with_sound(title, message, sound_type)
    }
}

// Enhanced notifiers with sound support
pub struct SoundNotifier {
    sound_player: Arc<Mutex<SoundPlayer>>,
    base_notifier: Arc<dyn Notifier + Send + Sync>,
}

impl SoundNotifier {
    pub fn new(
        sound_player: Arc<Mutex<SoundPlayer>>,
        base_notifier: Arc<dyn Notifier + Send + Sync>,
    ) -> Self {
        Self {
            sound_player,
            base_notifier,
        }
    }
}

impl Notifier for SoundNotifier {
    fn notify(&self, title: &str, message: &str) {
        // Forward to base notifier
        self.base_notifier.notify(title, message);
    }
    
    fn notify_with_sound(&self, title: &str, message: &str, sound_type: NotificationSound) {
        // First show visual notification
        self.base_notifier.notify(title, message);
        
        // Then play sound based on the notification type
        if let Ok(player) = self.sound_player.lock() {
            if player.is_enabled() {
                let _ = match sound_type {
                    NotificationSound::WorkDone => player.play_work_done(),
                    NotificationSound::BreakDone => player.play_break_done(),
                    NotificationSound::Start => player.play_start(),
                };
            }
        }
    }
}

// Desktop notification implementation
pub struct DesktopNotifier;

impl Notifier for DesktopNotifier {
    fn notify(&self, title: &str, message: &str) {
        if let Err(e) = Notification::new()
            .summary(title)
            .body(message)
            .timeout(5000) // 5 seconds
            .show() 
        {
            eprintln!("Failed to show desktop notification: {} - Check if your system supports notifications", e);
        }
    }
}

// Terminal notification implementation for systems without desktop notification support
pub struct TerminalNotifier;

impl Notifier for TerminalNotifier {
    fn notify(&self, title: &str, message: &str) {
        println!("\n\x07"); // Bell character
        println!("======================================");
        println!("🔔 {}", title);
        println!("   {}", message);
        println!("======================================");
        // Use a more robust approach to handle potential flush errors
        if let Err(e) = io::stdout().flush() {
            eprintln!("Failed to flush stdout: {}", e);
        }
    }
}

// Detect the best notification system to use
pub fn get_default_notifier() -> Arc<dyn Notifier + Send + Sync> {
    // Try to create a desktop notification, with a timeout to avoid hanging
    match Notification::new().summary("Pomodoro").body("Initializing...").timeout(1000).show() {
        Ok(_) => Arc::new(DesktopNotifier),
        Err(e) => {
            eprintln!("Desktop notifications not available ({}), falling back to terminal", e);
            Arc::new(TerminalNotifier)
        }
    }
}

// Get a notifier with sound support
pub fn get_sound_notifier(sound_enabled: bool) -> Arc<dyn Notifier + Send + Sync> {
    // Get a base notifier first
    let base_notifier = get_default_notifier();
    
    // Get a sound player
    let sound_player = crate::sound::get_default_sound_player(sound_enabled);
    
    // Create a sound notifier
    Arc::new(SoundNotifier::new(sound_player, base_notifier))
}
