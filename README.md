# MacSplit
Autosplitter for speedrunning on Mac.

IN DEVELOPMENT

![Macsplit](macsplit.png)

Currently supports 1 character runs for Binding of Isaac: Rebirth. More games to come.

## Building for Development

### Prerequisites
- Rust toolchain (install via [rustup](https://rustup.rs/))

### Build and Run
```bash
# Clone the repository
git clone <repo-url>
cd MacSplit

# Build in debug mode
cargo build

# Run the application
cargo run

# Run with terminal UI instead of GUI
cargo run -- --terminal

# Run with specific splits and log file
cargo run -- --splits path/to/splits.json --watch path/to/game.log
```

### Build for Release
```bash
cargo build --release
```
The binary will be in `target/release/MacSplit`.

## Adding New Autosplitters

To add support for a new game, create a folder in `autosplitters/` with two files:

### 1. Create the folder structure
```
autosplitters/
  your-game-name/
    config.json
    splits.json
```

### 2. config.json
Contains the game name and path to the log file (relative to home directory):
```json
{
    "game": "Your Game Name",
    "log_location": "Library/Application Support/YourGame/output.log"
}
```

### 3. splits.json
Defines the splits and trigger keywords to watch for in the log:
```json
{
    "game": "Your Game Name",
    "category": "Any%",
    "start_trigger": "keyword that appears when run starts",
    "reset_trigger": "keyword that appears on reset",
    "splits": [
        {
            "name": "Split 1",
            "trigger": "keyword for split 1"
        },
        {
            "name": "Split 2",
            "trigger": "keyword for split 2"
        }
    ]
}
```

### How it works
- The app watches the game's log file for specific keywords
- When `start_trigger` is found, the timer starts
- When a split's `trigger` is found, the timer splits to the next segment
- When `reset_trigger` is found, the timer resets

### Tips for finding triggers
1. Run the game and perform the actions you want to split on
2. Check the game's log file for unique keywords that appear at those moments
3. Use keywords that are specific enough to avoid false triggers

## Controls

| Key | Action |
|-----|--------|
| Space | Start timer / Split |
| P | Pause / Resume |
| R | Reset |
| U | Undo split |
| S | Skip split |
| Esc / Q | Quit |