//! Native modules are compiled without changing C's composition root.
#![allow(dead_code)]
#[path = "../../apps/desktop/src-tauri/src/storage/mod.rs"] pub mod storage;
#[path = "../../apps/desktop/src-tauri/src/audio/mod.rs"] pub mod audio;
#[path = "../../apps/desktop/src-tauri/src/capture/mod.rs"] pub mod capture;
#[path = "../../apps/desktop/src-tauri/src/tasks/mod.rs"] pub mod tasks;
#[path = "../../apps/desktop/src-tauri/src/ipc/mod.rs"] pub mod ipc;
#[path = "../../apps/desktop/src-tauri/src/windows/mod.rs"] pub mod windows;
#[path = "../../apps/desktop/src-tauri/src/lifecycle/mod.rs"] pub mod lifecycle;
#[path = "../../apps/desktop/src-tauri/src/library/mod.rs"] pub mod library;
#[path = "../../apps/desktop/src-tauri/src/search/mod.rs"] pub mod search;
#[path = "../../apps/desktop/src-tauri/src/visualizations/mod.rs"] pub mod visualizations;
pub const INITIAL_MIGRATION:&str=include_str!("../../apps/desktop/src-tauri/migrations/0001_initial.sql");
#[cfg(test)] mod tests;
