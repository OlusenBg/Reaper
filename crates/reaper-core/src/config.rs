use serde::Deserialize;
use std::path::{Path, PathBuf};

/// User configuration, `~/.config/reaper/config.toml`. Every field is optional.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Config {
    /// Packages never suggested for removal.
    pub protected: Vec<String>,
    /// Backends to skip (e.g. `["snap"]`).
    pub disabled_backends: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("cannot read {0}: {1}")]
    Io(PathBuf, std::io::Error),
    #[error("invalid config {0}: {1}")]
    Parse(PathBuf, toml::de::Error),
}

impl Config {
    pub fn parse(s: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(s)
    }

    /// Load from `path`; a missing file yields the defaults.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        match std::fs::read_to_string(path) {
            Ok(s) => Self::parse(&s).map_err(|e| ConfigError::Parse(path.to_owned(), e)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(ConfigError::Io(path.to_owned(), e)),
        }
    }

    pub fn default_path() -> Option<PathBuf> {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
        Some(base.join("reaper").join("config.toml"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_is_default() {
        assert_eq!(Config::parse("").unwrap(), Config::default());
    }

    #[test]
    fn parses_fields() {
        let c = Config::parse("protected=[\"linux\"]\ndisabled_backends=[\"snap\"]").unwrap();
        assert_eq!(c.protected, vec!["linux"]);
        assert_eq!(c.disabled_backends, vec!["snap"]);
    }

    #[test]
    fn missing_file_is_default() {
        assert_eq!(
            Config::load(Path::new("/nonexistent/x.toml")).unwrap(),
            Config::default()
        );
    }
}
