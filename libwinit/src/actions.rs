use crate::WindowHandle;
use std::os::raw::c_void;
use winit::window::WindowAttributes;

pub enum ApplicationAction {
    FunctionCall(FunctionCallAction),
    CreateWindow(CreateWindowAction)
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

