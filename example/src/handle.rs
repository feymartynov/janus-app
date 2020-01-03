use janus_app::{Error, IncomingMessage, IncomingMessageResponse, MediaEvent};
use serde::{de, ser};
use serde_derive::Serialize;

#[derive(Serialize)]
pub struct Handle {
    id: u64,
}

impl janus_app::Handle for Handle {
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

    fn handle_message<P: de::DeserializeOwned, J: de::DeserializeOwned, O: ser::Serialize>(
        &self,
        message: &IncomingMessage<P, J>,
    ) -> Result<IncomingMessageResponse<O>, Error> {
        println!("Got message on transaction {}", message.transaction());
        Ok(IncomingMessageResponse::Ack)
    }
}
