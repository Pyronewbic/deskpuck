//! Platform-independent core of Deskpuck: Joy-Con 2 report parsing, the input
//! engine, button-to-key mapping, and the JSON config.

pub mod config;
pub mod engine;
pub mod mapping;
pub mod packet;
