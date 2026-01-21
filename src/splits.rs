use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitDefinition {
    pub name: String,
    #[serde(default)]
    pub best_time_ms: Option<u64>,
    #[serde(default)]
    pub trigger: Option<String>, // Keyword to watch for in game log
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitsFile {
    pub game: String,
    pub category: String,
    pub splits: Vec<SplitDefinition>,
    #[serde(default)]
    pub start_trigger: Option<String>,
    #[serde(default)]
    pub reset_trigger: Option<String>,
}

impl SplitsFile {
    pub fn load(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let splits: SplitsFile = serde_json::from_str(&content)?;
        Ok(splits)
    }

    pub fn default_run() -> Self {
        SplitsFile {
            game: "Game".to_string(),
            category: "Any%".to_string(),
            splits: vec![
                SplitDefinition {
                    name: "Split 1".to_string(),
                    best_time_ms: None,
                    trigger: None,
                },
            ],
            start_trigger: None,
            reset_trigger: None,
        }
    }
}
