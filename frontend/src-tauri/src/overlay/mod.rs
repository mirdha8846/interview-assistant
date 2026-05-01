pub mod stealth;

// Re-export stealth overlay functions for compatibility
pub use stealth::{
    init,
    set_overlay_text,
    move_overlay, 
    resize_overlay,
    scroll_overlay,
    set_app_handle
};

// Wrapper function for toggle_overlay
pub fn toggle_overlay() {
    stealth::toggle_visibility();
}

pub fn clear_overlay_text() {
    stealth::set_overlay_text(String::from("Anti-Proctor Active"));
}
