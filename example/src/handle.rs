use janus_app::{Error, IncomingMessage, IncomingMessageResponse, MediaEvent, OutgoingMessage};
use serde_derive::{Deserialize, Serialize};

use crate::APP;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "lowercase", tag = "method")]
pub enum IncomingMessagePayload {
    Ping { data: String },
}

#[derive(Debug, Serialize)]
pub enum OutgoingMessagePayload {
    Pong { data: String },
}

#[derive(Serialize)]
pub struct Handle {
    id: u64,
}

impl janus_app::Handle for Handle {
    type IncomingMessagePayload = IncomingMessagePayload;
    type OutgoingMessagePayload = OutgoingMessagePayload;

    fn new(id: u64) -> Self {
        Self { id }
    }

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
        message: &IncomingMessage<Self::IncomingMessagePayload>,
    ) -> Result<IncomingMessageResponse<Self::OutgoingMessagePayload>, Error> {
        println!("Got message on transaction {}", message.transaction());

        // Send responses asynchronously for demonstration purpose.
        let future = match message.payload() {
            IncomingMessagePayload::Ping { data } => {
                self.pong(message.transaction().to_owned(), data.to_owned())
            }
        };

        // TODO: DI
        APP.with(|app_ref| match *app_ref.borrow() {
            Some(app) => app.plugin().thread_pool().spawn_ok(future),
            None => println!("Plugin not initialized"),
        });

        Ok(IncomingMessageResponse::Ack)
    }
}

impl Handle {
    async fn pong(&self, transaction: String, data: String) {
        self.push_event(OutgoingMessage::new(
            transaction.to_owned(),
            OutgoingMessagePayload::Pong { data },
        ))
        .unwrap_or_else(|err| println!("Failed to push event: {}", err));
    }
}
