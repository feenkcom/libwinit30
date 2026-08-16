use crate::{VirtualKeyCode, WindowHandle, WinitKeyLocation};
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::fmt::{Debug, Formatter};
use std::os::raw::c_void;
use std::sync::Arc;
use string_box::StringBox;
use value_box::{BorrowedPtr, OwnedPtr, ReturnBoxerResult};
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::{
    ButtonSource, ElementState, Ime, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent,
};
use winit::keyboard::{Key, KeyLocation, ModifiersKeyState};
use winit::window::WindowId;

#[derive(Clone)]
pub struct ApplicationEvents(Arc<Mutex<VecDeque<WinitWindowEvent>>>);

impl ApplicationEvents {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(VecDeque::new())))
    }

    pub fn pop_event(&self) -> Option<WinitWindowEvent> {
        self.0.lock().pop_front()
    }

    pub fn push_event(&self, event: WinitWindowEvent) {
        self.0.lock().push_back(event);
    }
}

impl Debug for ApplicationEvents {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApplicationEvents").finish_non_exhaustive()
    }
}

pub fn convert_event(event: WindowEvent, window: &WindowHandle) -> Vec<Box<dyn WinitEvent>> {
    match event {
        WindowEvent::SurfaceResized(size) => {
            let width = size.width;
            let height = size.height;

            // (Windows) when a window is minimized, its size is set to 0x0,
            // while it shouldn't change, so we just ignore the event
            if width == 0 && height == 0 {
                return vec![];
            }

            let surface_resized_event = WinitWindowResizedEvent { width, height };

            vec![Box::new(surface_resized_event)]
        }
        WindowEvent::Moved(position) => vec![Box::new(WinitWindowMovedEvent {
            x: position.x,
            y: position.y,
        })],
        WindowEvent::CloseRequested => {
            vec![Box::new(WinitWindowCloseRequestedEvent)]
        }
        WindowEvent::Destroyed => vec![],
        WindowEvent::Focused(focused) => {
            vec![Box::new(WinitWindowFocusedEvent {
                is_focused: focused,
            })]
        }
        WindowEvent::KeyboardInput {
            event,
            is_synthetic,
            ..
        } => {
            let mut keyboard_input = WinitEventKeyboardInput::default();
            match event.state {
                ElementState::Pressed => {
                    keyboard_input.state = WinitEventInputElementState::Pressed
                }
                ElementState::Released => {
                    keyboard_input.state = WinitEventInputElementState::Released
                }
            };

            let relevant_key = if event.location != KeyLocation::Numpad {
                event.key_without_modifiers
            }
            else {
                event.logical_key
            };

            match relevant_key {
                Key::Named(key) => {
                    keyboard_input.key_type = WinitKeyType::Named;
                    keyboard_input.named_key = VirtualKeyCode::from(key);
                }
                Key::Character(ch) => {
                    keyboard_input.key_type = WinitKeyType::Character;
                    keyboard_input.character_key = Some(ch.to_string());
                }
                _ => {
                    keyboard_input.key_type = WinitKeyType::Unknown;
                }
            }

            keyboard_input.key_location = WinitKeyLocation::from(event.location);
            keyboard_input.is_synthetic = is_synthetic;

            let mut events = vec![Box::new(keyboard_input) as Box<dyn WinitEvent>];

            if event.state == ElementState::Pressed {
                if let Some(text) = event.text_with_all_modifiers {
                    let text_event = WinitEventReceivedText {
                        text: text.to_string(),
                    };

                    events.push(Box::new(text_event) as Box<dyn WinitEvent>);
                }
            }

            events
        }
        WindowEvent::Ime(Ime::Commit(string)) => {
            let text_event = WinitEventReceivedText { text: string };

            vec![Box::new(text_event)]
        }
        WindowEvent::ModifiersChanged(modifiers) => {
            let modifiers_changed = WinitEventModifiersChanged {
                shift: modifiers.state().shift_key(),
                ctrl: modifiers.state().control_key(),
                alt: modifiers.state().alt_key(),
                logo: modifiers.state().meta_key(),
                num_lock: false,
                left_shift: modifiers.lshift_state().into(),
                right_shift: modifiers.rshift_state().into(),
                left_ctrl: modifiers.lcontrol_state().into(),
                right_ctrl: modifiers.rcontrol_state().into(),
                left_alt: modifiers.lalt_state().into(),
                right_alt: modifiers.ralt_state().into(),
                left_logo: modifiers.lsuper_state().into(),
                right_logo: modifiers.rsuper_state().into(),
            };
            vec![Box::new(modifiers_changed)]
        }
        WindowEvent::PointerMoved { position, .. } => {
            let cursor_moved = WinitCursorMovedEvent {
                device_id: 0,
                x: position.x,
                y: position.y,
            };

            vec![Box::new(cursor_moved)]
        }
        WindowEvent::PointerEntered { .. } => vec![],
        WindowEvent::PointerLeft { .. } => vec![],
        WindowEvent::MouseWheel { delta, phase, .. } => {
            let mut mouse_wheel_event = WinitMouseWheelEvent {
                device_id: 0,
                phase: Default::default(),
                delta: Default::default(),
            };

            match delta {
                MouseScrollDelta::LineDelta(x, y) => {
                    mouse_wheel_event.delta.delta_type = WinitEventMouseScrollDeltaType::LineDelta;
                    mouse_wheel_event.delta.x = -x as f64;
                    mouse_wheel_event.delta.y = y as f64;
                }
                MouseScrollDelta::PixelDelta(PhysicalPosition { x, y }) => {
                    mouse_wheel_event.delta.delta_type = WinitEventMouseScrollDeltaType::PixelDelta;
                    mouse_wheel_event.delta.x = -x;
                    mouse_wheel_event.delta.y = y.clone();
                }
            }

            match phase {
                TouchPhase::Started => {
                    mouse_wheel_event.phase = WinitEventTouchPhase::Started;
                }
                TouchPhase::Moved => {
                    mouse_wheel_event.phase = WinitEventTouchPhase::Moved;
                }
                TouchPhase::Ended => {
                    mouse_wheel_event.phase = WinitEventTouchPhase::Ended;
                }
                TouchPhase::Cancelled => {
                    mouse_wheel_event.phase = WinitEventTouchPhase::Cancelled;
                }
            }

            vec![Box::new(mouse_wheel_event)]
        }
        WindowEvent::PointerButton { state, button, .. } => {
            let mut mouse_input_event = WinitMouseInputEvent {
                device_id: 0,
                state: Default::default(),
                button: Default::default(),
            };

            match state {
                ElementState::Released => {
                    mouse_input_event.state = WinitEventInputElementState::Released;
                }
                ElementState::Pressed => {
                    mouse_input_event.state = WinitEventInputElementState::Pressed;
                }
            }

            match button {
                ButtonSource::Mouse(mouse_button) => match mouse_button {
                    MouseButton::Left => {
                        mouse_input_event.button.button_type = WinitEventMouseButtonType::Left;
                        mouse_input_event.button.button_code = 0;
                    }
                    MouseButton::Right => {
                        mouse_input_event.button.button_type = WinitEventMouseButtonType::Right;
                        mouse_input_event.button.button_code = 1;
                    }
                    MouseButton::Middle => {
                        mouse_input_event.button.button_type = WinitEventMouseButtonType::Middle;
                        mouse_input_event.button.button_code = 2;
                    }
                    MouseButton::Other(code) => {
                        mouse_input_event.button.button_type = WinitEventMouseButtonType::Other;
                        mouse_input_event.button.button_code = code;
                    }
                    MouseButton::Back => {
                        mouse_input_event.button.button_type = WinitEventMouseButtonType::Back;
                        mouse_input_event.button.button_code = 3;
                    }
                    MouseButton::Forward => {
                        mouse_input_event.button.button_type = WinitEventMouseButtonType::Forward;
                        mouse_input_event.button.button_code = 4;
                    }
                },
                ButtonSource::Touch { .. } => {
                    mouse_input_event.button.button_type = WinitEventMouseButtonType::Left;
                    mouse_input_event.button.button_code = 0;
                }
                ButtonSource::Unknown(code) => {
                    mouse_input_event.button.button_type = WinitEventMouseButtonType::Other;
                    mouse_input_event.button.button_code = code;
                }
            }

            vec![Box::new(mouse_input_event)]
        }
        WindowEvent::ScaleFactorChanged {
            scale_factor,
            mut surface_size_writer,
        } => {
            let current_logical_size: LogicalSize<f64> =
                window.surface_size().to_logical(window.scale_factor());
            let new_physical_size = current_logical_size.to_physical(scale_factor);

            let scale_factor_changed = WinitWindowScaleFactorChangedEvent {
                scale_factor,
                width: new_physical_size.width,
                height: new_physical_size.height,
            };

            let _ = surface_size_writer.request_surface_size(new_physical_size);

            vec![Box::new(scale_factor_changed)]
        }
        WindowEvent::RedrawRequested => vec![],
        _ => vec![],
    }
}

#[derive(Debug, Default)]
pub struct WinitWindowCloseRequestedEvent;

impl WinitEvent for WinitWindowCloseRequestedEvent {
    fn event_type(&self) -> WinitEventType {
        WinitEventType::WindowEventCloseRequested
    }
}

#[derive(Debug, Default)]
pub struct WinitTouchEvent {
    device_id: i64,
    phase: WinitEventTouchPhase,
    x: f64,
    y: f64,
    /// unique identifier of a finger.
    id: u64,
}

#[derive(Debug, Default)]
pub struct WinitMouseWheelEvent {
    device_id: i64,
    phase: WinitEventTouchPhase,
    delta: WinitMouseScrollDelta,
}

impl WinitEvent for WinitMouseWheelEvent {
    fn event_type(&self) -> WinitEventType {
        WinitEventType::WindowEventMouseWheel
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct WinitMouseInputEvent {
    device_id: i64,
    state: WinitEventInputElementState,
    button: WinitEventMouseButton,
}

impl WinitEvent for WinitMouseInputEvent {
    fn event_type(&self) -> WinitEventType {
        WinitEventType::WindowEventMouseInput
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct WinitCursorMovedEvent {
    device_id: i64,
    x: f64,
    y: f64,
}

impl WinitEvent for WinitCursorMovedEvent {
    fn event_type(&self) -> WinitEventType {
        WinitEventType::WindowEventCursorMoved
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct WinitWindowResizedEvent {
    width: u32,
    height: u32,
}

impl WinitEvent for WinitWindowResizedEvent {
    fn event_type(&self) -> WinitEventType {
        WinitEventType::WindowEventResized
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct WinitWindowScaleFactorChangedEvent {
    scale_factor: f64,
    width: u32,
    height: u32,
}

impl WinitEvent for WinitWindowScaleFactorChangedEvent {
    fn event_type(&self) -> WinitEventType {
        WinitEventType::WindowEventScaleFactorChanged
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct WinitWindowMovedEvent {
    x: i32,
    y: i32,
}

impl WinitEvent for WinitWindowMovedEvent {
    fn event_type(&self) -> WinitEventType {
        WinitEventType::WindowEventMoved
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct WinitWindowFocusedEvent {
    is_focused: bool,
}

impl WinitEvent for WinitWindowFocusedEvent {
    fn event_type(&self) -> WinitEventType {
        WinitEventType::WindowEventFocused
    }
}

#[derive(Debug, Clone)]
pub struct WinitEventKeyboardInput {
    device_id: i64,
    scan_code: u32,
    state: WinitEventInputElementState,
    key_type: WinitKeyType,
    key_location: WinitKeyLocation,
    named_key: VirtualKeyCode,
    character_key: Option<String>,
    is_synthetic: bool,
}

impl WinitEvent for WinitEventKeyboardInput {
    fn event_type(&self) -> WinitEventType {
        WinitEventType::Winit30WindowEventKeyboardInput
    }
}

impl Default for WinitEventKeyboardInput {
    fn default() -> Self {
        WinitEventKeyboardInput {
            device_id: Default::default(),
            scan_code: Default::default(),
            state: Default::default(),
            key_type: Default::default(),
            key_location: WinitKeyLocation::Standard,
            named_key: VirtualKeyCode::Unknown,
            character_key: None,
            is_synthetic: false,
        }
    }
}

#[derive(Debug, Copy, Clone)]
#[repr(u8)]
pub enum WinitKeyType {
    Unknown,
    Named,
    Character,
}

impl Default for WinitKeyType {
    fn default() -> Self {
        Self::Unknown
    }
}

#[derive(Debug)]
pub struct WinitEventReceivedText {
    text: String,
}

impl WinitEvent for WinitEventReceivedText {
    fn event_type(&self) -> WinitEventType {
        WinitEventType::Winit30WindowEventReceivedText
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct WinitMouseScrollDelta {
    delta_type: WinitEventMouseScrollDeltaType,
    x: f64,
    y: f64,
}

#[derive(Default, Debug, Clone, Copy)]
pub struct WinitEventModifiersChanged {
    /// The "shift" key
    shift: bool,
    /// The "control" key
    ctrl: bool,
    /// The "alt" key
    alt: bool,
    /// The "logo" key
    ///
    /// This is the "windows" key on PC and "command" key on Mac.
    logo: bool,
    num_lock: bool,

    left_shift: WinitModifierKeyState,
    right_shift: WinitModifierKeyState,
    left_ctrl: WinitModifierKeyState,
    right_ctrl: WinitModifierKeyState,
    left_alt: WinitModifierKeyState,
    right_alt: WinitModifierKeyState,
    left_logo: WinitModifierKeyState,
    right_logo: WinitModifierKeyState,
}

#[derive(Debug, Copy, Clone)]
#[repr(u8)]
pub enum WinitModifierKeyState {
    Unknown,
    Pressed,
}

impl From<ModifiersKeyState> for WinitModifierKeyState {
    fn from(value: ModifiersKeyState) -> Self {
        match value {
            ModifiersKeyState::Unknown => WinitModifierKeyState::Unknown,
            ModifiersKeyState::Pressed => WinitModifierKeyState::Pressed,
        }
    }
}

impl Default for WinitModifierKeyState {
    fn default() -> Self {
        Self::Unknown
    }
}

impl WinitEvent for WinitEventModifiersChanged {
    fn event_type(&self) -> WinitEventType {
        WinitEventType::Winit30WindowEventModifiersChanged
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct WinitEventMouseButton {
    button_type: WinitEventMouseButtonType,
    button_code: u16,
}

#[derive(Debug, Copy, Clone)]
#[repr(u32)]
pub enum WinitEventMouseButtonType {
    Unknown,
    Left,
    Right,
    Middle,
    Other,
    Back,
    Forward,
}

impl Default for WinitEventMouseButtonType {
    fn default() -> Self {
        WinitEventMouseButtonType::Unknown
    }
}

pub trait WinitEvent: Debug {
    fn event_type(&self) -> WinitEventType;
}

#[derive(Debug)]
pub struct WinitWindowEvent {
    pub window_id: WindowId,
    pub event: Box<dyn WinitEvent>,
}

impl WinitWindowEvent {
    pub fn window_id(&self) -> WindowId {
        self.window_id
    }

    pub fn event_type(&self) -> WinitEventType {
        self.event.event_type()
    }

    pub fn into_ptr(self) -> *mut c_void {
        Box::into_raw(self.event) as *mut c_void
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
#[repr(u32)]
pub enum WinitEventType {
    Unknown,
    WindowEventResized,
    WindowEventMoved,
    WindowEventCloseRequested,
    WindowEventDestroyed,
    WindowEventDroppedFile,
    WindowEventHoveredFile,
    WindowEventHoveredFileCancelled,
    WindowEventReceivedCharacter,
    WindowEventFocused,
    WindowEventKeyboardInput,
    WindowEventCursorMoved,
    WindowEventCursorEntered,
    WindowEventCursorLeft,
    WindowEventMouseWheel,
    WindowEventMouseInput,
    WindowEventTouchpadPressure,
    WindowEventAxisMotion,
    WindowEventTouch,
    WindowEventScaleFactorChanged,
    NewEvents,
    MainEventsCleared,
    LoopDestroyed,
    Suspended,
    Resumed,
    RedrawRequested,
    RedrawEventsCleared,
    ModifiersChanged,
    UserEvent,
    Winit30WindowEventModifiersChanged,
    Winit30WindowEventKeyboardInput,
    Winit30WindowEventReceivedText,
}

impl Default for WinitEventType {
    fn default() -> Self {
        WinitEventType::Unknown
    }
}

#[derive(Debug, Copy, Clone)]
#[repr(u32)]
pub enum WinitEventTouchPhase {
    Unknown,
    Started,
    Moved,
    Ended,
    Cancelled,
}

impl Default for WinitEventTouchPhase {
    fn default() -> Self {
        WinitEventTouchPhase::Unknown
    }
}

#[derive(Debug, Copy, Clone)]
#[repr(u32)]
pub enum WinitEventMouseScrollDeltaType {
    Unknown,
    LineDelta,
    PixelDelta,
}

impl Default for WinitEventMouseScrollDeltaType {
    fn default() -> Self {
        WinitEventMouseScrollDeltaType::Unknown
    }
}

#[derive(Debug, Copy, Clone)]
#[repr(u32)]
pub enum WinitEventInputElementState {
    Unknown,
    Pressed,
    Released,
}

impl Default for WinitEventInputElementState {
    fn default() -> Self {
        WinitEventInputElementState::Unknown
    }
}

#[no_mangle]
pub extern "C" fn winit_window_event_release(event: OwnedPtr<WinitWindowEvent>) {
    drop(event);
}

#[no_mangle]
pub extern "C" fn winit_window_event_get_type(
    event: BorrowedPtr<WinitWindowEvent>,
) -> WinitEventType {
    event
        .with_ref_ok(|event| event.event_type())
        .or_log(WinitEventType::Unknown)
}

#[no_mangle]
pub extern "C" fn winit_window_event_get_window_id(
    event: BorrowedPtr<WinitWindowEvent>,
) -> usize {
    event
        .with_ref_ok(|event| event.window_id().into_raw())
        .or_log(0)
}

#[no_mangle]
pub extern "C" fn winit_window_event_get_event_ptr(
    event: OwnedPtr<WinitWindowEvent>,
) -> *mut c_void {
    event
        .with_value_ok(|e| e.into_ptr())
        .or_log(std::ptr::null_mut())
}

// --- Event release functions ---

#[no_mangle]
pub extern "C" fn winit_event_cursor_moved_release(event: OwnedPtr<WinitCursorMovedEvent>) {
    drop(event);
}

#[no_mangle]
pub extern "C" fn winit_event_window_resized_release(event: OwnedPtr<WinitWindowResizedEvent>) {
    drop(event);
}

#[no_mangle]
pub extern "C" fn winit_event_window_moved_release(event: OwnedPtr<WinitWindowMovedEvent>) {
    drop(event);
}

#[no_mangle]
pub extern "C" fn winit_event_window_focused_release(event: OwnedPtr<WinitWindowFocusedEvent>) {
    drop(event);
}

#[no_mangle]
pub extern "C" fn winit_event_scale_factor_changed_release(event: OwnedPtr<WinitWindowScaleFactorChangedEvent>) {
    drop(event);
}

#[no_mangle]
pub extern "C" fn winit_event_mouse_input_release(event: OwnedPtr<WinitMouseInputEvent>) {
    drop(event);
}

#[no_mangle]
pub extern "C" fn winit_event_mouse_wheel_release(event: OwnedPtr<WinitMouseWheelEvent>) {
    drop(event);
}

#[no_mangle]
pub extern "C" fn winit_event_keyboard_input_release(event: OwnedPtr<WinitEventKeyboardInput>) {
    drop(event);
}

#[no_mangle]
pub extern "C" fn winit_event_received_text_release(event: OwnedPtr<WinitEventReceivedText>) {
    drop(event);
}

#[no_mangle]
pub extern "C" fn winit_event_modifiers_changed_release(event: OwnedPtr<WinitEventModifiersChanged>) {
    drop(event);
}

#[no_mangle]
pub extern "C" fn winit_event_touch_release(event: OwnedPtr<WinitTouchEvent>) {
    drop(event);
}

#[no_mangle]
pub extern "C" fn winit_event_close_requested_release(event: OwnedPtr<WinitWindowCloseRequestedEvent>) {
    drop(event);
}

// --- Event accessor functions ---

// CursorMoved accessors

#[no_mangle]
pub extern "C" fn winit_event_cursor_moved_device_id(event: BorrowedPtr<WinitCursorMovedEvent>) -> i64 {
    event.with_ref_ok(|e| e.device_id).or_log(0)
}

#[no_mangle]
pub extern "C" fn winit_event_cursor_moved_x(event: BorrowedPtr<WinitCursorMovedEvent>) -> f64 {
    event.with_ref_ok(|e| e.x).or_log(0.0)
}

#[no_mangle]
pub extern "C" fn winit_event_cursor_moved_y(event: BorrowedPtr<WinitCursorMovedEvent>) -> f64 {
    event.with_ref_ok(|e| e.y).or_log(0.0)
}

// WindowResized accessors

#[no_mangle]
pub extern "C" fn winit_event_window_resized_width(event: BorrowedPtr<WinitWindowResizedEvent>) -> u32 {
    event.with_ref_ok(|e| e.width).or_log(0)
}

#[no_mangle]
pub extern "C" fn winit_event_window_resized_height(event: BorrowedPtr<WinitWindowResizedEvent>) -> u32 {
    event.with_ref_ok(|e| e.height).or_log(0)
}

// WindowMoved accessors

#[no_mangle]
pub extern "C" fn winit_event_window_moved_x(event: BorrowedPtr<WinitWindowMovedEvent>) -> i32 {
    event.with_ref_ok(|e| e.x).or_log(0)
}

#[no_mangle]
pub extern "C" fn winit_event_window_moved_y(event: BorrowedPtr<WinitWindowMovedEvent>) -> i32 {
    event.with_ref_ok(|e| e.y).or_log(0)
}

// WindowFocused accessors

#[no_mangle]
pub extern "C" fn winit_event_window_focused_is_focused(event: BorrowedPtr<WinitWindowFocusedEvent>) -> bool {
    event.with_ref_ok(|e| e.is_focused).or_log(false)
}

// ScaleFactorChanged accessors

#[no_mangle]
pub extern "C" fn winit_event_scale_factor_changed_scale_factor(event: BorrowedPtr<WinitWindowScaleFactorChangedEvent>) -> f64 {
    event.with_ref_ok(|e| e.scale_factor).or_log(0.0)
}

#[no_mangle]
pub extern "C" fn winit_event_scale_factor_changed_width(event: BorrowedPtr<WinitWindowScaleFactorChangedEvent>) -> u32 {
    event.with_ref_ok(|e| e.width).or_log(0)
}

#[no_mangle]
pub extern "C" fn winit_event_scale_factor_changed_height(event: BorrowedPtr<WinitWindowScaleFactorChangedEvent>) -> u32 {
    event.with_ref_ok(|e| e.height).or_log(0)
}

// MouseInput accessors

#[no_mangle]
pub extern "C" fn winit_event_mouse_input_device_id(event: BorrowedPtr<WinitMouseInputEvent>) -> i64 {
    event.with_ref_ok(|e| e.device_id).or_log(0)
}

#[no_mangle]
pub extern "C" fn winit_event_mouse_input_state(event: BorrowedPtr<WinitMouseInputEvent>) -> u32 {
    event.with_ref_ok(|e| e.state as u32).or_log(0)
}

#[no_mangle]
pub extern "C" fn winit_event_mouse_input_button_type(event: BorrowedPtr<WinitMouseInputEvent>) -> u32 {
    event.with_ref_ok(|e| e.button.button_type as u32).or_log(0)
}

#[no_mangle]
pub extern "C" fn winit_event_mouse_input_button_code(event: BorrowedPtr<WinitMouseInputEvent>) -> u16 {
    event.with_ref_ok(|e| e.button.button_code).or_log(0)
}

// MouseWheel accessors

#[no_mangle]
pub extern "C" fn winit_event_mouse_wheel_device_id(event: BorrowedPtr<WinitMouseWheelEvent>) -> i64 {
    event.with_ref_ok(|e| e.device_id).or_log(0)
}

#[no_mangle]
pub extern "C" fn winit_event_mouse_wheel_phase(event: BorrowedPtr<WinitMouseWheelEvent>) -> u32 {
    event.with_ref_ok(|e| e.phase as u32).or_log(0)
}

#[no_mangle]
pub extern "C" fn winit_event_mouse_wheel_delta_type(event: BorrowedPtr<WinitMouseWheelEvent>) -> u32 {
    event.with_ref_ok(|e| e.delta.delta_type as u32).or_log(0)
}

#[no_mangle]
pub extern "C" fn winit_event_mouse_wheel_delta_x(event: BorrowedPtr<WinitMouseWheelEvent>) -> f64 {
    event.with_ref_ok(|e| e.delta.x).or_log(0.0)
}

#[no_mangle]
pub extern "C" fn winit_event_mouse_wheel_delta_y(event: BorrowedPtr<WinitMouseWheelEvent>) -> f64 {
    event.with_ref_ok(|e| e.delta.y).or_log(0.0)
}

// KeyboardInput accessors

#[no_mangle]
pub extern "C" fn winit_event_keyboard_input_device_id(event: BorrowedPtr<WinitEventKeyboardInput>) -> i64 {
    event.with_ref_ok(|e| e.device_id).or_log(0)
}

#[no_mangle]
pub extern "C" fn winit_event_keyboard_input_scan_code(event: BorrowedPtr<WinitEventKeyboardInput>) -> u32 {
    event.with_ref_ok(|e| e.scan_code).or_log(0)
}

#[no_mangle]
pub extern "C" fn winit_event_keyboard_input_state(event: BorrowedPtr<WinitEventKeyboardInput>) -> u32 {
    event.with_ref_ok(|e| e.state as u32).or_log(0)
}

#[no_mangle]
pub extern "C" fn winit_event_keyboard_input_key_type(event: BorrowedPtr<WinitEventKeyboardInput>) -> u8 {
    event.with_ref_ok(|e| e.key_type as u8).or_log(0)
}

#[no_mangle]
pub extern "C" fn winit_event_keyboard_input_key_location(event: BorrowedPtr<WinitEventKeyboardInput>) -> u8 {
    event.with_ref_ok(|e| e.key_location as u8).or_log(0)
}

#[no_mangle]
pub extern "C" fn winit_event_keyboard_input_named_key(event: BorrowedPtr<WinitEventKeyboardInput>) -> u32 {
    event.with_ref_ok(|e| e.named_key as u32).or_log(0)
}

#[no_mangle]
pub extern "C" fn winit_event_keyboard_input_character_key(
    event: BorrowedPtr<WinitEventKeyboardInput>,
) -> OwnedPtr<StringBox> {
    event
        .with_ref_ok(|e| match &e.character_key {
            Some(s) => OwnedPtr::new(StringBox::from_string(s.clone())),
            None => OwnedPtr::null(),
        })
        .or_log(OwnedPtr::null())
}

#[no_mangle]
pub extern "C" fn winit_event_keyboard_input_is_synthetic(event: BorrowedPtr<WinitEventKeyboardInput>) -> bool {
    event.with_ref_ok(|e| e.is_synthetic).or_log(false)
}

// ReceivedText accessors

#[no_mangle]
pub extern "C" fn winit_event_received_text_text(
    event: BorrowedPtr<WinitEventReceivedText>,
) -> OwnedPtr<StringBox> {
    event
        .with_ref_ok(|e| OwnedPtr::new(StringBox::from_string(e.text.clone())))
        .or_log(OwnedPtr::null())
}

// ModifiersChanged accessors

#[no_mangle]
pub extern "C" fn winit_event_modifiers_changed_shift(event: BorrowedPtr<WinitEventModifiersChanged>) -> bool {
    event.with_ref_ok(|e| e.shift).or_log(false)
}

#[no_mangle]
pub extern "C" fn winit_event_modifiers_changed_ctrl(event: BorrowedPtr<WinitEventModifiersChanged>) -> bool {
    event.with_ref_ok(|e| e.ctrl).or_log(false)
}

#[no_mangle]
pub extern "C" fn winit_event_modifiers_changed_alt(event: BorrowedPtr<WinitEventModifiersChanged>) -> bool {
    event.with_ref_ok(|e| e.alt).or_log(false)
}

#[no_mangle]
pub extern "C" fn winit_event_modifiers_changed_logo(event: BorrowedPtr<WinitEventModifiersChanged>) -> bool {
    event.with_ref_ok(|e| e.logo).or_log(false)
}

#[no_mangle]
pub extern "C" fn winit_event_modifiers_changed_num_lock(event: BorrowedPtr<WinitEventModifiersChanged>) -> bool {
    event.with_ref_ok(|e| e.num_lock).or_log(false)
}

#[no_mangle]
pub extern "C" fn winit_event_modifiers_changed_left_shift(event: BorrowedPtr<WinitEventModifiersChanged>) -> u8 {
    event.with_ref_ok(|e| e.left_shift as u8).or_log(0)
}

#[no_mangle]
pub extern "C" fn winit_event_modifiers_changed_right_shift(event: BorrowedPtr<WinitEventModifiersChanged>) -> u8 {
    event.with_ref_ok(|e| e.right_shift as u8).or_log(0)
}

#[no_mangle]
pub extern "C" fn winit_event_modifiers_changed_left_ctrl(event: BorrowedPtr<WinitEventModifiersChanged>) -> u8 {
    event.with_ref_ok(|e| e.left_ctrl as u8).or_log(0)
}

#[no_mangle]
pub extern "C" fn winit_event_modifiers_changed_right_ctrl(event: BorrowedPtr<WinitEventModifiersChanged>) -> u8 {
    event.with_ref_ok(|e| e.right_ctrl as u8).or_log(0)
}

#[no_mangle]
pub extern "C" fn winit_event_modifiers_changed_left_alt(event: BorrowedPtr<WinitEventModifiersChanged>) -> u8 {
    event.with_ref_ok(|e| e.left_alt as u8).or_log(0)
}

#[no_mangle]
pub extern "C" fn winit_event_modifiers_changed_right_alt(event: BorrowedPtr<WinitEventModifiersChanged>) -> u8 {
    event.with_ref_ok(|e| e.right_alt as u8).or_log(0)
}

#[no_mangle]
pub extern "C" fn winit_event_modifiers_changed_left_logo(event: BorrowedPtr<WinitEventModifiersChanged>) -> u8 {
    event.with_ref_ok(|e| e.left_logo as u8).or_log(0)
}

#[no_mangle]
pub extern "C" fn winit_event_modifiers_changed_right_logo(event: BorrowedPtr<WinitEventModifiersChanged>) -> u8 {
    event.with_ref_ok(|e| e.right_logo as u8).or_log(0)
}

// Touch accessors

#[no_mangle]
pub extern "C" fn winit_event_touch_device_id(event: BorrowedPtr<WinitTouchEvent>) -> i64 {
    event.with_ref_ok(|e| e.device_id).or_log(0)
}

#[no_mangle]
pub extern "C" fn winit_event_touch_phase(event: BorrowedPtr<WinitTouchEvent>) -> u32 {
    event.with_ref_ok(|e| e.phase as u32).or_log(0)
}

#[no_mangle]
pub extern "C" fn winit_event_touch_x(event: BorrowedPtr<WinitTouchEvent>) -> f64 {
    event.with_ref_ok(|e| e.x).or_log(0.0)
}

#[no_mangle]
pub extern "C" fn winit_event_touch_y(event: BorrowedPtr<WinitTouchEvent>) -> f64 {
    event.with_ref_ok(|e| e.y).or_log(0.0)
}

#[no_mangle]
pub extern "C" fn winit_event_touch_id(event: BorrowedPtr<WinitTouchEvent>) -> u64 {
    event.with_ref_ok(|e| e.id).or_log(0)
}
