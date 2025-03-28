use crate::{
    ApplicationAction, CreateWindowAction, SemaphoreSignaller, WakeUpSignaller, WindowHandle,
};
use parking_lot::Mutex;
use std::collections::{HashMap, VecDeque};
use std::error::Error;
use std::os::raw::c_void;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use value_box::{ReturnBoxerResult, ValueBox, ValueBoxPointer};
use winit::application::ApplicationHandler;
use winit::error::EventLoopError;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopBuilder, EventLoopProxy};
use winit::window::{Window, WindowAttributes, WindowId};

pub struct ApplicationBuilder {
    event_loop_builder: EventLoopBuilder,
    semaphore_signaller: Option<SemaphoreSignaller>,
    wakeup_signallers: Mutex<Vec<WakeUpSignaller>>,
}

impl ApplicationBuilder {
    pub fn new() -> Self {
        Self {
            event_loop_builder: EventLoop::builder(),
            semaphore_signaller: None,
            wakeup_signallers: Default::default(),
        }
    }

    pub fn add_wakeup_signaller(
        &self,
        callback: extern "C" fn(*const c_void),
        thunk: *const c_void,
    ) {
        self.wakeup_signallers
            .lock()
            .push(WakeUpSignaller::new(callback, thunk));
    }

    pub fn set_semaphore_signaller(&mut self, semaphore: SemaphoreSignaller) {
        self.semaphore_signaller = Some(semaphore);
    }

    pub fn build(mut self) -> Result<(Application, ApplicationHandle), EventLoopError> {
        let (sender, receiver) = mpsc::channel();
        let event_loop = self.event_loop_builder.build()?;

        let application = Application {
            event_loop,
            receiver,
            semaphore_signaller: self.semaphore_signaller,
            wakeup_signallers: self.wakeup_signallers,
        };

        let application_handle = ApplicationHandle {
            sender,
            event_loop: application.event_loop.create_proxy(),
        };

        Ok((application, application_handle))
    }
}

pub struct Application {
    event_loop: EventLoop,
    receiver: Receiver<ApplicationAction>,
    semaphore_signaller: Option<SemaphoreSignaller>,
    wakeup_signallers: Mutex<Vec<WakeUpSignaller>>,
}

impl Application {
    pub fn run(self) {
        let application = RunningApplication {
            receiver: self.receiver,
            windows: Default::default(),
            events: Default::default(),
            semaphore_signaller: self.semaphore_signaller,
            wakeup_signallers: self.wakeup_signallers,
        };

        // todo: handle errors
        self.event_loop.run_app(application).unwrap();
    }
}

pub struct ApplicationHandle {
    sender: Sender<ApplicationAction>,
    event_loop: EventLoopProxy,
}

impl ApplicationHandle {
    pub fn create_window(
        &self,
        window_attributes: WindowAttributes,
        callback: impl FnOnce(WindowHandle) + 'static,
    ) {
        self.enqueue_action(ApplicationAction::CreateWindow(CreateWindowAction {
            window_attributes,
            callback: Box::new(callback),
        }))
    }

    pub fn enqueue_action(&self, action: ApplicationAction) {
        self.sender.send(action).unwrap();
        self.wake_up();
    }

    pub fn wake_up(&self) {
        self.event_loop.wake_up();
    }
}

pub struct RunningApplication {
    receiver: Receiver<ApplicationAction>,
    windows: Mutex<HashMap<WindowId, (WindowHandle, Box<dyn Window>)>>,
    events: Mutex<VecDeque<WindowEvent>>,
    semaphore_signaller: Option<SemaphoreSignaller>,
    wakeup_signallers: Mutex<Vec<WakeUpSignaller>>,
}

impl RunningApplication {
    pub fn enqueue_event(&mut self, event: WindowEvent) {
        self.events.lock().push_back(event);
        if let Some(semaphore) = &self.semaphore_signaller {
            semaphore.signal();
        }
    }

    pub fn poll_event(&mut self) -> Option<WindowEvent> {
        self.events.lock().pop_front()
    }

    fn handle_action(&mut self, event_loop: &dyn ActiveEventLoop, action: ApplicationAction) {
        match action {
            ApplicationAction::FunctionCall(action) => {
                unsafe { (action.callback)(action.thunk) };
            }
            ApplicationAction::CreateWindow(action) => {
                if let Ok(window) = event_loop.create_window(action.window_attributes) {
                    let window_handle = WindowHandle::from(window.as_ref());
                    self.windows
                        .lock()
                        .insert(window.id(), (window_handle.clone(), window));
                    (action.callback)(window_handle);
                }
            }
        }
    }

    fn signal_wakeup(&self) {
        for signaller in self.wakeup_signallers.lock().iter() {
            signaller.signal()
        }
    }
}

impl ApplicationHandler for RunningApplication {
    fn can_create_surfaces(&mut self, _event_loop: &dyn ActiveEventLoop) {}

    fn proxy_wake_up(&mut self, event_loop: &dyn ActiveEventLoop) {
        while let Ok(action) = self.receiver.try_recv() {
            self.handle_action(event_loop, action)
        }
        self.signal_wakeup();
    }

    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let mut windows_lock = self.windows.lock();
        let window = match windows_lock.get_mut(&window_id) {
            Some(window) => window,
            None => return,
        };

        match &event {
            // WindowEvent::Resized(size) => {
            //     window.resize(size);
            // },
            _ => {}
        }

        drop(windows_lock);
        self.enqueue_event(event.clone());
    }
}

#[no_mangle]
pub extern "C" fn winit_application_builder_new() -> *mut ValueBox<ApplicationBuilder> {
    value_box!(ApplicationBuilder::new()).into_raw()
}

#[no_mangle]
pub extern "C" fn winit_application_builder_add_wakeup_signaller(
    application_builder: *mut ValueBox<ApplicationBuilder>,
    callback: extern "C" fn(*const c_void),
    thunk: *const c_void,
) {
    application_builder
        .with_ref_ok(|application_builder| {
            application_builder.add_wakeup_signaller(callback, thunk);
        })
        .log();
}

#[no_mangle]
pub extern "C" fn winit_application_builder_set_semaphore_signaller(
    application_builder: *mut ValueBox<ApplicationBuilder>,
    semaphore_signaller: *mut ValueBox<SemaphoreSignaller>,
) {
    application_builder
        .with_mut(|application_builder| {
            semaphore_signaller.take_value().map(|signaller| {
                application_builder.set_semaphore_signaller(signaller);
            })
        })
        .log();
}

#[no_mangle]
pub extern "C" fn winit_application_builder_build(
    application_builder: *mut ValueBox<ApplicationBuilder>,
    application_ptr: *mut *mut ValueBox<Application>,
    application_handle_ptr: *mut *mut ValueBox<ApplicationHandle>,
) {
    application_builder
        .take_value()
        .and_then(|builder| {
            builder
                .build()
                .map(|(application, application_handle)| unsafe {
                    *application_ptr = value_box!(application).into_raw();
                    *application_handle_ptr = value_box!(application_handle).into_raw();
                })
                .map_err(|error| (Box::new(error) as Box<dyn Error>).into())
        })
        .log();
}

#[no_mangle]
pub extern "C" fn winit_application_builder_release(
    application_builder: *mut ValueBox<ApplicationBuilder>,
) {
    application_builder.release();
}

#[no_mangle]
pub extern "C" fn winit_application_waker_function() -> extern "C" fn(*const c_void, u32) -> bool {
    winit_application_wake
}

#[no_mangle]
pub extern "C" fn winit_application_wake(application_handle: *const c_void, _event: u32) -> bool {
    let application_handle = application_handle as *mut ValueBox<ApplicationHandle>;
    application_handle
        .with_ref_ok(|application_handle| application_handle.wake_up())
        .map(|_| true)
        .or_log(false)
}

/// Run the application, must be called from a UI thread.
#[no_mangle]
pub extern "C" fn winit_application_run(application: *mut ValueBox<Application>) {
    application.take_value().map(|application| {
        application.run();
    }).log();
}

#[no_mangle]
pub extern "C" fn winit_application_release(application: *mut ValueBox<Application>) {
    application.release();
}

#[no_mangle]
pub extern "C" fn winit_application_handle_create_window(
    application_handle: *mut ValueBox<ApplicationHandle>,
    window_attributes: *mut ValueBox<WindowAttributes>,
    semaphore_signaller: *mut ValueBox<SemaphoreSignaller>,
    window_handle: *mut *mut ValueBox<WindowHandle>,
) {
    application_handle
        .with_ref(|application_handle| {
            window_attributes.take_value().map(|window_attributes| {
                application_handle.create_window(window_attributes, move |window| {
                    unsafe { *window_handle = value_box!(window).into_raw() };
                    semaphore_signaller
                        .with_ref_ok(|signaller| {
                            signaller.signal();
                        })
                        .log();
                })
            })
        })
        .log();
}

#[no_mangle]
pub extern "C" fn winit_application_handle_release(application_handle: *mut ValueBox<ApplicationHandle>) {
    application_handle.release();
}

#[cfg(test)]
mod tests {
    use crate::ApplicationHandle;

    #[allow(dead_code)]
    fn require_send<T: Send>() {}
    #[allow(dead_code)]
    fn require_sync<T: Sync>() {}

    #[test]
    fn application_handle_is_send_and_sync() {
        require_send::<ApplicationHandle>();
        require_sync::<ApplicationHandle>();
    }
}
