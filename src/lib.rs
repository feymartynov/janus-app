use std::fmt;
use std::path::Path;

use serde::{de::DeserializeOwned, ser::Serialize};

pub use crate::error::Error;
pub use crate::handle_registry::HandleRegistry;

///////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
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

#[derive(Debug)]
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

#[derive(Debug)]
pub struct IncomingMessage<P: DeserializeOwned, J: DeserializeOwned> {
    transaction: String,
    payload: P,
    jsep: J,
}

impl<P: DeserializeOwned, J: DeserializeOwned> IncomingMessage<P, J> {
    pub fn new(transaction: String, payload: P, jsep: J) -> Self {
        Self {
            transaction,
            payload,
            jsep,
        }
    }

    pub fn transaction(&self) -> &str {
        &self.transaction
    }

    pub fn payload(&self) -> &P {
        &self.payload
    }

    pub fn jsep(&self) -> &J {
        &self.jsep
    }
}

#[derive(Debug)]
pub struct OutgoingMessage<P: Serialize, J: Serialize> {
    transaction: String,
    payload: P,
    jsep: J,
}

impl<P: Serialize, J: Serialize> OutgoingMessage<P, J> {
    pub fn new(transaction: String, payload: P, jsep: J) -> Self {
        Self {
            transaction,
            payload,
            jsep,
        }
    }

    pub fn transaction(&self) -> &str {
        &self.transaction
    }

    pub fn payload(&self) -> &P {
        &self.payload
    }

    pub fn jsep(&self) -> &J {
        &self.jsep
    }
}

#[derive(Debug)]
pub enum IncomingMessageResponse<P: Serialize> {
    Ack,
    Syncronous(P),
}

pub trait Handle: Serialize {
    fn new(id: u64) -> Self;
    fn id(&self) -> u64;
    fn handle_media_event(&self, media_event: &MediaEvent);

    fn handle_message<P: DeserializeOwned, J: DeserializeOwned, O: Serialize>(
        &self,
        message: &IncomingMessage<P, J>,
    ) -> Result<IncomingMessageResponse<O>, Error>;
}

pub trait HandleCallbacks {
    fn log(&self, message: &str);
    fn relay_media_packet(&self, protocol: MediaProtocol, kind: MediaKind, buffer: &mut [i8]);
    fn relay_data_packet(&self, buffer: &mut [i8]);
    fn close_peer_connection(&self);
    fn end(&self);
    fn notify_event<E: Serialize>(&self, event: &E);
    fn push_event<P: Serialize, J: Serialize>(
        &self,
        message: OutgoingMessage<P, J>,
    ) -> Result<(), Error>;
}

pub trait Plugin {
    fn version() -> i32;
    fn description() -> &'static str;
    fn name() -> &'static str;
    fn author() -> &'static str;
    fn package() -> &'static str;
    fn is_events_enabled() -> bool;
    fn init(config_path: &Path) -> Result<Box<Self>, Error>;
}

///////////////////////////////////////////////////////////////////////////////

mod error;
mod ffi;
mod handle_registry;
