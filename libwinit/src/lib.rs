#![allow(non_snake_case)]

mod actions;
mod application;
mod event_loop_builder;
mod events;
mod signallers;
mod window;
mod window_attributes;

pub use actions::*;
pub use application::*;
pub use event_loop_builder::*;
pub use events::*;
pub use signallers::*;
pub use window::*;
pub use window_attributes::*;

#[macro_use]
extern crate log;

#[macro_use]
extern crate value_box;

#[no_mangle]
pub extern "C" fn winit_test() -> bool {
    true
}

#[no_mangle]
pub extern "C" fn winit_init_logger() {
    env_logger::init();
}
