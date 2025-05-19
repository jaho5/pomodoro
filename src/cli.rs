use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Path to the database file
    #[arg(short, long, default_value = "pomodoro.db")]
    pub database: PathBuf,
    
    /// Pomodoro duration in minutes
    #[arg(short = 'p', long, default_value_t = 25)]
    pub pomodoro_minutes: u64,
    
    /// Short break duration in minutes
    #[arg(short = 's', long, default_value_t = 5)]
    pub short_break_minutes: u64,
    
    /// Long break duration in minutes
    #[arg(short = 'l', long, default_value_t = 15)]
    pub long_break_minutes: u64,
    
    /// Number of pomodoros before a long break
    #[arg(short = 'n', long, default_value_t = 4)]
    pub pomodoros_until_long_break: usize,
    
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Start the Pomodoro timer
    Start,
    
    /// Stop the Pomodoro timer
    Stop,
    
    /// Skip to the next Pomodoro or break
    Next,
    
    /// Show statistics of past Pomodoro sessions
    Stats {
        /// Number of sessions to show
        #[arg(short, long, default_value_t = 10)]
        limit: i64,
        
        /// Number of days to show stats for
        #[arg(short, long, default_value_t = 7)]
        days: i64,
        
        /// Display type (sessions, daily, summary, types)
        #[arg(short = 't', long, default_value = "sessions")]
        display: String,
        
        /// Show chart visualization in terminal
        #[arg(short, long, default_value_t = false)]
        chart: bool,
    },
}
