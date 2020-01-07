use std::cell::RefCell;
use std::convert::TryInto;
use std::ffi::{CStr, CString};
use std::marker::PhantomData;
use std::os::raw::{c_char, c_int};
use std::path::Path;
use std::sync::mpsc;
use std::thread;

use jansson_sys::{json_dumps, json_loads, json_t};
use janus_plugin_sys::plugin::{
    janus_callbacks as JanusCallbacks, janus_plugin_result as JanusPluginResult,
    janus_plugin_result_type as JanusPluginResultType, janus_plugin_session as JanusPluginSession,
};
use serde::{de::DeserializeOwned, ser::Serialize};

use crate::{
    Error, Handle, IncomingMessage, IncomingMessageResponse, Jsep, MediaEvent, MediaKind,
    MediaProtocol, OutgoingMessage, Plugin,
};
use handle_registry::HandleRegistry;

pub use janus_plugin_sys::plugin::janus_plugin as JanusPlugin;

///////////////////////////////////////////////////////////////////////////////

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

        thread_local! {
            static APP: std::cell::RefCell<Option<janus_app::plugin::App<'static, $plugin>>> =
                std::cell::RefCell::new(None);
        }

        impl janus_app::plugin::PluginApp for $plugin {
            fn janus_plugin() -> *mut janus_app::plugin::JanusPlugin {
                &mut JANUS_PLUGIN
            }

            fn with_app<F, R>(f: F) -> R
            where
                F: Fn(&std::cell::RefCell<Option<janus_app::plugin::App<$plugin>>>) -> R,
            {
                APP.with(|app_ref| f(app_ref))
            }
        }

        // Required by Janus Gateway core to initialize the plugin.
        #[no_mangle]
        pub extern "C" fn create() -> *const janus_app::plugin::JanusPlugin {
            &JANUS_PLUGIN
        }
    };
}

///////////////////////////////////////////////////////////////////////////////

pub struct App<'a, P: 'static + PluginApp> {
    plugin: P,
    callback_dispatcher_tx: mpsc::Sender<CallbackDispatch<P::Handle>>,
    handle_registry: HandleRegistry<'a, P>,
}

impl<'a, P: 'static + PluginApp> App<'a, P> {
    fn new(plugin: P, janus_callbacks: *mut JanusCallbacks) -> Self {
        Self {
            plugin,
            callback_dispatcher_tx: CallbackDispatcherBackend::<P>::start(janus_callbacks),
            handle_registry: HandleRegistry::<P>::new(),
        }
    }

    pub fn plugin(&self) -> &P {
        &self.plugin
    }

    fn handle_registry(&self) -> &HandleRegistry<'a, P> {
        &self.handle_registry
    }

    fn handle_registry_mut(&mut self) -> &mut HandleRegistry<'a, P> {
        &mut self.handle_registry
    }

    fn build_handle(&self, id: u64) -> P::Handle {
        let handle_callback_dispatcher =
            CallbackDispatcher::<P::Handle>::new(self.callback_dispatcher_tx.clone(), id);

        self.plugin().build_handle(id, handle_callback_dispatcher)
    }
}

pub trait PluginApp: Send + Sized + Plugin {
    fn janus_plugin() -> *mut JanusPlugin;

    fn with_app<F, R>(f: F) -> R
    where
        F: Fn(&RefCell<Option<App<Self>>>) -> R;
}

///////////////////////////////////////////////////////////////////////////////

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
    P::with_app(|app_ref| {
        if (*app_ref.borrow()).is_some() {
            return Err(Error::new("Plugin already initialized"));
        }

        let config_path = unsafe { CStr::from_ptr(config_path) }
            .to_str()
            .map_err(|err| Error::new(&format!("Failed to cast config path: {}", err)))?;

        let plugin = P::init(&Path::new(config_path))
            .map_err(|err| Error::new(&format!("Failed to init plugin: {}", err)))?;

        *app_ref.borrow_mut() = Some(App::new(*plugin, unsafe { &mut *callbacks }));
        Ok(())
    })
}

pub extern "C" fn destroy<P: PluginApp>() {
    P::with_app(|app_ref| *app_ref.borrow_mut() = None);
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
    P::with_app(|app_ref| match &mut *app_ref.borrow_mut() {
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
    })
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
    P::with_app(|app_ref| match &*app_ref.borrow() {
        None => Err(Error::new("Plugin not initialized")),
        Some(app) => {
            let (_, plugin_handle) = app
                .handle_registry()
                .get_by_raw_handle(raw_handle)
                .ok_or_else(|| Error::new("Handle not found"))?;

            let transaction_str = unsafe { CString::from_raw(transaction) }
                .to_str()
                .map(|s| String::from(s))
                .map_err(|err| Error::new(&format!("Failed to cast transaction: {}", err)))?;

            let message = IncomingMessage::new(transaction_str, deserialize(payload)?);

            let message = match unsafe { jsep.as_mut() } {
                Some(jsep_ref) => message.set_jsep(deserialize::<Jsep>(jsep_ref)?),
                None => message,
            };

            match plugin_handle.handle_message(&message) {
                Err(err) => Err(Error::new(&format!("Error handlung message: {}", err))),
                Ok(IncomingMessageResponse::Ack) => Ok(JanusPluginResult {
                    type_: JanusPluginResultType::JANUS_PLUGIN_OK_WAIT,
                    text: CString::new("").expect("Failed to cast text").into_raw(),
                    content: std::ptr::null_mut(),
                }),
                Ok(IncomingMessageResponse::Syncronous(ref response_payload)) => {
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
    })
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
    P::with_app(|app_ref| match &mut *app_ref.borrow_mut() {
        None => Err(Error::new("Plugin not initialized")),
        Some(app) => app.handle_registry_mut().remove(raw_handle),
    })
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
    P::with_app(|app_ref| match &*app_ref.borrow() {
        None => Err(Error::new("Plugin not initialized")),
        Some(app) => {
            let (_, plugin_handle) = app
                .handle_registry()
                .get_by_raw_handle(raw_handle)
                .ok_or_else(|| Error::new("Handle not found"))?;

            serialize(plugin_handle)
        }
    })
}

///////////////////////////////////////////////////////////////////////////////

enum Callback<H: Handle> {
    RelayMediaPacket {
        protocol: MediaProtocol,
        kind: MediaKind,
        buffer: Vec<i8>,
    },
    RelayDataPacket {
        buffer: Vec<i8>,
    },
    ClosePeerConnection,
    EndHandle,
    NotifyEvent {
        event: H::Event,
    },
    PushEvent {
        message: OutgoingMessage<H::OutgoingMessagePayload>,
    },
}

struct CallbackDispatch<H: Handle> {
    handle_id: u64,
    callback: Callback<H>,
}

impl<H: Handle> CallbackDispatch<H> {
    fn new(handle_id: u64, callback: Callback<H>) -> Self {
        Self {
            handle_id,
            callback,
        }
    }
}

#[derive(Clone)]
pub struct CallbackDispatcher<H: Handle> {
    tx: mpsc::Sender<CallbackDispatch<H>>,
    handle_id: u64,
}

impl<H: Handle> CallbackDispatcher<H> {
    fn new(tx: mpsc::Sender<CallbackDispatch<H>>, handle_id: u64) -> Self {
        Self { tx, handle_id }
    }

    pub fn relay_media_packet(
        &self,
        protocol: MediaProtocol,
        kind: MediaKind,
        buffer: &[i8],
    ) -> Result<(), Error> {
        // TODO: Copying for thread safety is not good for performance.
        let mut owned_buffer = Vec::with_capacity(buffer.len());
        owned_buffer.copy_from_slice(buffer);

        self.dispatch(Callback::RelayMediaPacket {
            protocol,
            kind,
            buffer: owned_buffer,
        })
    }

    pub fn relay_data_packet(&self, buffer: &[i8]) -> Result<(), Error> {
        // TODO: Copying for thread safety is not good for performance.
        let mut owned_buffer = Vec::with_capacity(buffer.len());
        owned_buffer.copy_from_slice(buffer);

        self.dispatch(Callback::RelayDataPacket {
            buffer: owned_buffer,
        })
    }

    pub fn close_peer_connection(&self) -> Result<(), Error> {
        self.dispatch(Callback::ClosePeerConnection)
    }

    pub fn end_handle(&self) -> Result<(), Error> {
        self.dispatch(Callback::EndHandle)
    }

    pub fn notify_event(&self, event: H::Event) -> Result<(), Error> {
        self.dispatch(Callback::NotifyEvent { event })
    }

    pub fn push_event(
        &self,
        message: OutgoingMessage<H::OutgoingMessagePayload>,
    ) -> Result<(), Error> {
        self.dispatch(Callback::PushEvent { message })
    }

    fn dispatch(&self, callback: Callback<H>) -> Result<(), Error> {
        self.tx
            .send(CallbackDispatch::new(self.handle_id, callback))
            .map_err(|err| Error::new(&format!("Failed to dispatch callback: {}", err)))
    }
}

struct CallbackDispatcherBackend<P: PluginApp> {
    janus_callbacks: *mut JanusCallbacks,
    phantom: PhantomData<P>,
}

unsafe impl<P: PluginApp> Send for CallbackDispatcherBackend<P> {}
unsafe impl<P: PluginApp> Sync for CallbackDispatcherBackend<P> {}

impl<P: 'static + PluginApp> CallbackDispatcherBackend<P> {
    fn start(janus_callbacks: *mut JanusCallbacks) -> mpsc::Sender<CallbackDispatch<P::Handle>> {
        let (tx, rx) = mpsc::channel::<CallbackDispatch<P::Handle>>();

        let backend = Self {
            janus_callbacks,
            phantom: PhantomData,
        };

        thread::spawn(move || {
            for callback_dispatch in rx.into_iter() {
                if let Err(err) = backend.dispatch(callback_dispatch) {
                    janus_log(err.as_str());
                }
            }
        });

        tx
    }

    fn dispatch(&self, callback_dispatch: CallbackDispatch<P::Handle>) -> Result<(), Error> {
        P::with_app(|app_ref| match &*app_ref.borrow() {
            None => Err(Error::new("Plugin not initialized")),
            Some(app) => {
                let (_, handle) = app
                    .handle_registry()
                    .get_by_id(callback_dispatch.handle_id)
                    .ok_or_else(|| {
                        Error::new(&format!("Handle {} not found", callback_dispatch.handle_id))
                    })?;

                match callback_dispatch.callback {
                    Callback::RelayMediaPacket {
                        protocol,
                        kind,
                        ref buffer,
                    } => self.relay_media_packet(handle, protocol, kind, buffer),
                    Callback::RelayDataPacket { ref buffer } => {
                        self.relay_data_packet(handle, buffer)
                    }
                    Callback::ClosePeerConnection => self.close_peer_connection(handle),
                    Callback::EndHandle => self.end_handle(handle),
                    Callback::NotifyEvent { ref event } => self.notify_event(handle, event),
                    Callback::PushEvent { ref message } => self.push_event(handle, message),
                }
            }
        })
    }

    fn relay_media_packet(
        &self,
        handle: &P::Handle,
        protocol: MediaProtocol,
        kind: MediaKind,
        buffer: &[i8],
    ) -> Result<(), Error> {
        let janus_callback = match protocol {
            MediaProtocol::Rtp => self.janus_callbacks().relay_rtp,
            MediaProtocol::Rtcp => self.janus_callbacks().relay_rtcp,
        };

        let raw_handle = Self::raw_handle(handle)?;

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

    fn relay_data_packet(&self, handle: &P::Handle, buffer: &[i8]) -> Result<(), Error> {
        let janus_callback = self.janus_callbacks().relay_data;
        let raw_handle = Self::raw_handle(handle)?;
        janus_callback(raw_handle, buffer.as_ptr() as *mut i8, buffer.len() as i32);
        Ok(())
    }

    fn close_peer_connection(&self, handle: &P::Handle) -> Result<(), Error> {
        let janus_callback = self.janus_callbacks().close_pc;
        let raw_handle = Self::raw_handle(handle)?;
        janus_callback(raw_handle);
        Ok(())
    }

    fn end_handle(&self, handle: &P::Handle) -> Result<(), Error> {
        let janus_callback = self.janus_callbacks().end_session;
        let raw_handle = Self::raw_handle(handle)?;
        janus_callback(raw_handle);
        Ok(())
    }

    fn notify_event(
        &self,
        handle: &P::Handle,
        event: &<P::Handle as Handle>::Event,
    ) -> Result<(), Error> {
        let janus_callback = self.janus_callbacks().notify_event;
        let raw_handle = Self::raw_handle(handle)?;

        let event_json =
            serialize(event).map_err(|err| Error::new(&format!("Failed to serialize: {}", err)))?;

        janus_callback(P::janus_plugin(), raw_handle, event_json);
        Ok(())
    }

    fn push_event(
        &self,
        handle: &P::Handle,
        message: &OutgoingMessage<<P::Handle as Handle>::OutgoingMessagePayload>,
    ) -> Result<(), Error> {
        let janus_callback = self.janus_callbacks().push_event;
        let raw_handle = Self::raw_handle(handle)?;

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

    fn raw_handle(handle: &P::Handle) -> Result<*mut JanusPluginSession, Error> {
        P::with_app(|app_ref| match &mut *app_ref.borrow_mut() {
            None => Err(Error::new("Plugin not initialized")),
            Some(app) => {
                let (ref mut raw_handle, _) = app
                    .handle_registry_mut()
                    .get_by_id_mut(handle.id())
                    .ok_or_else(|| Error::new(&format!("Handle {} not found", handle.id())))?;

                Ok(*raw_handle as *mut JanusPluginSession)
            }
        })
    }

    fn janus_callbacks(&self) -> &JanusCallbacks {
        unsafe { &*self.janus_callbacks }
    }
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

fn dispatch_media_event<P: PluginApp>(
    raw_handle: *mut JanusPluginSession,
    media_event: &MediaEvent,
) -> Result<(), Error> {
    P::with_app(|app_ref| match &*app_ref.borrow() {
        None => Err(Error::new("Plugin not initialized")),
        Some(app) => match app.handle_registry().get_by_raw_handle(raw_handle) {
            None => Err(Error::new("Handle not found")),
            Some((_, plugin_handle)) => {
                plugin_handle.handle_media_event(media_event);
                Ok(())
            }
        },
    })
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
