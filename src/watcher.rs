use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;

pub enum WatchEvent {
    Start,
    Split(usize), // Index of split triggered
    Reset,
}

pub struct LogWatcher {
    path: PathBuf,
    reader: BufReader<File>,
    start_trigger: Option<String>,
    reset_trigger: Option<String>,
    split_triggers: Vec<Option<String>>,
    current_split: usize,
}

impl LogWatcher {
    pub fn new(
        path: PathBuf,
        start_trigger: Option<String>,
        reset_trigger: Option<String>,
        split_triggers: Vec<Option<String>>,
    ) -> Result<Self, std::io::Error> {
        let file = File::open(&path)?;
        let mut reader = BufReader::new(file);
        
        // Seek to end of file - we only want new content
        reader.seek(SeekFrom::End(0))?;

        Ok(Self {
            path,
            reader,
            start_trigger,
            reset_trigger,
            split_triggers,
            current_split: 0,
        })
    }

    pub fn reset_split_index(&mut self) {
        self.current_split = 0;
    }

    pub fn set_split_index(&mut self, index: usize) {
        self.current_split = index;
    }

    pub fn poll(&mut self) -> Vec<WatchEvent> {
        let mut events = Vec::new();
        let mut line = String::new();

        // Re-open file if it was truncated/rotated
        if let Ok(metadata) = std::fs::metadata(&self.path) {
            let current_pos = self.reader.stream_position().unwrap_or(0);
            if metadata.len() < current_pos {
                // File was truncated, re-open
                if let Ok(file) = File::open(&self.path) {
                    self.reader = BufReader::new(file);
                }
            }
        }

        loop {
            line.clear();
            match self.reader.read_line(&mut line) {
                Ok(0) => break, // No more data
                Ok(_) => {
                    let line = line.trim();
                    
                    // Check for reset trigger first
                    if let Some(ref trigger) = self.reset_trigger {
                        if line.contains(trigger.as_str()) {
                            events.push(WatchEvent::Reset);
                            self.current_split = 0;
                            continue;
                        }
                    }

                    // Check for start trigger
                    if let Some(ref trigger) = self.start_trigger {
                        if line.contains(trigger.as_str()) {
                            events.push(WatchEvent::Start);
                            continue;
                        }
                    }

                    // Check for current split trigger
                    if self.current_split < self.split_triggers.len() {
                        if let Some(ref trigger) = self.split_triggers[self.current_split] {
                            if line.contains(trigger.as_str()) {
                                events.push(WatchEvent::Split(self.current_split));
                                self.current_split += 1;
                            }
                        }
                    }
                }
                Err(_) => break,
            }
        }

        events
    }
}
