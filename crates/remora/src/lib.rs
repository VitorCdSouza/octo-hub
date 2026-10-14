//! The mouse side of Remora as a library, so other front ends (octo hub)
//! can drive the X6 without the window or the CLI.

pub mod hid;
pub mod keys;
pub mod link;
pub mod mock;
pub mod profile;
pub mod proto;
