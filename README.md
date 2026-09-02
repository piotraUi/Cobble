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

**Step 4 (done):** the `texturepacks` crate talks to the real Modrinth
API — searching resource packs for a game version, listing a project's
versions, and downloading + caching a pack's `.zip` by SHA-1 (so
re-selecting an already-downloaded pack never re-fetches it). It
validates `pack.mcmeta`'s `pack_format` (1 for 1.8.x, flagged but not
hard-rejected otherwise), checks how many of a hardcoded list of known
1.8.9 block/item textures the pack actually provides, and packs
whatever's present — plus an original, non-Mojang neutral gray
checkerboard fallback for anything missing — into one square texture
atlas with a UV rect per name.

**Step 5, part 1 (done):** the world renderer is texture-mapped —
`renderer::block_textures` maps the handful of block ids `client_core`
currently knows about to atlas tile names (grass/logs get distinct
top/bottom/side textures), vertices carry real UVs, and the shader
samples the atlas with nearest-neighbor filtering (pixel-perfect, no
blur) instead of flat per-vertex debug colors. A block id with no
texture mapping — which, against a real server, is most of them right
now, since only ~10 are mapped — renders with a reserved magenta/black
"missing texture" tile instead of silently going untextured.

**Step 5, part 2 (done):** the `ui` crate is a renderer-agnostic
Minecraft-styled UI — bitmap text (rasterized at startup from a bundled
public-domain Minecraft-style font, see `ui/assets/fonts/LICENSE.txt`;
not a Mojang asset, and used for UI text only, never the logo/wordmark),
buttons, a text field, and three screens (main menu, multiplayer address
entry, texture pack picker) plus an in-game crosshair/hotbar HUD — all
built from one primitive (a textured quad) so solid panels and glyphs
share a single draw call path. `renderer` turns a screen's `DrawList`
into an actual `wgpu` pass (2D orthographic, alpha blended, drawn over
the 3D world). In `app-desktop`, running `cobble` with no arguments now
opens that menu — Singleplayer starts the demo chunk, Multiplayer lets
you type a server address, and Texture Packs searches Modrinth live and
lets you pick a real pack, replacing the fallback atlas everywhere
(world included) once it downloads. Passing a server address on the
command line still skips the menu entirely, for quick testing:

```
cargo run --bin cobble                       # opens the main menu
cargo run --bin cobble -- <host[:port]> [username]   # skips straight into a server
```

Controls: WASD to move, Space to jump, Shift to sneak (slower walk),
click to look around/capture the mouse, Escape to release the mouse
(press again to return to the main menu).

The trickiest parts have unit tests: `cargo test -p protocol` covers the
1.8.9 chunk section byte layout (including the light nibble arrays'
packing), VarInt encoding, block Position packing, and packet
compression; `cargo test -p client-core` covers the AABB collision
(falling and landing, jumping, sliding to a stop against a wall) and
the block/sky light storage and lookup defaults (loaded vs. missing
section, in vs. out of world bounds); `cargo test -p texturepacks`
covers `pack.mcmeta` parsing, coverage counting, and atlas packing
against synthetic zips; `cargo test -p renderer` covers the
block-id-to-texture mapping, that meshing produces the right face count
and stays within the right atlas tile's UV rect, and that a fully-lit
block meshes visibly brighter than a fully-dark one; `cargo test -p ui`
covers font rasterization, text layout
(including a real baseline-alignment bug and a long-label overflow bug
this caught and fixed — verified by rendering the actual screens to PNG
with a throwaway software rasterizer, since this sandbox has no GPU),
button/text-field input handling, and the menu/picker screens' action
routing. There's also a network-gated integration test that exercises
the real Modrinth API end to end (search → download → cache → validate
→ atlas) — it's `#[ignore]`d by default since it needs network access:

```
cargo test -p texturepacks --test live_modrinth -- --ignored --nocapture
```

Still worth running the desktop client against a real server and
playtesting by hand — none of the networking/physics/rendering code was
exercised against live server traffic or a real GPU in development (see
the note below).

**Step 6 (done):** `app-android` runs the exact same
`client_core`/`renderer`/`ui`/`protocol`/`texturepacks` stack as
`app-desktop`, entered via `android-activity` instead of a desktop
window, and driven by `ui::TouchController` instead of keyboard/mouse:
a virtual joystick (bottom-left) for movement, drag-anywhere-else for
look, and jump/mine/place buttons (bottom-right); tapping a menu screen
is treated as a click on whatever's under it, so the exact same
`ui::screens::Screen` state machine drives both platforms. Reaching
this needed one real cross-platform fix: `reqwest`'s default TLS
backend (`native-tls`) needs a system OpenSSL install, which doesn't
exist for Android cross-compilation, so the whole workspace now uses
`rustls-tls` instead (pure Rust, no system dependency) — re-verified
against the live Modrinth API after the switch.

Build it with (needs the Android NDK r26+ and an SDK with
`platform-tools` + `build-tools;33.0.2` + `platforms;android-33`,
`ANDROID_NDK_HOME`/`ANDROID_HOME` pointing at them, and
`rustup target add aarch64-linux-android`):

```
cargo install cargo-apk   # once
cd app-android
cargo apk build --release --target aarch64-linux-android
# -> target/release/apk/app-android.apk
```

This produced a real, correctly-signed `.apk` (`net.cobble.client`,
`INTERNET`/`ACCESS_NETWORK_STATE` permissions, `arm64-v8a` native
library) in development — confirmed with `aapt dump badging` — but
**was never installed on a real device or emulator**, since none was
available in that environment. Treat the touch/IME handling in
`app-android/src/lib.rs` as reviewed, not verified; the underlying game
logic it drives (physics, rendering, protocol, texture packs) is the
same code exercised by `app-desktop`'s tests.

Hotbar slot selection isn't wired to touch taps yet (the HUD draws the
slots, but tapping one doesn't do anything — there's no inventory to
select from yet either), and mine/place have no world-editing effect
to trigger since that's not implemented on any platform yet.

**Step 7, part 1 (done):** real block/sky lighting. `protocol` now
actually decodes the block light and sky light nibble arrays in Chunk
Data (previously parsed off the wire and discarded — see the chunk
format doc comment in `protocol::chunk_data`) into `client_core::Chunk`,
which stores both per block (0-15 each, unpacked to a byte per block —
the packing only matters on the wire). `renderer`'s mesher now looks up
the real light at each face's neighbor position (`World::get_light`,
falling back to open sky for anywhere with no loaded column) and mixes
`max(block_light, sky_light) / 15` into the existing per-face
directional shade, with a small ambient floor so unlit faces read as
dim rather than pure black. The demo chunk is lit as a plain outdoor
scene (full sky light, no block light) since there's no light
*propagation* engine yet — this decodes and applies light a server
already computed and sent, it doesn't compute new light itself (e.g.
after a player places/breaks a block that should change nearby light).
There's also no day/night cycle, so sky light is always full brightness
regardless of server time. Real servers' actual computed light (cave
darkness, torches, etc.) now shows up correctly since that data was
already being sent — only rendering it was missing.

**Next up (step 7, remaining):** water/lava animation, sound, and
other players' entities.

### A note on testing this in CI/sandboxed environments

Development happened in a sandboxed container with no Vulkan/EGL GPU
drivers, so `wgpu` surface creation panics there — this is an
environment limitation, not a code issue (the workspace builds clean
with zero `clippy` warnings, and non-rendering logic has direct unit
test coverage as noted above). Run it on a real desktop with a GPU to
see it render. Similarly, there was no Android device or emulator
available to actually run the `.apk` on, even though it does build —
see the step 6 note above.

## Legal note

No original Mojang textures, sounds, or fonts are bundled in this
repository or any build. Texture packs are always fetched on demand from
Modrinth at the user's explicit request; fallback textures used for
packs with incomplete coverage are original artwork, not Mojang assets.
