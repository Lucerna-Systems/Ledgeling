//! Pet packs: data, never code. Nothing in this module evaluates anything but
//! the fixed condition grammar in `expr`, and no pack field is ever executed.
pub mod builtin;
pub mod decor;
pub mod diagnostics;
pub mod expr;
pub mod gallery;
pub mod limits;
pub mod loader;
pub mod manifest;
pub mod scaffold;
pub mod sprites;
pub mod studio;

pub use diagnostics::{Diagnostic, Report, Tier};
pub use expr::{Condition, Signal, Signals};
pub use loader::{ClipAtlas, Pack, PackListing, PackSummary};
pub use manifest::{Action, Event, LocomotionClass, Manifest, SurfaceOrientation};

#[cfg(test)]
mod tests;
