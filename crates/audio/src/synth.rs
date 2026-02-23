/// Virtual Korg MS-20 synthesizer voice.
///
/// The MS-20 signal chain consists of two VCOs, a Sallen-Key highpass filter,
/// a Korg-35 lowpass filter with resonance, two envelope generators, and a VCA.
/// The MS-20 also features an external signal processor (ESP) input that allows
/// external audio to be patched into the filter and modulation paths.
///
/// ## Signal Chain (Future Implementation)
///
/// ```text
/// External Audio In --+
///                     v
/// VCO1 --+--> Mixer --> HPF (Sallen-Key) --> LPF (Korg-35) --> VCA --> Out
/// VCO2 --+              ^                     ^                  ^
///                       EG1                   EG2                EG2
/// ```
///
/// ## Current State
///
/// Stub implementation. The `process` method outputs silence. Real DSP
/// synthesis will be implemented in a future milestone. The Korg-35 lowpass
/// filter will require RK4 or trapezoidal integration with Newton-Raphson
/// iteration for numerical stability at high resonance settings.
#[derive(Debug, Clone)]
pub struct Ms20Voice {
    /// Whether external audio input is patched into the signal chain.
    pub external_input_enabled: bool,
}

impl Ms20Voice {
    /// Create a new MS-20 voice in its default state.
    pub fn new() -> Self {
        Self {
            external_input_enabled: false,
        }
    }

    /// Process a mono audio buffer in place.
    ///
    /// Stub: writes silence regardless of input.
    pub fn process(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = 0.0;
        }
    }
}

impl Default for Ms20Voice {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_voice_defaults() {
        let voice = Ms20Voice::new();
        assert!(!voice.external_input_enabled);
    }

    #[test]
    fn process_writes_silence() {
        let mut voice = Ms20Voice::new();
        let mut buffer = [1.0f32; 32];
        voice.process(&mut buffer);
        for sample in &buffer {
            assert!(sample.abs() < f32::EPSILON);
        }
    }

    #[test]
    fn default_matches_new() {
        let a = Ms20Voice::new();
        let b = Ms20Voice::default();
        assert_eq!(a.external_input_enabled, b.external_input_enabled);
    }
}
