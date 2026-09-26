#![warn(rust_2018_idioms)]
#![allow(clippy::collapsible_match)]

pub mod backend;

// Re-export AnvilState so main.rs doesn't break
pub use backend::state::{AnvilState, ClientState};
