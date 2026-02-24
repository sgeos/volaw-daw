//! ADSR envelope generator for the MS-20 synthesizer.
//!
//! The MS-20 has two envelope generators (EG1 and EG2) that shape the
//! amplitude and filter cutoff over time. Each generates a control voltage
//! from 0.0 to 1.0 based on attack, decay, sustain, and release parameters.
//!
//! ## Stages
//!
//! ```text
//! 1.0 ──────────┐
//!              / │\
//!             /  │ \
//!            /   │  \──────── sustain level
//!           /    │             \
//!          /     │              \
//! 0.0 ───/──────────────────────\───
//!       attack  decay           release
//!       |<-- gate on -------->|
//! ```
//!
//! The original MS-20 uses a HAD (Hold/Attack/Decay) envelope for EG1
//! and HADSR for EG2. This implementation provides full ADSR for both,
//! which is a superset of the original behavior.

/// ADSR envelope stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvelopeStage {
    /// Envelope is idle (before first gate or after release completes).
    Idle,
    /// Attack stage: output rises from 0 to 1.
    Attack,
    /// Decay stage: output falls from 1 to sustain level.
    Decay,
    /// Sustain stage: output holds at sustain level while gate is on.
    Sustain,
    /// Release stage: output falls from current level to 0.
    Release,
}

/// ADSR envelope generator.
///
/// Generates a control signal from 0.0 to 1.0 based on gate state and
/// time parameters. Used to modulate filter cutoff (EG1 for HPF, EG2 for
/// LPF) and amplitude (EG2 for VCA).
#[derive(Debug, Clone)]
pub struct Adsr {
    /// Attack time in seconds. Time to rise from 0 to 1.
    pub attack: f64,
    /// Decay time in seconds. Time to fall from 1 to sustain level.
    pub decay: f64,
    /// Sustain level (0.0 to 1.0). Held while gate is on.
    pub sustain: f64,
    /// Release time in seconds. Time to fall from sustain to 0.
    pub release: f64,

    /// Current envelope stage.
    stage: EnvelopeStage,
    /// Current envelope output level (0.0 to 1.0).
    level: f64,
}

impl Adsr {
    /// Create a new ADSR envelope with the given parameters.
    pub fn new(attack: f64, decay: f64, sustain: f64, release: f64) -> Self {
        Self {
            attack: attack.max(0.001),
            decay: decay.max(0.001),
            sustain: sustain.clamp(0.0, 1.0),
            release: release.max(0.001),
            stage: EnvelopeStage::Idle,
            level: 0.0,
        }
    }

    /// Trigger the envelope (gate on). Starts the attack stage.
    pub fn gate_on(&mut self) {
        self.stage = EnvelopeStage::Attack;
    }

    /// Release the envelope (gate off). Starts the release stage from
    /// the current level.
    pub fn gate_off(&mut self) {
        if self.stage != EnvelopeStage::Idle {
            self.stage = EnvelopeStage::Release;
        }
    }

    /// Advance the envelope by one sample and return the current level.
    pub fn tick(&mut self, sample_rate: f64) -> f64 {
        let dt = 1.0 / sample_rate;

        match self.stage {
            EnvelopeStage::Idle => {
                self.level = 0.0;
            }
            EnvelopeStage::Attack => {
                let rate = 1.0 / self.attack;
                self.level += rate * dt;
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.stage = EnvelopeStage::Decay;
                }
            }
            EnvelopeStage::Decay => {
                let rate = (1.0 - self.sustain) / self.decay;
                self.level -= rate * dt;
                if self.level <= self.sustain {
                    self.level = self.sustain;
                    self.stage = EnvelopeStage::Sustain;
                }
            }
            EnvelopeStage::Sustain => {
                self.level = self.sustain;
            }
            EnvelopeStage::Release => {
                // Release from whatever the current level is.
                let rate = self.level.max(self.sustain) / self.release;
                self.level -= rate * dt;
                if self.level <= 0.0 {
                    self.level = 0.0;
                    self.stage = EnvelopeStage::Idle;
                }
            }
        }

        self.level
    }

    /// Return the current envelope level without advancing.
    pub fn level(&self) -> f64 {
        self.level
    }

    /// Return the current envelope stage.
    pub fn stage(&self) -> EnvelopeStage {
        self.stage
    }

    /// Reset the envelope to idle state.
    pub fn reset(&mut self) {
        self.stage = EnvelopeStage::Idle;
        self.level = 0.0;
    }
}

impl Default for Adsr {
    fn default() -> Self {
        // Moderate defaults suitable for general synthesis.
        Self::new(0.01, 0.1, 0.7, 0.3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_at_zero() {
        let mut env = Adsr::default();
        let level = env.tick(48000.0);
        assert!(level.abs() < f64::EPSILON);
        assert_eq!(env.stage(), EnvelopeStage::Idle);
    }

    #[test]
    fn attack_rises_to_one() {
        let mut env = Adsr::new(0.01, 0.1, 0.7, 0.3);
        env.gate_on();

        // Run for 20ms at 48kHz (960 samples). Attack is 10ms.
        let mut peak = 0.0f64;
        for _ in 0..960 {
            let level = env.tick(48000.0);
            if level > peak {
                peak = level;
            }
        }

        assert!(
            (peak - 1.0).abs() < 0.01,
            "Attack should reach 1.0: peak={peak}"
        );
    }

    #[test]
    fn decay_to_sustain() {
        let mut env = Adsr::new(0.001, 0.01, 0.5, 0.3);
        env.gate_on();

        // Run long enough for attack + decay to complete.
        for _ in 0..4800 {
            env.tick(48000.0);
        }

        let level = env.level();
        assert!(
            (level - 0.5).abs() < 0.05,
            "Should settle at sustain=0.5: {level}"
        );
        assert_eq!(env.stage(), EnvelopeStage::Sustain);
    }

    #[test]
    fn release_to_zero() {
        let mut env = Adsr::new(0.001, 0.01, 0.5, 0.01);
        env.gate_on();

        // Reach sustain.
        for _ in 0..2400 {
            env.tick(48000.0);
        }

        env.gate_off();
        assert_eq!(env.stage(), EnvelopeStage::Release);

        // Run release (10ms = 480 samples, give extra margin).
        for _ in 0..960 {
            env.tick(48000.0);
        }

        let level = env.level();
        assert!(level < 0.01, "Should release to near zero: {level}");
        assert_eq!(env.stage(), EnvelopeStage::Idle);
    }

    #[test]
    fn gate_off_from_attack() {
        let mut env = Adsr::new(0.1, 0.1, 0.5, 0.01);
        env.gate_on();

        // Run a few samples (still in attack).
        for _ in 0..48 {
            env.tick(48000.0);
        }
        assert_eq!(env.stage(), EnvelopeStage::Attack);

        // Release early.
        env.gate_off();
        assert_eq!(env.stage(), EnvelopeStage::Release);

        // Should release from current level to zero.
        for _ in 0..960 {
            env.tick(48000.0);
        }
        assert!(env.level() < 0.01);
    }

    #[test]
    fn level_stays_bounded() {
        let mut env = Adsr::new(0.01, 0.1, 0.7, 0.3);
        env.gate_on();

        for _ in 0..48000 {
            let level = env.tick(48000.0);
            assert!(
                (-f64::EPSILON..=1.0 + f64::EPSILON).contains(&level),
                "Level out of bounds: {level}"
            );
        }
    }

    #[test]
    fn reset_returns_to_idle() {
        let mut env = Adsr::default();
        env.gate_on();
        for _ in 0..480 {
            env.tick(48000.0);
        }
        assert!(env.level() > 0.0);

        env.reset();
        assert_eq!(env.stage(), EnvelopeStage::Idle);
        assert!(env.level().abs() < f64::EPSILON);
    }

    #[test]
    fn sustain_zero_decays_to_silence() {
        let mut env = Adsr::new(0.001, 0.01, 0.0, 0.3);
        env.gate_on();

        for _ in 0..4800 {
            env.tick(48000.0);
        }

        let level = env.level();
        assert!(level < 0.01, "Sustain=0 should decay to silence: {level}");
    }

    #[test]
    fn sustain_one_stays_at_peak() {
        let mut env = Adsr::new(0.001, 0.01, 1.0, 0.3);
        env.gate_on();

        for _ in 0..4800 {
            env.tick(48000.0);
        }

        let level = env.level();
        assert!(
            (level - 1.0).abs() < 0.01,
            "Sustain=1 should stay at peak: {level}"
        );
    }

    #[test]
    fn default_values() {
        let env = Adsr::default();
        assert!((env.attack - 0.01).abs() < 1e-10);
        assert!((env.decay - 0.1).abs() < 1e-10);
        assert!((env.sustain - 0.7).abs() < 1e-10);
        assert!((env.release - 0.3).abs() < 1e-10);
    }
}
