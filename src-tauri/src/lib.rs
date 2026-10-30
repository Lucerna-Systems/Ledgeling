//! Ledgeling: a small companion that lives on the edges of your windows.
//!
//! Rust owns all truth — world model, pet state, behavior — and the webview is
//! a renderer. Everything except the Tauri wiring in [`app`] is testable
//! headlessly through [`engine::Engine`] and a mock [`platform::Platform`].
pub mod app;
pub mod autostart;
pub mod awareness;
#[cfg(feature = "coding")]
pub mod coding;
pub mod companion;
pub mod engine;
pub mod jelly;
pub mod migrate;
pub mod net;
pub mod packs;
pub mod pet;
pub mod platform;
pub mod settings;
pub mod terminal;
pub mod windows_mgr;
pub mod world;

pub mod tasks;
