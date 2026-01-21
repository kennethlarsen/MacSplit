# Autosplit Timer

A terminal-based speedrun timer with auto-splitting support, built with `livesplit-core`.

## Building

```bash
cargo build --release
```

## Usage

### Basic timer (manual splits)

```bash
./target/release/autosplit-timer
```

### With splits file

```bash
./target/release/autosplit-timer --splits splits.json
```

### With auto-splitting (watching a game log)

```bash
./target/release/autosplit-timer --splits splits.json --watch /path/to/game.log
```

## Controls

| Key     | Action                          |
|---------|---------------------------------|
| `Space` | Start timer / Split             |
| `P`     | Pause / Resume                  |
| `R`     | Reset                           |
| `U`     | Undo last split                 |
| `S`     | Skip split                      |
| `Q`/`Esc` | Quit                          |

## Splits File Format

Create a JSON file with your splits and auto-split triggers:

```json
{
  "game": "Game Name",
  "category": "Any%",
  "start_trigger": "GAME_STARTED",
  "reset_trigger": "GAME_RESET",
  "splits": [
    {
      "name": "First Split",
      "trigger": "FIRST_AREA_COMPLETE"
    },
    {
      "name": "Second Split",
      "trigger": "SECOND_AREA_COMPLETE"
    },
    {
      "name": "Final Split",
      "trigger": "GAME_FINISHED"
    }
  ]
}
```

### Fields

- `game`: Game name displayed in the timer
- `category`: Category name displayed in the timer
- `start_trigger`: (optional) Text that triggers timer start when found in log
- `reset_trigger`: (optional) Text that triggers timer reset when found in log
- `splits`: Array of split definitions
  - `name`: Display name for the split
  - `trigger`: (optional) Text that triggers this split when found in log
  - `best_time_ms`: (optional) Best segment time in milliseconds

## Auto-Splitting

The timer watches the specified log file for new lines. When a line contains a trigger keyword, it performs the corresponding action:

1. **Start trigger**: Starts the timer (if not running)
2. **Split triggers**: Advances to the next split (in order)
3. **Reset trigger**: Resets the timer

The watcher only reads new content appended to the file, so it works with games that continuously write to a log file.

### Testing Auto-Split

```bash
# Terminal 1: Start the timer
./target/release/autosplit-timer -s examples/splits.json -w examples/game.log

# Terminal 2: Simulate game events
echo "FILE SELECT" >> examples/game.log     # Starts timer
echo "STAR GET BoB" >> examples/game.log    # Split 1
echo "STAR GET WF" >> examples/game.log     # Split 2
# ... etc
```

## Integration Tips

For real game integration, you'll need to:

1. Find or create a way to output game events to a text file
2. Identify the text patterns that indicate split points
3. Create a splits.json with the appropriate triggers

Common approaches:
- **Memory reading**: Use a tool to read game memory and write events to a log
- **Game mods**: Mod the game to write events to a file
- **Log parsing**: Some games have built-in debug/console logs
