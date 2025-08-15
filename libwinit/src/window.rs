use crate::{
    ApplicationAction, ApplicationHandle, RequestWindowSurfaceSizeAction, WinitCursorIcon,
};
use geometry_box::{PointBox, SizeBox};
use parking_lot::Mutex;
use raw_window_handle_extensions::{VeryRawDisplayHandle, VeryRawWindowHandle};
use std::error::Error;
use std::os::raw::c_void;
use std::sync::Arc;
use value_box::{BoxerError, ReturnBoxerResult, ValueBox, ValueBoxPointer};
use winit::cursor::{Cursor, CursorIcon};
use winit::dpi::{PhysicalPosition, PhysicalSize, Position, Size};
use winit::monitor::MonitorHandle;
use winit::raw_window_handle::{
    HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle,
};
use winit::window::{Window, WindowId};

#[derive(Debug, Clone)]
pub struct WindowHandle {
    id: WindowId,
    data: Arc<Mutex<WindowData>>,
    pub(crate) window: Arc<Mutex<Option<Box<dyn Window>>>>,
    application_handle: ApplicationHandle,
}

impl WindowHandle {
    pub fn for_window(application_handle: &ApplicationHandle, window: Box<dyn Window>) -> Self {
        Self {
            id: window.id(),
            data: Arc::new(Mutex::new(WindowData {
                outer_position: window
                    .outer_position()
                    .unwrap_or_else(|_| PhysicalPosition::default()),
                surface_size: window.surface_size(),
                scale_factor: window.scale_factor(),
                window_redraw_listeners: vec![],
                window_resize_listeners: vec![],
            })),
            window: Arc::from(Mutex::new(Some(window))),
            application_handle: application_handle.clone(),
        }
    }

    pub fn id(&self) -> WindowId {
        self.id
    }

    pub fn request_surface_size(&self, surface_size: Size) {
        self.application_handle
            .enqueue_action(ApplicationAction::RequestWindowSurfaceSize(
                RequestWindowSurfaceSizeAction {
                    surface_size,
                    window_id: self.id,
                },
            ))
    }

    pub fn on_window_resized(&self, size: &PhysicalSize<u32>) {
        // (Windows) when a window is minimized, its size is set to 0x0,
        // while it shouldn't change, so we just ignore the event
        if size.width == 0 && size.height == 0 {
            return;
        }

        let mut lock = self.data.lock();
        lock.surface_size = size.clone();
        for listener in &lock.window_resize_listeners {
            listener.on_window_resized(size);
        }
    }

    pub fn on_window_moved(&self, position: &PhysicalPosition<i32>) {
        let mut lock = self.data.lock();
        lock.outer_position = position.clone();
    }

    pub fn on_window_redraw(&self) {
        let lock = self.data.lock();
        for listener in &lock.window_redraw_listeners {
            listener.on_redraw_requested();
        }
    }

    pub fn surface_size(&self) -> PhysicalSize<u32> {
        self.data.lock().surface_size
    }

    pub fn scale_factor(&self) -> f64 {
        self.data.lock().scale_factor
    }

    pub fn outer_position(&self) -> PhysicalPosition<i32> {
        self.data.lock().outer_position
    }

    pub fn set_outer_position(&self, position: Position) {
        if let Some(window) = self.window.lock().as_ref() {
            window.set_outer_position(position);
        }
    }

    pub fn set_cursor(&self, cursor: impl Into<Cursor>) {
        if let Some(window) = self.window.lock().as_ref() {
            window.set_cursor(cursor.into());
        }
    }

    pub fn add_redraw_listener(&self, listener: WindowRedrawRequestedListener) {
        self.data.lock().window_redraw_listeners.push(listener);
    }

    pub fn add_resized_listener(&self, listener: WindowResizedListener) {
        self.data.lock().window_resize_listeners.push(listener);
    }

    pub fn focus_window(&self) {
        if let Some(window) = self.window.lock().as_ref() {
            window.focus_window();
        }
    }

    pub fn current_monitor(&self) -> Option<MonitorHandle> {
        self.window
            .lock()
            .as_ref()
            .and_then(|window| window.current_monitor())
    }

    pub fn close_window(&self) {
        let _ = self.window.lock().take();
    }

    pub fn raw_window_handle(&self) -> Result<RawWindowHandle, Box<dyn Error>> {
        self.window
            .lock()
            .as_ref()
            .ok_or_else(|| anyhow!("Window is closed").into())
            .and_then(|window| {
                window
                    .window_handle()
                    .map(|handle| handle.as_raw())
                    .map_err(|error| Box::new(error).into())
            })
    }

    pub fn raw_display_handle(&self) -> Result<RawDisplayHandle, Box<dyn Error>> {
        self.window
            .lock()
            .as_ref()
            .ok_or_else(|| anyhow!("Window is closed").into())
            .and_then(|window| {
                window
                    .display_handle()
                    .map(|handle| handle.as_raw())
                    .map_err(|error| Box::new(error).into())
            })
    }
}

#[derive(Debug)]
struct WindowData {
    outer_position: PhysicalPosition<i32>,
    surface_size: PhysicalSize<u32>,
    scale_factor: f64,
    window_redraw_listeners: Vec<WindowRedrawRequestedListener>,
    window_resize_listeners: Vec<WindowResizedListener>,
}

#[derive(Debug)]
pub struct WindowRedrawRequestedListener {
    thunk: *const c_void,
    callback: unsafe extern "C" fn(*const c_void),
}

impl WindowRedrawRequestedListener {
    pub fn new(callback: unsafe extern "C" fn(*const c_void), thunk: *const c_void) -> Self {
        Self { callback, thunk }
    }

    fn on_redraw_requested(&self) {
        unsafe {
            (self.callback)(self.thunk);
        }
    }
}

#[derive(Debug)]
pub struct WindowResizedListener {
    thunk: *const c_void,
    callback: unsafe extern "C" fn(*const c_void, u32, u32),
}

impl WindowResizedListener {
    pub fn new(
        callback: unsafe extern "C" fn(*const c_void, u32, u32),
        thunk: *const c_void,
    ) -> Self {
        Self { callback, thunk }
    }

    fn on_window_resized(&self, size: &PhysicalSize<u32>) {
        unsafe {
            (self.callback)(self.thunk, size.width, size.height);
        }
    }
}

#[no_mangle]
pub extern "C" fn winit_window_handle_get_id(window_handle: *mut ValueBox<WindowHandle>) -> usize {
    window_handle
        .with_ref_ok(|window_handle| window_handle.id().into_raw())
        .or_log(0)
}

#[no_mangle]
pub extern "C" fn winit_window_handle_get_scale_factor(
    window_handle: *mut ValueBox<WindowHandle>,
) -> f64 {
    window_handle
        .with_ref_ok(|window_handle| window_handle.scale_factor())
        .or_log(1.0)
}

#[no_mangle]
pub extern "C" fn winit_window_handle_get_surface_size(
    window: *mut ValueBox<WindowHandle>,
    surface_size: *mut ValueBox<SizeBox<u32>>,
) {
    window
        .with_ref(|window| {
            surface_size.with_mut_ok(|surface_size| {
                let window_size = window.surface_size();
                surface_size.width = window_size.width;
                surface_size.height = window_size.height;
            })
        })
        .log();
}

/// Get the outer position of the window. Can be called from any thread.
#[no_mangle]
pub extern "C" fn winit_window_handle_get_position(
    window: *mut ValueBox<WindowHandle>,
    position: *mut ValueBox<PointBox<i32>>,
) {
    window
        .with_ref(|window_ref| {
            position.with_mut_ok(|position| {
                let window_position = window_ref.outer_position();
                position.x = window_position.x;
                position.y = window_position.y;
            })
        })
        .log();
}

/// Must be called from a UI thread
#[no_mangle]
pub extern "C" fn winit_window_handle_set_outer_position(
    window: *mut ValueBox<WindowHandle>,
    x: i32,
    y: i32,
) {
    window
        .with_ref_ok(|window| {
            window.set_outer_position(Position::Physical(PhysicalPosition::new(x, y)))
        })
        .log();
}

/// Must be called from a UI thread
#[no_mangle]
pub extern "C" fn winit_window_handle_set_cursor_icon(
    window: *mut ValueBox<WindowHandle>,
    cursor: WinitCursorIcon,
) {
    window
        .with_ref_ok(|window| {
            window.set_cursor(CursorIcon::from(cursor));
        })
        .log();
}

/// Can be called from any thread
#[no_mangle]
pub extern "C" fn winit_window_handle_request_surface_size(
    window: *mut ValueBox<WindowHandle>,
    width: u32,
    height: u32,
) {
    window
        .with_ref_ok(|window| {
            window.request_surface_size(Size::Physical(PhysicalSize::new(width, height)));
        })
        .log();
}

/// Must be called from a UI thread
#[no_mangle]
pub extern "C" fn winit_window_handle_request_redraw(window: *mut ValueBox<WindowHandle>) {
    window
        .with_ref(|window| {
            window
                .window
                .lock()
                .as_ref()
                .ok_or_else(|| anyhow!("Window is closed").into())
                .map(|window| window.request_redraw())
        })
        .log();
}

#[no_mangle]
pub extern "C" fn winit_window_handle_add_redraw_listener(
    window: *mut ValueBox<WindowHandle>,
    callback: unsafe extern "C" fn(*const c_void),
    thunk: *const c_void,
) {
    window
        .with_ref_ok(|window| {
            window.add_redraw_listener(WindowRedrawRequestedListener::new(callback, thunk));
        })
        .log();
}

#[no_mangle]
pub extern "C" fn winit_window_handle_add_resize_listener(
    window: *mut ValueBox<WindowHandle>,
    callback: unsafe extern "C" fn(*const c_void, u32, u32),
    thunk: *const c_void,
) {
    window
        .with_ref_ok(|window| {
            window.add_resized_listener(WindowResizedListener::new(callback, thunk));
        })
        .log();
}

/// Must be called from a UI thread
#[no_mangle]
pub extern "C" fn winit_window_handle_focus_window(window: *mut ValueBox<WindowHandle>) {
    window.with_ref_ok(|window| window.focus_window()).log();
}

#[no_mangle]
pub extern "C" fn winit_window_handle_current_monitor(
    window: *mut ValueBox<WindowHandle>,
) -> *mut ValueBox<MonitorHandle> {
    window
        .with_ref_ok(|window| {
            window
                .current_monitor()
                .map(|monitor| ValueBox::new(monitor).into_raw())
                .unwrap_or(std::ptr::null_mut())
        })
        .or_log(std::ptr::null_mut())
}

/// Must be called from a UI thread
#[no_mangle]
pub extern "C" fn winit_window_handle_close(window_handle: *mut ValueBox<WindowHandle>) {
    window_handle
        .take_value()
        .map(|window_handle| window_handle.close_window())
        .log();
}

fn with_window_handle(
    window: *mut ValueBox<WindowHandle>,
    f: impl FnOnce(RawWindowHandle) -> Result<*mut c_void, BoxerError>,
) -> *mut c_void {
    window
        .with_ref(|window| {
            window
                .window
                .lock()
                .as_ref()
                .ok_or_else(|| anyhow!("Window is closed").into())
                .and_then(|window| {
                    window
                        .window_handle()
                        .map_err(|error| anyhow!(error).into())
                })
                .and_then(|handle| f(handle.as_raw()))
        })
        .or_log(std::ptr::null_mut())
}

#[allow(dead_code)]
fn with_display_handle(
    window: *mut ValueBox<WindowHandle>,
    f: impl FnOnce(RawDisplayHandle) -> Result<*mut c_void, BoxerError>,
) -> *mut c_void {
    window
        .with_ref(|window| {
            window
                .window
                .lock()
                .as_ref()
                .ok_or_else(|| anyhow!("Window is closed").into())
                .and_then(|window| {
                    window
                        .display_handle()
                        .map_err(|error| anyhow!(error).into())
                })
                .and_then(|handle| f(handle.as_raw()))
        })
        .or_log(std::ptr::null_mut())
}

#[no_mangle]
pub extern "C" fn winit_window_handle_raw_window_handle(
    window: *mut ValueBox<WindowHandle>,
) -> *mut VeryRawWindowHandle {
    window
        .with_ref(|window| window.raw_window_handle().map_err(|error| error.into()))
        .map(|handle| VeryRawWindowHandle::from(handle).into())
        .or_log(std::ptr::null_mut())
}

#[no_mangle]
pub extern "C" fn winit_window_handle_raw_display_handle(
    window: *mut ValueBox<WindowHandle>,
) -> *mut VeryRawDisplayHandle {
    window
        .with_ref(|window| window.raw_display_handle().map_err(|error| error.into()))
        .map(|handle| VeryRawDisplayHandle::from(handle).into())
        .or_log(std::ptr::null_mut())
}

/// Must be called from a UI thread
#[cfg(target_os = "macos")]
#[no_mangle]
pub extern "C" fn winit_window_handle_get_ns_view(
    window: *mut ValueBox<WindowHandle>,
) -> *mut c_void {
    with_window_handle(window, |handle| match handle {
        RawWindowHandle::AppKit(handle) => Ok(handle.ns_view.as_ptr()),
        handle => Err(anyhow!("Expected an AppKit, got {:?}", handle).into()),
    })
}

/// Must be called from a UI thread
#[cfg(target_os = "windows")]
#[no_mangle]
pub extern "C" fn winit_window_handle_get_hwnd(
    window: *mut ValueBox<WindowHandle>,
) -> *mut std::ffi::c_void {
    with_window_handle(window, |handle| match handle {
        RawWindowHandle::Win32(handle) => Ok(unsafe { std::mem::transmute(handle.hwnd) }),
        handle => Err(anyhow!("Expected a Win32, got {:?}", handle).into()),
    })
}

/// Must be called from a UI thread
#[cfg(x11_platform)]
#[no_mangle]
pub extern "C" fn winit_window_handle_get_xlib_display(
    window: *mut ValueBox<WindowHandle>,
) -> *mut std::ffi::c_void {
    with_display_handle(window, |handle| match handle {
        RawDisplayHandle::Xlib(handle) => Ok(handle
            .display
            .map(|display| display.as_ptr())
            .unwrap_or(std::ptr::null_mut())),
        handle => Err(anyhow!("Expected an Xlib, got {:?}", handle).into()),
    })
}

/// Must be called from a UI thread
#[cfg(x11_platform)]
#[no_mangle]
pub extern "C" fn winit_window_handle_get_xlib_window(
    window: *mut ValueBox<WindowHandle>,
) -> *mut std::ffi::c_void {
    with_window_handle(window, |handle| match handle {
        RawWindowHandle::Xlib(handle) => Ok(unsafe { std::mem::transmute(handle.window) }),
        handle => Err(anyhow!("Expected an Xlib, got {:?}", handle).into()),
    })
}

#[cfg(wayland_platform)]
#[no_mangle]
pub extern "C" fn winit_window_handle_get_wayland_surface(
    window: *mut ValueBox<WindowHandle>,
) -> *mut std::ffi::c_void {
    with_window_handle(window, |handle| match handle {
        RawWindowHandle::Wayland(handle) => Ok(handle.surface.as_ptr()),
        handle => Err(anyhow!("Expected a Wayland, got {:?}", handle).into()),
    })
}

#[cfg(wayland_platform)]
#[no_mangle]
pub extern "C" fn winit_window_handle_get_wayland_display(
    window: *mut ValueBox<WindowHandle>,
) -> *mut std::ffi::c_void {
    with_display_handle(window, |handle| match handle {
        RawDisplayHandle::Wayland(handle) => Ok(handle.display.as_ptr()),
        handle => Err(anyhow!("Expected a Wayland, got {:?}", handle).into()),
    })
}

#[no_mangle]
pub fn winit_window_handle_release(window_handle: *mut ValueBox<WindowHandle>) {
    window_handle.release();
}
