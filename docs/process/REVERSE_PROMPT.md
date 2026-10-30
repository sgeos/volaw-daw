# Reverse Prompt

> **Navigation**: [Process](./README.md)

A state file, overwritten whole after each task. It reports what was done, what was
decided and by whom, what was verified and how, what was not verified, and what the
operator needs to decide. Facts, inferences and hypotheses are marked as such.

## Report on 2026-10-30

**Task.** The operator relayed a user's wish for a composer in the manner of Audiotool
and Soundtrap, with more settings and instruments, naming Bitcrusher, Overdrive,
Distortion, Fuzz and "Tune". The operator asked for three steps. Make the existing
effects reachable by mouse, add missing effects, and rework the layout toward those
sites.

**Done.**
- A Studio page is the home page. It has an instrument and effect browser, a track list
  with mute, solo, volume and a note preview, and the selected track's effects rack.
- A page tab row with Play, Stop and Save is on every page. Every pedal has ON/OFF,
  move and Remove buttons, every chain ends in an Add effect card, and the picker shows
  all 29 effects at once with a description.
- Four effects. Wavefolder, Filter with an LFO sweep, Vibrato, and Pitch Corrector.
- `daw-core`'s application module is split into `mod.rs`, `handlers.rs`, `render.rs`,
  `widgets.rs`, `studio.rs` and `tests.rs`.
- Fixed on the way. The Phaser `stages` slider spanned 0 to 1. Moving a pedal after a
  track switch could panic. Clicks could pass through the open picker. The changelog
  repeated its section headings, which are now merged with every item kept.

**Decided by this line, open to revision.** "Tune" was read as pitch correction,
because Soundtrap advertises it. The Studio layout, the parameter labels, the slider
ranges and the effect descriptions.

**Verified.** Facts, observed on the macOS authoring host.
- The gate exited zero with 398 tests and 1 doctest, against 363 on `main`, and the
  core built for `thumbv7em-none-eabihf`.
- New tests click the Studio controls through `handle_input` and check the result,
  covering every page tab, Play and Stop, every effect in the browser, presets, mute,
  solo, the volume drag, the rack buttons, the add card and the picker. Another test
  checks that every listed pedal exists in the engine and accepts its parameters.
- The Pitch Corrector detects a 450 Hz sine within 3 Hz and computes the correction to
  440 Hz within 0.05 semitones. A major scale moves a C sharp by one semitone.
- Every page was rendered offscreen by the real SDL renderer with the `screenshots`
  example and inspected. Two layout defects found that way were fixed.
- The composer started under SDL's dummy drivers and ran for four seconds.

**Not verified.**
- Interactive use with a real display, mouse and audio device.
- How the new effects sound. The tests check signal properties, not musical quality.
  Inference. The Pitch Corrector inherits the roughness of the granular Pitch Shifter.
- Continuous integration on this branch at the time of writing.

**For the operator.**
- Run `cargo run -p daw`, try the Studio page, and ask the user whether "Tune" meant
  pitch correction or a tuner.
