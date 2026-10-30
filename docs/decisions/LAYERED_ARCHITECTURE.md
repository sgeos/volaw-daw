# Layered Architecture

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status.** Decided. The crate names are proposed and open to the operator's revision.
**Date.** 2026-10-28.
**Decided by.** The operator required the layering. The line `volaw-daw-b2` chose the
crate boundaries and names.

## Context

The operator's preferred Rust architecture is a `no_std` core, a platform layer that
implements the core's traits, and entry points that wire them together. The operator
also required a library that both volaw and the DAW depend on, which implies at least
two entry deliverables.

Facts about the starting point at `aa086d5`. The `daw` binary held about 5,900 lines in
`main.rs`, including the whole editing state machine, project persistence, WAV loading,
the engine command protocol, the lock-free queue and the SDL audio bridge. volaw kept a
separate SDL audio bridge of its own around `Arc<Mutex<AudioEngine>>`.

## Options

1. **Split the platform code out of the binary only.** Smaller change, but the state
   machine would stay in an entry point, against the stated architecture.
2. **Move the state machine into `daw-core` behind traits, and add platform and entry
   crates.** Larger change, and every concern lands in the layer the architecture
   names.

## Decision

Option 2. The binary is now about 250 lines of argument parsing and wiring. The engine
command protocol, queue and runner moved into `audio`, so the game and the DAW share one
lock-free path to the engine. `volaw-audio` is the shared entry library.

Two names were kept to avoid breaking volaw. `audio` keeps its name and path, because
volaw depends on `../volaw-daw/crates/audio`. `daw-core` keeps its name.

## Consequences

- The public API of `audio` grew. Nothing was removed, and volaw at `HEAD` passed
  `cargo check` against it.
- Persistence now rejects a referenced file name that is absolute or contains a `..`
  component. A project that referenced a file outside its root no longer loads.
- `toml` is built without its `std` feature in `daw-core`.
- Revisit when volaw adopts `volaw-audio`, or when the game needs song playback, which
  would move note scheduling out of `DawApp` into a shared module.
