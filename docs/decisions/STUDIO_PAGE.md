# Studio Page and New Effects

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status.** Decided.
**Date.** 2026-10-30
**Decided by.** The operator, who asked for steps 1 to 3 below, with the readings of
the request made by this line and open to revision.

## Context

The operator relayed a user's wish for a composer in the manner of Audiotool and
Soundtrap, with more settings and more instruments than those sites, and named
Bitcrusher, Overdrive, Distortion, Fuzz and "Tune", and more effects in general.

Facts at the start, on `main` at `54954f9`. The engine already had Bitcrusher,
Overdrive, Distortion and Fuzz among 25 pedals. They were reachable only by pressing
Tab to a chain page and then the A key, and no visible control pointed there. Page
navigation was by keyboard, and a pedal could be moved, bypassed or removed only by
keys.

Inference. The request for effects that existed shows that the user did not find them,
so discoverability was the main problem rather than missing features.

## Options

1. **Add effects only.** Leaves the existing ones as hidden as before.
2. **Make every editing action reachable by mouse, add effects, and add a home page in
   the manner of browser workstations.** Larger, but answers both the stated and the
   inferred problem.

## Decision

Option 2, as the operator asked.

- A Studio page is the home page. It has an instrument and effect browser, a track
  list with mute, solo, volume and a note preview, and the selected track's effects
  rack.
- A row of page tabs and Play, Stop and Save buttons sits under the toolbar on every
  page.
- Every pedal box has ON/OFF, move and Remove buttons, every chain ends in an Add
  effect card, and the picker shows all effects at once with descriptions.
- Four effects were added. They are Wavefolder, a distortion, Filter, a resonant
  multimode filter with an LFO sweep, Vibrato, and Pitch Corrector.

"Tune" was read as pitch correction in the manner of Auto-Tune, because Soundtrap, one
of the two named sites, advertises that effect. It may instead have meant a tuner or
the existing Pitch Shifter. The user has not confirmed the reading. Pitch correction
on this composer's synthesizer voices mainly gives the stepped, robotic sound, since
they are already in tune unless detuned or bent, and the composer cannot record a
voice.

## Consequences

- The application module was split into files by concern, because the Studio page
  would otherwise have added about 900 lines to a 5,900-line file.
- Content starts 28 pixels lower on every page. Each page was rendered and inspected
  with the screenshot example, which renders offscreen through SDL. No one has used
  the pages with a real display, mouse and audio device.
- The `depth_ms` slider range changed from 0 to 2000 ms to 0 to 10 ms, which suits
  chorus, flanger and vibrato, and the Phaser `stages` slider gained a range of 1 to
  8. Before, dragging it set one stage.
- Revisit "Tune" if the user meant a tuner. A tuner needs a path from the audio thread
  back to the interface, which does not exist yet.
