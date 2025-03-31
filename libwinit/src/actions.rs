use crate::WindowHandle;
use std::os::raw::c_void;
use winit::dpi::Size;
use winit::window::{WindowAttributes, WindowId};

pub enum ApplicationAction {
    FunctionCall(FunctionCallAction),
    CreateWindow(CreateWindowAction),
    RequestWindowSurfaceSize(RequestWindowSurfaceSizeAction)
}

pub struct FunctionCallAction {
    pub callback: unsafe extern "C" fn(*const c_void),
    pub thunk: *const c_void,
}

unsafe impl Send for FunctionCallAction {}
unsafe impl Sync for FunctionCallAction {}

pub struct CreateWindowAction {
    pub window_attributes: WindowAttributes,
    pub callback: Box<dyn FnOnce(WindowHandle) + 'static>,
}

pub struct RequestWindowSurfaceSizeAction {
    pub surface_size: Size,
    pub window_id: WindowId
}
