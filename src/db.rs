use rusqlite::{Connection, Result, params};
use chrono::{DateTime, Local};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    
    #[error("Failed to initialize database: {0}")]
    Initialization(String),
}

// Apply allow(dead_code) to the entire struct to silence warnings about unused fields
#[allow(dead_code)]
#[derive(Debug)]
pub struct PomodoroSession {
    // We're keeping id field since it might be used for future functionality
    pub id: Option<i64>,
    pub start_time: DateTime<Local>,
    // We're keeping end_time since it provides useful information for stats
    pub end_time: Option<DateTime<Local>>,
    pub duration_seconds: i64,
    pub completed: bool,
    pub session_type: String, // "work", "short_break", "long_break"
}

pub struct Database {
    conn: std::sync::Mutex<Connection>,
}

impl Database {
    pub fn new(db_path: &str) -> Result<Self, DatabaseError> {
        let conn = Connection::open(db_path)?;
        
        // Initialize the database schema if it doesn't exist
        conn.execute(
            "CREATE TABLE IF NOT EXISTS pomodoro_sessions (
                id INTEGER PRIMARY KEY,
                start_time TEXT NOT NULL,
                end_time TEXT,
                duration_seconds INTEGER NOT NULL,
                completed BOOLEAN NOT NULL,
                session_type TEXT NOT NULL
            )",
            [],
        )?;
        
        Ok(Self { conn: std::sync::Mutex::new(conn) })
    }
    
    pub fn start_session(&self, session_type: &str, duration_seconds: i64) -> Result<i64, DatabaseError> {
        let now = Local::now();
        let conn = self.conn.lock().map_err(|_| DatabaseError::Initialization("Failed to lock database connection".to_string()))?;
        
        conn.execute(
            "INSERT INTO pomodoro_sessions (start_time, duration_seconds, completed, session_type)
             VALUES (?, ?, 0, ?)",
            params![now.to_rfc3339(), duration_seconds, session_type],
        )?;
        
        Ok(conn.last_insert_rowid())
    }
    
    pub fn complete_session(&self, session_id: i64) -> Result<(), DatabaseError> {
        let now = Local::now();
        let conn = self.conn.lock().map_err(|_| DatabaseError::Initialization("Failed to lock database connection".to_string()))?;
        
        conn.execute(
            "UPDATE pomodoro_sessions SET end_time = ?, completed = 1 WHERE id = ?",
            params![now.to_rfc3339(), session_id],
        )?;
        
        Ok(())
    }
    
    // We'll keep this method but mark it as allowed dead code
    // in case we need it in future versions
    #[allow(dead_code)]
    pub fn cancel_session(&self, session_id: i64) -> Result<(), DatabaseError> {
        let now = Local::now();
        let conn = self.conn.lock().map_err(|_| DatabaseError::Initialization("Failed to lock database connection".to_string()))?;
        
        conn.execute(
            "UPDATE pomodoro_sessions SET end_time = ?, completed = 0 WHERE id = ?",
            params![now.to_rfc3339(), session_id],
        )?;
        
        Ok(())
    }
    
    pub fn get_session_stats(&self, limit: i64) -> Result<Vec<PomodoroSession>, DatabaseError> {
        let conn = self.conn.lock().map_err(|_| DatabaseError::Initialization("Failed to lock database connection".to_string()))?;
        
        let mut stmt = conn.prepare(
            "SELECT id, start_time, end_time, duration_seconds, completed, session_type 
             FROM pomodoro_sessions 
             ORDER BY start_time DESC 
             LIMIT ?",
        )?;
        
        let sessions = stmt.query_map(params![limit], |row| {
            let start_time_str: String = row.get(1)?;
            let end_time_str: Option<String> = row.get(2)?;
            
            let start_time = DateTime::parse_from_rfc3339(&start_time_str)
                .map(|dt| dt.with_timezone(&Local))
                .unwrap_or_else(|_| Local::now());
                
            let end_time = match end_time_str {
                Some(time_str) => DateTime::parse_from_rfc3339(&time_str)
                    .map(|dt| Some(dt.with_timezone(&Local)))
                    .unwrap_or(None),
                None => None,
            };
            
            Ok(PomodoroSession {
                id: Some(row.get(0)?),
                start_time,
                end_time,
                duration_seconds: row.get(3)?,
                completed: row.get(4)?,
                session_type: row.get(5)?,
            })
        })?;
        
        let mut result = Vec::new();
        for session in sessions {
            result.push(session?);
        }
        
        Ok(result)
    }
}
