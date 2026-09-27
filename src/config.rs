use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
        }
    }
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    3050
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct AppConfig {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub stations: HashMap<String, String>,
}

impl AppConfig {
    pub fn load<P: AsRef<Path>>(custom_path: Option<P>) -> Result<Self, Box<dyn std::error::Error>> {
        let candidate_paths = if let Some(p) = custom_path {
            vec![p.as_ref().to_path_buf()]
        } else {
            vec![
                std::path::PathBuf::from("config.toml"),
                std::path::PathBuf::from("config.example.toml"),
            ]
        };

        for path in candidate_paths {
            if path.exists() {
                let content = std::fs::read_to_string(&path)?;
                let config: AppConfig = toml::from_str(&content)?;
                tracing::info!("Loaded configuration from {}", path.display());
                return Ok(config);
            }
        }

        tracing::warn!("No config.toml or config.example.toml found, using default configuration");
        Ok(AppConfig::default())
    }
}
