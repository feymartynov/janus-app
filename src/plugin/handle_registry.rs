use std::collections::HashMap;

use janus_plugin_sys::plugin::janus_plugin_session as JanusPluginSession;

use crate::error::Error;
use crate::ffi::janus_ice_handle as JanusIceHandle;
use crate::{Handle, Plugin};

pub(crate) struct HandleRegistry<'a, P: Plugin> {
    handles: HashMap<u64, (&'a mut JanusPluginSession, P::Handle)>,
}

impl<'a, P: Plugin> HandleRegistry<'a, P> {
    pub(crate) fn new() -> Self {
        Self {
            handles: HashMap::new(),
        }
    }

    pub(crate) fn get_by_id(&self, id: u64) -> Option<&(&'a mut JanusPluginSession, P::Handle)> {
        self.handles.get(&id)
    }

    pub(crate) fn get_by_id_mut(
        &mut self,
        id: u64,
    ) -> Option<&mut (&'a mut JanusPluginSession, P::Handle)> {
        self.handles.get_mut(&id)
    }

    pub(crate) fn get_by_raw_handle(
        &self,
        raw_handle_ptr: *mut JanusPluginSession,
    ) -> Option<&(&'a mut JanusPluginSession, P::Handle)> {
        self.get_by_id(Self::fetch_id(raw_handle_ptr))
    }

    pub(crate) fn add(
        &mut self,
        raw_handle_ptr: *mut JanusPluginSession,
    ) -> Result<&(&mut JanusPluginSession, P::Handle), Error> {
        if self.get_by_raw_handle(raw_handle_ptr).is_some() {
            return Err(Error::new("Handle already registered"));
        }

        let mut raw_handle = unsafe { &mut *raw_handle_ptr };
        let id = Self::fetch_id(raw_handle_ptr);

        raw_handle.ref_.count += 1;
        self.handles.insert(id, (raw_handle, P::Handle::new(id)));

        self.get_by_id(id)
            .ok_or_else(|| Error::new(&format!("Failed to register handle with id {}", id)))
    }

    pub(crate) fn remove(&mut self, raw_handle_ptr: *mut JanusPluginSession) -> Result<(), Error> {
        let mut raw_handle = unsafe { &mut *raw_handle_ptr };
        self.handles.remove(&Self::fetch_id(raw_handle_ptr));
        raw_handle.ref_.count -= 1;
        Ok(())
    }

    fn fetch_id(raw_handle: *mut JanusPluginSession) -> u64 {
        unsafe {
            let ptr = (*raw_handle).gateway_handle as *const JanusIceHandle;
            (*ptr).handle_id
        }
    }
}

impl<'a, P: Plugin> Drop for HandleRegistry<'a, P> {
    fn drop(&mut self) {
        for (_, (ref mut raw_handle, _)) in self.handles.iter_mut() {
            raw_handle.ref_.count -= 1;
        }
    }
}
