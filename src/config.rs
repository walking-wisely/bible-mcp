use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub db_path: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            db_path: default_db_path(),
        }
    }
}

fn config_file() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("bible-mcp")
        .join("config.json")
}

pub fn default_db_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("bible-mcp")
        .join("bible.db")
}

pub fn load() -> Result<Config> {
    let path = config_file();
    if path.exists() {
        let text = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&text)?)
    } else {
        Ok(Config::default())
    }
}

pub fn save(cfg: &Config) -> Result<()> {
    let path = config_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(cfg)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn round_trip() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("bible.db");
        let cfg = Config { db_path: db_path.clone() };
        let json = serde_json::to_string(&cfg).unwrap();
        let loaded: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.db_path, db_path);
    }

    #[test]
    fn default_has_db_path() {
        let cfg = Config::default();
        assert!(cfg.db_path.to_string_lossy().contains("bible-mcp"));
    }
}
