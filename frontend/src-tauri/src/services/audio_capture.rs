//! Audio Capture Service
//! 
//! Abstracts audio capture for the interview assistant.
//! Rust equivalent of services/audioCapture.js
//! 
//! Uses WASAPI for system audio capture on Windows.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Audio level information
#[derive(Debug, Clone, Copy, Default)]
pub struct AudioLevel {
    /// RMS level (0.0 - 1.0)
    pub rms: f32,
    /// Peak level (0.0 - 1.0)
    pub peak: f32,
    /// dB level (-60 to 0)
    pub db: f32,
}

impl AudioLevel {
    /// Calculate from PCM samples
    pub fn from_samples(samples: &[i16]) -> Self {
        if samples.is_empty() {
            return Self::default();
        }

        let mut sum_sq = 0.0f64;
        let mut peak = 0i16;

        for &s in samples {
            sum_sq += (s as f64) * (s as f64);
            peak = peak.max(s.abs());
        }

        let rms = ((sum_sq / samples.len() as f64).sqrt() / i16::MAX as f64) as f32;
        let peak_norm = peak as f32 / i16::MAX as f32;
        let db = if rms > 0.0 {
            20.0 * rms.log10()
        } else {
            -60.0
        };

        Self {
            rms,
            peak: peak_norm,
            db: db.max(-60.0),
        }
    }
}

/// Audio capture state
static IS_CAPTURING: AtomicBool = AtomicBool::new(false);
static SAMPLE_RATE: AtomicU32 = AtomicU32::new(48000);

/// Check if capturing
pub fn is_capturing() -> bool {
    IS_CAPTURING.load(Ordering::SeqCst)
}

/// Get current sample rate
pub fn sample_rate() -> u32 {
    SAMPLE_RATE.load(Ordering::SeqCst)
}

/// Audio Capture Service
pub struct AudioCaptureService {
    /// Channel to send audio data
    audio_tx: Option<mpsc::Sender<Vec<u8>>>,
    /// Callback for audio level updates
    on_audio_level: Option<Box<dyn Fn(AudioLevel) + Send + Sync>>,
    /// Whether to capture microphone (default: false, system only)
    use_microphone: bool,
}

impl AudioCaptureService {
    pub fn new() -> Self {
        Self {
            audio_tx: None,
            on_audio_level: None,
            use_microphone: false,
        }
    }

    /// Set audio output channel
    pub fn set_audio_channel(&mut self, tx: mpsc::Sender<Vec<u8>>) {
        self.audio_tx = Some(tx);
    }

    /// Set audio level callback
    pub fn set_level_callback<F>(&mut self, callback: F)
    where
        F: Fn(AudioLevel) + Send + Sync + 'static,
    {
        self.on_audio_level = Some(Box::new(callback));
    }

    /// Enable/disable microphone capture
    pub fn set_use_microphone(&mut self, use_mic: bool) {
        self.use_microphone = use_mic;
    }

    /// Start system audio capture (WASAPI loopback)
    #[cfg(windows)]
    pub fn start_system_capture(&self, audio_tx: mpsc::Sender<Vec<u8>>) -> Result<(), String> {
        if IS_CAPTURING.load(Ordering::SeqCst) {
            return Err("Already capturing".to_string());
        }

        IS_CAPTURING.store(true, Ordering::SeqCst);

        // Spawn WASAPI capture in blocking thread
        std::thread::spawn(move || {
            crate::log_info!("🎧 Starting WASAPI system audio capture...");
            
            let result = crate::audio::wasapi::capture_loopback_to_async(audio_tx);
            
            if let Err(e) = result {
                crate::log_error!("❌ WASAPI capture error: {}", e);
            }
            
            IS_CAPTURING.store(false, Ordering::SeqCst);
            crate::log_info!("🎧 WASAPI capture stopped");
        });

        Ok(())
    }

    #[cfg(not(windows))]
    pub fn start_system_capture(&self, _audio_tx: mpsc::Sender<Vec<u8>>) -> Result<(), String> {
        Err("System audio capture only supported on Windows".to_string())
    }

    /// Stop audio capture
    pub fn stop_capture(&self) {
        IS_CAPTURING.store(false, Ordering::SeqCst);
        crate::audio::IS_LIVE_STREAMING.store(false, Ordering::SeqCst);
    }
}

impl Default for AudioCaptureService {
    fn default() -> Self {
        Self::new()
    }
}

/// Resample audio from source rate to 16kHz
/// 
/// Simple linear interpolation resampler.
pub fn resample_to_16k(samples: &[f32], source_rate: u32) -> Vec<f32> {
    if samples.is_empty() || source_rate == 0 {
        return samples.to_vec();
    }

    if source_rate == 16000 {
        return samples.to_vec();
    }

    let target_rate = 16000u32;
    let ratio = source_rate as f64 / target_rate as f64;
    let new_length = ((samples.len() as f64) / ratio).round() as usize;

    if new_length == 0 {
        return Vec::new();
    }

    let mut result = Vec::with_capacity(new_length);

    for i in 0..new_length {
        let source_index = i as f64 * ratio;
        let index0 = source_index.floor() as usize;
        let index1 = (index0 + 1).min(samples.len() - 1);
        let frac = (source_index - index0 as f64) as f32;

        let interpolated = samples[index0] * (1.0 - frac) + samples[index1] * frac;
        result.push(interpolated);
    }

    result
}

/// Convert f32 samples to i16 PCM bytes (little-endian)
pub fn f32_to_pcm_bytes(samples: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(samples.len() * 2);
    
    for &sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let i16_sample = (clamped * i16::MAX as f32) as i16;
        bytes.extend_from_slice(&i16_sample.to_le_bytes());
    }
    
    bytes
}

/// Convert stereo to mono by averaging channels
pub fn stereo_to_mono(samples: &[f32]) -> Vec<f32> {
    if samples.len() < 2 {
        return samples.to_vec();
    }

    let mut mono = Vec::with_capacity(samples.len() / 2);
    
    for chunk in samples.chunks(2) {
        if chunk.len() == 2 {
            mono.push((chunk[0] + chunk[1]) / 2.0);
        } else {
            mono.push(chunk[0]);
        }
    }
    
    mono
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_level() {
        let samples: Vec<i16> = vec![0, 1000, -1000, 500, -500];
        let level = AudioLevel::from_samples(&samples);
        assert!(level.rms > 0.0);
        assert!(level.peak > 0.0);
        assert!(level.db < 0.0);
    }

    #[test]
    fn test_resample() {
        let samples: Vec<f32> = (0..480).map(|i| (i as f32 / 480.0).sin()).collect();
        let resampled = resample_to_16k(&samples, 48000);
        assert_eq!(resampled.len(), 160); // 480 / 3 = 160
    }

    #[test]
    fn test_stereo_to_mono() {
        let stereo = vec![0.5, 0.5, 1.0, 0.0, -0.5, 0.5];
        let mono = stereo_to_mono(&stereo);
        assert_eq!(mono.len(), 3);
        assert!((mono[0] - 0.5).abs() < 0.001);
        assert!((mono[1] - 0.5).abs() < 0.001);
        assert!((mono[2] - 0.0).abs() < 0.001);
    }
}
