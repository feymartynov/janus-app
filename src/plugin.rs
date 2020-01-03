#![allow(dead_code)]

use std::cell::RefCell;
use std::convert::TryInto;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::path::Path;

use jansson_sys::{json_dumps, json_loads, json_t};
use janus_plugin_sys::plugin::{
    janus_callbacks as JanusCallbacks, janus_plugin as JanusPlugin,
    janus_plugin_result as JanusPluginResult, janus_plugin_result_type as JanusPluginResultType,
    janus_plugin_session as JanusPluginSession,
};
use serde::{de::DeserializeOwned, ser::Serialize};
use serde_json::Value as JsonValue;

use crate::{
    handle_registry::HandleRegistry, Error, Handle, HandleCallbacks, IncomingMessage,
    IncomingMessageResponse, MediaEvent, MediaKind, MediaProtocol, OutgoingMessage, Plugin,
};

///////////////////////////////////////////////////////////////////////////////

pub struct App<P: Plugin> {
    _plugin: P,
    callbacks: &'static JanusCallbacks,
}

pub trait PluginApp: Sized + Plugin {
    fn janus_plugin() -> JanusPlugin;

    fn with_app<F, R>(f: F) -> R
    where
        F: Fn(&RefCell<Option<App<Self>>>) -> R;
}

pub trait PluginHandle: Sized + Handle {
    type App: PluginApp;

    fn with_handle_registry<F, R>(f: F) -> R
    where
        F: Fn(&RefCell<HandleRegistry<'static, Self>>) -> R;
}

#[macro_export]
macro_rules! janus_plugin {
    ($plugin:ty, $handle:ty) => {
        use std::cell::RefCell;
        use std::thread::LocalKey;

        use janus_app::handle_registry::HandleRegistry;
        use janus_app::plugin::{App, PluginApp, PluginHandle};
        use janus_plugin_sys::plugin::janus_plugin as JanusPlugin;

        const JANUS_PLUGIN: JanusPlugin = JanusPlugin {
            init: janus_app::plugin::init::<$plugin>,
            destroy: janus_app::plugin::destroy::<$handle>,
            get_api_compatibility: janus_app::plugin::get_api_compatibility,
            get_version: janus_app::plugin::get_version::<$plugin>,
            get_version_string: janus_app::plugin::get_version_string::<$plugin>,
            get_description: janus_app::plugin::get_description::<$plugin>,
            get_name: janus_app::plugin::get_name::<$plugin>,
            get_author: janus_app::plugin::get_author::<$plugin>,
            get_package: janus_app::plugin::get_package::<$plugin>,
            create_session: janus_app::plugin::create_session::<$handle>,
            handle_message: janus_app::plugin::handle_message::<$handle>,
            setup_media: janus_app::plugin::setup_media::<$handle>,
            incoming_rtp: janus_app::plugin::incoming_rtp::<$handle>,
            incoming_rtcp: janus_app::plugin::incoming_rtcp::<$handle>,
            incoming_data: janus_app::plugin::incoming_data::<$handle>,
            slow_link: janus_app::plugin::slow_link::<$handle>,
            hangup_media: janus_app::plugin::hangup_media::<$handle>,
            destroy_session: janus_app::plugin::destroy_session::<$handle>,
            query_session: janus_app::plugin::query_session::<$handle>,
        };

        #[no_mangle]
        pub extern "C" fn create() -> *const JanusPlugin {
            &JANUS_PLUGIN
        }

        thread_local! {
            static APP: RefCell<Option<App<$plugin>>> = RefCell::new(None);
            static HANDLE_REGISTRY: RefCell<HandleRegistry<'static, $handle>> =
                RefCell::new(HandleRegistry::new());
        }

        impl PluginApp for $plugin {
            fn janus_plugin() -> JanusPlugin {
                JANUS_PLUGIN
            }

            fn with_app<F, R>(f: F) -> R
            where
                F: Fn(&RefCell<Option<App<Self>>>) -> R,
            {
                APP.with(|app| f(app))
            }
        }

        impl PluginHandle for $handle {
            type App = ExamplePlugin;

            fn with_handle_registry<F, R>(f: F) -> R
            where
                F: Fn(&RefCell<HandleRegistry<'static, Self>>) -> R,
            {
                HANDLE_REGISTRY.with(|handle_registry| f(handle_registry))
            }
        }
    };
}

pub extern "C" fn init<P: PluginApp>(
    callbacks: *mut JanusCallbacks,
    config_path: *const c_char,
) -> c_int {
    match init_impl::<P>(callbacks, config_path) {
        Ok(()) => 0,
        Err(ref err) => {
            janus_log(err);
            1
        }
    }
}

fn init_impl<P: PluginApp>(
    callbacks: *mut JanusCallbacks,
    config_path: *const c_char,
) -> Result<(), String> {
    P::with_app(|app| {
        if (*app.borrow()).is_some() {
            return Err(String::from("Plugin already initialized"));
        }

        let config_path = unsafe { CStr::from_ptr(config_path) }
            .to_str()
            .map_err(|err| format!("Failed to cast config path: {}", err))?;

        let plugin = P::init(&Path::new(config_path))
            .map_err(|err| format!("Failed to init plugin: {}", err))?;

        *app.borrow_mut() = Some(App {
            _plugin: *plugin,
            callbacks: unsafe { &*callbacks },
        });

        Ok(())
    })
}

pub extern "C" fn destroy<H: PluginHandle>() {
    H::App::with_app(|app| {
        if (*app.borrow()).is_some() {
            janus_log("Plugin not initialized");
            return;
        }

        *app.borrow_mut() = None;
        H::with_handle_registry(|r| (*r.borrow_mut()).clear());
    })
}

pub extern "C" fn get_api_compatibility() -> c_int {
    13
}

pub extern "C" fn get_version<P: Plugin>() -> c_int {
    P::version()
}

pub extern "C" fn get_version_string<P: Plugin>() -> *const c_char {
    CString::new(format!("{}", P::version()).as_bytes())
        .expect("Failed cast version string")
        .as_ptr()
}

pub extern "C" fn get_description<P: Plugin>() -> *const c_char {
    CString::new(P::description())
        .expect("Failed to cast description")
        .as_ptr()
}

pub extern "C" fn get_name<P: Plugin>() -> *const c_char {
    CString::new(P::name())
        .expect("Failed to cast name")
        .as_ptr()
}

pub extern "C" fn get_author<P: Plugin>() -> *const c_char {
    CString::new(P::author())
        .expect("Failed to cast author")
        .as_ptr()
}

pub extern "C" fn get_package<P: Plugin>() -> *const c_char {
    CString::new(P::package())
        .expect("Failed to cast package")
        .as_ptr()
}

pub extern "C" fn create_session<H: PluginHandle>(
    handle: *mut JanusPluginSession,
    error: *mut c_int,
) {
    let rc = match create_session_impl::<H>(handle) {
        Ok(()) => 0,
        Err(ref err) => {
            janus_log(err);
            1
        }
    };

    unsafe { *error = rc };
}

fn create_session_impl<H: PluginHandle>(raw_handle: *mut JanusPluginSession) -> Result<(), String> {
    H::with_handle_registry(|handle_registry| {
        let reg_mut = &mut *handle_registry.borrow_mut();

        match reg_mut.get_by_raw_handle(raw_handle) {
            Some(_) => Err(String::from("Handle already registered")),
            None => reg_mut
                .add(raw_handle)
                .map(|_| ())
                .map_err(|err| format!("Failed to register handle: {}", err)),
        }
    })
}

pub extern "C" fn handle_message<H: PluginHandle>(
    raw_handle: *mut JanusPluginSession,
    transaction: *mut c_char,
    payload: *mut json_t,
    jsep: *mut json_t,
) -> *mut JanusPluginResult {
    let mut plugin_result = match handle_message_impl::<H>(raw_handle, transaction, payload, jsep) {
        Ok(res) => res,
        Err(err) => {
            janus_log(&err);

            let text = CString::new(err).unwrap_or_else(|ref err| {
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

fn handle_message_impl<H: PluginHandle>(
    raw_handle: *mut JanusPluginSession,
    transaction: *mut c_char,
    payload: *mut json_t,
    jsep: *mut json_t,
) -> Result<JanusPluginResult, String> {
    H::with_handle_registry(|handle_registry| {
        let reg_ref = &*handle_registry.borrow();

        let (_, plugin_handle) = reg_ref
            .get_by_raw_handle(raw_handle)
            .ok_or_else(|| String::from("Handle not found"))?;

        let transaction_str = unsafe { CString::from_raw(transaction) }
            .to_str()
            .map(|s| String::from(s))
            .map_err(|err| format!("Failed to cast transaction: {}", err))?;

        let message =
            IncomingMessage::new(transaction_str, deserialize(payload)?, deserialize(jsep)?);

        match plugin_handle.handle_message(&message) {
            Err(err) => Err(format!("Error handlung message: {}", err)),
            Ok(IncomingMessageResponse::Ack) => Ok(JanusPluginResult {
                type_: JanusPluginResultType::JANUS_PLUGIN_OK_WAIT,
                text: CString::new("").expect("Failed to cast text").into_raw(),
                content: std::ptr::null_mut(),
            }),
            Ok(IncomingMessageResponse::Syncronous(response_payload)) => {
                // TODO: Pass custom type instead of `serde_json::Value`.
                serialize::<JsonValue>(response_payload)
                    .map(|content| JanusPluginResult {
                        type_: JanusPluginResultType::JANUS_PLUGIN_OK,
                        text: CString::new("").expect("Failed to cast text").into_raw(),
                        content,
                    })
                    .map_err(|err| format!("Failed to serialize response payload: {}", err))
            }
        }
    })
}

pub extern "C" fn setup_media<H: PluginHandle>(raw_handle: *mut JanusPluginSession) {
    if let Err(ref err) = dispatch_media_event::<H>(raw_handle, &MediaEvent::Setup) {
        janus_log(err);
    }
}

pub extern "C" fn incoming_rtp<H: PluginHandle>(
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

    if let Err(ref err) = dispatch_media_event::<H>(raw_handle, &media_event) {
        janus_log(err);
    }
}

pub extern "C" fn incoming_rtcp<H: PluginHandle>(
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

    if let Err(ref err) = dispatch_media_event::<H>(raw_handle, &media_event) {
        janus_log(err);
    }
}

pub extern "C" fn incoming_data<H: PluginHandle>(
    raw_handle: *mut JanusPluginSession,
    buffer: *mut c_char,
    len: c_int,
) {
    let media_event = MediaEvent::Data {
        buffer: unsafe { std::slice::from_raw_parts(buffer as *const i8, len as usize) },
    };

    if let Err(ref err) = dispatch_media_event::<H>(raw_handle, &media_event) {
        janus_log(err);
    }
}

pub extern "C" fn slow_link<H: PluginHandle>(
    raw_handle: *mut JanusPluginSession,
    uplink: c_int,
    is_video: c_int,
) {
    if let Err(ref err) = slow_link_impl::<H>(raw_handle, uplink, is_video) {
        janus_log(err);
    }
}

fn slow_link_impl<H: PluginHandle>(
    raw_handle: *mut JanusPluginSession,
    uplink: c_int,
    is_video: c_int,
) -> Result<(), String> {
    let uplink = uplink
        .try_into()
        .map_err(|err| format!("Failed to cast uplink: {}", err))?;

    let media_event = MediaEvent::SlowLink {
        kind: media_kind(is_video),
        uplink,
    };

    dispatch_media_event::<H>(raw_handle, &media_event)
}

pub extern "C" fn hangup_media<H: PluginHandle>(raw_handle: *mut JanusPluginSession) {
    if let Err(ref err) = dispatch_media_event::<H>(raw_handle, &MediaEvent::Hangup) {
        janus_log(err);
    }
}

pub extern "C" fn destroy_session<H: PluginHandle>(
    raw_handle: *mut JanusPluginSession,
    error: *mut c_int,
) {
    H::with_handle_registry(|handle_registry| {
        let rc = match (*handle_registry.borrow_mut()).remove(raw_handle) {
            Ok(()) => 0,
            Err(ref err) => {
                janus_log(&format!("Failed to destroy session: {}", err));
                1
            }
        };

        unsafe { *error = rc };
    })
}

pub extern "C" fn query_session<H: PluginHandle>(
    raw_handle: *mut JanusPluginSession,
) -> *mut json_t {
    match query_session_impl::<H>(raw_handle) {
        Ok(json) => json,
        Err(ref err) => {
            janus_log(err);
            std::ptr::null_mut()
        }
    }
}

fn query_session_impl<H: PluginHandle>(
    raw_handle: *mut JanusPluginSession,
) -> Result<*mut json_t, String> {
    H::with_handle_registry(|handle_registry| {
        let reg_ref = &*handle_registry.borrow();

        let (_, plugin_handle) = reg_ref
            .get_by_raw_handle(raw_handle)
            .ok_or_else(|| String::from("Handle not found"))?;

        serialize(plugin_handle)
    })
}

///////////////////////////////////////////////////////////////////////////////

impl<H: PluginHandle> HandleCallbacks for H {
    fn log(&self, message: &str) {
        janus_log(message);
    }

    fn relay_media_packet(&self, protocol: MediaProtocol, kind: MediaKind, buffer: &mut [i8]) {
        let result = handle_callback_relay_media_packet_impl::<H>(self, protocol, kind, buffer);

        if let Err(ref err) = result {
            janus_log(err);
        }
    }

    fn relay_data_packet(&self, buffer: &mut [i8]) {
        if let Err(ref err) = handle_callback_relay_data_packet_impl::<H>(self, buffer) {
            janus_log(err);
        }
    }

    fn close_peer_connection(&self) {
        if let Err(ref err) = handle_callback_close_peer_connection_impl::<H>(self) {
            janus_log(err);
        }
    }

    fn end(&self) {
        if let Err(ref err) = handle_callback_end_impl::<H>(self) {
            janus_log(err);
        }
    }

    fn notify_event<E: Serialize>(&self, event: &E) {
        if let Err(ref err) = handle_callback_notify_event_impl::<H, E>(self, event) {
            janus_log(err);
        }
    }

    fn push_event<MP: Serialize, MJ: Serialize>(
        &self,
        message: OutgoingMessage<MP, MJ>,
    ) -> Result<(), Error> {
        let callbacks = callbacks::<H::App>().map_err(|ref err| Error::new(err))?;
        let callback = callbacks.push_event;
        let raw_handle = raw_handle(self).map_err(|ref err| Error::new(err))?;

        let txn = CString::new(message.transaction().to_owned())
            .map_err(|err| Error::new(&format!("Failed to cast transaction: {}", err)))?;

        let payload = serialize(message.payload())
            .map_err(|err| Error::new(&format!("Failed to serialize payload: {}", err)))?;

        let jsep = serialize(message.jsep())
            .map_err(|err| Error::new(&format!("Failed to serialize JSEP: {}", err)))?;

        let rc = callback(
            raw_handle,
            &mut H::App::janus_plugin(),
            txn.into_raw(),
            payload,
            jsep,
        );

        match rc {
            0 => Ok(()),
            _ => Err(Error::new("Failed to push event")),
        }
    }
}

fn handle_callback_relay_media_packet_impl<H: PluginHandle>(
    handle: &H,
    protocol: MediaProtocol,
    kind: MediaKind,
    buffer: &mut [i8],
) -> Result<(), String> {
    let callbacks = callbacks::<H::App>()?;

    let callback = match protocol {
        MediaProtocol::Rtp => callbacks.relay_rtp,
        MediaProtocol::Rtcp => callbacks.relay_rtcp,
    };

    let raw_handle = raw_handle(handle)?;

    let is_video = match kind {
        MediaKind::Video => 1,
        MediaKind::Audio => 0,
    };

    callback(
        raw_handle,
        is_video,
        buffer.as_mut_ptr(),
        buffer.len() as i32,
    );

    Ok(())
}

fn handle_callback_relay_data_packet_impl<H: PluginHandle>(
    handle: &H,
    buffer: &mut [i8],
) -> Result<(), String> {
    let callback = callbacks::<H::App>()?.relay_data;
    let raw_handle = raw_handle(handle)?;
    callback(raw_handle, buffer.as_mut_ptr(), buffer.len() as i32);
    Ok(())
}

fn handle_callback_close_peer_connection_impl<H: PluginHandle>(handle: &H) -> Result<(), String> {
    let callback = callbacks::<H::App>()?.close_pc;
    let raw_handle = raw_handle(handle)?;
    callback(raw_handle);
    Ok(())
}

fn handle_callback_end_impl<H: PluginHandle>(handle: &H) -> Result<(), String> {
    let callback = callbacks::<H::App>()?.end_session;
    let raw_handle = raw_handle(handle)?;
    callback(raw_handle);
    Ok(())
}

fn handle_callback_notify_event_impl<H: PluginHandle, E: Serialize>(
    handle: &H,
    event: &E,
) -> Result<(), String> {
    let callback = callbacks::<H::App>()?.notify_event;
    let raw_handle = raw_handle(handle)?;
    let event_json = serialize(event).map_err(|err| format!("Failed to serialize: {}", err))?;
    callback(&mut H::App::janus_plugin(), raw_handle, event_json);
    Ok(())
}

fn callbacks<P: PluginApp>() -> Result<&'static JanusCallbacks, String> {
    P::with_app(|app| {
        let app_ref = &*app.borrow();

        app_ref
            .as_ref()
            .ok_or_else(|| String::from("Plugin not initialized"))
            .map(|a| a.callbacks)
    })
}

fn raw_handle<H: PluginHandle>(handle: &H) -> Result<*mut JanusPluginSession, String> {
    H::with_handle_registry(|handle_registry| {
        let reg_ref = &mut *handle_registry.borrow_mut();

        let (ref mut raw_handle, _) = reg_ref
            .get_by_id_mut(handle.id())
            .ok_or_else(|| format!("Handle {} not found", handle.id()))?;

        Ok(*raw_handle as *mut JanusPluginSession)
    })
}

///////////////////////////////////////////////////////////////////////////////

fn janus_log(message: &str) {
    // TODO: Add better logging with levels and colors.
    let c_message = CString::new(message).expect("Failed to cast error message");
    unsafe { janus_plugin_sys::janus_vprintf(c_message.as_ptr()) };
}

fn media_kind(is_video: c_int) -> MediaKind {
    match is_video {
        0 => MediaKind::Audio,
        _ => MediaKind::Video,
    }
}

fn dispatch_media_event<H: PluginHandle>(
    raw_handle: *mut JanusPluginSession,
    media_event: &MediaEvent,
) -> Result<(), String> {
    H::with_handle_registry(|handle_registry| {
        match (*handle_registry.borrow()).get_by_raw_handle(raw_handle) {
            None => Err(String::from("Handle not found")),
            Some((_, plugin_handle)) => {
                plugin_handle.handle_media_event(media_event);
                Ok(())
            }
        }
    })
}

fn serialize<S: Serialize>(object: &S) -> Result<*mut json_t, String> {
    // TODO: Dump JSON to string with serde and load back with jansson is suboptimal.
    //       It would be better to implement serde_jansson.
    let dump = serde_json::ser::to_string(object)
        .map_err(|err| format!("Failed to dump JSON: {}", err))?;

    let dump_cstring = CString::new(dump.as_str())
        .map_err(|err| format!("Failed to cast dumped JSON: {}", err))?;

    let ptr = unsafe { json_loads((&dump_cstring).as_ptr(), 0, std::ptr::null_mut()).as_mut() };

    ptr.map(|p| p as *mut json_t)
        .ok_or_else(|| String::from("Failed to load dumped JSON"))
}

fn deserialize<D: DeserializeOwned>(json: *mut json_t) -> Result<D, String> {
    // TODO: Dump JSON to string with jansson and load back with serde is suboptimal.
    //       It would be better to implement serde_jansson.
    let dump_cstring = match unsafe { json_dumps(json, 0).as_mut() } {
        Some(ptr) => unsafe { CString::from_raw(ptr) },
        None => return Err(String::from("Failed to dump JSON")),
    };

    let dump_str = dump_cstring
        .to_str()
        .map_err(|err| format!("Failed to cast dumped JSON: {}", err))?;

    serde_json::from_str::<D>(dump_str)
        .map_err(|err| format!("Failed to deserialize JSON: {}", err))
}
