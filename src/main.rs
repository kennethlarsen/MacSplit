mod splits;
mod watcher;
mod timer_app;
mod gui;

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "autosplit-timer")]
#[command(about = "Terminal speedrun timer with auto-splitting support")]
struct Args {
    /// Path to splits JSON file
    #[arg(short, long)]
    splits: Option<PathBuf>,

    /// Path to game log file to watch for auto-splitting
    #[arg(short, long)]
    watch: Option<PathBuf>,

    /// Use terminal UI instead of GUI
    #[arg(short, long)]
    terminal: bool,
}

fn main() {
    let args = Args::parse();

    let result = if args.terminal {
        timer_app::run(args.splits, args.watch)
    } else {
        gui::run_gui(args.splits, args.watch)
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
