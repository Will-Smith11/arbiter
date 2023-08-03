#![warn(missing_docs)]
#![warn(unsafe_code)]
//! Lib crate for describing simulations.

pub mod agent;
pub mod bindings;
pub mod environment;
pub mod manager;
pub mod math;
pub mod middleware;
pub mod strategies;
// #[cfg(test)] //TODO: UNCOMMENT THIS LATER
#[cfg(test)] //TODO: UNCOMMENT THIS LATER
pub mod tests;
pub mod utils;
