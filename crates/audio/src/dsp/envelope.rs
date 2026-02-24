//! Envelope follower for dynamics processing.
//!
//! Tracks the amplitude of an input signal using asymmetric attack and
//! release smoothing. Used by auto-wah (envelope-controlled filter),
//! compressor, and noise gate effects.

/// Amplitude envelope follower with configurable attack and release times.
#[derive(Debug, Clone)]
pub struct EnvelopeFollower {
    level: f64,
    attack_coeff: f64,
    release_coeff: f64,
}

impl EnvelopeFollower {
    /// Create a new envelope follower with the given attack and release
    /// times in milliseconds.
    pub fn new(attack_ms: f64, release_ms: f64, sample_rate: f64) -> Self {
        Self {
            level: 0.0,
            attack_coeff: Self::time_constant(attack_ms, sample_rate),
            release_coeff: Self::time_constant(release_ms, sample_rate),
        }
    }

    /// Compute the one-pole smoothing coefficient for a given time constant.
    fn time_constant(ms: f64, sample_rate: f64) -> f64 {
        if ms <= 0.0 || sample_rate <= 0.0 {
            return 0.0;
        }
        libm::exp(-1.0 / (ms * 0.001 * sample_rate))
    }

    /// Process a single sample and return the current envelope level.
    #[inline]
    pub fn tick(&mut self, input: f64) -> f64 {
        let abs_input = libm::fabs(input);
        let coeff = if abs_input > self.level {
            self.attack_coeff
        } else {
            self.release_coeff
        };
        self.level = coeff * self.level + (1.0 - coeff) * abs_input;
        self.level
    }

    /// Update attack and release times.
    pub fn set_times(&mut self, attack_ms: f64, release_ms: f64, sample_rate: f64) {
        self.attack_coeff = Self::time_constant(attack_ms, sample_rate);
        self.release_coeff = Self::time_constant(release_ms, sample_rate);
    }

    /// Get the current envelope level.
    pub fn level(&self) -> f64 {
        self.level
    }

    /// Reset the envelope to zero.
    pub fn reset(&mut self) {
        self.level = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_at_zero() {
        let env = EnvelopeFollower::new(1.0, 50.0, 48000.0);
        assert!(env.level().abs() < f64::EPSILON);
    }

    #[test]
    fn follows_rising_signal() {
        let mut env = EnvelopeFollower::new(0.1, 50.0, 48000.0);
        for _ in 0..480 {
            env.tick(1.0);
        }
        // After 10ms of unity signal with 0.1ms attack, should be close to 1.0
        assert!(env.level() > 0.9);
    }

    #[test]
    fn decays_after_silence() {
        let mut env = EnvelopeFollower::new(0.1, 10.0, 48000.0);
        // Ramp up
        for _ in 0..480 {
            env.tick(1.0);
        }
        let peak = env.level();
        // Feed silence
        for _ in 0..4800 {
            env.tick(0.0);
        }
        assert!(env.level() < peak * 0.1);
    }

    #[test]
    fn reset_clears_level() {
        let mut env = EnvelopeFollower::new(1.0, 50.0, 48000.0);
        env.tick(1.0);
        env.tick(1.0);
        env.reset();
        assert!(env.level().abs() < f64::EPSILON);
    }
}
