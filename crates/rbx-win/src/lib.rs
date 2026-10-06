//! Thin wrappers over the Windows APIs the bootstrapper needs. Every function has a
//! non-Windows fallback so the rest of the workspace builds and tests on any OS.

pub mod afk;
pub mod dialog;
pub mod notify;
pub mod process;
pub mod registry;
pub mod sync;
