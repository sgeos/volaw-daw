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

    /// Elevation high-shelf filter for the left channel.
    /// Simulates pinna filtering: above boosts highs, below cuts highs.
    elevation_shelf_left: Biquad,

    /// Elevation high-shelf filter for the right channel.
    elevation_shelf_right: Biquad,

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
            elevation_shelf_left: Biquad::passthrough(),
            elevation_shelf_right: Biquad::passthrough(),
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

        // Elevation: spectral tilt via high-shelf filter to simulate pinna
        // filtering. Above (+90): high-shelf boost. Below (-90): high-shelf cut.
        // Range is +-8 dB at 3 kHz, clearly audible across the full elevation range.
        let elev = self.elevation_deg.clamp(-90.0, 90.0);
        let elev_gain_db = elev / 90.0 * 8.0; // -8 dB to +8 dB.

        // Update distance lowpass and elevation shelf filters.
        if (sample_rate - self.cached_sample_rate).abs() > 1.0 || self.active {
            self.distance_lpf_left
                .set_coefficients(BiquadType::Lowpass, lpf_freq, 0.707, sample_rate);
            self.distance_lpf_right
                .set_coefficients(BiquadType::Lowpass, lpf_freq, 0.707, sample_rate);
            if elev_gain_db.abs() > 0.1 {
                self.elevation_shelf_left.set_coefficients(
                    BiquadType::HighShelf {
                        gain_db: elev_gain_db,
                    },
                    3000.0,
                    0.707,
                    sample_rate,
                );
                self.elevation_shelf_right.set_coefficients(
                    BiquadType::HighShelf {
                        gain_db: elev_gain_db,
                    },
                    3000.0,
                    0.707,
                    sample_rate,
                );
            } else {
                // Near zero elevation: passthrough (no shelf).
                self.elevation_shelf_left =
                    Biquad::passthrough();
                self.elevation_shelf_right =
                    Biquad::passthrough();
            }
            self.cached_sample_rate = sample_rate;
        }

        for i in 0..len {
            let sample = mono_in[i] * dist_gain;
            let left_sample = self.distance_lpf_left.tick(sample * gain_l);
            let right_sample = self.distance_lpf_right.tick(sample * gain_r);
            left_out[i] = self.elevation_shelf_left.tick(left_sample);
            right_out[i] = self.elevation_shelf_right.tick(right_sample);
        }
    }

    /// Reset all internal state.
    pub fn reset(&mut self) {
        self.distance_lpf_left.reset();
        self.distance_lpf_right.reset();
        self.elevation_shelf_left.reset();
        self.elevation_shelf_right.reset();
    }
}

impl Default for PositionalStage {
    fn default() -> Self {
        Self::new(48000.0)
    }
}

/// Compute 3D positional coordinates from Keplerian orbital elements.
///
/// Given a phase (0.0 to 1.0 representing one full orbit) and five orbital
/// parameters, returns (azimuth_deg, elevation_deg, distance) suitable for
/// feeding directly into the `PositionalStage` 3D processing.
///
/// ## Coordinate System
///
/// - x = front (positive toward listener front)
/// - y = left (positive toward listener left)
/// - z = up (positive above listener)
///
/// This matches the existing 3D position convention where azimuth 0 = front,
/// 90 = left, 180 = rear, 270 = right.
///
/// ## Special Cases
///
/// A circular equatorial orbit (e=0, i=0) produces a constant-distance
/// horizontal sweep, equivalent to an autopanning "lighthouse" effect.
///
/// ## Parameters
///
/// - `phase`: Position along the orbit, 0.0 to 1.0 (wraps).
/// - `a`: Semi-major axis, 0.0 to 1.0 (maps to distance scale).
/// - `e`: Eccentricity, 0.0 to < 1.0 (0 = circular).
/// - `i_deg`: Inclination in degrees (0 = equatorial, 90 = polar).
/// - `big_omega_deg`: Longitude of ascending node in degrees.
/// - `small_omega_deg`: Argument of periapsis in degrees.
pub fn compute_orbital_position(
    phase: f64,
    a: f64,
    e: f64,
    i_deg: f64,
    big_omega_deg: f64,
    small_omega_deg: f64,
) -> (f64, f64, f64) {
    let two_pi = 2.0 * core::f64::consts::PI;
    let deg_to_rad = core::f64::consts::PI / 180.0;

    // Clamp parameters to valid ranges.
    let a = a.clamp(0.0, 1.0);
    let e = e.clamp(0.0, 0.99);
    let i_rad = i_deg.clamp(0.0, 180.0) * deg_to_rad;
    let big_omega = big_omega_deg * deg_to_rad;
    let small_omega = small_omega_deg * deg_to_rad;

    // Mean anomaly from phase.
    let m = (phase % 1.0) * two_pi;

    // Solve Kepler equation: E - e * sin(E) = M via Newton-Raphson.
    let mut ea = m; // Initial guess: E = M.
    for _ in 0..10 {
        let f = ea - e * libm::sin(ea) - m;
        let fp = 1.0 - e * libm::cos(ea);
        if fp.abs() > 1e-12 {
            ea -= f / fp;
        }
    }

    // True anomaly from eccentric anomaly.
    let half_e = ea * 0.5;
    let nu = 2.0
        * libm::atan2(
            libm::sqrt(1.0 + e) * libm::sin(half_e),
            libm::sqrt(1.0 - e) * libm::cos(half_e),
        );

    // Orbital radius.
    let denom = 1.0 + e * libm::cos(nu);
    let r = if denom.abs() > 1e-12 {
        a * (1.0 - e * e) / denom
    } else {
        a
    };

    // Perifocal to reference frame rotation.
    // Combined angle: argument of latitude = nu + small_omega.
    let u = nu + small_omega;
    let cos_u = libm::cos(u);
    let sin_u = libm::sin(u);
    let cos_o = libm::cos(big_omega);
    let sin_o = libm::sin(big_omega);
    let cos_i = libm::cos(i_rad);
    let sin_i = libm::sin(i_rad);

    let x = r * (cos_u * cos_o - sin_u * sin_o * cos_i);
    let y = r * (cos_u * sin_o + sin_u * cos_o * cos_i);
    let z = r * (sin_u * sin_i);

    // Convert to spherical coordinates.
    let dist = libm::sqrt(x * x + y * y + z * z);

    if dist < 1e-12 {
        return (0.0, 0.0, 0.0);
    }

    let mut azimuth_rad = libm::atan2(y, x);
    if azimuth_rad < 0.0 {
        azimuth_rad += two_pi;
    }
    let azimuth_deg = azimuth_rad / deg_to_rad;

    let elevation_rad = libm::asin((z / dist).clamp(-1.0, 1.0));
    let elevation_deg = elevation_rad / deg_to_rad;

    (azimuth_deg, elevation_deg, dist.clamp(0.0, 1.0))
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
    fn elevation_above_boosts_highs() {
        let sr = 48000.0;
        // Generate a broadband test signal (white-ish noise approximation
        // via a high-frequency sine to exercise the shelf filter).
        let mut mono = [0.0_f64; 2048];
        for (i, sample) in mono.iter_mut().enumerate() {
            // Mix of 200 Hz (low) and 8000 Hz (high).
            *sample = 0.5 * libm::sin(2.0 * core::f64::consts::PI * 200.0 * i as f64 / sr)
                + 0.5 * libm::sin(2.0 * core::f64::consts::PI * 8000.0 * i as f64 / sr);
        }

        // Process with elevation +90 (above).
        let mut pos_above = PositionalStage::new(sr);
        pos_above.active = true;
        pos_above.position_3d_enabled = true;
        pos_above.elevation_deg = 90.0;
        let mut above_l = [0.0_f64; 2048];
        let mut above_r = [0.0_f64; 2048];
        pos_above.process(&mono, &mut above_l, &mut above_r, sr);

        // Process with elevation -90 (below).
        let mut pos_below = PositionalStage::new(sr);
        pos_below.active = true;
        pos_below.position_3d_enabled = true;
        pos_below.elevation_deg = -90.0;
        let mut below_l = [0.0_f64; 2048];
        let mut below_r = [0.0_f64; 2048];
        pos_below.process(&mono, &mut below_l, &mut below_r, sr);

        // Measure energy in the settled portion (skip first 512 samples for
        // filter transient). Above should have more energy due to +8 dB
        // high-shelf boost on the 8 kHz component.
        let above_energy: f64 = above_l[512..].iter().map(|s| s * s).sum();
        let below_energy: f64 = below_l[512..].iter().map(|s| s * s).sum();
        assert!(
            above_energy > below_energy * 1.5,
            "Elevation +90 (above) should have more energy than -90 (below) \
             due to high-shelf boost: above={above_energy:.4}, below={below_energy:.4}"
        );
    }

    #[test]
    fn elevation_zero_is_neutral() {
        let sr = 48000.0;
        let len = 1024;
        let mut mono = [0.0_f64; 1024];
        for (i, sample) in mono.iter_mut().enumerate().take(len) {
            *sample = libm::sin(2.0 * core::f64::consts::PI * 1000.0 * i as f64 / sr);
        }

        // Elevation at 0 should be passthrough (no shelf filter applied).
        let mut pos = PositionalStage::new(sr);
        pos.active = true;
        pos.position_3d_enabled = true;
        pos.elevation_deg = 0.0;
        let mut left = [0.0_f64; 1024];
        let mut right = [0.0_f64; 1024];
        pos.process(&mono, &mut left, &mut right, sr);

        // Compare with a reference at elevation 0 (should be roughly 0.5 * mono
        // since azimuth 0 gives gain_l = 0.5).
        let expected_gain = 0.5;
        for i in 256..len {
            let expected = mono[i] * expected_gain;
            assert!(
                (left[i] - expected).abs() < 0.05,
                "At elevation 0, output should match simple gain: \
                 sample {i}, got {}, expected {expected}",
                left[i]
            );
        }
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

    // --- Orbital position tests ---

    #[test]
    fn orbital_circular_equatorial_constant_distance() {
        // Circular orbit (e=0) in horizontal plane (i=0) should give
        // constant distance equal to the semi-major axis.
        let a = 0.5;
        for step in 0..36 {
            let phase = step as f64 / 36.0;
            let (_, _, dist) = compute_orbital_position(phase, a, 0.0, 0.0, 0.0, 0.0);
            assert!(
                (dist - a).abs() < 0.01,
                "Phase {phase}: distance {dist}, expected {a}"
            );
        }
    }

    #[test]
    fn orbital_circular_equatorial_azimuth_sweeps() {
        // Circular equatorial orbit should sweep azimuth from 0 to 360
        // linearly (the "lighthouse" autopan effect).
        let (az0, el0, _) = compute_orbital_position(0.0, 0.5, 0.0, 0.0, 0.0, 0.0);
        assert!(az0.abs() < 1.0, "Phase 0 should be near azimuth 0");
        assert!(el0.abs() < 1.0, "Equatorial orbit should have zero elevation");

        let (az25, _, _) = compute_orbital_position(0.25, 0.5, 0.0, 0.0, 0.0, 0.0);
        assert!(
            (az25 - 90.0).abs() < 5.0,
            "Phase 0.25 should be near azimuth 90: got {az25}"
        );

        let (az50, _, _) = compute_orbital_position(0.5, 0.5, 0.0, 0.0, 0.0, 0.0);
        assert!(
            (az50 - 180.0).abs() < 5.0,
            "Phase 0.5 should be near azimuth 180: got {az50}"
        );
    }

    #[test]
    fn orbital_polar_orbit_varies_elevation() {
        // Inclination 90 (polar orbit) should produce elevation changes.
        let mut max_elev = -100.0_f64;
        let mut min_elev = 100.0_f64;
        for step in 0..72 {
            let phase = step as f64 / 72.0;
            let (_, el, _) = compute_orbital_position(phase, 0.5, 0.0, 90.0, 0.0, 0.0);
            max_elev = max_elev.max(el);
            min_elev = min_elev.min(el);
        }
        assert!(
            max_elev > 45.0,
            "Polar orbit should reach high elevation: max {max_elev}"
        );
        assert!(
            min_elev < -45.0,
            "Polar orbit should reach low elevation: min {min_elev}"
        );
    }

    #[test]
    fn orbital_eccentric_distance_varies() {
        // Eccentric orbit (e=0.8) should have varying distance.
        let mut min_dist = 2.0_f64;
        let mut max_dist = -1.0_f64;
        for step in 0..72 {
            let phase = step as f64 / 72.0;
            let (_, _, dist) = compute_orbital_position(phase, 0.8, 0.8, 0.0, 0.0, 0.0);
            min_dist = min_dist.min(dist);
            max_dist = max_dist.max(dist);
        }
        // Periapsis distance = a * (1 - e) = 0.8 * 0.2 = 0.16
        // Apoapsis distance = a * (1 + e) = 0.8 * 1.8 = 1.44 (clamped to 1.0)
        assert!(
            min_dist < 0.25,
            "Periapsis should be close: min {min_dist}"
        );
        assert!(
            max_dist > 0.8,
            "Apoapsis should be far: max {max_dist}"
        );
    }

    #[test]
    fn orbital_no_nan_inf() {
        // Verify no NaN or Inf for a variety of parameters.
        let params = [
            (0.0, 0.5, 0.0, 0.0, 0.0, 0.0),
            (0.5, 1.0, 0.99, 90.0, 180.0, 270.0),
            (0.999, 0.01, 0.5, 45.0, 45.0, 45.0),
            (0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
            (1.0, 1.0, 0.0, 180.0, 360.0, 360.0),
        ];
        for (phase, a, e, i, o, w) in params {
            let (az, el, dist) = compute_orbital_position(phase, a, e, i, o, w);
            assert!(az.is_finite(), "NaN/Inf azimuth for params {phase},{a},{e},{i},{o},{w}");
            assert!(el.is_finite(), "NaN/Inf elevation for params {phase},{a},{e},{i},{o},{w}");
            assert!(dist.is_finite(), "NaN/Inf distance for params {phase},{a},{e},{i},{o},{w}");
        }
    }
}
