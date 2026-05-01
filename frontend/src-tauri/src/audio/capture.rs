use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

// =============================================================================
// 🔊 AUDIO BUFFER CONFIGURATION
// =============================================================================
// Buffer limits to prevent unbounded memory growth.
// At 48kHz stereo: 48000 * 2 * 4 bytes = ~384KB/second
// Max 5 minutes = ~115MB - we cap at 60 seconds (~23MB) for safety
// =============================================================================

/// Maximum buffer size in samples (60 seconds at 48kHz stereo = ~5.76M samples)
const MAX_BUFFER_SAMPLES: usize = 48000 * 2 * 60; // 60 seconds max

/// Warning threshold at 80% capacity
const BUFFER_WARNING_THRESHOLD: usize = (MAX_BUFFER_SAMPLES as f64 * 0.8) as usize;

/// Track if we've warned about buffer approaching limit (prevent log spam)
static BUFFER_WARNING_LOGGED: AtomicUsize = AtomicUsize::new(0);

// Buffer to store recorded audio (f32 samples) with size limit
lazy_static::lazy_static! {
    pub static ref AUDIO_BUFFER: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::with_capacity(MAX_BUFFER_SAMPLES / 4)));
}

pub fn start_recording() {
    crate::audio::IS_RECORDING.store(true, std::sync::atomic::Ordering::SeqCst);
    
    // Reset buffer warning flag for new recording session
    BUFFER_WARNING_LOGGED.store(0, Ordering::Relaxed);
    
    // Clear buffer
    if let Ok(mut buffer) = AUDIO_BUFFER.lock() {
        buffer.clear();
    }

    thread::spawn(|| {
        let host = cpal::default_host();
        
        // Find default output device for loopback
        let device = match host.default_output_device() {
            Some(d) => d,
            None => {
                crate::log_error!("[Audio] No output device found");
                crate::audio::IS_RECORDING.store(false, std::sync::atomic::Ordering::SeqCst);
                return;
            }
        };

        crate::log_info!("[Audio] Using device: {}", device.name().unwrap_or("Unknown".into()));

        let config = match device.default_output_config() {
            Ok(c) => c,
            Err(e) => {
                crate::log_error!("[Audio] Failed to get config: {}", e);
                crate::audio::IS_RECORDING.store(false, std::sync::atomic::Ordering::SeqCst);
                return;
            }
        };

        crate::log_info!("[Audio] Stream config: {:?}", config);

        let err_fn = |err| crate::log_error!("[Audio] Stream error: {}", err);
        
        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => device.build_input_stream(
                &config.into(),
                move |data: &[f32], _: &_| write_input_data(data),
                err_fn,
                None
            ),
            cpal::SampleFormat::I16 => device.build_input_stream(
                &config.into(),
                move |data: &[i16], _: &_| write_input_data_i16(data),
                err_fn,
                None
            ),
            cpal::SampleFormat::U16 => device.build_input_stream(
                &config.into(),
                move |data: &[u16], _: &_| write_input_data_u16(data),
                err_fn,
                None
            ),
            _ => return, // Unsupported
        };

        if let Ok(stream) = stream {
            if let Err(e) = stream.play() {
                crate::log_error!("[Audio] Failed to play stream: {}", e);
            }

            // Keep thread alive while recording
            while crate::audio::IS_RECORDING.load(std::sync::atomic::Ordering::SeqCst) {
                thread::sleep(std::time::Duration::from_millis(100));
            }
            
            // Stream drops when loop ends
        } else {
             crate::log_error!("[Audio] Failed to build stream");
             crate::audio::IS_RECORDING.store(false, std::sync::atomic::Ordering::SeqCst);
        }
    });
}

pub fn stop_recording() -> Option<Vec<u8>> {
    crate::audio::IS_RECORDING.store(false, std::sync::atomic::Ordering::SeqCst);
    
    // Removed: unnecessary 200ms sleep that was adding latency

    // Convert f32 buffer to WAV bytes
    if let Ok(buffer) = AUDIO_BUFFER.lock() {
        if buffer.is_empty() {
             crate::log_error!("[Audio] Buffer is empty on stop_recording!");
             return None;
        }
        
        // Create WAV in memory - OPTIMIZED for lower latency
        // Mono (1 channel) + 16kHz sample rate = ~85% smaller file
        let spec = hound::WavSpec {
            channels: 1, // Mono - speech doesn't need stereo, halves size
            sample_rate: 16000, // 16kHz is enough for speech recognition
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let mut wav_bytes = Vec::new();
        let cursor = std::io::Cursor::new(&mut wav_bytes);
        
        if let Ok(mut writer) = hound::WavWriter::new(cursor, spec) {
            // Downsample: take every 3rd sample (48kHz -> 16kHz) and mix stereo to mono
            let mut i = 0;
            while i + 1 < buffer.len() {
                // Mix stereo to mono: (left + right) / 2
                let mono_sample = (buffer[i] + buffer[i + 1]) / 2.0;
                let amplitude = i16::MAX as f32;
                let s = (mono_sample * amplitude) as i16;
                writer.write_sample(s).ok();
                i += 6; // Skip 6 samples = 3 stereo pairs (downsample 48k->16k)
            }
            writer.finalize().ok();
        }
        
        return Some(wav_bytes);
    }
    None
}

fn write_input_data(input: &[f32]) {
    // Use try_lock to avoid blocking audio callback - drop samples if locked
    if let Ok(mut buffer) = AUDIO_BUFFER.try_lock() {
        // Enforce buffer size limit to prevent unbounded memory growth
        if buffer.len() + input.len() > MAX_BUFFER_SAMPLES {
            // Buffer full - use ring buffer behavior: drop oldest samples
            let buf_len = buffer.len();
            let overflow = buffer.len() + input.len() - MAX_BUFFER_SAMPLES;
            buffer.drain(0..overflow.min(buf_len));
            
            // Log warning once per recording session
            if BUFFER_WARNING_LOGGED.load(Ordering::Relaxed) == 0 {
                BUFFER_WARNING_LOGGED.store(1, Ordering::Relaxed);
                crate::log_error!("[Audio] Buffer limit reached! Dropping old samples. Consider stopping recording.");
            }
        }
        
        // Warn when approaching limit (only once)
        if buffer.len() > BUFFER_WARNING_THRESHOLD && BUFFER_WARNING_LOGGED.load(Ordering::Relaxed) == 0 {
            crate::log_info!("[Audio] Buffer at 80% capacity. Recording may be truncated soon.");
        }
        
        buffer.extend_from_slice(input);
    }
    // Silently drop samples if lock is contended - prevents audio thread hang
}

fn write_input_data_i16(input: &[i16]) {
    if let Ok(mut buffer) = AUDIO_BUFFER.try_lock() {
        // Check buffer limit
        let buf_len = buffer.len();
        if buf_len + input.len() > MAX_BUFFER_SAMPLES {
            let overflow = buf_len + input.len() - MAX_BUFFER_SAMPLES;
            buffer.drain(0..overflow.min(buf_len));
        }
        
        let remaining = MAX_BUFFER_SAMPLES.saturating_sub(buffer.len());
        buffer.reserve(input.len().min(remaining));
        for &sample in input {
            if buffer.len() >= MAX_BUFFER_SAMPLES { break; }
            buffer.push(sample as f32 / i16::MAX as f32);
        }
    }
}

fn write_input_data_u16(input: &[u16]) {
    if let Ok(mut buffer) = AUDIO_BUFFER.try_lock() {
        // Check buffer limit
        let buf_len = buffer.len();
        if buf_len + input.len() > MAX_BUFFER_SAMPLES {
            let overflow = buf_len + input.len() - MAX_BUFFER_SAMPLES;
            buffer.drain(0..overflow.min(buf_len));
        }
        
        let remaining = MAX_BUFFER_SAMPLES.saturating_sub(buffer.len());
        buffer.reserve(input.len().min(remaining));
        for &sample in input {
            if buffer.len() >= MAX_BUFFER_SAMPLES { break; }
            buffer.push((sample as f32 - u16::MAX as f32 / 2.0) / (u16::MAX as f32 / 2.0));
        }
    }
}
