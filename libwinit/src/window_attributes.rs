use winit::dpi::LogicalSize;

use string_box::StringBox;
use value_box::{BorrowedPtr, OwnedPtr, ReturnBoxerResult};
use winit::window::{WindowAttributes, WindowLevel};

#[no_mangle]
pub extern "C" fn winit_window_attributes_new() -> OwnedPtr<WindowAttributes> {
    OwnedPtr::new(WindowAttributes::default())
}

#[no_mangle]
pub extern "C" fn winit_window_attributes_release(window_attributes: OwnedPtr<WindowAttributes>) {
    drop(window_attributes);
}

#[no_mangle]
pub extern "C" fn winit_window_attributes_with_title(
    mut window_attributes: BorrowedPtr<WindowAttributes>,
    window_title: BorrowedPtr<StringBox>,
) {
    window_title
        .with_ref(|window_title| {
            window_attributes.with_mut_ok(|attrs| {
                let taken = std::mem::take(attrs);
                *attrs = taken.with_title(window_title.to_string());
            })
        })
        .log();
}

#[no_mangle]
pub extern "C" fn winit_window_attributes_with_decorations(
    mut window_attributes: BorrowedPtr<WindowAttributes>,
    with_decorations: bool,
) {
    window_attributes
        .with_mut_ok(|attrs| {
            let taken = std::mem::take(attrs);
            *attrs = taken.with_decorations(with_decorations);
        })
        .log();
}

#[no_mangle]
pub extern "C" fn winit_window_attributes_with_transparency(
    mut window_attributes: BorrowedPtr<WindowAttributes>,
    with_transparency: bool,
) {
    window_attributes
        .with_mut_ok(|attrs| {
            let taken = std::mem::take(attrs);
            *attrs = taken.with_transparent(with_transparency);
        })
        .log();
}

#[no_mangle]
pub extern "C" fn winit_window_attributes_with_resizable(
    mut window_attributes: BorrowedPtr<WindowAttributes>,
    with_resizable: bool,
) {
    window_attributes
        .with_mut_ok(|attrs| {
            let taken = std::mem::take(attrs);
            *attrs = taken.with_resizable(with_resizable);
        })
        .log();
}

#[no_mangle]
pub extern "C" fn winit_window_attributes_with_dimensions(
    mut window_attributes: BorrowedPtr<WindowAttributes>,
    width: f64,
    height: f64,
) {
    window_attributes
        .with_mut_ok(|attrs| {
            let taken = std::mem::take(attrs);
            *attrs = taken.with_surface_size(LogicalSize::new(width, height));
        })
        .log();
}

#[no_mangle]
pub extern "C" fn winit_window_attributes_with_maximized(
    mut window_attributes: BorrowedPtr<WindowAttributes>,
    with_maximized: bool,
) {
    window_attributes
        .with_mut_ok(|attrs| {
            let taken = std::mem::take(attrs);
            *attrs = taken.with_maximized(with_maximized);
        })
        .log();
}

#[no_mangle]
pub extern "C" fn winit_window_attributes_with_visibility(
    mut window_attributes: BorrowedPtr<WindowAttributes>,
    with_visibility: bool,
) {
    window_attributes
        .with_mut_ok(|attrs| {
            let taken = std::mem::take(attrs);
            *attrs = taken.with_visible(with_visibility);
        })
        .log();
}

#[no_mangle]
pub extern "C" fn winit_window_attributes_with_always_on_top(
    mut window_attributes: BorrowedPtr<WindowAttributes>,
    with_always_on_top: bool,
) {
    window_attributes
        .with_mut_ok(|attrs| {
            let taken = std::mem::take(attrs);
            let level = match with_always_on_top {
                true => WindowLevel::AlwaysOnTop,
                false => WindowLevel::Normal,
            };
            *attrs = taken.with_window_level(level);
        })
        .log();
}

#[cfg(not(target_os = "macos"))]
#[no_mangle]
pub extern "C" fn winit_window_attributes_with_full_size(
    _ptr_window_attributes: BorrowedPtr<WindowAttributes>,
    _with_full_size: bool,
) {
}

#[cfg(target_os = "macos")]
#[no_mangle]
pub extern "C" fn winit_window_attributes_with_full_size(
    mut window_attributes: BorrowedPtr<WindowAttributes>,
    with_full_size: bool,
) {
    use winit::platform::macos::WindowAttributesMacOS;

    window_attributes
        .with_mut_ok(|attrs| {
            let taken = std::mem::take(attrs);
            let macos_attributes = WindowAttributesMacOS::default()
                .with_titlebar_transparent(with_full_size)
                .with_title_hidden(with_full_size)
                .with_fullsize_content_view(with_full_size);

            *attrs = taken.with_platform_attributes(Box::new(macos_attributes));
        })
        .log();
}
