use std::convert::TryInto;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::path::Path;
use std::sync::{
    atomic::{AtomicPtr, Ordering},
    RwLock,
};

use jansson_sys::{json_dumps, json_loads, json_t};
use janus_plugin_sys::plugin::{
    janus_callbacks as JanusCallbacks, janus_plugin_result as JanusPluginResult,
    janus_plugin_result_type as JanusPluginResultType, janus_plugin_session as JanusPluginSession,
};
use serde::{de::DeserializeOwned, ser::Serialize};

use crate::{
    Error, Handle, IncomingMessage, Jsep, MediaEvent, MediaKind, MediaProtocol, MessageResponse,
    OutgoingMessage, Plugin,
};
use handle_registry::HandleRegistry;

pub use janus_plugin_sys::plugin::janus_plugin as JanusPlugin;

///////////////////////////////////////////////////////////////////////////////

/// This macro defines low-level stuff to make the project a proper Janus plugin.
///
/// Call it in the main module of you project with a type that implements `Plugin` trait.
#[macro_export]
macro_rules! janus_plugin {
    ($plugin:ty) => {
        const JANUS_PLUGIN: janus_app::plugin::JanusPlugin = janus_app::plugin::JanusPlugin {
            init: janus_app::plugin::init::<$plugin>,
            destroy: janus_app::plugin::destroy::<$plugin>,
            get_api_compatibility: janus_app::plugin::get_api_compatibility,
            get_version: janus_app::plugin::get_version::<$plugin>,
            get_version_string: janus_app::plugin::get_version_string::<$plugin>,
            get_description: janus_app::plugin::get_description::<$plugin>,
            get_name: janus_app::plugin::get_name::<$plugin>,
            get_author: janus_app::plugin::get_author::<$plugin>,
            get_package: janus_app::plugin::get_package::<$plugin>,
            create_session: janus_app::plugin::create_session::<$plugin>,
            handle_message: janus_app::plugin::handle_message::<$plugin>,
            setup_media: janus_app::plugin::setup_media::<$plugin>,
            incoming_rtp: janus_app::plugin::incoming_rtp::<$plugin>,
            incoming_rtcp: janus_app::plugin::incoming_rtcp::<$plugin>,
            incoming_data: janus_app::plugin::incoming_data::<$plugin>,
            slow_link: janus_app::plugin::slow_link::<$plugin>,
            hangup_media: janus_app::plugin::hangup_media::<$plugin>,
            destroy_session: janus_app::plugin::destroy_session::<$plugin>,
            query_session: janus_app::plugin::query_session::<$plugin>,
        };

        janus_app::lazy_static! {
            static ref APP: std::sync::RwLock<Option<janus_app::plugin::App<$plugin>>> =
                std::sync::RwLock::new(None);
        }

        impl janus_app::plugin::PluginApp for $plugin {
            fn janus_plugin() -> *mut janus_app::plugin::JanusPlugin {
                &mut JANUS_PLUGIN
            }

            fn app() -> &'static std::sync::RwLock<Option<janus_app::plugin::App<$plugin>>> {
                &APP
            }
        }

        // Required by Janus Gateway core to initialize the plugin.
        #[no_mangle]
        pub extern "C" fn create() -> *const janus_app::plugin::JanusPlugin {
            &JANUS_PLUGIN
        }
    };
}

macro_rules! c_str {
    ($s:expr) => {
        unsafe { CStr::from_ptr($s.as_ptr() as *const c_char) }
    };
}

///////////////////////////////////////////////////////////////////////////////

pub struct App<P: PluginApp> {
    plugin: P,
    janus_callbacks: AtomicPtr<JanusCallbacks>,
    handle_registry: HandleRegistry<P>,
}

impl<P: PluginApp> App<P> {
    fn new(plugin: P, janus_callbacks: *mut JanusCallbacks) -> Self {
        Self {
            plugin,
            janus_callbacks: AtomicPtr::new(janus_callbacks),
            handle_registry: HandleRegistry::<P>::new(),
        }
    }

    pub fn plugin(&self) -> &P {
        &self.plugin
    }

    fn handle_registry(&self) -> &HandleRegistry<P> {
        &self.handle_registry
    }

    fn handle_registry_mut(&mut self) -> &mut HandleRegistry<P> {
        &mut self.handle_registry
    }

    fn janus_callbacks(&self) -> *mut JanusCallbacks {
        self.janus_callbacks.load(Ordering::Relaxed)
    }

    fn build_handle(&self, id: u64) -> P::Handle {
        self.plugin().build_handle(id)
    }

    pub fn handle(&self, id: u64) -> Option<&P::Handle> {
        self.handle_registry
            .get_by_id(id)
            .map(|entry| entry.plugin_handle())
    }

    pub fn handle_mut(&mut self, id: u64) -> Option<&mut P::Handle> {
        self.handle_registry
            .get_by_id_mut(id)
            .map(|entry| entry.plugin_handle_mut())
    }
}

pub trait PluginApp: 'static + Send + Sized + Plugin {
    fn janus_plugin() -> *mut JanusPlugin;
    fn app() -> &'static RwLock<Option<App<Self>>>;
}

///////////////////////////////////////////////////////////////////////////////

pub extern "C" fn get_api_compatibility() -> c_int {
    13
}

pub extern "C" fn get_version<P: Plugin>() -> c_int {
    P::VERSION
}

pub extern "C" fn get_version_string<P: Plugin>() -> *const c_char {
    c_str!(P::VERSION_STRING).as_ptr()
}

pub extern "C" fn get_description<P: Plugin>() -> *const c_char {
    c_str!(P::DESCRIPTION).as_ptr()
}

pub extern "C" fn get_name<P: Plugin>() -> *const c_char {
    c_str!(P::NAME).as_ptr()
}

pub extern "C" fn get_author<P: Plugin>() -> *const c_char {
    c_str!(P::AUTHOR).as_ptr()
}

pub extern "C" fn get_package<P: Plugin>() -> *const c_char {
    c_str!(P::PACKAGE).as_ptr()
}

pub extern "C" fn init<P: PluginApp>(
    callbacks: *mut JanusCallbacks,
    config_path: *const c_char,
) -> c_int {
    match init_impl::<P>(callbacks, config_path) {
        Ok(()) => 0,
        Err(err) => {
            janus_log(err.as_str());
            1
        }
    }
}

fn init_impl<P: PluginApp>(
    callbacks: *mut JanusCallbacks,
    config_path: *const c_char,
) -> Result<(), Error> {
    let mut app_ref = P::app()
        .write()
        .map_err(|err| Error::new(&format!("Failed to acquire app write lock: {}", err)))?;

    if (*app_ref).is_some() {
        return Err(Error::new("Plugin already initialized"));
    }

    let config_path = unsafe { CStr::from_ptr(config_path) }
        .to_str()
        .map_err(|err| Error::new(&format!("Failed to cast config path: {}", err)))?;

    let plugin = P::init(&Path::new(config_path))
        .map_err(|err| Error::new(&format!("Failed to init plugin: {}", err)))?;

    *app_ref = Some(App::new(*plugin, unsafe { &mut *callbacks }));
    Ok(())
}

pub extern "C" fn destroy<P: PluginApp>() {
    match P::app().write() {
        Ok(mut app_ref) => *app_ref = None,
        Err(err) => janus_log(&format!("Failed to acquire app write lock: {}", err)),
    }
}

pub extern "C" fn create_session<P: PluginApp>(handle: *mut JanusPluginSession, error: *mut c_int) {
    let return_code = match create_session_impl::<P>(handle) {
        Ok(()) => 0,
        Err(err) => {
            janus_log(err.as_str());
            1
        }
    };

    unsafe { *error = return_code };
}

fn create_session_impl<P: PluginApp>(raw_handle: *mut JanusPluginSession) -> Result<(), Error> {
    let mut app_ref = P::app()
        .write()
        .map_err(|err| Error::new(&format!("Failed to acquire app write lock: {}", err)))?;

    match &mut *app_ref {
        None => Err(Error::new("Plugin not initialized")),
        Some(app) => {
            let handle_id = HandleRegistry::<P>::fetch_id(raw_handle);
            let plugin_handle = app.build_handle(handle_id);
            let handle_registry = app.handle_registry_mut();

            match handle_registry.get_by_raw_handle(raw_handle) {
                Some(_) => Err(Error::new("Handle already registered")),
                None => handle_registry
                    .add(raw_handle, plugin_handle)
                    .map(|_| ())
                    .map_err(|err| Error::new(&format!("Failed to register handle: {}", err))),
            }
        }
    }
}

pub extern "C" fn handle_message<P: PluginApp>(
    raw_handle: *mut JanusPluginSession,
    transaction: *mut c_char,
    payload: *mut json_t,
    jsep: *mut json_t,
) -> *mut JanusPluginResult {
    let mut plugin_result = match handle_message_impl::<P>(raw_handle, transaction, payload, jsep) {
        Ok(res) => res,
        Err(err) => {
            janus_log(err.as_str());

            let text = CString::new(err.as_str()).unwrap_or_else(|ref err| {
                janus_log(&format!("Failed to cast error message text: {}", err));
                CString::new("").expect("Failed to cast text")
            });

            JanusPluginResult {
                type_: JanusPluginResultType::JANUS_PLUGIN_ERROR,
                text: text.into_raw(),
                content: std::ptr::null_mut(),
            }
        }
    };

    &mut plugin_result
}

fn handle_message_impl<P: PluginApp>(
    raw_handle: *mut JanusPluginSession,
    transaction: *mut c_char,
    payload: *mut json_t,
    jsep: *mut json_t,
) -> Result<JanusPluginResult, Error> {
    let app_ref = P::app()
        .read()
        .map_err(|err| Error::new(&format!("Failed to acquire app read lock: {}", err)))?;

    match &*app_ref {
        None => Err(Error::new("Plugin not initialized")),
        Some(app) => {
            let plugin_handle = app
                .handle_registry()
                .get_by_raw_handle(raw_handle)
                .ok_or_else(|| Error::new("Handle not found"))?
                .plugin_handle();

            let transaction_str = unsafe { CString::from_raw(transaction) }
                .to_str()
                .map(|s| String::from(s))
                .map_err(|err| Error::new(&format!("Failed to cast transaction: {}", err)))?;

            let message = IncomingMessage::new(transaction_str, deserialize(payload)?);

            let message = match unsafe { jsep.as_mut() } {
                Some(jsep_ref) => message.set_jsep(deserialize::<Jsep>(jsep_ref)?),
                None => message,
            };

            match plugin_handle.handle_message(message) {
                Err(err) => Err(Error::new(&format!("Error handlung message: {}", err))),
                Ok(MessageResponse::Ack) => Ok(JanusPluginResult {
                    type_: JanusPluginResultType::JANUS_PLUGIN_OK_WAIT,
                    text: CString::new("").expect("Failed to cast text").into_raw(),
                    content: std::ptr::null_mut(),
                }),
                Ok(MessageResponse::Syncronous(ref response_payload)) => {
                    serialize(response_payload)
                        .map(|content| JanusPluginResult {
                            type_: JanusPluginResultType::JANUS_PLUGIN_OK,
                            text: CString::new("").expect("Failed to cast text").into_raw(),
                            content,
                        })
                        .map_err(|err| {
                            Error::new(&format!("Failed to serialize response payload: {}", err))
                        })
                }
            }
        }
    }
}

pub extern "C" fn setup_media<P: PluginApp>(raw_handle: *mut JanusPluginSession) {
    if let Err(err) = dispatch_media_event::<P>(raw_handle, &MediaEvent::Setup) {
        janus_log(err.as_str());
    }
}

pub extern "C" fn incoming_rtp<P: PluginApp>(
    raw_handle: *mut JanusPluginSession,
    is_video: c_int,
    buffer: *mut c_char,
    len: c_int,
) {
    let media_event = MediaEvent::Media {
        protocol: MediaProtocol::Rtp,
        kind: media_kind(is_video),
        buffer: unsafe { std::slice::from_raw_parts(buffer as *const i8, len as usize) },
    };

    if let Err(err) = dispatch_media_event::<P>(raw_handle, &media_event) {
        janus_log(err.as_str());
    }
}

pub extern "C" fn incoming_rtcp<P: PluginApp>(
    raw_handle: *mut JanusPluginSession,
    is_video: c_int,
    buffer: *mut c_char,
    len: c_int,
) {
    let media_event = MediaEvent::Media {
        protocol: MediaProtocol::Rtcp,
        kind: media_kind(is_video),
        buffer: unsafe { std::slice::from_raw_parts(buffer as *const i8, len as usize) },
    };

    if let Err(err) = dispatch_media_event::<P>(raw_handle, &media_event) {
        janus_log(err.as_str());
    }
}

pub extern "C" fn incoming_data<P: PluginApp>(
    raw_handle: *mut JanusPluginSession,
    buffer: *mut c_char,
    len: c_int,
) {
    let media_event = MediaEvent::Data {
        buffer: unsafe { std::slice::from_raw_parts(buffer as *const i8, len as usize) },
    };

    if let Err(err) = dispatch_media_event::<P>(raw_handle, &media_event) {
        janus_log(err.as_str());
    }
}

pub extern "C" fn slow_link<P: PluginApp>(
    raw_handle: *mut JanusPluginSession,
    uplink: c_int,
    is_video: c_int,
) {
    if let Err(err) = slow_link_impl::<P>(raw_handle, uplink, is_video) {
        janus_log(err.as_str());
    }
}

fn slow_link_impl<P: PluginApp>(
    raw_handle: *mut JanusPluginSession,
    uplink: c_int,
    is_video: c_int,
) -> Result<(), Error> {
    let uplink = uplink
        .try_into()
        .map_err(|err| Error::new(&format!("Failed to cast uplink: {}", err)))?;

    let media_event = MediaEvent::SlowLink {
        kind: media_kind(is_video),
        uplink,
    };

    dispatch_media_event::<P>(raw_handle, &media_event)
}

pub extern "C" fn hangup_media<P: PluginApp>(raw_handle: *mut JanusPluginSession) {
    if let Err(err) = dispatch_media_event::<P>(raw_handle, &MediaEvent::Hangup) {
        janus_log(err.as_str());
    }
}

pub extern "C" fn destroy_session<P: PluginApp>(
    raw_handle: *mut JanusPluginSession,
    error: *mut c_int,
) {
    let return_code = match destroy_session_impl::<P>(raw_handle) {
        Ok(()) => 0,
        Err(err) => {
            janus_log(err.as_str());
            1
        }
    };

    unsafe { *error = return_code };
}

fn destroy_session_impl<P: PluginApp>(raw_handle: *mut JanusPluginSession) -> Result<(), Error> {
    let mut app_ref = P::app()
        .write()
        .map_err(|err| Error::new(&format!("Failed to acquire app write lock: {}", err)))?;

    match &mut *app_ref {
        None => Err(Error::new("Plugin not initialized")),
        Some(app) => app.handle_registry_mut().remove(raw_handle),
    }
}

pub extern "C" fn query_session<P: PluginApp>(raw_handle: *mut JanusPluginSession) -> *mut json_t {
    match query_session_impl::<P>(raw_handle) {
        Ok(json) => json,
        Err(err) => {
            janus_log(err.as_str());
            std::ptr::null_mut()
        }
    }
}

fn query_session_impl<P: PluginApp>(
    raw_handle: *mut JanusPluginSession,
) -> Result<*mut json_t, Error> {
    let app_ref = P::app()
        .read()
        .map_err(|err| Error::new(&format!("Failed to acquire app read lock: {}", err)))?;

    match &*app_ref {
        None => Err(Error::new("Plugin not initialized")),
        Some(app) => {
            let plugin_handle = app
                .handle_registry()
                .get_by_raw_handle(raw_handle)
                .ok_or_else(|| Error::new("Handle not found"))?
                .plugin_handle();

            serialize(plugin_handle)
        }
    }
}

///////////////////////////////////////////////////////////////////////////////

/// This trait contains methods to interact with Janus core.
/// It's being automatically implemented for any type that is a plugin [Handle](trait.Handle.html).
pub trait Callbacks<P: PluginApp>: Handle {
    /// Sends a binary media `buffer` of `kind` type to the current handle by `protocol`.
    fn relay_media_packet(
        &self,
        protocol: MediaProtocol,
        kind: MediaKind,
        buffer: &[i8],
    ) -> Result<(), Error>;

    /// Sends a binary `buffer` to the current handle via data channel.
    fn relay_data_packet(&self, buffer: &[i8]) -> Result<(), Error>;

    /// Tells Janus to close the PeerConnection for the current handle.
    fn close_peer_connection(&self) -> Result<(), Error>;

    /// Tells Janus to finish the current handle.
    fn end_handle(&self) -> Result<(), Error>;

    /// Sends a broadcast event which will be delivered via event handler plugins.
    fn notify_event<E: Serialize>(&self, event: &E) -> Result<(), Error>;

    /// Sends an event message to the current handle.
    /// This may be used for unicast notifications as well as for asynchronous responses.
    fn push_event(
        &self,
        message: &OutgoingMessage<Self::OutgoingMessagePayload>,
    ) -> Result<(), Error>;
}

impl<P: PluginApp> Callbacks<P> for P::Handle {
    fn relay_media_packet(
        &self,
        protocol: MediaProtocol,
        kind: MediaKind,
        buffer: &[i8],
    ) -> Result<(), Error> {
        let callbacks = janus_callbacks::<P>()?;

        let janus_callback = match protocol {
            MediaProtocol::Rtp => callbacks.relay_rtp,
            MediaProtocol::Rtcp => callbacks.relay_rtcp,
        };

        let raw_handle = raw_handle::<P>(self.id())?;

        let is_video = match kind {
            MediaKind::Video => 1,
            MediaKind::Audio => 0,
        };

        janus_callback(
            raw_handle,
            is_video,
            buffer.as_ptr() as *mut i8,
            buffer.len() as i32,
        );

        Ok(())
    }

    fn relay_data_packet(&self, buffer: &[i8]) -> Result<(), Error> {
        let janus_callback = janus_callbacks::<P>()?.relay_data;
        let raw_handle = raw_handle::<P>(self.id())?;
        janus_callback(raw_handle, buffer.as_ptr() as *mut i8, buffer.len() as i32);
        Ok(())
    }

    fn close_peer_connection(&self) -> Result<(), Error> {
        let janus_callback = janus_callbacks::<P>()?.close_pc;
        let raw_handle = raw_handle::<P>(self.id())?;
        janus_callback(raw_handle);
        Ok(())
    }

    fn end_handle(&self) -> Result<(), Error> {
        let janus_callback = janus_callbacks::<P>()?.end_session;
        let raw_handle = raw_handle::<P>(self.id())?;
        janus_callback(raw_handle);
        Ok(())
    }

    fn notify_event<E: Serialize>(&self, event: &E) -> Result<(), Error> {
        let janus_callback = janus_callbacks::<P>()?.notify_event;
        let raw_handle = raw_handle::<P>(self.id())?;

        let event_json =
            serialize(event).map_err(|err| Error::new(&format!("Failed to serialize: {}", err)))?;

        janus_callback(P::janus_plugin(), raw_handle, event_json);
        Ok(())
    }

    fn push_event(
        &self,
        message: &OutgoingMessage<Self::OutgoingMessagePayload>,
    ) -> Result<(), Error> {
        let janus_callback = janus_callbacks::<P>()?.push_event;
        let raw_handle = raw_handle::<P>(self.id())?;

        let txn = CString::new(message.transaction().to_owned())
            .map_err(|err| Error::new(&format!("Failed to cast transaction: {}", err)))?;

        let payload = serialize(message.payload())
            .map_err(|err| Error::new(&format!("Failed to serialize payload: {}", err)))?;

        let jsep_ptr = match message.jsep() {
            None => std::ptr::null_mut(),
            Some(jsep) => serialize::<Jsep>(jsep)
                .map_err(|err| Error::new(&format!("Failed to serialize JSEP: {}", err)))?,
        };

        let return_code = janus_callback(
            raw_handle,
            P::janus_plugin(),
            txn.into_raw(),
            payload,
            jsep_ptr,
        );

        match return_code {
            0 => Ok(()),
            _ => Err(Error::new("Failed to push event")),
        }
    }
}

///////////////////////////////////////////////////////////////////////////////

fn janus_log(message: &str) {
    // TODO: Add better logging with levels and colors.
    let message_nl = format!("{}\n", message);
    let c_message = CString::new(message_nl.as_str()).expect("Failed to cast error message");
    unsafe { janus_plugin_sys::janus_vprintf(c_message.as_ptr()) };
}

fn media_kind(is_video: c_int) -> MediaKind {
    match is_video {
        0 => MediaKind::Audio,
        _ => MediaKind::Video,
    }
}

fn dispatch_media_event<P: PluginApp>(
    raw_handle: *mut JanusPluginSession,
    media_event: &MediaEvent,
) -> Result<(), Error> {
    let app_ref = P::app()
        .read()
        .map_err(|err| Error::new(&format!("Failed to acquire app read lock: {}", err)))?;

    match &*app_ref {
        None => Err(Error::new("Plugin not initialized")),
        Some(app) => match app.handle_registry().get_by_raw_handle(raw_handle) {
            None => Err(Error::new("Handle not found")),
            Some(entry) => {
                let plugin_handle = entry.plugin_handle();
                plugin_handle.handle_media_event(media_event);
                Ok(())
            }
        },
    }
}

fn raw_handle<P: PluginApp>(id: u64) -> Result<*mut JanusPluginSession, Error> {
    let mut app_ref = P::app()
        .write()
        .map_err(|err| Error::new(&format!("Failed to acquire app write lock: {}", err)))?;

    match &mut *app_ref {
        None => Err(Error::new("Plugin not initialized")),
        Some(app) => Ok(app
            .handle_registry_mut()
            .get_by_id_mut(id)
            .ok_or_else(|| Error::new(&format!("Handle {} not found", id)))?
            .raw_handle_mut()),
    }
}

fn janus_callbacks<P: PluginApp>() -> Result<&'static JanusCallbacks, Error> {
    let mut app_ref = P::app()
        .write()
        .map_err(|err| Error::new(&format!("Failed to acquire app write lock: {}", err)))?;

    match &mut *app_ref {
        None => Err(Error::new("Plugin not initialized")),
        Some(app) => {
            let callbacks = app.janus_callbacks();
            Ok(unsafe { &*callbacks })
        }
    }
}

fn serialize<S: Serialize>(object: &S) -> Result<*mut json_t, Error> {
    // TODO: Dump JSON to string with serde and load back with jansson is suboptimal.
    //       It would be better to implement serde_jansson.
    let dump = serde_json::ser::to_string(object)
        .map_err(|err| Error::new(&format!("Failed to dump JSON: {}", err)))?;

    let dump_cstring = CString::new(dump.as_str())
        .map_err(|err| Error::new(&format!("Failed to cast dumped JSON: {}", err)))?;

    let ptr = unsafe { json_loads((&dump_cstring).as_ptr(), 0, std::ptr::null_mut()).as_mut() };

    ptr.map(|p| p as *mut json_t)
        .ok_or_else(|| Error::new("Failed to load dumped JSON"))
}

fn deserialize<D: DeserializeOwned>(json: *mut json_t) -> Result<D, Error> {
    // TODO: Dump JSON to string with jansson and load back with serde is suboptimal.
    //       It would be better to implement serde_jansson.
    let dump_cstring = match unsafe { json_dumps(json, 0).as_mut() } {
        Some(ptr) => unsafe { CString::from_raw(ptr) },
        None => return Err(Error::new("Failed to dump JSON")),
    };

    let dump_str = dump_cstring
        .to_str()
        .map_err(|err| Error::new(&format!("Failed to cast dumped JSON: {}", err)))?;

    serde_json::from_str::<D>(dump_str)
        .map_err(|err| Error::new(&format!("Failed to deserialize JSON: {}", err)))
}

///////////////////////////////////////////////////////////////////////////////

mod handle_registry;
