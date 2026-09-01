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
- `texturepacks` — Modrinth integration: search, download, validate, fallback
- `app-desktop` — desktop entry point (`winit` + `wgpu`)
- `app-android` — Android entry point (`android-activity` + `wgpu`)

## Status

**Step 1 (done):** workspace skeleton + `wgpu` renderer showing a single
hardcoded 16x16x16 chunk (colored cubes, face-culled) with a free-fly FPS
camera driven by WASD + mouse look on desktop. No networking or texture
packs yet.

Run it with:

```
cargo run --bin cobble
```

Controls: WASD to move, click the window to capture the mouse and look
around, Space/Shift to fly up/down, Escape to release the mouse.

**Next up (step 2):** implement the `protocol` crate — connect to a real
1.8.9 server, parse Chunk Data packets, and render the actual world
instead of the hardcoded demo chunk.

## Legal note

No original Mojang textures, sounds, or fonts are bundled in this
repository or any build. Texture packs are always fetched on demand from
Modrinth at the user's explicit request; fallback textures used for
packs with incomplete coverage are original artwork, not Mojang assets.
