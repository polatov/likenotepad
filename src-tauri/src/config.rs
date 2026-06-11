use std::fs;
use tauri::Manager;

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(default)]
pub struct Config {
    pub theme: String,
    pub recent_files: Vec<String>,
    pub last_dir: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            theme: "auto".to_string(),
            recent_files: Vec::new(),
            last_dir: None,
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
