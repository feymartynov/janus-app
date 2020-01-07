use std::fmt;
use std::marker::Sized;
use std::path::Path;

use serde::{de, ser};
use serde_derive::{Deserialize, Serialize};

use plugin::CallbackDispatcher;

pub use error::Error;

///////////////////////////////////////////////////////////////////////////////

#[derive(Clone, Copy, Debug)]
pub enum MediaProtocol {
    Rtp,
    Rtcp,
}

impl fmt::Display for MediaProtocol {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Rtp => write!(fmt, "RTP"),
            Self::Rtcp => write!(fmt, "RTCP"),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum MediaKind {
    Video,
    Audio,
}

impl fmt::Display for MediaKind {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Video => write!(fmt, "videp"),
            Self::Audio => write!(fmt, "audio"),
        }
    }
}

#[derive(Debug)]
pub enum MediaEvent<'a> {
    Setup,
    Media {
        protocol: MediaProtocol,
        kind: MediaKind,
        buffer: &'a [i8],
    },
    Data {
        buffer: &'a [i8],
    },
    SlowLink {
        kind: MediaKind,
        uplink: isize,
    },
    Hangup,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase", tag = "type")]
pub enum Jsep {
    // TODO: Parse SDP.
    Offer { sdp: String },
    Answer { sdp: String },
}

#[derive(Debug)]
pub struct IncomingMessage<P: Clone + de::DeserializeOwned> {
    transaction: String,
    payload: P,
    jsep: Option<Jsep>,
}

impl<P: Clone + de::DeserializeOwned> IncomingMessage<P> {
    pub fn new(transaction: String, payload: P) -> Self {
        Self {
            transaction,
            payload,
            jsep: None,
        }
    }

    pub fn set_jsep(self, jsep: Jsep) -> Self {
        Self {
            jsep: Some(jsep),
            ..self
        }
    }

    pub fn transaction(&self) -> &str {
        &self.transaction
    }

    pub fn payload(&self) -> &P {
        &self.payload
    }

    pub fn jsep(&self) -> Option<&Jsep> {
        self.jsep.as_ref()
    }
}

#[derive(Debug, Serialize)]
pub struct OutgoingMessage<P: ser::Serialize> {
    transaction: String,
    payload: P,
    jsep: Option<Jsep>,
}

impl<P: ser::Serialize> OutgoingMessage<P> {
    pub fn new(transaction: String, payload: P) -> Self {
        Self {
            transaction,
            payload,
            jsep: None,
        }
    }

    pub fn set_jsep(self, jsep: Jsep) -> Self {
        Self {
            jsep: Some(jsep),
            ..self
        }
    }

    pub fn transaction(&self) -> &str {
        &self.transaction
    }

    pub fn payload(&self) -> &P {
        &self.payload
    }

    pub fn jsep(&self) -> Option<&Jsep> {
        self.jsep.as_ref()
    }
}

#[derive(Debug)]
pub enum IncomingMessageResponse<P: ser::Serialize> {
    Ack,
    Syncronous(P),
}

pub trait Handle: Clone + Sized + ser::Serialize {
    type IncomingMessagePayload: Clone + de::DeserializeOwned;
    type OutgoingMessagePayload: Send + ser::Serialize;
    type Event: Send + ser::Serialize;

    fn id(&self) -> u64;
    fn handle_media_event(&self, media_event: &MediaEvent);

    fn handle_message(
        &self,
        message: &IncomingMessage<Self::IncomingMessagePayload>,
    ) -> Result<IncomingMessageResponse<Self::OutgoingMessagePayload>, Error>;
}

pub trait Plugin {
    type Handle: Handle;

    fn version() -> i32;
    fn version_string() -> &'static str;
    fn description() -> &'static str;
    fn name() -> &'static str;
    fn author() -> &'static str;
    fn package() -> &'static str;
    fn is_events_enabled() -> bool;
    fn init(config_path: &Path) -> Result<Box<Self>, Error>;

    fn build_handle(
        &self,
        id: u64,
        callback_dispatcher: CallbackDispatcher<Self::Handle>,
    ) -> Self::Handle;
}

///////////////////////////////////////////////////////////////////////////////

mod error;
mod ffi;
pub mod plugin;
