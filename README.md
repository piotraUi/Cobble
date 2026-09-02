# Cobble

A native Minecraft 1.8.9-style client written in Rust, targeting Android
and desktop (Windows/Linux) from one codebase. Cobble connects to real
Minecraft 1.8.9 servers over the vanilla protocol; it is not a game
engine or a server.

## Workspace layout

- `protocol` — Minecraft 1.8.9 network protocol (protocol version 47)
- `renderer` — voxel renderer on `wgpu` (chunk meshing, texture atlas, camera)
- `client-core` — game state: blocks, chunks, camera, player, physics
- `ui` — Minecraft-styled UI (menus, HUD) rendered directly with `wgpu`
- `texturepacks` — Modrinth integration: search, download, validate, atlas
- `app-desktop` — desktop entry point (`winit` + `wgpu`)
- `app-android` — Android entry point (`android-activity` + `wgpu`)

## Status

**Step 1 (done):** workspace skeleton + `wgpu` renderer showing a single
hardcoded 16x16x16 chunk (colored cubes, face-culled) with a free-fly FPS
camera driven by WASD + mouse look on desktop.

**Step 2 (done):** the `protocol` crate implements the Minecraft 1.8.9
wire protocol (protocol version 47) — VarInt framing, zlib packet
compression, and offline-mode Handshaking → Login → Play. It connects to
a real 1.8.9 server, decodes Chunk Data and Chunk Data Bulk into real
block data, and answers Keep Alive so the server doesn't time us out.
`app-desktop` now renders the actual connected world instead of the demo
chunk, spawns at the server's given position, and sends player position
updates.

**Step 3 (done):** the player is a real 0.6×1.8 AABB now
(`client_core::physics`) — gravity, jumping, and per-axis sliding
collision against the world's blocks, resolved one axis at a time so
walking into a corner slides along the wall instead of stopping dead.
This replaces the free-fly camera in both the demo chunk and networked
modes; the demo chunk is now a real physics playground (spawns falling
onto it) instead of just something to fly around. Entities and inventory
still aren't implemented (later steps).

Run without arguments for the old hardcoded demo chunk:

```
cargo run --bin cobble
```

Or connect to a real (offline-mode) 1.8.9 server:

```
cargo run --bin cobble -- <host[:port]> [username]
```

Controls: WASD to move, Space to jump, Shift to sneak (slower walk),
click the window to capture the mouse and look around, Escape to
release the mouse.

The trickiest parts have unit tests: `cargo test -p protocol` covers the
1.8.9 chunk section byte layout, VarInt encoding, block Position
packing, and packet compression; `cargo test -p client-core` covers the
AABB collision (falling and landing, jumping, sliding to a stop against
a wall); `cargo test -p texturepacks` covers `pack.mcmeta` parsing,
coverage counting, and atlas packing against synthetic zips. There's
also a network-gated integration test that exercises the real Modrinth
API end to end (search → download → cache → validate → atlas) — it's
`#[ignore]`d by default since it needs network access:

```
cargo test -p texturepacks --test live_modrinth -- --ignored --nocapture
```

Still worth running the desktop client against a real server and
playtesting by hand — none of the networking/physics code was
exercised against live server traffic or a real GPU in development (see
the note below).

**Step 4 (done):** the `texturepacks` crate talks to the real Modrinth
API — searching resource packs for a game version, listing a project's
versions, and downloading + caching a pack's `.zip` by SHA-1 (so
re-selecting an already-downloaded pack never re-fetches it). It
validates `pack.mcmeta`'s `pack_format` (1 for 1.8.x, flagged but not
hard-rejected otherwise), checks how many of a hardcoded list of known
1.8.9 block/item textures the pack actually provides, and packs
whatever's present — plus an original, non-Mojang neutral gray
checkerboard fallback for anything missing — into one square texture
atlas with a UV rect per name. Not wired into rendering yet (still flat
debug colors); that, plus the pack-picker UI, is step 5.

**Next up (step 5):** the `ui` crate — a Minecraft-styled main menu and
texture pack picker rendered directly with `wgpu`, plus wiring the new
atlas into the world renderer and HUD instead of flat debug colors.

### A note on testing this in CI/sandboxed environments

Development happened in a sandboxed container with no Vulkan/EGL GPU
drivers, so `wgpu` surface creation panics there — this is an
environment limitation, not a code issue (the workspace builds clean
with zero `clippy` warnings, and non-rendering logic has direct unit
test coverage as noted above). Run it on a real desktop with a GPU to
see it render.

## Legal note

No original Mojang textures, sounds, or fonts are bundled in this
repository or any build. Texture packs are always fetched on demand from
Modrinth at the user's explicit request; fallback textures used for
packs with incomplete coverage are original artwork, not Mojang assets.
