use std::{
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StrictnessMode {
    Gentle,
    #[default]
    Standard,
    Hard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    Light,
    Dark,
    #[default]
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RendererMode {
    #[default]
    Auto,
    RichGraphics,
    UnicodeRich,
    PlaintextSafe,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FocusSettings {
    pub show_timer: bool,
    pub confirm_before_quit: bool,
    pub restore_unsent_drafts: bool,
}

impl Default for FocusSettings {
    fn default() -> Self {
        Self {
            show_timer: true,
            confirm_before_quit: true,
            restore_unsent_drafts: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppConfig {
    pub default_course: String,
    pub default_session_minutes: u16,
    pub strictness: StrictnessMode,
    pub theme: ThemeMode,
    pub renderer_mode: RendererMode,
    pub reduced_motion: bool,
    pub focus: FocusSettings,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            default_course: "Matrix Algebra & Linear Models".to_string(),
            default_session_minutes: 45,
            strictness: StrictnessMode::Standard,
            theme: ThemeMode::System,
            renderer_mode: RendererMode::Auto,
            reduced_motion: false,
            focus: FocusSettings::default(),
        }
    }
}

impl AppConfig {
    pub fn load_or_default(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let raw = fs::read_to_string(path)?;
        Ok(toml::from_str(&raw)?)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    pub root_dir: PathBuf,
    pub data_dir: PathBuf,
    pub config_path: PathBuf,
    pub database_path: PathBuf,
    pub courses_dir: PathBuf,
    pub deadlines_path: PathBuf,
    pub timetable_path: PathBuf,
    pub materials_dir: PathBuf,
}

impl AppPaths {
    pub fn discover(base_dir: &Path) -> Self {
        let root_dir = env::var_os("STUDYOS_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| base_dir.join(".studyos"));

        Self {
            config_path: root_dir.join("config.toml"),
            database_path: root_dir.join("studyos.db"),
            courses_dir: root_dir.join("courses"),
            deadlines_path: root_dir.join("deadlines.json"),
            timetable_path: root_dir.join("timetable.json"),
            materials_dir: root_dir.join("materials"),
            data_dir: root_dir.clone(),
            root_dir,
        }
    }

    pub fn ensure(&self) -> Result<()> {
        fs::create_dir_all(&self.data_dir)?;
        fs::create_dir_all(&self.courses_dir)?;
        fs::create_dir_all(&self.materials_dir)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{env, fs};

    use super::{AppConfig, AppPaths};

    #[test]
    fn load_or_default_returns_defaults_for_missing_file() {
        let path = env::temp_dir().join("studyos-missing-config.toml");
        if path.exists() {
            let _ = fs::remove_file(&path);
        }

        let config = AppConfig::load_or_default(&path)
            .unwrap_or_else(|err| panic!("config load failed: {err}"));

        assert_eq!(config.default_session_minutes, 45);
    }

    #[test]
    fn ensure_creates_expected_directories() {
        let root = env::temp_dir().join(format!("studyos-config-test-{}", std::process::id()));
        if root.exists() {
            let _ = fs::remove_dir_all(&root);
        }

        let paths = AppPaths::discover(&root);
        paths
            .ensure()
            .unwrap_or_else(|err| panic!("path ensure failed: {err}"));

        assert!(paths.data_dir.exists());
        assert!(paths.courses_dir.exists());
        assert!(paths.materials_dir.exists());

        let _ = fs::remove_dir_all(&root);
    }
}
