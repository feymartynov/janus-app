use std::collections::HashMap;
use std::sync::atomic::{AtomicPtr, Ordering};

use janus_plugin_sys::plugin::janus_plugin_session as JanusPluginSession;

use crate::error::Error;
use crate::ffi::janus_ice_handle as JanusIceHandle;
use crate::Plugin;

pub(crate) struct Entry<P: Plugin> {
    raw_handle: AtomicPtr<JanusPluginSession>,
    plugin_handle: P::Handle,
}

impl<P: Plugin> Entry<P> {
    fn new(raw_handle: AtomicPtr<JanusPluginSession>, plugin_handle: P::Handle) -> Self {
        Self {
            raw_handle,
            plugin_handle,
        }
    }

    pub(crate) fn raw_handle_mut(&mut self) -> *mut JanusPluginSession {
        self.raw_handle.load(Ordering::Relaxed)
    }

    pub(crate) fn plugin_handle(&self) -> &P::Handle {
        &self.plugin_handle
    }

    pub(crate) fn plugin_handle_mut(&mut self) -> &mut P::Handle {
        &mut self.plugin_handle
    }
}

pub(crate) struct HandleRegistry<P: Plugin> {
    handles: HashMap<u64, Entry<P>>,
}

impl<P: Plugin> HandleRegistry<P> {
    pub(crate) fn new() -> Self {
        Self {
            handles: HashMap::new(),
        }
    }

    pub(crate) fn get_by_id(&self, id: u64) -> Option<&Entry<P>> {
        self.handles.get(&id)
    }

    pub(crate) fn get_by_id_mut(&mut self, id: u64) -> Option<&mut Entry<P>> {
        self.handles.get_mut(&id)
    }

    pub(crate) fn get_by_raw_handle(
        &self,
        raw_handle_ptr: *mut JanusPluginSession,
    ) -> Option<&Entry<P>> {
        self.get_by_id(Self::fetch_id(raw_handle_ptr))
    }

    pub(crate) fn add(
        &mut self,
        raw_handle_ptr: *mut JanusPluginSession,
        plugin_handle: P::Handle,
    ) -> Result<&Entry<P>, Error> {
        if self.get_by_raw_handle(raw_handle_ptr).is_some() {
            return Err(Error::new("Handle already registered"));
        }

        let id = Self::fetch_id(raw_handle_ptr);
        let raw_handle = AtomicPtr::new(raw_handle_ptr);

        self.handles
            .insert(id, Entry::new(raw_handle, plugin_handle));

        self.get_by_id(id)
            .ok_or_else(|| Error::new(&format!("Failed to register handle with id {}", id)))
    }

    pub(crate) fn remove(&mut self, raw_handle_ptr: *mut JanusPluginSession) -> Result<(), Error> {
        self.handles.remove(&Self::fetch_id(raw_handle_ptr));
        Ok(())
    }

    pub(crate) fn fetch_id(raw_handle: *mut JanusPluginSession) -> u64 {
        unsafe {
            let ptr = (*raw_handle).gateway_handle as *const JanusIceHandle;
            (*ptr).handle_id
        }
    }
}
