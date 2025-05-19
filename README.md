# Pomodoro CLI

A feature-rich command-line Pomodoro timer with SQLite session tracking and productivity statistics visualization.

## Features

- **Pomodoro Timer**
  - Customizable work, short break, and long break durations
  - Configurable number of pomodoros before a long break
  - Manual control for start, stop, and next operations
  - Pause/resume functionality that preserves session state

- **Interactive Terminal UI**
  - Color-coded state display
  - Real-time countdown timer
  - Today's progress statistics
  - Clean interface with keyboard controls

- **Session Tracking**
  - SQLite database for persistent storage
  - Complete session history with timestamps
  - Tracks completion status of each session

- **Comprehensive Statistics**
  - Recent sessions list with details
  - Daily statistics with work minutes and completion rates
  - Summary statistics with streak tracking
  - Session type breakdown analytics
  - Terminal-based visualization charts

## Installation

### Prerequisites

- Rust and Cargo (1.54.0 or newer recommended)
- SQLite (included as a dependency)

### Building from Source

1. Clone the repository:
   ```
   git clone https://github.com/yourusername/pomodoro-cli.git
   cd pomodoro-cli
   ```

2. Build the project:
   ```
   cargo build --release
   ```

3. Run the binary:
   ```
   ./target/release/pomodoro-cli
   ```

## Usage

### Basic Commands

```
# Start the timer with default settings
pomodoro-cli

# Start the timer explicitly
pomodoro-cli start

# Stop/pause the timer
pomodoro-cli stop

# Skip to the next interval (work/break)
pomodoro-cli next
```

### Configuration Options

```
# Set custom durations
pomodoro-cli -p 30 -s 10 -l 20 -n 3
  -p, --pomodoro-minutes <MINUTES>          # Work duration (default: 25)
  -s, --short-break-minutes <MINUTES>       # Short break duration (default: 5)
  -l, --long-break-minutes <MINUTES>        # Long break duration (default: 15)
  -n, --pomodoros-until-long-break <COUNT>  # Number before long break (default: 4)

# Specify a different database file
pomodoro-cli -d mypomodoro.db
```

### Statistics Commands

```
# Show recent sessions (default: last 10)
pomodoro-cli stats

# Show more sessions
pomodoro-cli stats --limit 20

# Show daily statistics
pomodoro-cli stats -t daily

# Show summary statistics
pomodoro-cli stats -t summary

# Show session type breakdown
pomodoro-cli stats -t types

# Enable visualization charts
pomodoro-cli stats -t daily --chart

# Specify time range for daily stats
pomodoro-cli stats -t daily --days 14
```

### Interactive Mode Controls

When in interactive mode, the following keyboard controls are available:

- `s` - Start/Resume timer
- `p` - Pause timer
- `n` - Next interval (skip current)
- `q` - Quit the application

## Screenshots

(Screenshots would go here)

## Database Schema

The application uses SQLite to store session data with the following schema:

```sql
CREATE TABLE pomodoro_sessions (
    id INTEGER PRIMARY KEY,
    start_time TEXT NOT NULL,
    end_time TEXT,
    duration_seconds INTEGER NOT NULL,
    completed BOOLEAN NOT NULL,
    session_type TEXT NOT NULL
);
```

## Dependencies

- [rusqlite](https://github.com/rusqlite/rusqlite) - SQLite bindings
- [chrono](https://github.com/chronotope/chrono) - Date and time handling
- [clap](https://github.com/clap-rs/clap) - Command line argument parsing
- [crossterm](https://github.com/crossterm-rs/crossterm) - Terminal manipulation
- [notify-rust](https://github.com/hoodie/notify-rust) - Desktop notifications
- [tokio](https://github.com/tokio-rs/tokio) - Async runtime

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
