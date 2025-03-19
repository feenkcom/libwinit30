use winit::dpi::LogicalSize;
#[cfg(target_os = "macos")]
use winit::platform::macos::WindowAttributesExtMacOS;

use string_box::StringBox;
use value_box::{ReturnBoxerResult, ValueBox, ValueBoxPointer};
use winit::window::{WindowAttributes, WindowLevel};

#[no_mangle]
pub extern "C" fn winit_window_attributes_new() -> *mut ValueBox<WindowAttributes> {
    ValueBox::new(WindowAttributes::default()).into_raw()
}

#[no_mangle]
pub extern "C" fn winit_window_attributes_release(
    window_attributes: *mut ValueBox<WindowAttributes>,
) {
    window_attributes.release();
}

#[no_mangle]
pub extern "C" fn winit_window_attributes_with_title(
    window_attributes: *mut ValueBox<WindowAttributes>,
    window_title: *mut ValueBox<StringBox>,
) {
    window_title
        .with_ref_ok(|window_title| {
            window_attributes.replace_value(|window_attributes| {
                window_attributes.with_title(window_title.to_string())
            })
        })
        .log();
}

#[no_mangle]
pub extern "C" fn winit_window_attributes_with_decorations(
    window_attributes: *mut ValueBox<WindowAttributes>,
    with_decorations: bool,
) {
    window_attributes
        .replace_value(|window_attributes| window_attributes.with_decorations(with_decorations))
        .log();
}

#[no_mangle]
pub extern "C" fn winit_window_attributes_with_transparency(
    window_attributes: *mut ValueBox<WindowAttributes>,
    with_transparency: bool,
) {
    window_attributes
        .replace_value(|window_attributes| window_attributes.with_transparent(with_transparency))
        .log();
}

#[no_mangle]
pub extern "C" fn winit_window_attributes_with_resizable(
    window_attributes: *mut ValueBox<WindowAttributes>,
    with_resizable: bool,
) {
    window_attributes
        .replace_value(|window_attributes| window_attributes.with_resizable(with_resizable))
        .log();
}

#[no_mangle]
pub extern "C" fn winit_window_attributes_with_dimensions(
    window_attributes: *mut ValueBox<WindowAttributes>,
    width: f64,
    height: f64,
) {
    window_attributes
        .replace_value(|window_attributes| {
            window_attributes.with_surface_size(LogicalSize::new(width, height))
        })
        .log();
}

#[no_mangle]
pub extern "C" fn winit_window_attributes_with_maximized(
    window_attributes: *mut ValueBox<WindowAttributes>,
    with_maximized: bool,
) {
    window_attributes
        .replace_value(|window_attributes| window_attributes.with_maximized(with_maximized))
        .log();
}

#[no_mangle]
pub extern "C" fn winit_window_attributes_with_visibility(
    window_attributes: *mut ValueBox<WindowAttributes>,
    with_visibility: bool,
) {
    window_attributes
        .replace_value(|window_attributes| window_attributes.with_visible(with_visibility))
        .log();
}

#[no_mangle]
pub extern "C" fn winit_window_attributes_with_always_on_top(
    window_attributes: *mut ValueBox<WindowAttributes>,
    with_always_on_top: bool,
) {
    window_attributes
        .replace_value(|window_attributes| {
            let level = match with_always_on_top {
                true => WindowLevel::AlwaysOnTop,
                false => WindowLevel::Normal,
            };
            window_attributes.with_window_level(level)
        })
        .log();
}

#[cfg(not(target_os = "macos"))]
#[no_mangle]
pub extern "C" fn winit_window_attributes_with_full_size(
    _ptr_window_attributes: *mut ValueBox<WindowAttributes>,
    _with_full_size: bool,
) {
}

#[cfg(target_os = "macos")]
#[no_mangle]
pub extern "C" fn winit_window_attributes_with_full_size(
    window_attributes: *mut ValueBox<WindowAttributes>,
    with_full_size: bool,
) {
    window_attributes
        .replace_value(|window_attributes| {
            window_attributes
                .with_titlebar_transparent(with_full_size)
                .with_fullsize_content_view(with_full_size)
                .with_title_hidden(with_full_size)
        })
        .log();
}
