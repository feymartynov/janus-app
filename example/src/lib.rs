use std::path::Path;
use std::sync::Arc;

use futures::executor::ThreadPool;
use janus_app::{janus_plugin, plugin::CallbackDispatcher, Error, Plugin};

use crate::{config::Config, handle::Handle};

pub struct ExamplePlugin {
    #[allow(dead_code)]
    config: Config,
    thread_pool: Arc<ThreadPool>,
}

impl ExamplePlugin {
    fn new(config: Config, thread_pool: ThreadPool) -> Self {
        Self {
            config,
            thread_pool: Arc::new(thread_pool),
        }
    }

    #[allow(dead_code)]
    fn config(&self) -> &Config {
        &self.config
    }
}

impl Plugin for ExamplePlugin {
    type Handle = Handle;

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

    fn build_handle(
        &self,
        id: u64,
        callback_dispatcher: CallbackDispatcher<Self::Handle>,
    ) -> Self::Handle {
        Handle::new(id, callback_dispatcher, self.thread_pool.clone())
    }
}

janus_plugin!(ExamplePlugin);

mod config;
mod handle;
