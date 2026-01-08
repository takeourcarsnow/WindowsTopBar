//! Render module - graphics and drawing
//!
//! Contains all rendering-related code including the main renderer,
//! and icon handling.

#![allow(dead_code, unused_unsafe)]

mod context;
mod drawing;
mod icons;
mod modules;
mod quick_search;
mod renderer;

pub use quick_search::show_quick_search;
pub use renderer::Renderer;
