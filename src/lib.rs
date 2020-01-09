//! This is an experimental high-level Rust binding to
//! [Janus Gateway](https://github.com/meetecho/janus-gateway)'s application plugin API.
//!
//! **WARNING!** This project is an experimental and not tested WIP.
//! Don't try to use it for creating actual plugins.
//!
//! # The concept
//!
//! There's already [janus-plugin](https://github.com/mozilla/janus-plugin-rs) crate which enables
//! creating plugins for Janus Gateway but its API is
//! [too low-level and unsafe](https://github.com/mozilla/janus-plugin-rs/issues/10).
//!
//! This crate enables writing plugins in a more idiomatic Rust way:
//!
//! * Plugin code has nothing to do with raw pointers and unsafe C functions. These things are abstracted out by the crate.
//! * A plugin is a trait implementation, not a bunch of `extern "C"` functions.
//! * Object-oriented API instead of procedural.
//! * A plugin and each of its handles may have their state.
//! * Plugin handles' core C state is not mixed together with plugin's Rust state.
//! * Dispatching callbacks from other plugin's threads is thread safe. This enables the plugin to handle messages and media events in an asynchronous non-blocking way.
//! * [Serde](https://github.com/serde-rs/serde) library is being used for (de)serialization within the plugin as de-facto Rust's standard. No need to tackle C's [Jansson](https://github.com/akheron/jansson) that is being used on the low-level API.
//! * Unit testing is possible for plugins because they aren't coupled to C code.
//!
//!
//! # How to write a plugin
//!
//! The simpliest way is to copy-paste and change the
//! [example](https://github.com/feymartynov/janus-app/tree/master/example) plugin but here
//! is the explanation of how to write it from scratch.
//!
//! ## Creating a project
//!
//! At first you need to create a project with `cargo new my_plugin --lib`.
//!
//! The plugin should be compiled as dynamically linked C library so add to your `Cargo.toml`:
//!
//! ```toml
//! [lib]
//! crate-type = ["cdylib"]
//! ```
//!
//! Then then add `janus_app` to dependencies.
//! The library is in its early development state and it's not yet released on crates.io so you
//! have to pull from GitHub. Add to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! janus-app = "*"
//!
//! [patch.crates-io]
//! janus-app = { git = "https://github.com/feymartynov/janus-app" }
//! ```
//!
//! We will also need serde family dependencies to deal with JSON messages.
//! So add to `[dependencies]` section:
//!
//! ```toml
//! serde = "1.0"
//! serde_derive = "1.0"
//! serde_json = "1.0"
//! ```
//!
//!
//! ## Definining a plugin
//!
//! To write a plugin you need to define a public struct and implement [Plugin](trait.Plugin.html)
//! trait on it. After that call [janus_plugin](macro.janus_plugin.html) macro on that type.
//!
//! In your `src/lib.rs` add:
//!
//! ```rust
//! use janus_app::{janus_plugin, Plugin};
//!
//! pub struct MyPlugin {
//! }
//!
//! impl Plugin for MyPlugin {
//!   type Handle = MyHandle;
//!
//!   const VERSION: i32 = 1;
//!   const VERSION_STRING: &'static str = "0.0.1";
//!   const NAME: &'static str = "Author name";
//!   const DESCRIPTION: &'static str = "My plugin description";
//!   const PACKAGE: &'static str = "janus.plugin.my_plugin";
//!
//!   fn init(_config_path: &Path) -> Result<Box<Self>, Error> {
//!     Ok(Box::new(Self {}))
//!   }
//!
//!   fn build_handle(&self, id: u64) -> Self::Handle {
//!     Self::Handle::new(id)
//!   }
//! }
//!
//! janus_plugin!(MyPlugin);
//! ```
//!
//! The [Plugin](trait.Plugin.html) trait requires to define an
//! [associated type for plugin handle](trait.Plugin.html#associatedtype.Handle) which we'll define
//! later, [plugin info constants](trait.Plugin.html#associated-const),
//! [init](trait.Plugin.html#tymethod.init) function and
//! [build_handle](trait.Plugin.html#tymethod.build_handle) method.
//!
//! [init](trait.Plugin.html#tymethod.init) function is being called to create an instance of the
//! plugin. Here we can initialize the plugin state. `config_path` argument is being supplied and
//! contains config *directory* path. You may use it to read and parse the plugin's config then
//! store it in the plugin object but this is out of scope now since we're building a minimal setup.
//! See example plugin for details.
//!
//! [build_handle](trait.Plugin.html#tymethod.build_handle) method is for creating a plugin handle
//! instance. Here we may want to pass in some data from the plugin state.
//!
//!
//! ## Defining a handle
//!
//! To interact with a plugin a client must create a Janus session (that's up to Janus core)
//! and a plugin handle. Then it may send messages or media events to that handle.
//! Each handle may have its own state so we define a separate struct for it.
//!
//! First import necessary types:
//!
//! ```rust
//! use janus_app::{
//!     plugin::Callbacks, Error, IncomingMessage, MessageResponse, MediaEvent, OutgoingMessage,
//! };
//!
//! use serde_derive::{Deserialize, Serialize};
//! ```
//!
//! ### Defininng message types
//!
//! Before defining the handle type let's define message types to associate it with:
//!
//! ```rust
//! #[derive(Clone, Debug, Deserialize)]
//! #[serde(rename_all = "lowercase", tag = "method")]
//! pub enum IncomingMessagePayload {
//! }
//!
//! #[derive(Debug, Serialize)]
//! pub enum OutgoingMessagePayload {
//! }
//! ```
//!
//! [IncomingMessagePayload](trait.Handle.html#associatedtype.IncomingMessagePayload) is a enum for
//! parsing incoming messages. We did a simple routing by `method` field. Serde will parse incoming
//! JSON, look at the value of `method` field and choose the corresponding variant of the enum.
//! Then in [handle_message](trait.Handle.html#tymethod.handle_message) function we can `match`
//! on this enum and apply coresponding logic.
//!
//! [OutgoingMessagePayload](trait.Handle.html#associatedtype.OutgoingMessagePayload) is for
//! responses. We return it from [handle_message](trait.Handle.html#tymethod.handle_message) and
//! then it gets serialized to JSON by serde.
//!
//! ### Defining the handle struct
//!
//! ```rust
//! #[derive(Clone, Serialize)]
//! struct MyHandle {
//!   id: u64,
//! }
//!
//! impl MyHandle {
//!   pub(crate) fn new(id: u64) -> Self {
//!     Self { id }
//!   }
//! }
//! ```
//!
//! This is simple. We have a struct that stores the handle ID.
//! The constructor is the one we called earlier in `MyPlugin::build_handle`.
//! We may also initialize some handle state here.
//!
//! `MyHandle` is serializable because we need to serialize it when replying to `query_handle`
//! Janus's call.
//!
//!
//! ### Implementing `Handle` trait
//!
//! For our `MyHandle` type we must implement [Handle](trait.Handle.html) trait which requires
//! associated data types we've just defined, [id](trait.Handle.html#tymethod.id) getter,
//! [handle_media_event](trait.Handle.html#tymethod.handle_media_event) for handling media-related
//! things like incoming RTP/RTCP packets etc. and
//! [handle_message](trait.Handle.html#tymethod.handle_message) for handling incoming messages.
//!
//! ```rust
//! impl Handle for MyHandle {
//!   type IncomingMessagePayload = IncomingMessagePayload;
//!   type OutgoingMessagePayload = OutgoingMessagePayload;
//!
//!   fn id(&self) -> u64 {
//!     self.id
//!   }
//!
//!   fn handle_media_event(&self, _media_event: &MediaEvent) {
//!   }
//!
//!   fn handle_message(
//!     &self,
//!     _message: IncomingMessage<Self::IncomingMessagePayload>
//!   ) -> Result<MessageResponse<Self::OutgoingMessagePayload>, Error> {
//!     Ok(MessageResponse::Ack)
//!   }
//! }
//! ```
//!
//! [handle_media](trait.Handle.html#tymethod.handle_media) is the place to handle media-related
//! events like RTP/RTCP packets and so on. Check out [MediaEvent](enum.MediaEvent.html) docs
//! to see all possible variants.
//!
//! [handle_message](trait.Handle.html#tymethod.handle_message) must return an
//! [MessageResponse](enum.MessageResponse.html) variant which is
//! [Synchronous(P)](enum.MessageResponse.html#variant.Syncronous) for immediate response
//! or [Ack](enum.MessageResponse.html#variant.Ack) for deferred response.
//! In this case an ack response will be sent immediately and further event(s) on this transaction
//! may be sent using [push_event](plugin/trait.Callbacks.html#method.push_event).
//!
//!
//! ## Calling callbacks
//!
//! For calling back Janus core use [Callbacks](plugin/trait.Callbacks.html) trait which is being
//! automatically defined on a type that implements [Handle](trait.Handle.html).
//!
//! To enable you need to bring it into the scope which we've already done
//! [above](#defining-a-handle).
//!
//! Check out [Callbacks](plugin/trait.Callbacks.html) trait docs for available methods.
//!
//! [Callbacks](plugin/trait.Callbacks.html) is a generic trait so it requires the plugin type
//! as a generic type parameter so a method call looks like this:
//!
//! ```rust
//! Callbacks::<MyPlugin>::push_event(self, &message);
//! ```
//!
//!
//! ## Compiling and installing
//!
//! That's it, we're all set with the code. Now we can compile the project and copy the compiled
//! library to Janus's plugins directory.
//!
//! Assuming Janus is installed to `/opt/janus` for Linux it would be:
//!
//! ```bash
//! cargo build --release
//! cp target/release/libjanus_my_plugin.so /opt/janus/lib/janus/plugins/libjanus_my_plugin.so
//! ```

///////////////////////////////////////////////////////////////////////////////

use std::fmt;
use std::marker::Sized;
use std::path::Path;

use serde::{de, ser};
use serde_derive::{Deserialize, Serialize};

pub use error::Error;
pub use lazy_static::lazy_static;

///////////////////////////////////////////////////////////////////////////////

/// Protocol for incoming/outgoing media buffer: RTP or RTCP.
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

/// Buffer media type: video or audio.
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

/// Media-related event.
#[derive(Debug)]
pub enum MediaEvent<'a> {
    /// PeerConnection set up.
    Setup,
    /// Incoming media buffer.
    Media {
        protocol: MediaProtocol,
        kind: MediaKind,
        buffer: &'a [i8],
    },
    /// Incoming buffer from data channel.
    Data { buffer: &'a [i8] },
    /// Slow link detected by Janus core.
    SlowLink { kind: MediaKind, uplink: isize },
    /// PeerConnection hanged up.
    Hangup,
}

/// JSEP (Javascript Session Establishment Protocol) object containing
/// SDP (Session Description Protocol offer wither answer.
/// Being used for signalling.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase", tag = "type")]
pub enum Jsep {
    // TODO: Parse SDP.
    Offer { sdp: String },
    Answer { sdp: String },
}

/// Incoming message sent by Janus's `message` request.
#[derive(Debug)]
pub struct IncomingMessage<P: de::DeserializeOwned> {
    transaction: String,
    payload: P,
    jsep: Option<Jsep>,
}

impl<P: de::DeserializeOwned> IncomingMessage<P> {
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

/// Outgoing message to send with `push_event` callback.
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

/// Response for `IncomingMessage`.
#[derive(Debug)]
pub enum MessageResponse<P: ser::Serialize> {
    /// Immediate (synchronous) response with the provided payload.
    Syncronous(P),
    /// Deferred (asynchronous) response using
    /// [push_event](plugin/trait.Callbacks.html#method.push_event) later on.
    Ack,
}

/// Plugin handle trait.
pub trait Handle: Clone + Sized + ser::Serialize {
    type IncomingMessagePayload: de::DeserializeOwned;
    type OutgoingMessagePayload: ser::Serialize;

    /// Handle ID getter.
    fn id(&self) -> u64;

    /// Media event handler.
    fn handle_media_event(&self, media_event: &MediaEvent);

    /// Incoming message handler.
    fn handle_message(
        &self,
        message: IncomingMessage<Self::IncomingMessagePayload>,
    ) -> Result<MessageResponse<Self::OutgoingMessagePayload>, Error>;
}

/// The trait to define a plugin.
pub trait Plugin {
    /// The plugin handle type.
    type Handle: Handle;

    /// Numeric plugin version.
    /// Increment this with each release no matter whether it's major or minor.
    const VERSION: i32;

    /// Semantic plugin version as string literal.
    const VERSION_STRING: &'static str;

    /// Plugin name as string literal.
    const NAME: &'static str;

    /// Plugin description as string literal.
    const DESCRIPTION: &'static str;

    /// Plugin author name as string literal.
    const AUTHOR: &'static str;

    /// Package name as string literal.
    /// This value must be used by clients to create a plugin handle with Janus's `attach` call.
    const PACKAGE: &'static str;

    /// This is being called when initializing the plugin to create its instance.
    ///
    /// `config_path` is a path to the *directory* with configs.
    fn init(config_path: &Path) -> Result<Box<Self>, Error>;

    /// A method to build a handle object.
    /// Being called when a client calls Janus's `attach` method.
    fn build_handle(&self, id: u64) -> Self::Handle;
}

///////////////////////////////////////////////////////////////////////////////

mod error;
mod ffi;
pub mod plugin;
