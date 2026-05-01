pub mod capture;
pub use capture::*;
pub mod wasapi;

use std::sync::atomic::{AtomicBool, Ordering};

pub static IS_RECORDING: AtomicBool = AtomicBool::new(false);
pub static IS_LIVE_STREAMING: AtomicBool = AtomicBool::new(false);

pub fn is_live_streaming() -> bool {
    IS_LIVE_STREAMING.load(Ordering::SeqCst)
}

pub fn is_recording() -> bool {
    IS_RECORDING.load(Ordering::SeqCst)
}
