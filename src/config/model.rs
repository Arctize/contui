use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub docker: DockerConfig,
    #[serde(default)]
    pub keybindings: KeyBindings,
    #[serde(default)]
    pub registries: Vec<Registry>,
    #[serde(default)]
    pub monitoring: MonitoringConfig,
    #[serde(default)]
    pub logging: LogConfig,
    #[serde(default)]
    pub update: UpdateConfig,
}

/// General application settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    #[serde(default = "default_poll_interval")]
    pub poll_interval_ms: u64,
    #[serde(default = "default_metrics_retention")]
    pub metrics_retention_seconds: u64,
    #[serde(default = "default_log_tail")]
    pub default_log_tail: u64,
    #[serde(default)]
    pub timezone: String,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            poll_interval_ms: default_poll_interval(),
            metrics_retention_seconds: default_metrics_retention(),
            default_log_tail: default_log_tail(),
            timezone: "UTC".to_string(),
        }
    }
}

/// UI customization settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default)]
    pub colors: CustomColors,
    #[serde(default)]
    pub layout: LayoutConfig,
    #[serde(default = "default_true")]
    pub mouse_enabled: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            colors: CustomColors::default(),
            layout: LayoutConfig::default(),
            mouse_enabled: true,
        }
    }
}

/// Custom color overrides
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CustomColors {
    pub running: Option<String>,
    pub stopped: Option<String>,
    pub paused: Option<String>,
    pub healthy: Option<String>,
    pub unhealthy: Option<String>,
    pub selection: Option<String>,
}

/// Layout configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LayoutConfig {
    pub sidebar_width: Option<u16>,
    pub log_panel_height: Option<u16>,
    #[serde(default = "default_true")]
    pub show_header: bool,
    #[serde(default = "default_true")]
    pub show_footer: bool,
}

/// Docker connection settings
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DockerConfig {
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub tls_verify: bool,
    #[serde(default)]
    pub cert_path: Option<PathBuf>,
    #[serde(default)]
    pub compose_files: Vec<String>,
}

/// Keybinding configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KeyBindings {
    #[serde(default)]
    pub global: HashMap<String, String>,
    #[serde(default)]
    pub containers: HashMap<String, String>,
    #[serde(default)]
    pub images: HashMap<String, String>,
    #[serde(default)]
    pub logs: HashMap<String, String>,
}

/// Registry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Registry {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub auth: Option<RegistryAuth>,
    #[serde(default)]
    pub is_default: bool,
}

/// Registry authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryAuth {
    pub username: String,
    #[serde(skip_serializing)]
    pub password: Option<String>,
    #[serde(skip_serializing)]
    pub token: Option<String>,
}

/// Monitoring and alerting settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    #[serde(default)]
    pub alerts_enabled: bool,
    #[serde(default)]
    pub cpu_threshold: Option<f64>,
    #[serde(default)]
    pub memory_threshold: Option<f64>,
    #[serde(default = "default_cooldown")]
    pub alert_cooldown_seconds: u64,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            alerts_enabled: false,
            cpu_threshold: None,
            memory_threshold: None,
            alert_cooldown_seconds: default_cooldown(),
        }
    }
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default)]
    pub file: Option<PathBuf>,
    #[serde(default = "default_max_size")]
    pub max_size_mb: u64,
    #[serde(default = "default_max_files")]
    pub max_files: u32,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            file: None,
            max_size_mb: default_max_size(),
            max_files: default_max_files(),
        }
    }
}

/// Update check frequency
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CheckFrequency {
    /// Check on every startup
    Always,
    /// Check once per day (default)
    #[default]
    Daily,
    /// Check once per week
    Weekly,
    /// Never check automatically
    Never,
}

/// Update checking configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConfig {
    /// Enable/disable automatic update checks on startup
    #[serde(default = "default_true")]
    pub check_on_startup: bool,
    /// How often to check for updates
    #[serde(default)]
    pub check_frequency: CheckFrequency,
    /// Network timeout in seconds for update checks
    #[serde(default = "default_update_timeout")]
    pub timeout_seconds: u64,
    /// Whether to prompt the user to install updates
    #[serde(default = "default_true")]
    pub prompt_to_install: bool,
    /// Skip a specific version (user chose to skip)
    #[serde(default)]
    pub skip_version: Option<String>,
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self {
            check_on_startup: true,
            check_frequency: CheckFrequency::default(),
            timeout_seconds: default_update_timeout(),
            prompt_to_install: true,
            skip_version: None,
        }
    }
}

// Default value functions
fn default_poll_interval() -> u64 {
    1000
}

fn default_metrics_retention() -> u64 {
    3600
}

fn default_log_tail() -> u64 {
    1000
}

fn default_theme() -> String {
    "auto".to_string()
}

fn default_cooldown() -> u64 {
    300
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_max_size() -> u64 {
    100
}

fn default_max_files() -> u32 {
    5
}

fn default_true() -> bool {
    true
}

fn default_update_timeout() -> u64 {
    3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_values() {
        let general = GeneralConfig::default();
        assert_eq!(general.poll_interval_ms, 1000);
        assert_eq!(general.metrics_retention_seconds, 3600);
        assert_eq!(general.default_log_tail, 1000);
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let toml_str = toml::to_string(&config).unwrap();
        assert!(!toml_str.is_empty());
    }
}
