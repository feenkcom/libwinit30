use crate::{VirtualKeyCode, WindowHandle, WinitKeyLocation};
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::fmt::{Debug, Formatter};
use std::os::raw::c_void;
use std::sync::Arc;
use string_box::StringBox;
use value_box::{ValueBox, ValueBoxPointer};
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
                    keyboard_input.character_key =
                        ValueBox::new(StringBox::from_string(ch.to_string())).into_raw();
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
                        text: ValueBox::new(StringBox::from_string(text.to_string())).into_raw(),
                    };

                    events.push(Box::new(text_event) as Box<dyn WinitEvent>);
                }
            }

            events
        }
        WindowEvent::Ime(Ime::Commit(string)) => {
            let text_event = WinitEventReceivedText {
                text: ValueBox::new(StringBox::from_string(string)).into_raw(),
            };

            vec![Box::new(text_event)]
        }
        WindowEvent::ModifiersChanged(modifiers) => {
            let modifiers_changed = WinitEventModifiersChanged {
                shift: modifiers.state().shift_key(),
                ctrl: modifiers.state().control_key(),
                alt: modifiers.state().alt_key(),
                logo: modifiers.state().meta_key(),
                num_lock: false,//modifiers.state().num_lock_key(),
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
#[repr(C)]
pub struct WinitWindowCloseRequestedEvent;

impl WinitEvent for WinitWindowCloseRequestedEvent {
    fn event_type(&self) -> WinitEventType {
        WinitEventType::WindowEventCloseRequested
    }
}

#[derive(Debug, Default)]
#[repr(C)]
pub struct WinitTouchEvent {
    device_id: i64,
    phase: WinitEventTouchPhase,
    x: f64,
    y: f64,
    /// unique identifier of a finger.
    id: u64,
}

#[derive(Debug, Default)]
#[repr(C)]
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
#[repr(C)]
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
#[repr(C)]
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
#[repr(C)]
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
#[repr(C)]
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
#[repr(C)]
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
#[repr(C)]
pub struct WinitWindowFocusedEvent {
    is_focused: bool,
}

impl WinitEvent for WinitWindowFocusedEvent {
    fn event_type(&self) -> WinitEventType {
        WinitEventType::WindowEventFocused
    }
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct WinitEventKeyboardInput {
    device_id: i64,
    scan_code: u32,
    state: WinitEventInputElementState,
    key_type: WinitKeyType,
    key_location: WinitKeyLocation,
    named_key: VirtualKeyCode,
    character_key: *mut ValueBox<StringBox>,
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
            character_key: std::ptr::null_mut(),
            is_synthetic: false,
        }
    }
}

impl Drop for WinitEventKeyboardInput {
    fn drop(&mut self) {
        if !self.character_key.is_null() {
            self.character_key.release();
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
#[repr(C)]
pub struct WinitEventReceivedText {
    text: *mut ValueBox<StringBox>,
}

impl Drop for WinitEventReceivedText {
    fn drop(&mut self) {
        if !self.text.is_null() {
            self.text.release();
        }
    }
}

impl WinitEvent for WinitEventReceivedText {
    fn event_type(&self) -> WinitEventType {
        WinitEventType::Winit30WindowEventReceivedText
    }
}

#[derive(Debug, Copy, Clone, Default)]
#[repr(C)]
pub struct WinitMouseScrollDelta {
    delta_type: WinitEventMouseScrollDeltaType,
    x: f64,
    y: f64,
}

#[derive(Default, Debug, Clone, Copy)]
#[repr(C)]
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
#[repr(C)]
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

    pub fn as_ptr(&self) -> *mut c_void {
        self.event.as_ref() as *const _ as *mut c_void
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
pub extern "C" fn winit_window_event_release(event: *mut ValueBox<WinitWindowEvent>) {
    event.release();
}
