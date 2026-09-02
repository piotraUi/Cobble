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
camera driven by WASD + mouse look on desktop.

**Step 2 (done):** the `protocol` crate implements the Minecraft 1.8.9
wire protocol (protocol version 47) — VarInt framing, zlib packet
compression, and offline-mode Handshaking → Login → Play. It connects to
a real 1.8.9 server, decodes Chunk Data and Chunk Data Bulk into real
block data, and answers Keep Alive so the server doesn't time us out.
`app-desktop` now renders the actual connected world instead of the demo
chunk, spawns at the server's given position, and sends player position
updates. Entities, inventory, and full physics/collision aren't
implemented yet (see steps 3+); movement is still a free-fly camera.

Run without arguments for the old hardcoded demo chunk:

```
cargo run --bin cobble
```

Or connect to a real (offline-mode) 1.8.9 server:

```
cargo run --bin cobble -- <host[:port]> [username]
```

Controls: WASD to move, click the window to capture the mouse and look
around, Space/Shift to fly up/down, Escape to release the mouse.

The trickiest part — decoding the 1.8.9 chunk section byte layout, VarInt
encoding, block Position packing, and packet compression — has unit
tests in `protocol` (`cargo test -p protocol`); run against a real server
to be sure, since none of this was exercised against live server traffic
in development.

**Next up (step 3):** player movement physics — gravity and AABB
collision against the now-real world, instead of flying through it.

## Legal note

No original Mojang textures, sounds, or fonts are bundled in this
repository or any build. Texture packs are always fetched on demand from
Modrinth at the user's explicit request; fallback textures used for
packs with incomplete coverage are original artwork, not Mojang assets.
