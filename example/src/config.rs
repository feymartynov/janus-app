use std::path::Path;

use config;
use serde_derive::Deserialize;

const CONFIG_FILE_NAME: &str = "janus.plugin.example.toml";

#[derive(Debug, Deserialize)]
pub(crate) struct Config {
    pub dummy: String,
}

impl Config {
    pub(crate) fn from_path(path: &Path) -> Result<Self, config::ConfigError> {
        let mut path_buf = path.to_path_buf();
        path_buf.push(CONFIG_FILE_NAME);
        let path_str = path_buf.to_string_lossy();

        let mut parser = config::Config::default();
        parser.merge(config::File::new(&path_str, config::FileFormat::Toml))?;
        parser.merge(config::Environment::with_prefix("APP").separator("__"))?;
        parser.try_into::<Config>()
    }
}
