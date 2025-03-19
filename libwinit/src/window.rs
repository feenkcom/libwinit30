use parking_lot::Mutex;
use std::sync::Arc;
use value_box::{ReturnBoxerResult, ValueBox, ValueBoxPointer};
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::window::{Window, WindowId};

#[derive(Clone)]
pub struct WindowHandle {
    id: WindowId,
    data: Arc<Mutex<WindowData>>,
}

impl WindowHandle {
    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        self.data.lock().surface_size = size;
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
}

impl From<&dyn Window> for WindowHandle {
    fn from(window: &dyn Window) -> Self {
        WindowHandle {
            id: window.id(),
            data: Arc::new(Mutex::new(WindowData {
                outer_position: window
                    .outer_position()
                    .unwrap_or_else(|_| PhysicalPosition::default()),
                surface_size: window.surface_size(),
                scale_factor: window.scale_factor(),
            })),
        }
    }
}

struct WindowData {
    outer_position: PhysicalPosition<i32>,
    surface_size: PhysicalSize<u32>,
    scale_factor: f64,
}

#[no_mangle]
pub fn winit_window_handle_get_scale_factor(window_handle: *mut ValueBox<WindowHandle>) -> f64 {
    window_handle
        .with_ref_ok(|window_handle| window_handle.scale_factor())
        .or_log(1.0)
}

#[no_mangle]
pub fn winit_window_handle_release(window_handle: *mut ValueBox<WindowHandle>) {
    window_handle.release();
}
