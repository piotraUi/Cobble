//! Shared color palette for the whole UI — a Minecraft-ish gray/blue
//! button scheme, but original values (not sampled from the game).

use crate::geometry::Color;

pub const PANEL_BG: Color = Color::rgba(0.02, 0.02, 0.02, 0.55);
pub const BUTTON_BASE: Color = Color::rgb(0.35, 0.35, 0.35);
pub const BUTTON_HOVER: Color = Color::rgb(0.55, 0.55, 0.68);
pub const BUTTON_DISABLED: Color = Color::rgb(0.22, 0.22, 0.22);
pub const BUTTON_BORDER: Color = Color::rgb(0.08, 0.08, 0.08);
pub const TEXT_PRIMARY: Color = Color::WHITE;
pub const TEXT_DISABLED: Color = Color::rgb(0.6, 0.6, 0.6);
pub const TEXT_FIELD_BG: Color = Color::rgb(0.1, 0.1, 0.1);
pub const TEXT_FIELD_FOCUSED_BORDER: Color = Color::rgb(0.8, 0.8, 0.4);
pub const HUD_CROSSHAIR: Color = Color::rgba(1.0, 1.0, 1.0, 0.85);
pub const HOTBAR_SLOT: Color = Color::rgba(0.15, 0.15, 0.15, 0.75);
pub const HOTBAR_BORDER: Color = Color::rgba(0.05, 0.05, 0.05, 0.9);
