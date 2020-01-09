use std::sync::Arc;

use futures::executor::ThreadPool;
use janus_app::{
    plugin::Callbacks, Error, IncomingMessage, MediaEvent, MessageResponse, OutgoingMessage,
};
use serde_derive::{Deserialize, Serialize};

use crate::config::Config;
use crate::ExamplePlugin;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "lowercase", tag = "method")]
pub enum IncomingMessagePayload {
    Ping { data: String },
}

#[derive(Debug, Serialize)]
pub enum OutgoingMessagePayload {
    Pong { data: String },
}

#[derive(Clone, Serialize)]
pub struct Handle {
    id: u64,
    #[serde(skip)]
    config: Arc<Config>,
    #[serde(skip)]
    thread_pool: Arc<ThreadPool>,
}

impl Handle {
    pub(crate) fn new(id: u64, config: Arc<Config>, thread_pool: Arc<ThreadPool>) -> Self {
        Self {
            id,
            config,
            thread_pool,
        }
    }
}

impl janus_app::Handle for Handle {
    type IncomingMessagePayload = IncomingMessagePayload;
    type OutgoingMessagePayload = OutgoingMessagePayload;

    fn id(&self) -> u64 {
        self.id
    }

    fn handle_media_event(&self, media_event: &MediaEvent) {
        match media_event {
            MediaEvent::Setup => {
                println!("Media setup");
            }
            MediaEvent::Media {
                protocol,
                kind,
                buffer,
            } => {
                println!("Got {} bytes of {} by {}", buffer.len(), kind, protocol);
            }
            MediaEvent::Data { buffer } => {
                println!("Got {} bytes of data", buffer.len());
            }
            MediaEvent::SlowLink { kind, uplink } => {
                println!("Slow link on {} media: {}", kind, uplink);
            }
            MediaEvent::Hangup => {
                println!("Media hangup");
            }
        }
    }

    fn handle_message(
        &self,
        message: IncomingMessage<Self::IncomingMessagePayload>,
    ) -> Result<MessageResponse<Self::OutgoingMessagePayload>, Error> {
        let id = self.id();

        let future = async move {
            use janus_app::plugin::PluginApp;

            // TODO: Add a more beautiful way to get plugin handle by ID.
            match ExamplePlugin::app().read() {
                Err(err) => println!("Failed to acquire app read lock: {}", err),
                Ok(app_ref) => match &*app_ref {
                    None => println!("Plugin not initialized"),
                    Some(app) => match app.handle(id) {
                        None => println!("Handle {} not found", id),
                        Some(handle) => match message.payload() {
                            IncomingMessagePayload::Ping { ref data } => {
                                handle.ping(message.transaction(), data);
                            }
                        },
                    },
                },
            }
        };

        self.thread_pool.spawn_ok(future);
        Ok(MessageResponse::Ack)
    }
}

impl Handle {
    fn ping(&self, transaction: &str, data: &str) {
        let message = OutgoingMessage::new(
            transaction.to_owned(),
            OutgoingMessagePayload::Pong {
                data: format!("{} {}", data, self.config.ping_response),
            },
        );

        if let Err(err) = Callbacks::<ExamplePlugin>::push_event(self, &message) {
            println!("{}", err);
        }
    }
}
