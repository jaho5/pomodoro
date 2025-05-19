use crate::db::{PomodoroSession, StatsDaily, SessionTypeSummary};
use std::io;
use crossterm::{
    style::{Color, Stylize},
    terminal,
};

const MAX_CHART_WIDTH: usize = 60;

/// Display a bar chart of session durations
pub fn display_session_chart(sessions: &[PomodoroSession]) -> io::Result<()> {
    if sessions.is_empty() {
        return Ok(());
    }

    // Only get work sessions for the chart
    let work_sessions: Vec<&PomodoroSession> = sessions.iter()
        .filter(|s| s.session_type == "work")
        .collect();
    
    if work_sessions.is_empty() {
        println!("\nNo work sessions to display in chart.");
        return Ok(());
    }

    // Get terminal width
    let (width, _) = terminal::size()?;
    let chart_width = std::cmp::min(MAX_CHART_WIDTH, width as usize - 20);
    
    // Find the maximum duration for scaling
    let max_duration = work_sessions.iter()
        .map(|s| s.duration_seconds / 60)
        .max()
        .unwrap_or(25);
    
    println!("\nWork Session Durations (minutes):");
    println!("{}", "-".repeat(chart_width + 10));
    
    // Display up to the last 10 sessions in reverse order (most recent first)
    let sessions_to_display = if work_sessions.len() > 10 {
        &work_sessions[0..10]
    } else {
        &work_sessions[..]
    };
    
    for (_, session) in sessions_to_display.iter().enumerate() {
        let minutes = session.duration_seconds / 60;
        let bar_length = ((minutes as f64 / max_duration as f64) * chart_width as f64) as usize;
        let bar = "█".repeat(bar_length);
        
        let color = if session.completed {
            Color::Green
        } else {
            Color::Red
        };
        
        let date_str = session.start_time.format("%m-%d %H:%M").to_string();
        
        print!("{:>8} ", date_str);
        println!("{} {}", bar.with(color), minutes);
    }
    
    println!("{}", "-".repeat(chart_width + 10));
    Ok(())
}

/// Display a bar chart of daily stats
pub fn display_daily_chart(stats: &[StatsDaily]) -> io::Result<()> {
    if stats.is_empty() {
        return Ok(());
    }

    // Prepare data for daily work minutes chart
    let dates: Vec<String> = stats.iter().map(|s| s.date.clone()).collect();
    let minutes: Vec<i64> = stats.iter().map(|s| s.total_work_minutes).collect();
    let max_minutes = *minutes.iter().max().unwrap_or(&60);
    
    // Display work minutes chart
    draw_horizontal_bar_chart(&dates, &minutes, max_minutes, "Daily Work Minutes", Color::Cyan)?;
    
    // Prepare data for session counts chart
    let sessions: Vec<i64> = stats.iter().map(|s| s.work_sessions).collect();
    let max_sessions = *sessions.iter().max().unwrap_or(&10);
    
    // Display sessions chart
    draw_horizontal_bar_chart(&dates, &sessions, max_sessions, "Daily Work Sessions", Color::Yellow)?;
    
    // Prepare data for completion rate chart
    let completion_rates: Vec<i64> = stats.iter()
        .map(|s| (s.completion_rate * 100.0).round() as i64)
        .collect();
    
    // Display completion rate chart
    draw_horizontal_bar_chart(&dates, &completion_rates, 100, "Completion Rates (%)", Color::Green)?;
    
    Ok(())
}

/// Display a bar chart of session type stats
pub fn display_type_chart(stats: &[SessionTypeSummary]) -> io::Result<()> {
    if stats.is_empty() {
        return Ok(());
    }

    // Get terminal width
    let (width, _) = terminal::size()?;
    let chart_width = std::cmp::min(MAX_CHART_WIDTH, width as usize - 20);
    
    // Find maximum values for scaling
    let max_count = stats.iter()
        .map(|s| s.count)
        .max()
        .unwrap_or(10);
    
    let max_minutes = stats.iter()
        .map(|s| s.total_minutes)
        .max()
        .unwrap_or(60);
    
    println!("\nSession Counts by Type:");
    println!("{}", "-".repeat(chart_width + 10));
    
    for stat in stats {
        let bar_length = ((stat.count as f64 / max_count as f64) * chart_width as f64) as usize;
        let bar = "█".repeat(bar_length);
        
        // Different colors for different session types
        let color = match stat.session_type.as_str() {
            "work" => Color::Red,
            "short_break" => Color::Green,
            "long_break" => Color::Blue,
            _ => Color::White,
        };
        
        print!("{:>12} ", stat.session_type);
        println!("{} {}", bar.with(color), stat.count);
    }
    
    println!("{}", "-".repeat(chart_width + 10));
    
    // Now create a chart showing minutes by type
    println!("\nTotal Minutes by Type:");
    println!("{}", "-".repeat(chart_width + 10));
    
    for stat in stats {
        let bar_length = ((stat.total_minutes as f64 / max_minutes as f64) * chart_width as f64) as usize;
        let bar = "█".repeat(bar_length);
        
        // Different colors for different session types
        let color = match stat.session_type.as_str() {
            "work" => Color::Red,
            "short_break" => Color::Green,
            "long_break" => Color::Blue,
            _ => Color::White,
        };
        
        print!("{:>12} ", stat.session_type);
        println!("{} {}", bar.with(color), stat.total_minutes);
    }
    
    println!("{}", "-".repeat(chart_width + 10));
    Ok(())
}

/// Create a horizontal bar chart from a set of data points
pub fn draw_horizontal_bar_chart<T: AsRef<str>>(
    labels: &[T],
    values: &[i64],
    max_value: i64,
    title: &str,
    color: Color,
) -> io::Result<()> {
    if labels.is_empty() || values.is_empty() || labels.len() != values.len() {
        return Ok(());
    }
    
    // Get terminal width
    let (width, _) = terminal::size()?;
    let chart_width = std::cmp::min(MAX_CHART_WIDTH, width as usize - 20);
    
    println!("\n{}:", title);
    println!("{}", "-".repeat(chart_width + 10));
    
    for (_, (label, &value)) in labels.iter().zip(values.iter()).enumerate() {
        let bar_length = ((value as f64 / max_value as f64) * chart_width as f64) as usize;
        let bar = "█".repeat(bar_length);
        
        print!("{:>12} ", label.as_ref());
        println!("{} {}", bar.with(color), value);
    }
    
    println!("{}", "-".repeat(chart_width + 10));
    Ok(())
}
