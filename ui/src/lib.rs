//! Minecraft-styled UI (menus, HUD, buttons, bitmap font), rendered
//! directly with `wgpu` quads and text — no system widgets, no
//! third-party immediate-mode UI framework, so the look stays
//! pixel-perfect and nearest-neighbor filtered like the original.
//!
//! Not implemented yet — this is roadmap step 5 (texture pack picker,
//! main menu, in-game HUD). It will share the same texture atlas the
//! world renderer uses.
