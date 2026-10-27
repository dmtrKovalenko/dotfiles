//! tmux-fingers for Herdr: hints over everything worth copying in a pane.
//!
//! `domain` is the pure kernel (parsing, matching, hint labels, the picking
//! state machine); `usecases` say what the plugin does against ports;
//! `adapters` implement those ports with Herdr, the terminal, the clipboard
//! and the file system; `app` wires them together for each CLI subcommand.

pub mod adapters;
pub mod app;
pub mod custom;
pub mod domain;
pub mod usecases;
