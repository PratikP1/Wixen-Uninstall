//! Wixen Uninstall — core library.
//!
//! Author: PratikP1
//!
//! Modules are laid out so that each concern is separate and independently
//! testable.  The Windows-specific I/O (registry writes, file deletion,
//! service stops) is always behind `#[cfg(target_os = "windows")]` so that
//! the entire test suite can be run on any platform.

pub mod elevation;
pub mod executor;
pub mod menu;
pub mod paths;
pub mod plan;
pub mod product;
pub mod ui;
