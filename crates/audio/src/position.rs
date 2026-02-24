//! Positional audio stage for mono-to-stereo conversion.
//!
//! This module handles the conversion point where a mono signal is split
//! into stereo based on spatial parameters. It sits between the pre-positional
//! (mono) pedal chain and the post-positional (per-speaker) pedal chains.
//!
//! ## Signal Path
//!
//! ```text
//! Mono In --> Pan --> 3D Position --> (Left Out, Right Out)
//! ```
//!
//! ## Pan
//!
//! Standard constant-power pan law. Position ranges from -1.0 (hard left)
//! through 0.0 (center) to 1.0 (hard right).
//!
//! ## 3D Position
//!
//! Inspired by Soundtrap's 3D positional effect. Maps spherical coordinates
//! to stereo output using amplitude panning, distance attenuation, and
//! spectral filtering.
//!
//! - **Azimuth**: 360-degree horizontal angle (0 = front, 90 = left,
//!   180 = rear, 270 = right).
//! - **Elevation**: Vertical angle (-90 = below, 0 = ear level, 90 = above).
//! - **Distance**: Relative distance from the listener (0.0 = near, 1.0 = far).

use crate::dsp::biquad::{Biquad, BiquadType};

/// Output from the positional stage: a stereo sample pair.
#[derive(Debug, Clone, Copy, Default)]
pub struct StereoSample {
    pub left: f64,
    pub right: f64,
}

/// Positional audio processor.
///
/// Converts a mono signal to stereo based on pan position and optional
/// 3D spatial parameters.
#[derive(Debug, Clone)]
pub struct PositionalStage {
    /// Whether the positional stage is active. When inactive, the mono
    /// signal is passed to both channels using the legacy volume_left
    /// and volume_right values.
    pub active: bool,

    /// Pan position: -1.0 (hard left) to 1.0 (hard right). 0.0 = center.
    /// Uses constant-power pan law: left = cos(theta), right = sin(theta).
    pub pan: f64,

    /// Whether 3D positioning is enabled (overrides pan when active).
    pub position_3d_enabled: bool,

    /// Horizontal angle in degrees. 0 = front, 90 = left, 180 = rear,
    /// 270 = right. Wraps at 360.
    pub azimuth_deg: f64,

    /// Vertical angle in degrees. -90 = below, 0 = ear level, 90 = above.
    pub elevation_deg: f64,

    /// Distance from listener: 0.0 = closest, 1.0 = farthest.
    /// Affects volume attenuation and high-frequency rolloff.
    pub distance: f64,

    /// Distance attenuation lowpass filter for the left channel.
    distance_lpf_left: Biquad,

    /// Distance attenuation lowpass filter for the right channel.
    distance_lpf_right: Biquad,

    /// Cached sample rate for filter coefficient updates.
    cached_sample_rate: f64,
}

impl PositionalStage {
    /// Create a new positional stage (inactive by default, centered pan).
    pub fn new(sample_rate: f64) -> Self {
        Self {
            active: false,
            pan: 0.0,
            position_3d_enabled: false,
            azimuth_deg: 0.0,
            elevation_deg: 0.0,
            distance: 0.0,
            distance_lpf_left: Biquad::new(BiquadType::Lowpass, 20000.0, 0.707, sample_rate),
            distance_lpf_right: Biquad::new(BiquadType::Lowpass, 20000.0, 0.707, sample_rate),
            cached_sample_rate: sample_rate,
        }
    }

    /// Process a mono buffer into separate left and right output buffers.
    ///
    /// When inactive, the mono signal is copied to both output buffers unchanged.
    pub fn process(
        &mut self,
        mono_in: &[f64],
        left_out: &mut [f64],
        right_out: &mut [f64],
        sample_rate: f64,
    ) {
        let len = mono_in.len().min(left_out.len()).min(right_out.len());

        if !self.active {
            // Passthrough: copy mono to both channels.
            left_out[..len].copy_from_slice(&mono_in[..len]);
            right_out[..len].copy_from_slice(&mono_in[..len]);
            return;
        }

        if self.position_3d_enabled {
            self.process_3d(mono_in, left_out, right_out, len, sample_rate);
        } else {
            self.process_pan(mono_in, left_out, right_out, len);
        }
    }

    /// Process using simple pan law.
    fn process_pan(
        &self,
        mono_in: &[f64],
        left_out: &mut [f64],
        right_out: &mut [f64],
        len: usize,
    ) {
        // Constant-power pan law: map pan [-1, 1] to angle [0, PI/2].
        let clamped = self.pan.clamp(-1.0, 1.0);
        let theta = core::f64::consts::FRAC_PI_4 * (clamped + 1.0);
        let gain_l = libm::cos(theta);
        let gain_r = libm::sin(theta);

        for i in 0..len {
            left_out[i] = mono_in[i] * gain_l;
            right_out[i] = mono_in[i] * gain_r;
        }
    }

    /// Process using 3D positional parameters.
    fn process_3d(
        &mut self,
        mono_in: &[f64],
        left_out: &mut [f64],
        right_out: &mut [f64],
        len: usize,
        sample_rate: f64,
    ) {
        // Normalize azimuth to [0, 360).
        let mut az = self.azimuth_deg % 360.0;
        if az < 0.0 {
            az += 360.0;
        }
        let az_rad = az * core::f64::consts::PI / 180.0;

        // Horizontal panning from azimuth.
        // 0 deg = front (center), 90 = left, 180 = rear (center), 270 = right.
        // sin(0)=0 gives equal L/R at front, sin(PI/2)=1 gives full left at 90 deg,
        // sin(3PI/2)=-1 gives full right at 270 deg.
        let gain_l = 0.5 + 0.5 * libm::sin(az_rad);
        let gain_r = 0.5 - 0.5 * libm::sin(az_rad);

        // Rear attenuation: sources behind the listener lose high frequencies.
        let rear_factor = libm::cos(az_rad); // 1.0 = front, -1.0 = rear
        let rear_lpf_freq = if rear_factor < 0.0 {
            // Behind the listener: progressively cut highs.
            2000.0 + (1.0 + rear_factor) * 9000.0
        } else {
            20000.0 // Front: wide open.
        };

        // Distance attenuation: inverse distance scaling.
        let dist = self.distance.clamp(0.0, 1.0);
        let dist_gain = 1.0 / (1.0 + dist * 4.0); // 1.0 at 0, 0.2 at 1.0.

        // Distance also reduces high-frequency content.
        let dist_lpf_freq = 20000.0 - dist * 16000.0; // 20kHz near, 4kHz far.

        // Combined LPF frequency.
        let lpf_freq = rear_lpf_freq.min(dist_lpf_freq).max(200.0);

        // Elevation: apply a subtle spectral tilt.
        // Above: slight high boost. Below: slight high cut.
        let elev = self.elevation_deg.clamp(-90.0, 90.0);
        let elev_factor = 1.0 + elev / 90.0 * 0.15; // 0.85 to 1.15.

        // Update distance lowpass filters.
        if (sample_rate - self.cached_sample_rate).abs() > 1.0 || self.active {
            self.distance_lpf_left
                .set_coefficients(BiquadType::Lowpass, lpf_freq, 0.707, sample_rate);
            self.distance_lpf_right
                .set_coefficients(BiquadType::Lowpass, lpf_freq, 0.707, sample_rate);
            self.cached_sample_rate = sample_rate;
        }

        for i in 0..len {
            let sample = mono_in[i] * dist_gain * elev_factor;
            left_out[i] = self.distance_lpf_left.tick(sample * gain_l);
            right_out[i] = self.distance_lpf_right.tick(sample * gain_r);
        }
    }

    /// Reset all internal state.
    pub fn reset(&mut self) {
        self.distance_lpf_left.reset();
        self.distance_lpf_right.reset();
    }
}

impl Default for PositionalStage {
    fn default() -> Self {
        Self::new(48000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inactive_copies_mono_to_both() {
        let mut pos = PositionalStage::new(48000.0);
        pos.active = false;
        let mono = [0.5_f64; 16];
        let mut left = [0.0_f64; 16];
        let mut right = [0.0_f64; 16];
        pos.process(&mono, &mut left, &mut right, 48000.0);
        for i in 0..16 {
            assert!((left[i] - 0.5).abs() < f64::EPSILON);
            assert!((right[i] - 0.5).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn pan_center_equal_power() {
        let mut pos = PositionalStage::new(48000.0);
        pos.active = true;
        pos.pan = 0.0;
        let mono = [1.0_f64; 16];
        let mut left = [0.0_f64; 16];
        let mut right = [0.0_f64; 16];
        pos.process(&mono, &mut left, &mut right, 48000.0);
        // At center, both channels should be approximately equal.
        for i in 0..16 {
            assert!((left[i] - right[i]).abs() < 0.01);
            // Constant power: each channel is cos(PI/4) = ~0.707.
            assert!((left[i] - 0.707).abs() < 0.01);
        }
    }

    #[test]
    fn pan_hard_left() {
        let mut pos = PositionalStage::new(48000.0);
        pos.active = true;
        pos.pan = -1.0;
        let mono = [1.0_f64; 16];
        let mut left = [0.0_f64; 16];
        let mut right = [0.0_f64; 16];
        pos.process(&mono, &mut left, &mut right, 48000.0);
        for i in 0..16 {
            assert!(left[i] > 0.95, "Left should be near 1.0");
            assert!(right[i] < 0.05, "Right should be near 0.0");
        }
    }

    #[test]
    fn pan_hard_right() {
        let mut pos = PositionalStage::new(48000.0);
        pos.active = true;
        pos.pan = 1.0;
        let mono = [1.0_f64; 16];
        let mut left = [0.0_f64; 16];
        let mut right = [0.0_f64; 16];
        pos.process(&mono, &mut left, &mut right, 48000.0);
        for i in 0..16 {
            assert!(left[i] < 0.05, "Left should be near 0.0");
            assert!(right[i] > 0.95, "Right should be near 1.0");
        }
    }

    #[test]
    fn position_3d_front_center() {
        let mut pos = PositionalStage::new(48000.0);
        pos.active = true;
        pos.position_3d_enabled = true;
        pos.azimuth_deg = 0.0;
        pos.distance = 0.0;
        pos.elevation_deg = 0.0;
        let mono = [0.5_f64; 64];
        let mut left = [0.0_f64; 64];
        let mut right = [0.0_f64; 64];
        pos.process(&mono, &mut left, &mut right, 48000.0);
        // Front center at zero distance: both channels should be approximately equal.
        for i in 32..64 {
            assert!(
                (left[i] - right[i]).abs() < 0.05,
                "Front center should be balanced"
            );
        }
    }

    #[test]
    fn position_3d_distance_attenuates() {
        let mut pos_near = PositionalStage::new(48000.0);
        pos_near.active = true;
        pos_near.position_3d_enabled = true;
        pos_near.azimuth_deg = 0.0;
        pos_near.distance = 0.0;

        let mut pos_far = PositionalStage::new(48000.0);
        pos_far.active = true;
        pos_far.position_3d_enabled = true;
        pos_far.azimuth_deg = 0.0;
        pos_far.distance = 1.0;

        let mono = [0.5_f64; 64];
        let mut near_l = [0.0_f64; 64];
        let mut near_r = [0.0_f64; 64];
        let mut far_l = [0.0_f64; 64];
        let mut far_r = [0.0_f64; 64];

        pos_near.process(&mono, &mut near_l, &mut near_r, 48000.0);
        pos_far.process(&mono, &mut far_l, &mut far_r, 48000.0);

        let near_energy: f64 = near_l[32..].iter().map(|s| s * s).sum();
        let far_energy: f64 = far_l[32..].iter().map(|s| s * s).sum();
        assert!(
            near_energy > far_energy * 2.0,
            "Near should be louder than far"
        );
    }

    #[test]
    fn position_3d_left_azimuth() {
        let mut pos = PositionalStage::new(48000.0);
        pos.active = true;
        pos.position_3d_enabled = true;
        pos.azimuth_deg = 90.0; // Pure left.
        pos.distance = 0.0;
        let mono = [0.5_f64; 64];
        let mut left = [0.0_f64; 64];
        let mut right = [0.0_f64; 64];
        pos.process(&mono, &mut left, &mut right, 48000.0);
        let left_energy: f64 = left[32..].iter().map(|s| s * s).sum();
        let right_energy: f64 = right[32..].iter().map(|s| s * s).sum();
        assert!(
            left_energy > right_energy * 2.0,
            "90 deg azimuth should favor left"
        );
    }

    #[test]
    fn reset_clears_state() {
        let mut pos = PositionalStage::new(48000.0);
        pos.active = true;
        pos.position_3d_enabled = true;
        pos.distance = 0.5;
        let mono = [0.5_f64; 64];
        let mut left = [0.0_f64; 64];
        let mut right = [0.0_f64; 64];
        pos.process(&mono, &mut left, &mut right, 48000.0);
        pos.reset();
        // After reset, filters should be cleared.
        // Process again and verify no artifacts.
        let mono2 = [0.0_f64; 16];
        let mut left2 = [1.0_f64; 16];
        let mut right2 = [1.0_f64; 16];
        pos.process(&mono2, &mut left2, &mut right2, 48000.0);
        // With zero input after reset, output should be near zero (small filter transient).
        for i in 0..16 {
            assert!(left2[i].abs() < 0.1);
            assert!(right2[i].abs() < 0.1);
        }
    }
}
