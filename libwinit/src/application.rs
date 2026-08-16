use crate::{
    convert_event, ApplicationAction, ApplicationEvents, CreateWindowAction, FunctionCallAction,
    SemaphoreSignaller, WakeUpSignaller, WindowHandle, WinitWindowEvent,
};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::os::raw::c_void;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use value_box::{BoxerError, BorrowedPtr, OwnedPtr, ReturnBoxerResult};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopBuilder, EventLoopProxy};
use winit::raw_window_handle::{HasDisplayHandle, RawDisplayHandle};
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

    #[cfg(android_platform)]
    pub fn with_android_app(&mut self, app: winit::platform::android::activity::AndroidApp) {
        use winit::platform::android::EventLoopBuilderExtAndroid;
        self.event_loop_builder.with_android_app(app);
    }

    pub fn add_wakeup_signaller(&self, wake_up_signaller: WakeUpSignaller) {
        self.wakeup_signallers.lock().push(wake_up_signaller);
    }

    pub fn set_semaphore_signaller(&mut self, semaphore: SemaphoreSignaller) {
        self.semaphore_signaller = Some(semaphore);
    }

    pub fn build(mut self) -> anyhow::Result<(Application, ApplicationHandle)> {
        let (sender, receiver) = mpsc::channel();
        let event_loop = self.event_loop_builder.build()?;
        let display_handle = event_loop.display_handle()?.as_raw();

        let events = ApplicationEvents::new();

        let application_handle = ApplicationHandle {
            sender,
            event_loop: event_loop.create_proxy(),
            events,
            event_loop_type: WinitEventLoopType::from(display_handle),
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
    event_loop_type: WinitEventLoopType,
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

    pub fn get_type(&self) -> WinitEventLoopType {
        self.event_loop_type
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
                    window.set_ime_allowed(true);

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
            WindowEvent::Moved(position) => {
                if let Some(window_handle) = self.windows.lock().get(&window_id) {
                    window_handle.on_window_moved(position);
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum WinitEventLoopType {
    Windows,
    MacOS,
    X11,
    Wayland,
    Unknown,
}

impl From<RawDisplayHandle> for WinitEventLoopType {
    fn from(value: RawDisplayHandle) -> Self {
        match value {
            RawDisplayHandle::AppKit(_) => Self::MacOS,
            RawDisplayHandle::Xlib(_) => Self::X11,
            RawDisplayHandle::Wayland(_) => Self::Wayland,
            RawDisplayHandle::Windows(_) => Self::Windows,
            _ => Self::Unknown,
        }
    }
}

#[no_mangle]
pub extern "C" fn winit_application_builder_new() -> OwnedPtr<ApplicationBuilder> {
    OwnedPtr::new(ApplicationBuilder::new())
}

#[no_mangle]
pub extern "C" fn winit_application_builder_add_wakeup_signaller(
    mut application_builder: BorrowedPtr<ApplicationBuilder>,
    wakeup_signaller: OwnedPtr<WakeUpSignaller>,
) {
    application_builder
        .with_mut_ok(|application_builder| {
            wakeup_signaller
                .with_value_ok(|signaller| {
                    application_builder.add_wakeup_signaller(signaller);
                })
                .log();
        })
        .log();
}

#[cfg(android_platform)]
#[no_mangle]
pub extern "C" fn winit_application_builder_with_android_app(
    mut application_builder: BorrowedPtr<ApplicationBuilder>,
    android_app: *mut winit::platform::android::activity::AndroidApp,
) {
    application_builder
        .with_mut_ok(|application_builder| {
            let android_app = unsafe { *Box::from_raw(android_app) };
            println!("Assign AndroidApp: {:?}", &android_app);
            application_builder.with_android_app(android_app);
        })
        .log();
}

#[no_mangle]
pub extern "C" fn winit_application_builder_set_semaphore_signaller(
    mut application_builder: BorrowedPtr<ApplicationBuilder>,
    semaphore_signaller: OwnedPtr<SemaphoreSignaller>,
) {
    application_builder
        .with_mut_ok(|application_builder| {
            semaphore_signaller
                .with_value_ok(|signaller| {
                    application_builder.set_semaphore_signaller(signaller);
                })
                .log();
        })
        .log();
}

#[no_mangle]
pub extern "C" fn winit_application_builder_build(
    application_builder: OwnedPtr<ApplicationBuilder>,
    application_ptr: *mut *mut Application,
    application_handle_ptr: *mut *mut ApplicationHandle,
) {
    application_builder
        .with_value(|builder| {
            builder
                .build()
                .map(|(application, application_handle)| unsafe {
                    *application_ptr = Box::into_raw(Box::new(application));
                    *application_handle_ptr = Box::into_raw(Box::new(application_handle));
                })
                .map_err(|error| BoxerError::from(error.to_string()))
        })
        .log();
}

#[no_mangle]
pub extern "C" fn winit_application_builder_release(application_builder: OwnedPtr<ApplicationBuilder>) {
    drop(application_builder);
}

#[no_mangle]
pub extern "C" fn winit_application_waker_function() -> extern "C" fn(*const c_void, u32) -> bool {
    winit_application_wake
}

#[no_mangle]
pub extern "C" fn winit_application_call_function(
    application_handle: *const c_void,
    callback: extern "C" fn(*const c_void),
    thunk: *const c_void,
) -> bool {
    let application_handle = unsafe { BorrowedPtr::<ApplicationHandle>::from_raw(application_handle as *mut _) };
    application_handle
        .with_ref_ok(|application_handle| {
            application_handle.enqueue_action(ApplicationAction::FunctionCall(FunctionCallAction {
                callback,
                thunk,
            }))
        })
        .map(|_| true)
        .or_log(false)
}

#[no_mangle]
pub extern "C" fn winit_application_wake(application_handle: *const c_void, _event: u32) -> bool {
    let application_handle = unsafe { BorrowedPtr::<ApplicationHandle>::from_raw(application_handle as *mut _) };
    application_handle
        .with_ref_ok(|application_handle| application_handle.wake_up())
        .map(|_| true)
        .or_log(false)
}

/// Run the application, must be called from a UI thread.
#[no_mangle]
pub extern "C" fn winit_application_run(application: OwnedPtr<Application>) {
    application
        .with_value_ok(|application| {
            application.run();
        })
        .log();
}

#[no_mangle]
pub extern "C" fn winit_application_release(application: OwnedPtr<Application>) {
    drop(application);
}

#[no_mangle]
pub extern "C" fn winit_application_handle_create_window(
    application_handle: BorrowedPtr<ApplicationHandle>,
    window_attributes: OwnedPtr<WindowAttributes>,
    semaphore_signaller: BorrowedPtr<SemaphoreSignaller>,
    window_handle: *mut *mut WindowHandle,
) {
    application_handle
        .with_ref(|application_handle| {
            window_attributes.with_value_ok(|window_attributes| {
                application_handle.create_window(window_attributes, move |window| {
                    unsafe { *window_handle = Box::into_raw(Box::new(window)) };
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
pub extern "C" fn winit_application_handle_try_pop_event(
    application_handle: BorrowedPtr<ApplicationHandle>,
) -> OwnedPtr<WinitWindowEvent> {
    application_handle
        .with_ref_ok(|application_handle| {
            application_handle
                .pop_event()
                .map(|window_event| {
                    debug!("Pop window event: {:?}", &window_event);
                    OwnedPtr::new(window_event)
                })
                .unwrap_or_else(|| OwnedPtr::null())
        })
        .or_log(OwnedPtr::null())
}

#[no_mangle]
pub extern "C" fn winit_application_handle_release_get_type(
    application_handle: BorrowedPtr<ApplicationHandle>,
) -> WinitEventLoopType {
    application_handle
        .with_ref_ok(|application_handle| application_handle.get_type())
        .or_log(WinitEventLoopType::Unknown)
}

#[no_mangle]
pub extern "C" fn winit_application_handle_release(application_handle: OwnedPtr<ApplicationHandle>) {
    drop(application_handle);
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
