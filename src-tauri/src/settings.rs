use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub color_working: String,
    pub color_waiting: String,
    pub color_idle: String,
    pub text_size: TextSize,
    pub sound_on_complete: bool,
    pub theme: Theme,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TextSize {
    Small,
    Medium,
    Large,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Dark,
    Light,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            color_working: "#a78bfa".to_string(),
            color_waiting: "#fbbf24".to_string(),
            color_idle: "#71717a".to_string(),
            text_size: TextSize::Medium,
            sound_on_complete: true,
            theme: Theme::Dark,
        }
    }
}

pub struct SettingsStore {
    path: PathBuf,
    settings: Settings,
}

impl SettingsStore {
    pub fn new(app_data_dir: PathBuf) -> Self {
        let path = app_data_dir.join("settings.json");
        let settings = Self::load_from(&path);
        SettingsStore { path, settings }
    }

    fn load_from(path: &PathBuf) -> Settings {
        if path.exists() {
            fs::read_to_string(path)
                .ok()
                .and_then(|content| serde_json::from_str(&content).ok())
                .unwrap_or_default()
        } else {
            Settings::default()
        }
    }

    pub fn get(&self) -> &Settings {
        &self.settings
    }

    pub fn get_cloned(&self) -> Settings {
        self.settings.clone()
    }

    pub fn reset(&mut self) {
        self.settings = Settings::default();
        let _ = self.save();
    }

    pub fn update_field(&mut self, key: &str, value: &str) -> Result<(), String> {
        match key {
            "colorWorking" => {
                self.settings.color_working = value.to_string();
            }
            "colorWaiting" => {
                self.settings.color_waiting = value.to_string();
            }
            "colorIdle" => {
                self.settings.color_idle = value.to_string();
            }
            "textSize" => {
                self.settings.text_size = serde_json::from_str(&format!("\"{}\"", value))
                    .map_err(|_| format!("Invalid text size: {}", value))?;
            }
            "theme" => {
                self.settings.theme = serde_json::from_str(&format!("\"{}\"", value))
                    .map_err(|_| format!("Invalid theme: {}", value))?;
            }
            "soundOnComplete" => {
                self.settings.sound_on_complete = value
                    .parse::<bool>()
                    .map_err(|_| format!("Invalid boolean: {}", value))?;
            }
            _ => return Err(format!("Unknown setting: {}", key)),
        }
        self.save()
    }

    fn save(&self) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create settings dir: {}", e))?;
        }
        let content = serde_json::to_string_pretty(&self.settings)
            .map_err(|e| format!("Failed to serialize settings: {}", e))?;
        fs::write(&self.path, content).map_err(|e| format!("Failed to write settings: {}", e))?;
        Ok(())
    }
}
