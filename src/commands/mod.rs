//! Tauri commands: what the window can ask the Rust side to do, grouped by area.

#![allow(unused_imports)]
use crate::*;

pub mod ai;
pub mod app;
pub mod files;
pub mod hosting;
pub mod repo;
pub mod settings;
pub mod terminal;

pub(crate) use app::{blocking, err, read_blocking, AppState, WatchSlot};
