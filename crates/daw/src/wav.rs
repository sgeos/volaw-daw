//! Minimal WAV file loader for PCM wavetable data.
//!
//! Supports uncompressed PCM (format tag 1) with 8, 16, 24, or 32-bit
//! integer samples, and IEEE float (format tag 3) with 32-bit samples.
//! Stereo files are mixed down to mono by averaging channels.

use std::fs;
use std::path::Path;

/// RIFF/WAV format tags.
const FORMAT_PCM: u16 = 1;
const FORMAT_IEEE_FLOAT: u16 = 3;

/// Load a WAV file and return its samples as normalized f64 in [-1.0, 1.0].
///
/// Returns an error string if the file cannot be read or the format is
/// unsupported.
pub fn load_wav(path: &Path) -> Result<Vec<f64>, String> {
    let data = fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    if data.len() < 44 {
        return Err("file too small for WAV header".into());
    }

    // Validate RIFF header.
    if &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return Err("not a valid WAV file".into());
    }

    // Find fmt and data chunks.
    let mut fmt_chunk: Option<&[u8]> = None;
    let mut data_chunk: Option<&[u8]> = None;
    let mut pos = 12;
    while pos + 8 <= data.len() {
        let chunk_id = &data[pos..pos + 4];
        let chunk_size =
            u32::from_le_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]])
                as usize;
        let chunk_end = (pos + 8 + chunk_size).min(data.len());

        if chunk_id == b"fmt " {
            fmt_chunk = Some(&data[pos + 8..chunk_end]);
        } else if chunk_id == b"data" {
            data_chunk = Some(&data[pos + 8..chunk_end]);
        }
        // Chunks are word-aligned.
        pos = pos + 8 + ((chunk_size + 1) & !1);
    }

    let fmt = fmt_chunk.ok_or("missing fmt chunk")?;
    let raw = data_chunk.ok_or("missing data chunk")?;

    if fmt.len() < 16 {
        return Err("fmt chunk too small".into());
    }

    let format_tag = u16::from_le_bytes([fmt[0], fmt[1]]);
    let channels = u16::from_le_bytes([fmt[2], fmt[3]]) as usize;
    let bits_per_sample = u16::from_le_bytes([fmt[14], fmt[15]]) as usize;

    if channels == 0 {
        return Err("zero channels".into());
    }

    let bytes_per_sample = bits_per_sample / 8;
    if bytes_per_sample == 0 {
        return Err(format!("unsupported bits per sample: {bits_per_sample}"));
    }

    let frame_size = bytes_per_sample * channels;
    if frame_size == 0 {
        return Err("zero frame size".into());
    }

    let num_frames = raw.len() / frame_size;
    let mut samples = Vec::with_capacity(num_frames);

    for frame in 0..num_frames {
        let frame_offset = frame * frame_size;
        let mut mono_sum = 0.0_f64;

        for ch in 0..channels {
            let offset = frame_offset + ch * bytes_per_sample;
            let sample = match (format_tag, bytes_per_sample) {
                (FORMAT_PCM, 1) => {
                    // 8-bit unsigned: 0..255, center at 128.
                    let val = data_at_u8(raw, offset);
                    (val as f64 - 128.0) / 128.0
                }
                (FORMAT_PCM, 2) => {
                    // 16-bit signed.
                    let val = data_at_i16(raw, offset);
                    val as f64 / 32768.0
                }
                (FORMAT_PCM, 3) => {
                    // 24-bit signed.
                    let val = data_at_i24(raw, offset);
                    val as f64 / 8_388_608.0
                }
                (FORMAT_PCM, 4) => {
                    // 32-bit signed.
                    let val = data_at_i32(raw, offset);
                    val as f64 / 2_147_483_648.0
                }
                (FORMAT_IEEE_FLOAT, 4) => {
                    // 32-bit IEEE float.
                    let val = data_at_f32(raw, offset);
                    val as f64
                }
                _ => {
                    return Err(format!(
                        "unsupported format: tag={format_tag}, bits={bits_per_sample}"
                    ));
                }
            };
            mono_sum += sample;
        }

        samples.push(mono_sum / channels as f64);
    }

    Ok(samples)
}

fn data_at_u8(data: &[u8], offset: usize) -> u8 {
    data.get(offset).copied().unwrap_or(0)
}

fn data_at_i16(data: &[u8], offset: usize) -> i16 {
    if offset + 2 > data.len() {
        return 0;
    }
    i16::from_le_bytes([data[offset], data[offset + 1]])
}

fn data_at_i24(data: &[u8], offset: usize) -> i32 {
    if offset + 3 > data.len() {
        return 0;
    }
    // Sign-extend 24-bit to 32-bit.
    let lo = data[offset] as i32;
    let mid = data[offset + 1] as i32;
    let hi = data[offset + 2] as i32;
    let val = lo | (mid << 8) | (hi << 16);
    // Sign extend from bit 23.
    if val & 0x80_0000 != 0 {
        val | (0xFF << 24)
    } else {
        val
    }
}

fn data_at_i32(data: &[u8], offset: usize) -> i32 {
    if offset + 4 > data.len() {
        return 0;
    }
    i32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

fn data_at_f32(data: &[u8], offset: usize) -> f32 {
    if offset + 4 > data.len() {
        return 0.0;
    }
    f32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal valid WAV file in memory.
    fn build_wav(format_tag: u16, channels: u16, bits: u16, sample_data: &[u8]) -> Vec<u8> {
        let fmt_size: u32 = 16;
        let data_size = sample_data.len() as u32;
        let riff_size = 4 + (8 + fmt_size) + (8 + data_size);
        let sample_rate: u32 = 44100;
        let byte_rate = sample_rate * channels as u32 * (bits as u32 / 8);
        let block_align = channels * (bits / 8);

        let mut buf = Vec::new();
        buf.extend_from_slice(b"RIFF");
        buf.extend_from_slice(&riff_size.to_le_bytes());
        buf.extend_from_slice(b"WAVE");
        // fmt chunk.
        buf.extend_from_slice(b"fmt ");
        buf.extend_from_slice(&fmt_size.to_le_bytes());
        buf.extend_from_slice(&format_tag.to_le_bytes());
        buf.extend_from_slice(&channels.to_le_bytes());
        buf.extend_from_slice(&sample_rate.to_le_bytes());
        buf.extend_from_slice(&byte_rate.to_le_bytes());
        buf.extend_from_slice(&block_align.to_le_bytes());
        buf.extend_from_slice(&bits.to_le_bytes());
        // data chunk.
        buf.extend_from_slice(b"data");
        buf.extend_from_slice(&data_size.to_le_bytes());
        buf.extend_from_slice(sample_data);
        buf
    }

    #[test]
    fn load_16bit_mono() {
        // Two samples: 0 and max positive (32767).
        let sample_data: Vec<u8> = [0i16, 32767i16]
            .iter()
            .flat_map(|s| s.to_le_bytes())
            .collect();
        let wav = build_wav(FORMAT_PCM, 1, 16, &sample_data);
        let tmp = std::env::temp_dir().join("test_16bit_mono.wav");
        fs::write(&tmp, &wav).unwrap();
        let samples = load_wav(&tmp).unwrap();
        assert_eq!(samples.len(), 2);
        assert!((samples[0] - 0.0).abs() < 0.001);
        assert!((samples[1] - (32767.0 / 32768.0)).abs() < 0.001);
        let _ = fs::remove_file(&tmp);
    }

    #[test]
    fn load_16bit_stereo_mixes_to_mono() {
        // One frame: left = -32768, right = 32767. Should average to near 0.
        let mut sample_data = Vec::new();
        sample_data.extend_from_slice(&(-32768i16).to_le_bytes());
        sample_data.extend_from_slice(&32767i16.to_le_bytes());
        let wav = build_wav(FORMAT_PCM, 2, 16, &sample_data);
        let tmp = std::env::temp_dir().join("test_16bit_stereo.wav");
        fs::write(&tmp, &wav).unwrap();
        let samples = load_wav(&tmp).unwrap();
        assert_eq!(samples.len(), 1);
        assert!(samples[0].abs() < 0.001);
        let _ = fs::remove_file(&tmp);
    }

    #[test]
    fn load_8bit_unsigned() {
        // Samples: 0 (min), 128 (center), 255 (max).
        let sample_data = vec![0u8, 128, 255];
        let wav = build_wav(FORMAT_PCM, 1, 8, &sample_data);
        let tmp = std::env::temp_dir().join("test_8bit.wav");
        fs::write(&tmp, &wav).unwrap();
        let samples = load_wav(&tmp).unwrap();
        assert_eq!(samples.len(), 3);
        assert!((samples[0] - (-1.0)).abs() < 0.01);
        assert!((samples[1] - 0.0).abs() < 0.01);
        assert!((samples[2] - (127.0 / 128.0)).abs() < 0.01);
        let _ = fs::remove_file(&tmp);
    }

    #[test]
    fn reject_non_wav() {
        let tmp = std::env::temp_dir().join("test_not_wav.wav");
        fs::write(&tmp, b"not a wav file at all!!!!!!!!!!!!!!!!!!!!!!!!!!!").unwrap();
        assert!(load_wav(&tmp).is_err());
        let _ = fs::remove_file(&tmp);
    }
}
