use crate::{domain::AppConfig, error::AppError};
use std::{env, fs, io::Write, path::PathBuf};

const DEFAULT_CONFIG: &str = include_str!("../config.toml");

#[derive(Debug, Clone)]
pub struct LoadedConfig {
    pub config: AppConfig,
    pub path: Option<PathBuf>,
}

pub struct ConfigLoader;

impl ConfigLoader {
    pub fn load() -> Result<LoadedConfig, AppError> {
        for path in Self::candidate_paths() {
            if path.is_file() {
                let text = fs::read_to_string(&path)?;
                let config: AppConfig = toml::from_str(&text)
                    .map_err(|error| AppError::message(format!("invalid config: {error}")))?;
                return Ok(LoadedConfig {
                    config,
                    path: Some(path),
                });
            }
        }

        let config: AppConfig = toml::from_str(DEFAULT_CONFIG)
            .map_err(|error| AppError::message(format!("invalid built-in config: {error}")))?;
        Ok(LoadedConfig { config, path: None })
    }

    pub fn fallback() -> LoadedConfig {
        let config = toml::from_str(DEFAULT_CONFIG).unwrap_or_default();
        LoadedConfig { config, path: None }
    }

    pub fn create_user_config() -> Result<PathBuf, AppError> {
        let path = user_config_path().ok_or_else(|| {
            AppError::message("Could not locate the user configuration directory")
        })?;
        let parent = path
            .parent()
            .ok_or_else(|| AppError::message("Configuration path has no parent directory"))?;
        fs::create_dir_all(parent)?;

        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => file.write_all(DEFAULT_CONFIG.as_bytes())?,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }

        Ok(path)
    }

    fn candidate_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();

        if let Ok(executable) = env::current_exe()
            && let Some(directory) = executable.parent()
        {
            paths.push(directory.join("config.toml"));
        }

        if let Ok(current_dir) = env::current_dir() {
            paths.push(current_dir.join("config.toml"));
        }

        if let Some(path) = user_config_path() {
            paths.push(path);
        }

        paths
    }
}

fn user_config_path() -> Option<PathBuf> {
    let config_home = env::var_os("XDG_CONFIG_HOME")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .or_else(|| home_dir().map(|home| home.join(".config")))?;
    Some(config_home.join("xExtract").join("config.toml"))
}

fn home_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        env::var_os("USERPROFILE").map(PathBuf::from)
    } else {
        env::var_os("HOME").map(PathBuf::from)
    }
}

#[cfg(test)]
mod tests {
    use super::ConfigLoader;

    #[test]
    fn built_in_config_supports_boolean_and_language_filters() {
        let loaded = ConfigLoader::fallback();
        assert_eq!(loaded.config.profiles.len(), 4);
        assert!(loaded.config.profiles[0].chapters_enabled());
        assert!(loaded.config.profiles[0].output_dir.is_none());
        assert!(!loaded.config.profiles[1].audio_filter().enabled);
        assert_eq!(
            loaded.config.profiles[2].audio_filter().languages,
            vec!["pt-BR", "por"]
        );
        assert_eq!(
            loaded.config.patterns.subtitle_tracks,
            "{FilenameNoExt}.Track{TrackID:00}.{Language}.{Flags}"
        );
    }
}
