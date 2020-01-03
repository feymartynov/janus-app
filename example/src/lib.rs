use std::path::Path;

use janus_app::{janus_plugin, Error, Plugin};

use crate::{config::Config, handle::Handle};

pub struct ExamplePlugin {
    config: Config,
}

impl ExamplePlugin {
    fn new(config: Config) -> Self {
        Self { config }
    }
}

impl Plugin for ExamplePlugin {
    fn version() -> i32 {
        1
    }

    fn description() -> &'static str {
        "Example plugin"
    }

    fn name() -> &'static str {
        "Example"
    }

    fn author() -> &'static str {
        "Anonymous"
    }

    fn package() -> &'static str {
        "example"
    }

    fn is_events_enabled() -> bool {
        false
    }

    fn init(config_path: &Path) -> Result<Box<Self>, Error> {
        let config = Config::from_path(config_path)
            .map_err(|err| Error::new(&format!("Failed to load config: {}", err)))?;

        Ok(Box::new(Self::new(config)))
    }
}

janus_plugin!(ExamplePlugin, Handle);

mod config;
mod handle;
