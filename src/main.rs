mod splits;
mod watcher;
mod timer_app;

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
}

fn main() {
    let args = Args::parse();
    
    if let Err(e) = timer_app::run(args.splits, args.watch) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
