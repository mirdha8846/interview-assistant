use std::sync::mpsc::Sender;
use windows::{
    core::*,
    Win32::Media::Audio::{IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator, IAudioClient, IAudioCaptureClient, eRender, eConsole, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK, AUDCLNT_BUFFERFLAGS_SILENT},
    Win32::System::Com::{CoInitializeEx, CoCreateInstance, COINIT_MULTITHREADED, CLSCTX_ALL},
};

/// Capture loopback audio using sync channel (original function)
pub fn capture_loopback(tx: Sender<Vec<u8>>) -> Result<()> {
    capture_loopback_internal(|data| tx.send(data).is_ok())
}

/// Capture loopback audio directly to a tokio async channel
/// This is the preferred method for live streaming - avoids the bridge thread overhead
pub fn capture_loopback_to_async(tx: tokio::sync::mpsc::Sender<Vec<u8>>) -> Result<()> {
    capture_loopback_internal(|data| tx.blocking_send(data).is_ok())
}

/// Internal implementation that accepts a generic sender function
fn capture_loopback_internal<F>(mut send_fn: F) -> Result<()> 
where F: FnMut(Vec<u8>) -> bool
{
    unsafe {
        // Initialize COM
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

        // Get audio device enumerator
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;

        // Get default audio output device (we'll capture from it via loopback)
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
        
        crate::log_info!("🎧 WASAPI Loopback capturing from default output device");

        // Activate audio client
        let audio_client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;

        // Get default format
        let format_ptr = audio_client.GetMixFormat()?;
        let format = &*format_ptr;
        
        // Copy fields to avoid packed struct issues
        let sample_rate = format.nSamplesPerSec;
        let channels = format.nChannels;
        let bits_per_sample = format.wBitsPerSample;
        
        crate::log_info!("📊 Audio Format: {} Hz, {} channels, {} bits", 
            sample_rate, channels, bits_per_sample);

        // Initialize audio client in LOOPBACK mode
        audio_client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_LOOPBACK,
            10_000_000, // 1 second buffer
            0,
            format_ptr,
            None,
        )?;

        // Get capture client
        let capture_client: IAudioCaptureClient = audio_client.GetService()?;

        // Start capturing
        audio_client.Start()?;
        crate::log_info!("🔴 Recording started... Play audio in any app!\n");
        
        while crate::audio::IS_LIVE_STREAMING.load(std::sync::atomic::Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(20));

            let packet_length_result = capture_client.GetNextPacketSize();
            let mut packet_length = match packet_length_result {
                Ok(len) => len,
                Err(_) => {
                    // Audio device might have been reset, try to continue
                    continue;
                }
            };
            
            while packet_length > 0 {
                let mut data_ptr: *mut u8 = std::ptr::null_mut();
                let mut num_frames = 0u32;
                let mut flags = 0u32;

                if capture_client.GetBuffer(
                    &mut data_ptr,
                    &mut num_frames,
                    &mut flags,
                    None,
                    None,
                ).is_err() {
                    break; // Exit inner loop on error
                }

                if num_frames > 0 {
                    let is_silent = (flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32) != 0;
                    
                    let pcm_data = if is_silent {
                        // Generate silence (zeros) for the duration of the gap
                        // Target rate is 16000Hz. 
                        // We need to calculate how many frames 16000Hz corresponds to `num_frames` at `sample_rate`
                        let ratio = sample_rate as f64 / 16000.0;
                        let output_frames = (num_frames as f64 / ratio) as usize;
                        vec![0u8; output_frames * 2] // 16-bit = 2 bytes per sample
                    } else if !data_ptr.is_null() {
                        let bytes_per_frame = (channels * bits_per_sample / 8) as usize;
                        let data_size = num_frames as usize * bytes_per_frame;
                        let audio_data = std::slice::from_raw_parts(data_ptr, data_size);

                        convert_to_pcm16_mono_16khz(
                            audio_data,
                            bits_per_sample,
                            channels,
                            sample_rate,
                        )
                    } else {
                        Vec::new()
                    };
                    
                    if !pcm_data.is_empty() {
                        if !send_fn(pcm_data) {
                            // Channel closed (receiver dropped) - exit completely
                            let _ = capture_client.ReleaseBuffer(num_frames);
                            crate::log_info!("🔌 Audio channel closed, exiting WASAPI capture");
                            // Stop and cleanup before returning
                            let _ = audio_client.Stop();
                            return Ok(()); // ← EXIT ENTIRE FUNCTION, not just inner loop!
                        }
                    }
                }

                let _ = capture_client.ReleaseBuffer(num_frames);
                packet_length = capture_client.GetNextPacketSize().unwrap_or(0);
            }
        }
        
        // Stop and cleanup
        let _ = audio_client.Stop();
        crate::log_info!("🛑 WASAPI capture stopped");
    }
    Ok(())
}

// Convert audio to 16-bit PCM mono at 16kHz (required by Gemini)
fn convert_to_pcm16_mono_16khz(data: &[u8], bits: u16, channels: u16, src_rate: u32) -> Vec<u8> {
    // First convert to mono f32 samples
    let mono_samples: Vec<f32> = match bits {
        32 => {
            let samples: Vec<f32> = data
                .chunks_exact(4)
                .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .collect();
            samples.chunks(channels as usize)
                .map(|frame| frame.iter().sum::<f32>() / channels as f32)
                .collect()
        }
        16 => {
            let samples: Vec<i16> = data
                .chunks_exact(2)
                .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
                .collect();
            samples.chunks(channels as usize)
                .map(|frame| {
                    let mono: i32 = frame.iter().map(|&s| s as i32).sum::<i32>() / channels as i32;
                    mono as f32 / i16::MAX as f32
                })
                .collect()
        }
        _ => return Vec::new(),
    };

    // Resample from src_rate to 16000 Hz
    let target_rate = 16000u32;
    let ratio = src_rate as f64 / target_rate as f64;
    let output_len = (mono_samples.len() as f64 / ratio) as usize;
    
    let mut output = Vec::with_capacity(output_len * 2);
    for i in 0..output_len {
        let src_idx = (i as f64 * ratio) as usize;
        if src_idx < mono_samples.len() {
            let sample = mono_samples[src_idx];
            let sample_16 = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            output.extend_from_slice(&sample_16.to_le_bytes());
        }
    }
    
    output
}
