use std::os::raw::c_void;
use value_box::{ValueBox, ValueBoxPointer};

#[derive(Debug)]
pub struct WakeUpSignaller {
    callback: unsafe extern "C" fn(*const c_void),
    thunk: *const c_void,
}

impl WakeUpSignaller {
    pub fn new(callback: unsafe extern "C" fn(*const c_void), thunk: *const c_void) -> Self {
        Self { callback, thunk }
    }

    pub fn signal(&self) {
        let callback = self.callback;
        unsafe { callback(self.thunk) };
    }
}

#[derive(Debug)]
pub struct SemaphoreSignaller {
    semaphore_callback: unsafe extern "C" fn(usize, *const c_void),
    semaphore_index: usize,
    semaphore_thunk: *const c_void,
}

impl SemaphoreSignaller {
    pub fn new(
        semaphore_callback: unsafe extern "C" fn(usize, *const c_void),
        semaphore_index: usize,
        semaphore_thunk: *const c_void,
    ) -> Self {
        Self {
            semaphore_callback,
            semaphore_index,
            semaphore_thunk,
        }
    }

    pub fn signal(&self) {
        let callback = self.semaphore_callback;
        unsafe { callback(self.semaphore_index, self.semaphore_thunk) };
    }
}

#[no_mangle]
pub fn winit_wakeup_signaller_new(
    callback: unsafe extern "C" fn(*const c_void),
    thunk: *const c_void,
) -> *mut ValueBox<WakeUpSignaller> {
    value_box!(WakeUpSignaller::new(callback, thunk)).into_raw()
}

#[no_mangle]
pub fn winit_wakeup_signaller_release(signaller: *mut ValueBox<WakeUpSignaller>) {
    signaller.release();
}

#[no_mangle]
pub fn winit_semaphore_signaller_release(signaller: *mut ValueBox<SemaphoreSignaller>) {
    signaller.release();
}

#[no_mangle]
pub fn winit_semaphore_signaller_new(
    semaphore_callback: unsafe extern "C" fn(usize, *const c_void),
    semaphore_index: usize,
    semaphore_thunk: *const c_void,
) -> *mut ValueBox<SemaphoreSignaller> {
    value_box!(SemaphoreSignaller::new(
        semaphore_callback,
        semaphore_index,
        semaphore_thunk
    ))
    .into_raw()
}
