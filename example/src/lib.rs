use std::path::Path;

use futures::executor::ThreadPool;
use janus_app::{janus_plugin, Error, Plugin};

use crate::{config::Config, event::Event, handle::Handle};

pub struct ExamplePlugin {
    #[allow(dead_code)]
    config: Config,
    thread_pool: ThreadPool,
}

impl ExamplePlugin {
    fn new(config: Config, thread_pool: ThreadPool) -> Self {
        Self {
            config,
            thread_pool,
        }
    }

    #[allow(dead_code)]
    fn config(&self) -> &Config {
        &self.config
    }

    fn thread_pool(&self) -> &ThreadPool {
        &self.thread_pool
    }
}

impl Plugin for ExamplePlugin {
    type Handle = Handle;
    type Event = Event;

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

        let thread_pool = ThreadPool::new()
            .map_err(|err| Error::new(&format!("Failed to start thread pool: {}", err)))?;

        Ok(Box::new(Self::new(config, thread_pool)))
    }
}

janus_plugin!(ExamplePlugin);

mod config;
mod event;
mod handle;
