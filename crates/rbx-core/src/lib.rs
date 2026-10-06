//! The bootstrapper's logic, independent of any UI: where files live, what state we keep,
//! which channel to use, and the install-then-launch flow.

pub mod bootstrap;
pub mod channel;
pub mod paths;
pub mod settings;
pub mod state;

pub use bootstrap::{Error, LaunchOptions, Status, StatusFn, run};
pub use paths::Paths;
pub use settings::Settings;
