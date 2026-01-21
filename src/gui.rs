use crate::splits::SplitsFile;
use crate::watcher::{LogWatcher, WatchEvent};
use eframe::egui;
use livesplit_core::{Run, Segment, Timer, TimerPhase, TimeSpan};
use serde::Deserialize;
use std::path::PathBuf;

const DARK_BG: egui::Color32 = egui::Color32::from_rgb(20, 20, 25);
const HEADER_BG: egui::Color32 = egui::Color32::from_rgb(30, 30, 40);
const SPLIT_BG: egui::Color32 = egui::Color32::from_rgb(25, 25, 35);
const SPLIT_CURRENT_BG: egui::Color32 = egui::Color32::from_rgb(40, 40, 60);

const TEXT_WHITE: egui::Color32 = egui::Color32::from_rgb(255, 255, 255);
const TEXT_GRAY: egui::Color32 = egui::Color32::from_rgb(170, 170, 170);
const TIME_GREEN: egui::Color32 = egui::Color32::from_rgb(50, 205, 50);
const TIME_RED: egui::Color32 = egui::Color32::from_rgb(220, 60, 60);
const TIME_GOLD: egui::Color32 = egui::Color32::from_rgb(255, 215, 0);
const TIME_BLUE: egui::Color32 = egui::Color32::from_rgb(100, 149, 237);
const ACCENT_COLOR: egui::Color32 = egui::Color32::from_rgb(139, 69, 255);

#[derive(Debug, Clone, Deserialize)]
struct GameConfig {
    game: String,
    log_location: String,
}

#[derive(Debug, Clone)]
struct AvailableGame {
    display_name: String,
    folder_name: String,
    config: GameConfig,
}

fn discover_autosplitters() -> Vec<AvailableGame> {
    let mut games = Vec::new();

    // Get the executable's directory or current directory
    let autosplitters_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("autosplitters");

    // Also check current working directory
    let cwd_autosplitters = PathBuf::from("autosplitters");

    for dir in [autosplitters_dir, cwd_autosplitters] {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let config_path = path.join("config.json");
                    let splits_path = path.join("splits.json");

                    if config_path.exists() && splits_path.exists() {
                        if let Ok(config_content) = std::fs::read_to_string(&config_path) {
                            if let Ok(config) = serde_json::from_str::<GameConfig>(&config_content) {
                                let folder_name = path
                                    .file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_default();

                                // Avoid duplicates
                                if !games.iter().any(|g: &AvailableGame| g.folder_name == folder_name) {
                                    games.push(AvailableGame {
                                        display_name: config.game.clone(),
                                        folder_name,
                                        config,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    games
}

fn get_autosplitters_base_dir() -> PathBuf {
    // Check current working directory first
    let cwd_autosplitters = PathBuf::from("autosplitters");
    if cwd_autosplitters.exists() {
        return cwd_autosplitters;
    }

    // Fall back to executable directory
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("autosplitters")
}

pub struct LiveSplitApp {
    timer: Timer,
    splits_file: SplitsFile,
    watcher: Option<LogWatcher>,
    available_games: Vec<AvailableGame>,
    selected_game_index: Option<usize>,
    pending_game_change: Option<usize>,
}

impl LiveSplitApp {
    pub fn new(
        splits_path: Option<PathBuf>,
        watch_path: Option<PathBuf>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let available_games = discover_autosplitters();

        let (splits_file, watcher, selected_game_index) = if splits_path.is_some() || watch_path.is_some() {
            // Use provided paths
            let splits_file = match splits_path {
                Some(ref path) => SplitsFile::load(path)?,
                None => SplitsFile::default_run(),
            };

            let watcher = if let Some(ref path) = watch_path {
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

            (splits_file, watcher, None)
        } else {
            // Default run, no game selected
            (SplitsFile::default_run(), None, None)
        };

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

        let timer = Timer::new(run).map_err(|_| "Failed to create timer")?;

        Ok(Self {
            timer,
            splits_file,
            watcher,
            available_games,
            selected_game_index,
            pending_game_change: None,
        })
    }

    fn load_game(&mut self, game_index: usize) -> Result<(), Box<dyn std::error::Error>> {
        let game = &self.available_games[game_index];
        let base_dir = get_autosplitters_base_dir();
        let game_dir = base_dir.join(&game.folder_name);

        let splits_path = game_dir.join("splits.json");
        let splits_file = SplitsFile::load(&splits_path)?;

        // Resolve log location (relative to home directory)
        let home_dir = dirs_next::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        let log_path = home_dir.join(&game.config.log_location);

        // Create watcher
        let split_triggers: Vec<Option<String>> = splits_file
            .splits
            .iter()
            .map(|s| s.trigger.clone())
            .collect();

        let watcher = LogWatcher::new(
            log_path,
            splits_file.start_trigger.clone(),
            splits_file.reset_trigger.clone(),
            split_triggers,
        ).ok();

        // Create new timer
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

        let timer = Timer::new(run).map_err(|_| "Failed to create timer")?;

        self.timer = timer;
        self.splits_file = splits_file;
        self.watcher = watcher;
        self.selected_game_index = Some(game_index);

        Ok(())
    }

    fn poll_watcher(&mut self) {
        if let Some(ref mut w) = self.watcher {
            for event in w.poll() {
                match event {
                    WatchEvent::Start => {
                        if self.timer.current_phase() == TimerPhase::NotRunning {
                            self.timer.start();
                        }
                    }
                    WatchEvent::Split(_) => {
                        if self.timer.current_phase() == TimerPhase::Running {
                            self.timer.split();
                        }
                    }
                    WatchEvent::Reset => {
                        self.timer.reset(true);
                        w.reset_split_index();
                    }
                }
            }
        }
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
                    format!("{}{}:{:02}:{:02}.{:02}", sign, hours, mins, secs, ms / 10)
                } else {
                    format!("{}{}:{:02}.{:02}", sign, mins, secs, ms / 10)
                }
            }
            None => "-".to_string(),
        }
    }

    fn format_time_ms(time_span: Option<TimeSpan>) -> String {
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
                } else if mins > 0 {
                    format!("{}{}:{:02}.{:03}", sign, mins, secs, ms)
                } else {
                    format!("{}{}.{:03}", sign, secs, ms)
                }
            }
            None => "0.000".to_string(),
        }
    }
}

impl eframe::App for LiveSplitApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle pending game change
        if let Some(game_index) = self.pending_game_change.take() {
            let _ = self.load_game(game_index);
        }

        self.poll_watcher();

        // Request continuous repaints for timer updates
        ctx.request_repaint();

        // Handle keyboard input
        ctx.input(|i| {
            if i.key_pressed(egui::Key::Space) {
                match self.timer.current_phase() {
                    TimerPhase::NotRunning => self.timer.start(),
                    TimerPhase::Running => self.timer.split(),
                    TimerPhase::Ended => {}
                    TimerPhase::Paused => self.timer.resume(),
                }
            }
            if i.key_pressed(egui::Key::R) {
                self.timer.reset(true);
                if let Some(ref mut w) = self.watcher {
                    w.reset_split_index();
                }
            }
            if i.key_pressed(egui::Key::P) {
                match self.timer.current_phase() {
                    TimerPhase::Running => self.timer.pause(),
                    TimerPhase::Paused => self.timer.resume(),
                    _ => {}
                }
            }
            if i.key_pressed(egui::Key::U) {
                self.timer.undo_split();
                if let Some(ref mut w) = self.watcher {
                    let idx = self.timer.current_split_index().unwrap_or(0);
                    w.set_split_index(idx);
                }
            }
            if i.key_pressed(egui::Key::S) {
                self.timer.skip_split();
                if let Some(ref mut w) = self.watcher {
                    let idx = self.timer.current_split_index().unwrap_or(0);
                    w.set_split_index(idx);
                }
            }
        });

        let snapshot = self.timer.snapshot();
        let current_time = snapshot.current_time().real_time;
        let phase = self.timer.current_phase();
        let current_split_idx = self.timer.current_split_index().unwrap_or(0);

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(DARK_BG))
            .show(ctx, |ui| {
                ui.style_mut().visuals.override_text_color = Some(TEXT_WHITE);

                // Game selector dropdown
                if !self.available_games.is_empty() {
                    egui::Frame::none()
                        .fill(HEADER_BG)
                        .inner_margin(egui::Margin::symmetric(12.0, 8.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new("Game:")
                                        .size(12.0)
                                        .color(TEXT_GRAY),
                                );

                                let current_selection = self
                                    .selected_game_index
                                    .map(|i| self.available_games[i].display_name.clone())
                                    .unwrap_or_else(|| "Select a game...".to_string());

                                egui::ComboBox::from_id_salt("game_selector")
                                    .selected_text(&current_selection)
                                    .width(200.0)
                                    .show_ui(ui, |ui| {
                                        for (i, game) in self.available_games.iter().enumerate() {
                                            let is_selected = self.selected_game_index == Some(i);
                                            if ui.selectable_label(is_selected, &game.display_name).clicked() {
                                                self.pending_game_change = Some(i);
                                            }
                                        }
                                    });
                            });
                        });

                    ui.add_space(2.0);
                }

                // Header - Game and Category
                egui::Frame::none()
                    .fill(HEADER_BG)
                    .inner_margin(egui::Margin::symmetric(12.0, 8.0))
                    .show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.label(
                                egui::RichText::new(&self.splits_file.game)
                                    .size(18.0)
                                    .strong()
                                    .color(TEXT_WHITE),
                            );
                            ui.label(
                                egui::RichText::new(&self.splits_file.category)
                                    .size(14.0)
                                    .color(TEXT_GRAY),
                            );
                        });
                    });

                ui.add_space(2.0);

                // Splits list
                let run = self.timer.run();
                let mut prev_split_time: Option<TimeSpan> = None;
                for (i, split) in self.splits_file.splits.iter().enumerate() {
                    let segment = run.segment(i);
                    let split_time = segment.split_time().real_time;
                    let best_segment = segment.best_segment_time().real_time;

                    // Calculate current segment time (time for this segment only)
                    let current_segment_time = if let Some(split) = split_time {
                        if let Some(prev) = prev_split_time {
                            Some(TimeSpan::from_seconds(split.total_seconds() - prev.total_seconds()))
                        } else {
                            Some(split)
                        }
                    } else {
                        None
                    };

                    let is_current = i == current_split_idx && phase == TimerPhase::Running;
                    let is_completed = i < current_split_idx;

                    let bg_color = if is_current {
                        SPLIT_CURRENT_BG
                    } else {
                        SPLIT_BG
                    };

                    egui::Frame::none()
                        .fill(bg_color)
                        .inner_margin(egui::Margin::symmetric(10.0, 6.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                // Split name
                                let name_color = if is_completed {
                                    TEXT_GRAY
                                } else if is_current {
                                    TEXT_WHITE
                                } else {
                                    TEXT_GRAY
                                };

                                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                    ui.label(
                                        egui::RichText::new(&split.name)
                                            .size(14.0)
                                            .color(name_color),
                                    );
                                });

                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    // Split time (right aligned)
                                    if is_completed {
                                        let time_str = Self::format_time(split_time);
                                        ui.label(
                                            egui::RichText::new(time_str)
                                                .size(14.0)
                                                .color(TEXT_WHITE)
                                                .monospace(),
                                        );

                                        // Delta: compare current segment time vs best segment time
                                        if let (Some(current_seg), Some(best_seg)) = (current_segment_time, best_segment) {
                                            let delta = current_seg.total_seconds() - best_seg.total_seconds();
                                            let delta_color = if delta < 0.0 {
                                                TIME_GREEN
                                            } else if delta < 1.0 {
                                                TIME_GOLD
                                            } else {
                                                TIME_RED
                                            };
                                            let delta_str = if delta >= 0.0 {
                                                format!("+{:.2}", delta)
                                            } else {
                                                format!("{:.2}", delta)
                                            };
                                            ui.add_space(10.0);
                                            ui.label(
                                                egui::RichText::new(delta_str)
                                                    .size(12.0)
                                                    .color(delta_color)
                                                    .monospace(),
                                            );
                                        }
                                    } else if !is_current {
                                        // Show best time for upcoming splits
                                        if let Some(_best) = best_segment {
                                            let time_str = Self::format_time(best_segment);
                                            ui.label(
                                                egui::RichText::new(time_str)
                                                    .size(14.0)
                                                    .color(TEXT_GRAY)
                                                    .monospace(),
                                            );
                                        } else {
                                            ui.label(
                                                egui::RichText::new("-")
                                                    .size(14.0)
                                                    .color(TEXT_GRAY)
                                                    .monospace(),
                                            );
                                        }
                                    }
                                });
                            });
                        });

                    // Track previous split time for segment calculation
                    if is_completed {
                        prev_split_time = split_time;
                    }

                    ui.add_space(1.0);
                }

                ui.add_space(4.0);

                // Main timer display
                egui::Frame::none()
                    .fill(HEADER_BG)
                    .inner_margin(egui::Margin::symmetric(12.0, 16.0))
                    .show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            let time_color = match phase {
                                TimerPhase::NotRunning => TEXT_WHITE,
                                TimerPhase::Running => TIME_GREEN,
                                TimerPhase::Paused => TIME_GOLD,
                                TimerPhase::Ended => TIME_BLUE,
                            };

                            let time_str = Self::format_time_ms(current_time);
                            ui.label(
                                egui::RichText::new(time_str)
                                    .size(48.0)
                                    .strong()
                                    .color(time_color)
                                    .monospace(),
                            );
                        });
                    });

                ui.add_space(8.0);

                // Controls hint
                egui::Frame::none()
                    .fill(SPLIT_BG)
                    .inner_margin(egui::Margin::symmetric(10.0, 8.0))
                    .show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.label(
                                egui::RichText::new("Space: Start/Split | P: Pause | R: Reset")
                                    .size(11.0)
                                    .color(TEXT_GRAY),
                            );
                            ui.label(
                                egui::RichText::new("U: Undo | S: Skip | Esc: Quit")
                                    .size(11.0)
                                    .color(TEXT_GRAY),
                            );
                            if self.watcher.is_some() {
                                ui.add_space(4.0);
                                ui.label(
                                    egui::RichText::new("Auto-split active")
                                        .size(11.0)
                                        .color(ACCENT_COLOR),
                                );
                            }
                        });
                    });
            });
    }
}

pub fn run_gui(
    splits_path: Option<PathBuf>,
    watch_path: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let app = LiveSplitApp::new(splits_path, watch_path)?;

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([300.0, 500.0])
            .with_min_inner_size([250.0, 350.0])
            .with_title("MacSplit"),
        ..Default::default()
    };

    eframe::run_native(
        "MacSplit",
        options,
        Box::new(|_cc| Ok(Box::new(app))),
    )?;

    Ok(())
}

/// Calculate the segment time from cumulative split times
///
/// # Arguments
/// * `current_split` - The cumulative time at the current split
/// * `previous_split` - The cumulative time at the previous split (None for first segment)
///
/// # Returns
/// The time for just this segment (current - previous)
fn calculate_segment_time(current_split: Option<TimeSpan>, previous_split: Option<TimeSpan>) -> Option<TimeSpan> {
    current_split.map(|current| {
        if let Some(prev) = previous_split {
            TimeSpan::from_seconds(current.total_seconds() - prev.total_seconds())
        } else {
            current
        }
    })
}

/// Calculate the delta between current segment time and best segment time
///
/// # Arguments
/// * `current_segment` - The time for the current segment
/// * `best_segment` - The best recorded time for this segment
///
/// # Returns
/// The delta in seconds (positive = slower, negative = faster)
fn calculate_delta(current_segment: Option<TimeSpan>, best_segment: Option<TimeSpan>) -> Option<f64> {
    match (current_segment, best_segment) {
        (Some(current), Some(best)) => Some(current.total_seconds() - best.total_seconds()),
        _ => None,
    }
}

/// Determine the color for a delta value
///
/// # Arguments
/// * `delta` - The delta in seconds
///
/// # Returns
/// Green if ahead (negative), Gold if close (0-1s behind), Red if behind (>1s)
fn delta_color(delta: f64) -> egui::Color32 {
    if delta < 0.0 {
        TIME_GREEN
    } else if delta < 1.0 {
        TIME_GOLD
    } else {
        TIME_RED
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_segment_time_first_segment() {
        // First segment: no previous split, so segment time = split time
        let current = Some(TimeSpan::from_seconds(30.0));
        let previous = None;

        let result = calculate_segment_time(current, previous);

        assert!(result.is_some());
        assert!((result.unwrap().total_seconds() - 30.0).abs() < 0.001);
    }

    #[test]
    fn test_calculate_segment_time_subsequent_segment() {
        // Second segment: split at 60s, previous at 30s, so segment = 30s
        let current = Some(TimeSpan::from_seconds(60.0));
        let previous = Some(TimeSpan::from_seconds(30.0));

        let result = calculate_segment_time(current, previous);

        assert!(result.is_some());
        assert!((result.unwrap().total_seconds() - 30.0).abs() < 0.001);
    }

    #[test]
    fn test_calculate_segment_time_none_current() {
        let current = None;
        let previous = Some(TimeSpan::from_seconds(30.0));

        let result = calculate_segment_time(current, previous);

        assert!(result.is_none());
    }

    #[test]
    fn test_calculate_delta_faster_than_best() {
        // Current segment: 25s, Best segment: 30s = -5s delta (faster)
        let current = Some(TimeSpan::from_seconds(25.0));
        let best = Some(TimeSpan::from_seconds(30.0));

        let delta = calculate_delta(current, best);

        assert!(delta.is_some());
        assert!((delta.unwrap() - (-5.0)).abs() < 0.001);
    }

    #[test]
    fn test_calculate_delta_slower_than_best() {
        // Current segment: 35s, Best segment: 30s = +5s delta (slower)
        let current = Some(TimeSpan::from_seconds(35.0));
        let best = Some(TimeSpan::from_seconds(30.0));

        let delta = calculate_delta(current, best);

        assert!(delta.is_some());
        assert!((delta.unwrap() - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_calculate_delta_equal_to_best() {
        // Current segment: 30s, Best segment: 30s = 0s delta
        let current = Some(TimeSpan::from_seconds(30.0));
        let best = Some(TimeSpan::from_seconds(30.0));

        let delta = calculate_delta(current, best);

        assert!(delta.is_some());
        assert!(delta.unwrap().abs() < 0.001);
    }

    #[test]
    fn test_calculate_delta_no_best() {
        // No best segment recorded
        let current = Some(TimeSpan::from_seconds(30.0));
        let best = None;

        let delta = calculate_delta(current, best);

        assert!(delta.is_none());
    }

    #[test]
    fn test_delta_color_ahead() {
        // Negative delta = ahead = green
        assert_eq!(delta_color(-5.0), TIME_GREEN);
        assert_eq!(delta_color(-0.1), TIME_GREEN);
    }

    #[test]
    fn test_delta_color_slightly_behind() {
        // 0 to 1 second behind = gold
        assert_eq!(delta_color(0.0), TIME_GOLD);
        assert_eq!(delta_color(0.5), TIME_GOLD);
        assert_eq!(delta_color(0.99), TIME_GOLD);
    }

    #[test]
    fn test_delta_color_behind() {
        // More than 1 second behind = red
        assert_eq!(delta_color(1.0), TIME_RED);
        assert_eq!(delta_color(5.0), TIME_RED);
        assert_eq!(delta_color(100.0), TIME_RED);
    }

    #[test]
    fn test_full_run_scenario_all_segments_faster() {
        // Simulate a run where all segments are faster than best
        // Best times: 30s, 30s, 30s (segments)
        // Current times: 25s, 25s, 25s (segments)
        // Cumulative: 25s, 50s, 75s vs best cumulative would be 30s, 60s, 90s

        let best_segments = vec![
            TimeSpan::from_seconds(30.0),
            TimeSpan::from_seconds(30.0),
            TimeSpan::from_seconds(30.0),
        ];

        let current_splits = vec![
            TimeSpan::from_seconds(25.0),  // Split 1: 25s cumulative
            TimeSpan::from_seconds(50.0),  // Split 2: 50s cumulative
            TimeSpan::from_seconds(75.0),  // Split 3: 75s cumulative
        ];

        let mut prev_split: Option<TimeSpan> = None;

        for (i, &current_split) in current_splits.iter().enumerate() {
            let segment_time = calculate_segment_time(Some(current_split), prev_split);
            let delta = calculate_delta(segment_time, Some(best_segments[i]));

            // All segments should be 5 seconds faster
            assert!(delta.is_some());
            assert!((delta.unwrap() - (-5.0)).abs() < 0.001,
                "Segment {} delta should be -5.0, got {}", i, delta.unwrap());

            // Color should be green (ahead)
            assert_eq!(delta_color(delta.unwrap()), TIME_GREEN);

            prev_split = Some(current_split);
        }
    }

    #[test]
    fn test_full_run_scenario_mixed_performance() {
        // Simulate a run with mixed performance
        // Best segments: 30s, 30s, 30s
        // Current segments: 25s (fast), 35s (slow), 30s (same)
        // Cumulative: 25s, 60s, 90s

        let best_segments = vec![
            TimeSpan::from_seconds(30.0),
            TimeSpan::from_seconds(30.0),
            TimeSpan::from_seconds(30.0),
        ];

        let current_splits = vec![
            TimeSpan::from_seconds(25.0),  // Segment 1: 25s (5s ahead)
            TimeSpan::from_seconds(60.0),  // Segment 2: 35s (5s behind)
            TimeSpan::from_seconds(90.0),  // Segment 3: 30s (even)
        ];

        let expected_deltas = vec![-5.0, 5.0, 0.0];
        let expected_colors = vec![TIME_GREEN, TIME_RED, TIME_GOLD];

        let mut prev_split: Option<TimeSpan> = None;

        for (i, &current_split) in current_splits.iter().enumerate() {
            let segment_time = calculate_segment_time(Some(current_split), prev_split);
            let delta = calculate_delta(segment_time, Some(best_segments[i]));

            assert!(delta.is_some());
            assert!((delta.unwrap() - expected_deltas[i]).abs() < 0.001,
                "Segment {} delta should be {}, got {}", i, expected_deltas[i], delta.unwrap());

            assert_eq!(delta_color(delta.unwrap()), expected_colors[i],
                "Segment {} color mismatch", i);

            prev_split = Some(current_split);
        }
    }
}
