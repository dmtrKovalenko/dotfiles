//! The kernel: pure, synchronous, deterministic. Nothing in here talks to
//! Herdr, the terminal, the clipboard or the file system.

pub mod alphabet;
pub mod ansi;
pub mod geometry;
pub mod hints;
pub mod matcher;
pub mod patterns;
pub mod screen;
pub mod session;
pub mod settings;
pub mod style;
