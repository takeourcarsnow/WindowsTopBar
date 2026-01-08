//! Renderer integration for window management
//!
//! Thread-local storage for the renderer and helper functions.

use crate::render::Renderer;

/// Thread-local storage for the renderer (contains non-Send HWND)
thread_local! {
    #[allow(clippy::missing_const_for_thread_local)]
    static RENDERER: std::cell::RefCell<Option<Renderer>> = const { std::cell::RefCell::new(None) };
}

/// Set the renderer
pub fn set_renderer(renderer: Renderer) {
    RENDERER.with(|r| {
        *r.borrow_mut() = Some(renderer);
    });
}

/// Access the renderer
pub fn with_renderer<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut Renderer) -> R,
{
    RENDERER.with(|r| r.borrow_mut().as_mut().map(f))
}