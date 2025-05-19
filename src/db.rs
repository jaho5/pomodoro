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

#[derive(Debug)]
pub struct StatsDaily {
    pub date: String,
    pub work_sessions: i64,
    pub total_work_minutes: i64,
    pub completed_work_sessions: i64,
    pub completion_rate: f64,
}

#[derive(Debug)]
pub struct StatsSummary {
    pub total_work_sessions: i64,
    pub total_work_minutes: i64,
    pub completed_sessions: i64,
    pub completion_rate: f64,
    pub avg_sessions_per_day: f64,
    pub longest_streak_days: i64,
    pub current_streak_days: i64,
}

#[derive(Debug)]
pub struct SessionTypeSummary {
    pub session_type: String,
    pub count: i64,
    pub total_minutes: i64,
    pub completion_rate: f64,
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
    
    pub fn get_daily_stats(&self, days: i64) -> Result<Vec<StatsDaily>, DatabaseError> {
        let conn = self.conn.lock().map_err(|_| DatabaseError::Initialization("Failed to lock database connection".to_string()))?;
        
        // Get stats grouped by day for the last N days
        let mut stmt = conn.prepare(
            "SELECT 
                strftime('%Y-%m-%d', start_time) as day,
                COUNT(*) as total_sessions,
                SUM(CASE WHEN session_type = 'work' THEN 1 ELSE 0 END) as work_sessions,
                CAST(SUM(CASE WHEN session_type = 'work' THEN duration_seconds ELSE 0 END) / 60 AS INTEGER) as work_minutes,
                SUM(CASE WHEN session_type = 'work' AND completed = 1 THEN 1 ELSE 0 END) as completed_work,
                CASE 
                    WHEN SUM(CASE WHEN session_type = 'work' THEN 1 ELSE 0 END) > 0 
                    THEN CAST(SUM(CASE WHEN session_type = 'work' AND completed = 1 THEN 1 ELSE 0 END) AS FLOAT) / 
                         SUM(CASE WHEN session_type = 'work' THEN 1 ELSE 0 END)
                    ELSE 0
                END as completion_rate
            FROM pomodoro_sessions
            WHERE start_time >= datetime('now', '-' || ? || ' days')
            GROUP BY day
            ORDER BY day DESC"
        )?;
        
        let daily_stats = stmt.query_map(params![days], |row| {
            Ok(StatsDaily {
                date: row.get(0)?,
                work_sessions: row.get(2)?,
                total_work_minutes: row.get(3)?,
                completed_work_sessions: row.get(4)?,
                completion_rate: row.get(5)?,
            })
        })?;
        
        let mut result = Vec::new();
        for stat in daily_stats {
            result.push(stat?);
        }
        
        Ok(result)
    }
    
    pub fn get_summary_stats(&self) -> Result<StatsSummary, DatabaseError> {
        let conn = self.conn.lock().map_err(|_| DatabaseError::Initialization("Failed to lock database connection".to_string()))?;
        
        // Get overall summary stats
        let mut stmt = conn.prepare(
            "SELECT 
                COUNT(CASE WHEN session_type = 'work' THEN 1 ELSE NULL END) as total_work_sessions,
                CAST(SUM(CASE WHEN session_type = 'work' THEN duration_seconds ELSE 0 END) / 60 AS INTEGER) as total_work_minutes,
                COUNT(CASE WHEN session_type = 'work' AND completed = 1 THEN 1 ELSE NULL END) as completed_sessions,
                CASE 
                    WHEN COUNT(CASE WHEN session_type = 'work' THEN 1 ELSE NULL END) > 0 
                    THEN CAST(COUNT(CASE WHEN session_type = 'work' AND completed = 1 THEN 1 ELSE NULL END) AS FLOAT) / 
                         COUNT(CASE WHEN session_type = 'work' THEN 1 ELSE NULL END)
                    ELSE 0
                END as completion_rate,
                CASE 
                    WHEN COUNT(DISTINCT strftime('%Y-%m-%d', start_time)) > 0 
                    THEN CAST(COUNT(CASE WHEN session_type = 'work' THEN 1 ELSE NULL END) AS FLOAT) / 
                         COUNT(DISTINCT strftime('%Y-%m-%d', start_time))
                    ELSE 0
                END as avg_sessions_per_day
            FROM pomodoro_sessions"
        )?;
        
        let mut summary = stmt.query_map([], |row| {
            Ok(StatsSummary {
                total_work_sessions: row.get(0)?,
                total_work_minutes: row.get(1)?,
                completed_sessions: row.get(2)?,
                completion_rate: row.get(3)?,
                avg_sessions_per_day: row.get(4)?,
                longest_streak_days: 0, // Will calculate below
                current_streak_days: 0,  // Will calculate below
            })
        })?.next().ok_or(DatabaseError::Initialization("Failed to get summary stats".into()))??;
        
        // Calculate streaks
        let mut streak_stmt = conn.prepare(
            "WITH dates AS (
                SELECT DISTINCT strftime('%Y-%m-%d', start_time) as day
                FROM pomodoro_sessions
                WHERE session_type = 'work'
                ORDER BY day
            ),
            gaps AS (
                SELECT 
                    day, 
                    julianday(day) - julianday(LAG(day) OVER (ORDER BY day)) AS diff
                FROM dates
            ),
            streaks AS (
                SELECT 
                    day,
                    SUM(CASE WHEN diff <= 1.0 THEN 0 ELSE 1 END) OVER (ORDER BY day) AS streak_group
                FROM gaps
            ),
            streak_lengths AS (
                SELECT 
                    streak_group, 
                    COUNT(*) AS streak_length,
                    MAX(day) AS last_day
                FROM streaks
                GROUP BY streak_group
            )
            SELECT 
                MAX(streak_length) AS longest_streak,
                (SELECT streak_length FROM streak_lengths 
                 WHERE last_day = (SELECT MAX(day) FROM dates)) AS current_streak
            FROM streak_lengths"
        )?;
        
        let streak_result = streak_stmt.query_row([], |row| {
            let longest: Result<i64, _> = row.get(0);
            let current: Result<i64, _> = row.get(1);
            Ok((longest.unwrap_or(0), current.unwrap_or(0)))
        })?;
        
        summary.longest_streak_days = streak_result.0;
        summary.current_streak_days = streak_result.1;
        
        Ok(summary)
    }
    
    pub fn get_session_type_stats(&self) -> Result<Vec<SessionTypeSummary>, DatabaseError> {
        let conn = self.conn.lock().map_err(|_| DatabaseError::Initialization("Failed to lock database connection".to_string()))?;
        
        let mut stmt = conn.prepare(
            "SELECT 
                session_type,
                COUNT(*) as count,
                CAST(SUM(duration_seconds) / 60 AS INTEGER) as total_minutes,
                CASE 
                    WHEN COUNT(*) > 0 
                    THEN CAST(SUM(CASE WHEN completed = 1 THEN 1 ELSE 0 END) AS FLOAT) / COUNT(*)
                    ELSE 0
                END as completion_rate
            FROM pomodoro_sessions
            GROUP BY session_type
            ORDER BY count DESC"
        )?;
        
        let type_stats = stmt.query_map([], |row| {
            Ok(SessionTypeSummary {
                session_type: row.get(0)?,
                count: row.get(1)?,
                total_minutes: row.get(2)?,
                completion_rate: row.get(3)?,
            })
        })?;
        
        let mut result = Vec::new();
        for stat in type_stats {
            result.push(stat?);
        }
        
        Ok(result)
    }
}
