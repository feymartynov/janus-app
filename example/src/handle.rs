use std::sync::Arc;

use futures::executor::ThreadPool;
use janus_app::{
    plugin::CallbackDispatcher, Error, IncomingMessage, IncomingMessageResponse, MediaEvent,
    OutgoingMessage,
};
use serde_derive::{Deserialize, Serialize};

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
pub struct Event {}

#[derive(Clone, Serialize)]
pub struct Handle {
    id: u64,
    #[serde(skip)]
    callback_dispatcher: CallbackDispatcher<Self>,
    #[serde(skip)]
    thread_pool: Arc<ThreadPool>,
}

impl Handle {
    pub(crate) fn new(
        id: u64,
        callback_dispatcher: CallbackDispatcher<Self>,
        thread_pool: Arc<ThreadPool>,
    ) -> Self {
        Self {
            id,
            callback_dispatcher,
            thread_pool,
        }
    }
}

impl janus_app::Handle for Handle {
    type IncomingMessagePayload = IncomingMessagePayload;
    type OutgoingMessagePayload = OutgoingMessagePayload;
    type Event = Event;

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

        let future = match message.payload() {
            IncomingMessagePayload::Ping { data } => pong(
                self.callback_dispatcher.clone(),
                message.transaction().to_owned(),
                data.to_owned(),
            ),
        };

        self.thread_pool.spawn_ok(future);
        Ok(IncomingMessageResponse::Ack)
    }
}

async fn pong(dispatcher: CallbackDispatcher<Handle>, transaction: String, data: String) {
    dispatcher
        .push_event(OutgoingMessage::new(
            transaction,
            OutgoingMessagePayload::Pong { data },
        ))
        .unwrap_or_else(|err| println!("{}", err));
}
