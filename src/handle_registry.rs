use std::collections::HashMap;

use janus_plugin_sys::plugin::janus_plugin_session as JanusPluginSession;

use crate::error::Error;
use crate::ffi::janus_ice_handle as JanusIceHandle;
use crate::Handle;

pub struct HandleRegistry<'a, H: Handle> {
    handles: HashMap<u64, (&'a mut JanusPluginSession, H)>,
}

impl<'a, H: Handle> HandleRegistry<'a, H> {
    pub fn new() -> Self {
        Self {
            handles: HashMap::new(),
        }
    }

    pub fn get_by_id(&self, id: u64) -> Option<&(&'a mut JanusPluginSession, H)> {
        self.handles.get(&id)
    }

    pub fn get_by_id_mut(&mut self, id: u64) -> Option<&mut (&'a mut JanusPluginSession, H)> {
        self.handles.get_mut(&id)
    }

    pub fn get_by_raw_handle(
        &self,
        raw_handle_ptr: *mut JanusPluginSession,
    ) -> Option<&(&'a mut JanusPluginSession, H)> {
        self.get_by_id(Self::fetch_id(raw_handle_ptr))
    }

    pub fn add(
        &mut self,
        raw_handle_ptr: *mut JanusPluginSession,
    ) -> Result<&(&mut JanusPluginSession, H), Error> {
        if self.get_by_raw_handle(raw_handle_ptr).is_some() {
            return Err(Error::new("Handle already registered"));
        }

        let mut raw_handle = unsafe { &mut *raw_handle_ptr };
        let id = Self::fetch_id(raw_handle_ptr);

        raw_handle.ref_.count += 1;
        self.handles.insert(id, (raw_handle, H::new(id)));

        self.get_by_id(id)
            .ok_or_else(|| Error::new(&format!("Failed to register handle with id {}", id)))
    }

    pub fn remove(&mut self, raw_handle_ptr: *mut JanusPluginSession) -> Result<(), Error> {
        let mut raw_handle = unsafe { &mut *raw_handle_ptr };
        self.handles.remove(&Self::fetch_id(raw_handle_ptr));
        raw_handle.ref_.count -= 1;
        Ok(())
    }

    pub fn clear(&mut self) {
        for (_, (ref mut raw_handle, _)) in self.handles.iter_mut() {
            raw_handle.ref_.count -= 1;
        }

        self.handles.clear();
    }

    fn fetch_id(raw_handle: *mut JanusPluginSession) -> u64 {
        unsafe {
            let ptr = (*raw_handle).gateway_handle as *const JanusIceHandle;
            (*ptr).handle_id
        }
    }
}
