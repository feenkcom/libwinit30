use crate::{
    convert_event, ApplicationAction, ApplicationEvents, CreateWindowAction, SemaphoreSignaller,
    WakeUpSignaller, WindowHandle, WinitEventType, WinitWindowEvent,
};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::error::Error;
use std::os::raw::c_void;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use value_box::{ReturnBoxerResult, ValueBox, ValueBoxPointer};
use winit::application::ApplicationHandler;
use winit::error::EventLoopError;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopBuilder, EventLoopProxy};
use winit::window::{WindowAttributes, WindowId};

pub struct ApplicationBuilder {
    event_loop_builder: EventLoopBuilder,
    semaphore_signaller: Option<SemaphoreSignaller>,
    wakeup_signallers: Mutex<Vec<WakeUpSignaller>>,
}

impl ApplicationBuilder {
    pub fn new() -> Self {
        #[allow(unused_mut)]
        let mut event_loop_builder = EventLoop::builder();

        #[cfg(windows_platform)]
        {
            use winit::platform::windows::EventLoopBuilderExtWindows;
            event_loop_builder.with_any_thread(true);
        }

        Self {
            event_loop_builder,
            semaphore_signaller: None,
            wakeup_signallers: Default::default(),
        }
    }

    pub fn add_wakeup_signaller(&self, wake_up_signaller: WakeUpSignaller) {
        self.wakeup_signallers.lock().push(wake_up_signaller);
    }

    pub fn set_semaphore_signaller(&mut self, semaphore: SemaphoreSignaller) {
        self.semaphore_signaller = Some(semaphore);
    }

    pub fn build(mut self) -> Result<(Application, ApplicationHandle), EventLoopError> {
        let (sender, receiver) = mpsc::channel();
        let event_loop = self.event_loop_builder.build()?;

        let events = ApplicationEvents::new();

        let application_handle = ApplicationHandle {
            sender,
            event_loop: event_loop.create_proxy(),
            events,
        };

        let application = Application {
            event_loop,
            application_handle: application_handle.clone(),
            receiver,
            semaphore_signaller: self.semaphore_signaller,
            wakeup_signallers: self.wakeup_signallers,
        };

        Ok((application, application_handle))
    }
}

#[derive(Debug)]
pub struct Application {
    event_loop: EventLoop,
    application_handle: ApplicationHandle,
    receiver: Receiver<ApplicationAction>,
    semaphore_signaller: Option<SemaphoreSignaller>,
    wakeup_signallers: Mutex<Vec<WakeUpSignaller>>,
}

impl Application {
    pub fn run(self) {
        let application = RunningApplication {
            receiver: self.receiver,
            windows: Default::default(),
            application_handle: self.application_handle,
            semaphore_signaller: self.semaphore_signaller,
            wakeup_signallers: self.wakeup_signallers,
        };

        info!("Running application: {:?}", application);
        // todo: handle errors
        self.event_loop.run_app(application).unwrap();
    }
}

#[derive(Debug, Clone)]
pub struct ApplicationHandle {
    sender: Sender<ApplicationAction>,
    event_loop: EventLoopProxy,
    events: ApplicationEvents,
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

    pub fn push_event(&self, event: WinitWindowEvent) {
        self.events.push_event(event);
    }

    pub fn pop_event(&self) -> Option<WinitWindowEvent> {
        self.events.pop_event()
    }
}

#[derive(Debug)]
pub struct RunningApplication {
    receiver: Receiver<ApplicationAction>,
    windows: Mutex<HashMap<WindowId, WindowHandle>>,
    application_handle: ApplicationHandle,
    semaphore_signaller: Option<SemaphoreSignaller>,
    wakeup_signallers: Mutex<Vec<WakeUpSignaller>>,
}

impl RunningApplication {
    pub fn enqueue_event(&mut self, event: WindowEvent, window_id: WindowId) {
        if let Some(window) = self.windows.lock().get(&window_id) {
            let events = convert_event(event, window);
            let has_events = !events.is_empty();

            for event in events {
                self.application_handle
                    .push_event(WinitWindowEvent { window_id, event });
            }

            if has_events {
                if let Some(semaphore) = &self.semaphore_signaller {
                    semaphore.signal();
                }
            }
        }
    }

    fn handle_action(&mut self, event_loop: &dyn ActiveEventLoop, action: ApplicationAction) {
        match action {
            ApplicationAction::FunctionCall(action) => {
                unsafe { (action.callback)(action.thunk) };
            }
            ApplicationAction::CreateWindow(action) => {
                if let Ok(window) = event_loop.create_window(action.window_attributes) {
                    let window_handle = WindowHandle::for_window(&self.application_handle, window);
                    self.windows
                        .lock()
                        .insert(window_handle.id(), window_handle.clone());
                    (action.callback)(window_handle);
                }
            }
            ApplicationAction::RequestWindowSurfaceSize(action) => {
                if let Some(handle) = self.windows.lock().get(&action.window_id) {
                    if let Some(window) = handle.window.lock().as_ref() {
                        let _ = window.request_surface_size(action.surface_size);
                    }
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
    fn can_create_surfaces(&mut self, _event_loop: &dyn ActiveEventLoop) {
        info!("Application is able to create a surfaces now");
    }

    fn proxy_wake_up(&mut self, event_loop: &dyn ActiveEventLoop) {
        while let Ok(action) = self.receiver.try_recv() {
            self.handle_action(event_loop, action)
        }
        self.signal_wakeup();
    }

    fn window_event(
        &mut self,
        _event_loop: &dyn ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        match &event {
            WindowEvent::SurfaceResized(size) => {
                if let Some(window_handle) = self.windows.lock().get(&window_id) {
                    window_handle.on_window_resized(size);
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(window_handle) = self.windows.lock().get(&window_id) {
                    window_handle.on_window_redraw();
                }
            }
            _ => {}
        }
        self.enqueue_event(event, window_id);
    }
}

#[no_mangle]
pub extern "C" fn winit_application_builder_new() -> *mut ValueBox<ApplicationBuilder> {
    value_box!(ApplicationBuilder::new()).into_raw()
}

#[no_mangle]
pub extern "C" fn winit_application_builder_add_wakeup_signaller(
    application_builder: *mut ValueBox<ApplicationBuilder>,
    wakeup_signaller: *mut ValueBox<WakeUpSignaller>,
) {
    application_builder
        .with_mut(|application_builder| {
            wakeup_signaller.take_value().map(|signaller| {
                application_builder.add_wakeup_signaller(signaller);
            })
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
    application
        .take_value()
        .map(|application| {
            application.run();
        })
        .log();
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
pub extern "C" fn winit_application_handle_pop_event(
    application_handle: *mut ValueBox<ApplicationHandle>,
    window_id: *mut usize,
    event_type: *mut WinitEventType,
    event_ptr: *mut *mut c_void,
) -> *mut ValueBox<WinitWindowEvent> {
    application_handle
        .with_ref_ok(|application_handle| {
            application_handle
                .pop_event()
                .map(|window_event| {
                    debug!("Pop window event: {:?}", &window_event);

                    unsafe {
                        *window_id = window_event.window_id().into_raw();
                        *event_type = window_event.event_type();
                        *event_ptr = window_event.as_ptr();
                    };
                    value_box!(window_event).into_raw()
                })
                .unwrap_or_else(|| std::ptr::null_mut())
        })
        .or_log(std::ptr::null_mut())
}

#[no_mangle]
pub extern "C" fn winit_application_handle_release(
    application_handle: *mut ValueBox<ApplicationHandle>,
) {
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
