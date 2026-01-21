use crate::splits::SplitsFile;
use crate::watcher::{LogWatcher, WatchEvent};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    style::{Color, Print, SetForegroundColor, ResetColor},
    terminal::{self, ClearType},
};
use livesplit_core::{Run, Segment, Timer, TimerPhase, TimeSpan};
use std::io::{stdout, Write};
use std::path::PathBuf;
use std::time::Duration;

pub fn run(
    splits_path: Option<PathBuf>,
    watch_path: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Load splits
    let splits_file = match splits_path {
        Some(ref path) => SplitsFile::load(path)?,
        None => SplitsFile::default_run(),
    };

    // Create livesplit Run
    let mut run = Run::new();
    run.set_game_name(splits_file.game.as_str());
    run.set_category_name(splits_file.category.as_str());

    for split in &splits_file.splits {
        let mut segment = Segment::new(&split.name);
        if let Some(best_ms) = split.best_time_ms {
            let time = livesplit_core::Time::new()
                .with_real_time(Some(TimeSpan::from_milliseconds(best_ms as f64)));
            segment.set_best_segment_time(time);
        }
        run.push_segment(segment);
    }

    // Create timer
    let mut timer = Timer::new(run).map_err(|_| "Failed to create timer")?;

    // Setup log watcher if path provided
    let mut watcher = if let Some(ref path) = watch_path {
        let split_triggers: Vec<Option<String>> = splits_file
            .splits
            .iter()
            .map(|s| s.trigger.clone())
            .collect();

        Some(LogWatcher::new(
            path.clone(),
            splits_file.start_trigger.clone(),
            splits_file.reset_trigger.clone(),
            split_triggers,
        )?)
    } else {
        None
    };

    // Setup terminal
    terminal::enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide)?;

    // Main loop
    let result = main_loop(&mut timer, &mut watcher, &splits_file);

    // Cleanup terminal
    execute!(stdout, terminal::LeaveAlternateScreen, cursor::Show)?;
    terminal::disable_raw_mode()?;

    result
}

fn main_loop(
    timer: &mut Timer,
    watcher: &mut Option<LogWatcher>,
    splits_file: &SplitsFile,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut stdout = stdout();

    loop {
        // Poll log watcher for auto-split events
        if let Some(ref mut w) = watcher {
            for event in w.poll() {
                match event {
                    WatchEvent::Start => {
                        if timer.current_phase() == TimerPhase::NotRunning {
                            timer.start();
                        }
                    }
                    WatchEvent::Split(_) => {
                        if timer.current_phase() == TimerPhase::Running {
                            timer.split();
                        }
                    }
                    WatchEvent::Reset => {
                        timer.reset(true);
                        w.reset_split_index();
                    }
                }
            }
        }

        // Handle keyboard input
        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Char(' ') => {
                            match timer.current_phase() {
                                TimerPhase::NotRunning => timer.start(),
                                TimerPhase::Running => timer.split(),
                                TimerPhase::Ended => {}
                                TimerPhase::Paused => timer.resume(),
                            }
                        }
                        KeyCode::Char('r') => {
                            timer.reset(true);
                            if let Some(ref mut w) = watcher {
                                w.reset_split_index();
                            }
                        }
                        KeyCode::Char('p') => {
                            match timer.current_phase() {
                                TimerPhase::Running => timer.pause(),
                                TimerPhase::Paused => timer.resume(),
                                _ => {}
                            }
                        }
                        KeyCode::Char('u') => {
                            timer.undo_split();
                            if let Some(ref mut w) = watcher {
                                let idx = timer.current_split_index().unwrap_or(0);
                                w.set_split_index(idx);
                            }
                        }
                        KeyCode::Char('s') => {
                            timer.skip_split();
                            if let Some(ref mut w) = watcher {
                                let idx = timer.current_split_index().unwrap_or(0);
                                w.set_split_index(idx);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        // Render UI
        render(&mut stdout, timer, splits_file, watcher.is_some())?;
    }

    Ok(())
}

fn format_time(time_span: Option<TimeSpan>) -> String {
    match time_span {
        Some(ts) => {
            let total_secs = ts.total_seconds();
            let negative = total_secs < 0.0;
            let total_secs = total_secs.abs();

            let hours = (total_secs / 3600.0) as u32;
            let mins = ((total_secs % 3600.0) / 60.0) as u32;
            let secs = (total_secs % 60.0) as u32;
            let ms = ((total_secs * 1000.0) as u32) % 1000;

            let sign = if negative { "-" } else { "" };

            if hours > 0 {
                format!("{}{}:{:02}:{:02}.{:03}", sign, hours, mins, secs, ms)
            } else {
                format!("{}{}:{:02}.{:03}", sign, mins, secs, ms)
            }
        }
        None => "-:--:---".to_string(),
    }
}

fn render(
    stdout: &mut std::io::Stdout,
    timer: &Timer,
    splits_file: &SplitsFile,
    watching: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    execute!(stdout, cursor::MoveTo(0, 0), terminal::Clear(ClearType::All))?;

    let snapshot = timer.snapshot();
    let current_time = snapshot.current_time().real_time;
    let phase = timer.current_phase();
    let current_split_idx = timer.current_split_index().unwrap_or(0);

    // Header
    execute!(
        stdout,
        SetForegroundColor(Color::Cyan),
        Print(format!(" {} - {}\n", splits_file.game, splits_file.category)),
        ResetColor,
    )?;

    // Splits list
    execute!(stdout, Print("\n"))?;
    let run = timer.run();
    for (i, split) in splits_file.splits.iter().enumerate() {
        let segment = run.segment(i);
        let split_time = segment.split_time().real_time;

        let (bullet, color) = if i < current_split_idx {
            ("  • ✓", Color::Green)
        } else if i == current_split_idx && phase == TimerPhase::Running {
            ("  • ▶", Color::Yellow)
        } else {
            ("  •  ", Color::DarkGrey)
        };

        execute!(
            stdout,
            SetForegroundColor(color),
            Print(format!("{} {:<28}", bullet, split.name)),
        )?;

        if i < current_split_idx {
            execute!(
                stdout,
                Print(format!("  {}", format_time(split_time))),
            )?;
        }

        execute!(stdout, ResetColor, Print("\n"))?;
    }

    // Current time (big display)
    execute!(stdout, Print("\n"))?;
    
    let time_color = match phase {
        TimerPhase::NotRunning => Color::White,
        TimerPhase::Running => Color::Green,
        TimerPhase::Paused => Color::Yellow,
        TimerPhase::Ended => Color::Cyan,
    };

    execute!(
        stdout,
        Print(" "),
        SetForegroundColor(time_color),
        Print(format!(" {} ", format_time(current_time))),
        ResetColor,
    )?;

    let status = match phase {
        TimerPhase::NotRunning => "[READY]",
        TimerPhase::Running => "[RUNNING]",
        TimerPhase::Paused => "[PAUSED]",
        TimerPhase::Ended => "[FINISHED]",
    };
    execute!(stdout, Print(format!("  {}\n", status)))?;

    // Controls
    execute!(
        stdout,
        SetForegroundColor(Color::DarkGrey),
        Print(" [Space] Start/Split  [P] Pause  [R] Reset\n"),
        Print(" [U] Undo split  [S] Skip split  [Q] Quit\n"),
    )?;

    if watching {
        execute!(
            stdout,
            SetForegroundColor(Color::Magenta),
            Print(" 👁 Auto-split active\n"),
        )?;
    }

    execute!(stdout, ResetColor)?;
    stdout.flush()?;

    Ok(())
}