//! The keyboard side of Hidra as a library, so other front ends (octo hub)
//! can drive a VIA keyboard without the window or the CLI.

pub mod definition;
pub mod hid;
pub mod keycode;
pub mod keycode_table;
pub mod macros;
pub mod menus;
pub mod mock;
pub mod modes;
pub mod proto;
pub mod session;
