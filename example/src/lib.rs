use std::path::Path;
use std::sync::Arc;

use futures::executor::ThreadPool;
use janus_app::{janus_plugin, plugin::CallbackDispatcher, Error, Plugin};

use crate::{config::Config, handle::Handle};

pub struct ExamplePlugin {
    #[allow(dead_code)]
    config: Arc<Config>,
    thread_pool: Arc<ThreadPool>,
}

impl ExamplePlugin {
    fn new(config: Config, thread_pool: ThreadPool) -> Self {
        Self {
            config: Arc::new(config),
            thread_pool: Arc::new(thread_pool),
        }
    }
}

impl Plugin for ExamplePlugin {
    type Handle = Handle;

    fn version() -> i32 {
        1
    }

    fn version_string() -> &'static str {
        "0.0.1"
    }

    fn description() -> &'static str {
        "Example plugin"
    }

    fn name() -> &'static str {
        "Example"
    }

    fn author() -> &'static str {
        "Fey Martynov"
    }

    fn package() -> &'static str {
        "janus.plugin.app_example"
    }

    fn is_events_enabled() -> bool {
        false
    }

    fn init(config_path: &Path) -> Result<Box<Self>, Error> {
        let config = Config::from_path(config_path)
            .map_err(|err| Error::new(&format!("Failed to load config: {}", err)))?;

        let thread_pool = ThreadPool::new()
            .map_err(|err| Error::new(&format!("Failed to start thread pool: {}", err)))?;

        let plugin = Self::new(config, thread_pool);
        println!("Example plugin initialized");
        Ok(Box::new(plugin))
    }

    fn build_handle(
        &self,
        id: u64,
        callback_dispatcher: CallbackDispatcher<Self::Handle>,
    ) -> Self::Handle {
        Handle::new(
            id,
            callback_dispatcher,
            self.config.clone(),
            self.thread_pool.clone(),
        )
    }
}

impl Drop for ExamplePlugin {
    fn drop(&mut self) {
        println!("Example plugin destroyed");
    }
}

janus_plugin!(ExamplePlugin);

mod config;
mod handle;
