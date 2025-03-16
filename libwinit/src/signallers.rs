use std::os::raw::c_void;

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