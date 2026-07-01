use std::fs;
use tauri::Manager;

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(default)]
pub struct Config {
    pub theme: String,
    pub recent_files: Vec<String>,
    pub last_dir: Option<String>,
    pub word_wrap: bool,
    pub status_bar: bool,
    pub font_name: String,
    pub font_size: f64,
    pub font_weight: String,
    pub font_style: String,
    pub use_tabs: bool,
    pub show_counter: bool,
    pub auto_name: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            theme: "auto".to_string(),
            recent_files: Vec::new(),
            last_dir: None,
            word_wrap: false,
            status_bar: true,
            font_name: "Menlo".to_string(),
            font_size: 13.0,
            font_weight: "normal".to_string(),
            font_style: "normal".to_string(),
            use_tabs: false,
            show_counter: false,
            auto_name: false,
        }
    }
}

fn config_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("config.json"))
}

pub fn load(app: &tauri::AppHandle) -> Config {
    let path = match config_path(app) {
        Ok(p) => p,
        Err(_) => return Config::default(),
    };
    let contents = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return Config::default(),
    };
    serde_json::from_str(&contents).unwrap_or_default()
}

pub fn save(app: &tauri::AppHandle, config: &Config) -> Result<(), String> {
    let path = config_path(app)?;
    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())
}
