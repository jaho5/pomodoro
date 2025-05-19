use notify_rust::Notification;
use std::io::{self, Write};
use std::sync::Arc;
use std::ops::Deref;

pub trait Notifier {
    fn notify(&self, title: &str, message: &str);
}

// Implement Notifier for Arc<Notifier> to allow Arc wrapping
impl<T: Notifier + ?Sized> Notifier for Arc<T> {
    fn notify(&self, title: &str, message: &str) {
        self.deref().notify(title, message)
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
            eprintln!("Failed to show notification: {}", e);
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
        io::stdout().flush().unwrap();
    }
}

// Detect the best notification system to use
pub fn get_default_notifier() -> Arc<dyn Notifier + Send + Sync> {
    // Try to create a desktop notification
    match Notification::new().summary("Pomodoro").body("Initializing...").show() {
        Ok(_) => Arc::new(DesktopNotifier),
        Err(_) => Arc::new(TerminalNotifier),
    }
}
