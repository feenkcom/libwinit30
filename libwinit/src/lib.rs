#![allow(non_snake_case)]

#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;
#[macro_use]
extern crate value_box;
mod actions;
mod application;
mod cursor;
mod events;
mod keyboard;
mod monitor;
mod signallers;
mod window;
mod window_attributes;

pub use actions::*;
pub use application::*;
pub use cursor::*;
pub use events::*;
pub use keyboard::*;
pub use monitor::*;
pub use signallers::*;
pub use window::*;
pub use window_attributes::*;

pub use value_box_ffi::*;

#[no_mangle]
pub extern "C" fn winit_test() -> bool {
    true
}

#[no_mangle]
pub extern "C" fn winit_init_logger() {
    env_logger::init();
}
