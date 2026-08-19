use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// User settings for Light Stripe.
///
/// Load order:
/// 1. Built-in defaults (`Default`)
/// 2. Optional TOML file (`~/.config/light-stripe/config.toml` on Linux)
///
/// Missing file is fine — defaults are used. Invalid file prints a warning
/// and also falls back to defaults.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Config {
    /// How often the TUI refreshes data (seconds).
    #[serde(default = "default_refresh_secs")]
    pub refresh_secs: u64,

    /// Listening ports hidden in the TUI Ports tab.
    #[serde(default = "default_ignored_ports")]
    pub ignored_ports: Vec<u16>,

    /// Extra substrings treated as "dev" processes (added on top of builtins).
    #[serde(default)]
    pub extra_dev_markers: Vec<String>,

    #[serde(default)]
    pub docker_host: Option<String>,
}

fn default_refresh_secs() -> u64 {
    3
}

fn default_ignored_ports() -> Vec<u16> {
    vec![53, 323, 5353, 0]
}

impl Default for Config {
    fn default() -> Self {
        Self {
            refresh_secs: default_refresh_secs(),
            ignored_ports: default_ignored_ports(),
            extra_dev_markers: Vec::new(),
            docker_host: None,
        }
    }
}

impl Config {
    /// Trimmed docker host override, if set
    pub fn docker_host(&self) -> Option<&str> {
        self.docker_host
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
    }
}

/// Standard config path for this OS, e.g. `~/.config/light-stripe/config.toml`.
pub fn config_path() -> PathBuf {
    if let Some(dirs) = ProjectDirs::from("dev", "light-stripe", "light-stripe") {
        return dirs.config_dir().join("config.toml");
    }
    PathBuf::from("light-stripe.toml")
}

/// Load config from the default path.
pub fn load() -> Config {
    load_from(&config_path())
}

/// Load config from an explicit path (CLI `--config`).
pub fn load_from(path: &std::path::Path) -> Config {
    let Ok(text) = fs::read_to_string(path) else {
        return Config::default();
    };

    match toml::from_str::<Config>(&text) {
        Ok(config) => config,
        Err(error) => {
            eprintln!(
                "light-stripe: invalid config at {}: {error}; using defaults",
                path.display()
            );
            Config::default()
        }
    }
}

/// Pretty-print effective config (for `light-stripe config`).
pub fn print_effective(config: &Config) {
    let path = config_path();
    println!("config path: {}", path.display());
    if path.is_file() {
        println!("status:      loaded from file");
    } else {
        println!("status:      file not found, using defaults");
    }
    println!();
    match toml::to_string_pretty(config) {
        Ok(text) => print!("{text}"),
        Err(error) => eprintln!("failed to serialize config: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn default_values() {
        let config = Config::default();
        assert_eq!(config.refresh_secs, 3);
        assert_eq!(config.ignored_ports, vec![53, 323, 5353, 0]);
        assert!(config.extra_dev_markers.is_empty());
    }

    #[test]
    fn missing_file_uses_defaults() {
        let config = load_from(std::path::Path::new(
            "/tmp/light-stripe-does-not-exist-12345.toml",
        ));
        assert_eq!(config, Config::default());
    }

    #[test]
    fn loads_partial_toml_with_serde_defaults() {
        let dir = std::env::temp_dir();
        let path = dir.join("light-stripe-config-test.toml");
        let mut file = fs::File::create(&path).expect("create temp config");
        writeln!(file, "refresh_secs = 5").expect("write config");
        drop(file);

        let config = load_from(&path);
        let _ = fs::remove_file(&path);

        assert_eq!(config.refresh_secs, 5);
        // fields omitted in file keep defaults
        assert_eq!(config.ignored_ports, vec![53, 323, 5353, 0]);
        assert!(config.extra_dev_markers.is_empty());
    }

    #[test]
    fn loads_extra_markers() {
        let dir = std::env::temp_dir();
        let path = dir.join("light-stripe-config-markers-test.toml");
        let mut file = fs::File::create(&path).expect("create temp config");
        write!(
            file,
            r#"
refresh_secs = 3
ignored_ports = [53, 5353]
extra_dev_markers = ["webpack-dev-server", "my-legacy-app"]
"#
        )
        .expect("write config");
        drop(file);

        let config = load_from(&path);
        let _ = fs::remove_file(&path);

        assert_eq!(config.refresh_secs, 3);
        assert_eq!(config.ignored_ports, vec![53, 5353]);
        assert_eq!(
            config.extra_dev_markers,
            vec!["webpack-dev-server", "my-legacy-app"]
        );
    }
}
