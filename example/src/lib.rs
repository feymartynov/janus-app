use std::path::Path;
use std::sync::Arc;

use futures::executor::ThreadPool;
use janus_app::{janus_plugin, Error, Plugin};

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

    const VERSION: i32 = 1;
    const VERSION_STRING: &'static str = "0.0.1";
    const NAME: &'static str = "Example";
    const DESCRIPTION: &'static str = "Example plugin";
    const AUTHOR: &'static str = "Fey Martynov";
    const PACKAGE: &'static str = "janus.plugin.app_example";

    fn init(config_path: &Path) -> Result<Box<Self>, Error> {
        let config = Config::from_path(config_path)
            .map_err(|err| Error::new(&format!("Failed to load config: {}", err)))?;

        let thread_pool = ThreadPool::new()
            .map_err(|err| Error::new(&format!("Failed to start thread pool: {}", err)))?;

        let plugin = Self::new(config, thread_pool);
        println!("Example plugin initialized");
        Ok(Box::new(plugin))
    }

    fn build_handle(&self, id: u64) -> Self::Handle {
        Handle::new(id, self.config.clone(), self.thread_pool.clone())
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
