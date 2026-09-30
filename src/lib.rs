#![warn(rust_2018_idioms)]
#![allow(clippy::collapsible_match)]

pub mod backend;
pub mod protocol;
pub mod wm;

// Re-export AnvilState so main.rs doesn't break
pub use backend::state::{AnvilState, ClientState};
