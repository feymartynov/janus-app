/// necessary low-level stuff missing in janus-plugin-sys crate.

#[repr(C)]
#[derive(Debug)]
pub(crate) struct janus_ice_handle {
    pub session: *const std::ffi::c_void,
    pub handle_id: u64,
    // There are a lot more fields but we need only `handle_id`.
}
