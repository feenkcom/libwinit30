use std::os::raw::c_void;

pub enum ApplicationAction {
    FunctionCall(FunctionCallAction)
}

pub struct FunctionCallAction {
    pub callback: unsafe extern "C" fn(*const c_void),
    pub thunk: *const c_void,
}

unsafe impl Send for FunctionCallAction {}
unsafe impl Sync for FunctionCallAction {}