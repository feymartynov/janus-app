//! # How do I make a plugin?
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
//! }
//!
//! janus_plugin!(MyPlugin);
//! ```
//!
//! The [Plugin](trait.Plugin.html) trait requires to define an associated type for plugin handle
//! which we'll define later, plugin info methods and also [init](trait.Plugin.html#tymethod.init)
//! and [build_handle](trait.Plugin.html#tymethod.build_handle) methods.
//!
//!
//! ###  Adding plugin info methods
//!
//! Janus core requires us to provide some info about our plugin.
//! Add the following to `impl Plugin for MyPlugin` block:
//!
//! ```rust
//! fn version() -> i32 {
//!     1
//! }
//! 
//! fn version_string() -> &'static str {
//!     "0.0.1"
//! }
//! 
//! fn description() -> &'static str {
//!     "My plugin description"
//! }
//! 
//! fn name() -> &'static str {
//!     "My plugin name"
//! }
//! 
//! fn author() -> &'static str {
//!     "Author name"
//! }
//! 
//! fn package() -> &'static str {
//!     "janus.plugin.my_plugin"
//! }
//! 
//! fn is_events_enabled() -> bool {
//!     false
//! }
//! ```
//!
//! This is pretty self-explanatory but requires some notes.
//!
//! First, each time you release a new version of your plugin, no matter major or minor, you have to
//! increment the number in [version](trait.Plugin.html#tymethod.version) function by one.
//! For your actual semantic version change the literal in
//! [version_string](trait.Plugin.html#tymethod.version_string).
//!
//! The value retuned by [package](trait.Plugin.html#tymethod.package) function is the one clients
//! must use in `plugin` parameter when creating a plugin handle.
//!
//!
//! ### Implementing [init](trait.Plugin.html#tymethod.init)
//!
//! ```rust
//! fn init(_config_path: &Path) -> Result<Box<Self>, Error> {
//!   Ok(Box::new(Self {}))
//! }
//! ```
//!
//! This is pretty straightforward. `config_path` argument is being supplied by Janus core and
//! contains config *directory* path. You may use it to read and parse the plugin's config
//! then store it in the plugin object but this is out of scope now since we're building
//! a minimal setup. See example plugin for details.
//!
//!
//! ### Implementing [build_handle](trait.Plugin.html#tymethod.build_handle)
//!
//! Finally we must provide a handler builder method. If you want to pass something from
//! the plugin's state to the handle this is the place but here we go simply as:
//!
//! ```rust
//! fn build_handle(
//!   &self,
//!   id: u64,
//!   callback_dispatcher: CallbackDispatcher<Self::Handle>,
//! ) -> Self::Handle {
//!   Handle::new(id, callback_dispatcher)
//! }
//! ```
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
//!     plugin::CallbackDispatcher, Error, IncomingMessage, IncomingMessageResponse, MediaEvent,
//!     OutgoingMessage,
//! };
//!
//! use serde_derive::{Deserialize, Serialize};
//! ```
//!
//! ### Defininng data types
//!
//! Before defining the handle type let's define some data types to associate it with:
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
//!
//! #[derive(Serialize)]
//! #[serde(rename_all = "lowercase", tag = "label")]
//! pub enum Event {
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
//! [Event](trait.Handle.html#associatedtype.Event) enum is for broadcasting events with Janus's
//! event handlers. It's being used as and argument for
//! [notify_event](plugin/struct.CallbackDispatcher.html#method.notify_event) callback.
//!
//!
//! ### Defining the handle struct
//!
//! ```rust
//! #[derive(Clone, Serialize)]
//! struct MyHandle {
//!   id: u64,
//!   #[serde(skip)]
//!   callback_dispatcher: CallbackDispatcher<Self>,
//! }
//!
//! impl MyHandle {
//!   pub(crate) fn new(id: u64, callback_dispatcher: CallbackDispatcher<Self>) -> Self {
//!     Self { id, callback_dispatcher }
//!   }
//! }
//! ```
//!
//! This is simple. We have a struct that stores the ID and the callback
//! dispatcher which is an object to call Janus's callbacks with in a thread-safe way.
//! We'll see how to use it later.
//!
//! The constructor is the one we called earlier in `MyPlugin::build_handle`.
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
//!   type Event = Event;
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
//!     _message: &IncomingMessage<Self::IncomingMessagePayload>
//!   ) -> Result<IncomingMessageResponse<Self::OutgoingMessagePayload>, Error> {
//!     Ok(IncomingMessageResponse::Ack)
//!   }
//! }
//! ```
//!
//! [handle_message](trait.Handle.html#tymethod.handle_message) must return an
//! [IncomingMessageResponse](enum.IncomingMessageResponse.html) variant which is
//! [Synchronous(P)](enum.IncomingMessageResponse.html#variant.Syncronous) for immediate response
//! or [Ack](enum.IncomingMessageResponse.html#variant.Ack) for deferred response.
//! In this case an ack response will be sent immediately and further event(s) on this transaction
//! may be sent using [push_event](plugin/struct.CallbackDispatcher.html#method.push_event).
//!
//!
//! ## Calling callbacks
//!
//! For calling back Janus core use [CallbackDispatcher](plugin/struct.CallbackDispatcher.html)
//! object was provided to `MyPlugin::build_handle` and then passed to `MyHandle::new`.
//!
//! This object is thread-safe so you can clone it to another thread and interact with Janus core
//! from there.
//!
//! See [CallbackDispatcher](plugin/struct.CallbackDispatcher.html) type docs for available methods.
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

use plugin::CallbackDispatcher;

pub use error::Error;

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
    Data {
        buffer: &'a [i8],
    },
    /// Slow link detected by Janus core.
    SlowLink {
        kind: MediaKind,
        uplink: isize,
    },
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
pub enum IncomingMessageResponse<P: ser::Serialize> {
    /// Immediate (synchronous) response with the provided payload.
    Syncronous(P),
    /// Deferred (asynchronous) response using
    /// [push_event](plugin/struct.CallbackDispatcher.html#method.push_event) later on.
    Ack,
}

/// Plugin handle trait.
pub trait Handle: Clone + Sized + ser::Serialize {
    /// Deserializable payload for [IncomingMessage](struct.IncomingMessage.html).
    type IncomingMessagePayload: Clone + de::DeserializeOwned;

    /// Serializable payload for [OutgoingMessage](struct.OutgoingMessage.html).
    type OutgoingMessagePayload: Send + ser::Serialize;

    /// Serializable event data for broadcasting with
    /// [notify_event](plugin/struct.CallbackDispatcher.html#method.notify_event) callback.
    type Event: Send + ser::Serialize;

    /// Handle ID getter.
    fn id(&self) -> u64;

    /// Media event handler.
    fn handle_media_event(&self, media_event: &MediaEvent);

    /// Incoming message handler.
    fn handle_message(
        &self,
        message: &IncomingMessage<Self::IncomingMessagePayload>,
    ) -> Result<IncomingMessageResponse<Self::OutgoingMessagePayload>, Error>;
}

/// The trait to define a plugin.
pub trait Plugin {
    /// The plugin handle type.
    type Handle: Handle;

    /// Numeric plugin version.
    /// Increment this with each release no matter whether it's major or minor.
    fn version() -> i32;

    /// Semantic plugin version as string literal.
    fn version_string() -> &'static str;

    /// Plugin description as string literal.
    fn description() -> &'static str;

    /// Plugin name as string literal.
    fn name() -> &'static str;

    /// Plugin author name as string literal.
    fn author() -> &'static str;

    /// Package name as string literal.
    /// This value must be used by clients to create a plugin handle with Janus's `attach` call.
    fn package() -> &'static str;

    /// Whether to enable events.
    fn is_events_enabled() -> bool;

    /// This is being called when initializing the plugin to create its instance.
    ///
    /// `config_path` is a path to the *directory* with configs.
    fn init(config_path: &Path) -> Result<Box<Self>, Error>;

    /// A method to build a handle object.
    /// Being called when a client calls Janus's `attach` method.
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
