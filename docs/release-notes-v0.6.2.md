# Nostyra 0.6.2 — release notes

> **Old Worlds. New Ways.**

## Download

| File | Platform |
|------|----------|
| `Nostyra-0.6.2-macos-universal.dmg` | macOS, Apple Silicon and Intel (signed and notarized) |

## What's new in 0.6.2

- **Play online between Windows and macOS (Experimental).** A Mac can now host or join a room with Windows PCs.
  Both players still need the same core and the same core version (for example `snes9x 1.63 fae2fea`); if the
  versions differ, update the cores on both computers. The core binary no longer has to be identical when the two
  computers run different systems.
- A core whose save states depend on the platform is still refused across platforms ("Different platform").
- If the two computers drift apart anyway, the desync warning appears as before and the host can press **Resync**.
  Start with 2D systems (NES, SNES, Game Boy); 3D systems are more likely to desync across Windows and macOS.

## Compatibility

- **Everyone in a room needs 0.6.2 or later.** The online lobby protocol changed: a 0.6.0 / 0.6.1 player who tries
  to join a 0.6.2 room (or the other way round) sees "Different Nostyra version: update Nostyra".
