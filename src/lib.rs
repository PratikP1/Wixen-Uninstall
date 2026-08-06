//! Wixen Uninstall — core library.
//!
//! Author: PratikP1
//!
//! Modules are laid out so that each concern is separate and independently
//! testable.  The Windows-specific I/O (registry writes, file deletion,
//! service stops) is always behind `#[cfg(target_os = "windows")]` so that
//! the entire test suite can be run on any platform.

pub mod elevation;
pub mod escalation;
pub mod executor;
pub mod forceful;
pub mod menu;
pub mod paths;
pub mod plan;
pub mod product;
pub mod reboot;
pub mod resume;
pub mod stats_ini;
pub mod system_exec;
pub mod ui;
pub mod uninstall;
pub mod vendor;
