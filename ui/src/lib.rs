//! Minecraft-styled UI (menus, HUD, buttons, bitmap font) as a
//! renderer-agnostic layer: screens hold their own state and, each
//! frame, turn input into actions and produce a `DrawList` of textured
//! quads. A host crate (see `app-desktop`) turns that into actual
//! `wgpu` draw calls — this crate never touches the GPU directly, so
//! it stays testable without one.
//!
//! Text uses a bundled public-domain Minecraft-style pixel font (see
//! `assets/fonts/LICENSE.txt`) — not a Mojang asset, and used for UI
//! text only, never for reproducing the Minecraft logo/wordmark.

pub mod draw_list;
pub mod font;
pub mod geometry;
pub mod input;
pub mod screens;
pub mod style;
pub mod widgets;

pub use draw_list::{DrawList, Painter, Quad};
pub use font::Font;
pub use geometry::{Color, Rect};
pub use input::UiInput;
pub use screens::{Action, Screen};
pub use widgets::{Button, TextField};
