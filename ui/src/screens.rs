//! The Minecraft-styled screen flow: main menu -> (multiplayer address
//! entry | texture pack picker) -> back to menu, plus a separate `Hud`
//! overlay drawn during gameplay. Each screen only knows how to lay
//! itself out against the current viewport, turn input into an
//! `Action`, and draw — the host (`app-desktop`) owns the actual game
//! session, network calls, and GPU state those actions drive.

use texturepacks::modrinth::SearchHit;

use crate::draw_list::Painter;
use crate::geometry::{Color, Rect};
use crate::input::UiInput;
use crate::style;
use crate::widgets::{Button, TextField};

const BUTTON_WIDTH: f32 = 240.0;
const BUTTON_HEIGHT: f32 = 32.0;
const BUTTON_GAP: f32 = 8.0;
/// Wider than the standard menu button — pack titles plus a download
/// count run longer than "Multiplayer"/"Quit" (truncated with "..." by
/// `Button::draw` if a title is still too long even at this width).
const PICKER_RESULT_WIDTH: f32 = 560.0;

/// What the host should do in response to this frame's input —
/// `Screen::update` returns exactly one, `None` most frames.
pub enum Action {
    None,
    Quit,
    StartSingleplayer,
    GoToMultiplayer,
    GoToTexturePacks,
    Connect { host: String, username: String },
    RequestTexturePackSearch,
    SelectTexturePack { index: usize },
    BackToMenu,
}

pub enum Screen {
    MainMenu,
    Multiplayer(MultiplayerScreen),
    TexturePackPicker(TexturePackPickerScreen),
}

impl Screen {
    pub fn update(&mut self, input: &UiInput, viewport: (f32, f32)) -> Action {
        match self {
            Screen::MainMenu => main_menu_update(input, viewport),
            Screen::Multiplayer(s) => s.update(input, viewport),
            Screen::TexturePackPicker(s) => s.update(input, viewport),
        }
    }

    pub fn draw(&self, painter: &mut Painter, viewport: (f32, f32), mouse: (f32, f32)) {
        match self {
            Screen::MainMenu => main_menu_draw(painter, viewport, mouse),
            Screen::Multiplayer(s) => s.draw(painter, viewport, mouse),
            Screen::TexturePackPicker(s) => s.draw(painter, viewport, mouse),
        }
    }
}

fn button_column(viewport: (f32, f32), count: usize, start_y: f32) -> Vec<Rect> {
    button_column_width(viewport, count, start_y, BUTTON_WIDTH)
}

fn button_column_width(viewport: (f32, f32), count: usize, start_y: f32, width: f32) -> Vec<Rect> {
    let x = viewport.0 / 2.0 - width / 2.0;
    (0..count)
        .map(|i| Rect::new(x, start_y + i as f32 * (BUTTON_HEIGHT + BUTTON_GAP), width, BUTTON_HEIGHT))
        .collect()
}

const MAIN_MENU_LABELS: [&str; 4] = ["Singleplayer", "Multiplayer", "Texture Packs", "Quit"];
const MAIN_MENU_ACTIONS: [Action; 4] = [
    Action::StartSingleplayer,
    Action::GoToMultiplayer,
    Action::GoToTexturePacks,
    Action::Quit,
];

fn main_menu_buttons(viewport: (f32, f32)) -> Vec<Button> {
    let rects = button_column(viewport, MAIN_MENU_LABELS.len(), viewport.1 / 2.0 - 40.0);
    MAIN_MENU_LABELS.iter().zip(rects).map(|(label, rect)| Button::new(rect, *label)).collect()
}

fn main_menu_update(input: &UiInput, viewport: (f32, f32)) -> Action {
    for (button, action) in main_menu_buttons(viewport).into_iter().zip(MAIN_MENU_ACTIONS) {
        if button.clicked(input) {
            return action;
        }
    }
    Action::None
}

fn main_menu_draw(painter: &mut Painter, viewport: (f32, f32), mouse: (f32, f32)) {
    painter.text_centered("Cobble", viewport.0 / 2.0, viewport.1 / 2.0 - 120.0, style::TEXT_PRIMARY);
    for button in main_menu_buttons(viewport) {
        button.draw(painter, mouse);
    }
}

pub struct MultiplayerScreen {
    pub host_field: TextField,
    pub username_field: TextField,
}

impl MultiplayerScreen {
    pub fn new(viewport: (f32, f32), default_username: &str) -> Self {
        let cx = viewport.0 / 2.0 - BUTTON_WIDTH / 2.0;
        let cy = viewport.1 / 2.0;
        let mut username_field = TextField::new(Rect::new(cx, cy - 60.0, BUTTON_WIDTH, BUTTON_HEIGHT), "username");
        username_field.value = default_username.to_string();
        Self {
            host_field: TextField::new(Rect::new(cx, cy - 20.0, BUTTON_WIDTH, BUTTON_HEIGHT), "server address"),
            username_field,
        }
    }

    fn buttons(&self, viewport: (f32, f32)) -> [Button; 2] {
        let cx = viewport.0 / 2.0 - BUTTON_WIDTH / 2.0;
        let cy = viewport.1 / 2.0;
        let mut connect = Button::new(Rect::new(cx, cy + 30.0, BUTTON_WIDTH, BUTTON_HEIGHT), "Connect");
        connect.enabled = !self.host_field.value.trim().is_empty();
        let back = Button::new(Rect::new(cx, cy + 30.0 + BUTTON_HEIGHT + BUTTON_GAP, BUTTON_WIDTH, BUTTON_HEIGHT), "Back");
        [connect, back]
    }

    pub fn update(&mut self, input: &UiInput, viewport: (f32, f32)) -> Action {
        self.host_field.rect.x = viewport.0 / 2.0 - BUTTON_WIDTH / 2.0;
        self.username_field.rect.x = self.host_field.rect.x;

        let host_confirmed = self.host_field.update(input);
        let username_confirmed = self.username_field.update(input);

        let [connect, back] = self.buttons(viewport);
        let host = self.host_field.value.trim().to_string();
        let username = self.username_field.value.trim().to_string();

        if back.clicked(input) {
            return Action::BackToMenu;
        }
        if !host.is_empty() && (connect.clicked(input) || host_confirmed || username_confirmed) {
            return Action::Connect {
                host,
                username: if username.is_empty() { "Cobble".to_string() } else { username },
            };
        }
        Action::None
    }

    pub fn draw(&self, painter: &mut Painter, viewport: (f32, f32), mouse: (f32, f32)) {
        painter.text_centered("Multiplayer", viewport.0 / 2.0, viewport.1 / 2.0 - 100.0, style::TEXT_PRIMARY);
        self.username_field.draw(painter);
        self.host_field.draw(painter);
        for button in self.buttons(viewport) {
            button.draw(painter, mouse);
        }
    }
}

pub enum PickerStatus {
    Loading,
    Loaded(Vec<SearchHit>),
    Downloading { title: String },
    PackReady { title: String, coverage_percent: f32 },
    Error(String),
}

pub struct TexturePackPickerScreen {
    pub status: PickerStatus,
}

impl TexturePackPickerScreen {
    pub fn new() -> Self {
        Self {
            status: PickerStatus::Loading,
        }
    }

    fn back_button(viewport: (f32, f32)) -> Button {
        Button::new(
            Rect::new(viewport.0 / 2.0 - BUTTON_WIDTH / 2.0, viewport.1 - 60.0, BUTTON_WIDTH, BUTTON_HEIGHT),
            "Back",
        )
    }

    fn result_buttons(&self, viewport: (f32, f32)) -> Vec<Button> {
        let PickerStatus::Loaded(hits) = &self.status else {
            return Vec::new();
        };
        let rects = button_column_width(viewport, hits.len(), 100.0, PICKER_RESULT_WIDTH);
        hits.iter()
            .zip(rects)
            .map(|(hit, rect)| Button::new(rect, format!("{} ({} downloads)", hit.title, hit.downloads)))
            .collect()
    }

    pub fn update(&mut self, input: &UiInput, viewport: (f32, f32)) -> Action {
        if Self::back_button(viewport).clicked(input) {
            return Action::BackToMenu;
        }
        for (index, button) in self.result_buttons(viewport).into_iter().enumerate() {
            if button.clicked(input) {
                return Action::SelectTexturePack { index };
            }
        }
        Action::None
    }

    pub fn draw(&self, painter: &mut Painter, viewport: (f32, f32), mouse: (f32, f32)) {
        painter.text_centered("Texture Packs", viewport.0 / 2.0, 40.0, style::TEXT_PRIMARY);
        match &self.status {
            PickerStatus::Loading => {
                painter.text_centered("Searching Modrinth...", viewport.0 / 2.0, viewport.1 / 2.0, style::TEXT_DISABLED);
            }
            PickerStatus::Loaded(hits) if hits.is_empty() => {
                painter.text_centered("No 1.8.9 resource packs found.", viewport.0 / 2.0, viewport.1 / 2.0, style::TEXT_DISABLED);
            }
            PickerStatus::Loaded(_) => {
                for button in self.result_buttons(viewport) {
                    button.draw(painter, mouse);
                }
            }
            PickerStatus::Downloading { title } => {
                painter.text_centered(&format!("Downloading {title}..."), viewport.0 / 2.0, viewport.1 / 2.0, style::TEXT_DISABLED);
            }
            PickerStatus::PackReady { title, coverage_percent } => {
                painter.text_centered(
                    &format!("Using {title} ({coverage_percent:.0}% coverage)"),
                    viewport.0 / 2.0,
                    viewport.1 / 2.0,
                    style::TEXT_PRIMARY,
                );
            }
            PickerStatus::Error(message) => {
                painter.text_centered(message, viewport.0 / 2.0, viewport.1 / 2.0, Color::rgb(0.9, 0.3, 0.3));
            }
        }
        Self::back_button(viewport).draw(painter, mouse);
    }
}

impl Default for TexturePackPickerScreen {
    fn default() -> Self {
        Self::new()
    }
}

/// Minimal in-game overlay: crosshair + hotbar outline. No inventory
/// state to actually show yet, so the hotbar is just 9 empty slots.
pub fn draw_hud(painter: &mut Painter, viewport: (f32, f32)) {
    let (cx, cy) = (viewport.0 / 2.0, viewport.1 / 2.0);
    let crosshair_len = 8.0;
    let crosshair_thickness = 2.0;
    painter.rect(
        Rect::new(cx - crosshair_len / 2.0, cy - crosshair_thickness / 2.0, crosshair_len, crosshair_thickness),
        style::HUD_CROSSHAIR,
    );
    painter.rect(
        Rect::new(cx - crosshair_thickness / 2.0, cy - crosshair_len / 2.0, crosshair_thickness, crosshair_len),
        style::HUD_CROSSHAIR,
    );

    let slot_size = 40.0;
    let slot_gap = 4.0;
    let hotbar_width = 9.0 * slot_size + 8.0 * slot_gap;
    let start_x = cx - hotbar_width / 2.0;
    let y = viewport.1 - slot_size - 12.0;
    for i in 0..9 {
        let rect = Rect::new(start_x + i as f32 * (slot_size + slot_gap), y, slot_size, slot_size);
        painter.rect(rect, style::HOTBAR_SLOT);
        painter.border(rect, 2.0, style::HOTBAR_BORDER);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIEWPORT: (f32, f32) = (1280.0, 720.0);

    fn click_at(mouse: (f32, f32)) -> UiInput {
        UiInput {
            mouse_pos: mouse,
            clicked: true,
            ..Default::default()
        }
    }

    fn button_center(viewport: (f32, f32), index: usize, start_y: f32) -> (f32, f32) {
        let rect = button_column(viewport, index + 1, start_y)[index];
        (rect.center_x(), rect.y + rect.h / 2.0)
    }

    #[test]
    fn each_main_menu_button_yields_its_own_action() {
        let start_y = VIEWPORT.1 / 2.0 - 40.0;
        for (index, expect_quit) in [(0, false), (1, false), (2, false), (3, true)] {
            let mouse = button_center(VIEWPORT, index, start_y);
            let action = main_menu_update(&click_at(mouse), VIEWPORT);
            match (index, action) {
                (0, Action::StartSingleplayer) => {}
                (1, Action::GoToMultiplayer) => {}
                (2, Action::GoToTexturePacks) => {}
                (3, Action::Quit) => assert!(expect_quit),
                (i, _) => panic!("button {i} produced the wrong action"),
            }
        }
    }

    #[test]
    fn clicking_empty_space_on_main_menu_does_nothing() {
        let action = main_menu_update(&click_at((5.0, 5.0)), VIEWPORT);
        assert!(matches!(action, Action::None));
    }

    #[test]
    fn multiplayer_connect_requires_a_non_empty_host() {
        let mut screen = MultiplayerScreen::new(VIEWPORT, "Steve");
        let [connect, _back] = screen.buttons(VIEWPORT);
        let action = screen.update(&click_at((connect.rect.center_x(), connect.rect.y + 1.0)), VIEWPORT);
        assert!(matches!(action, Action::None), "empty host should not connect");

        screen.host_field.value = "localhost".to_string();
        let [connect, _back] = screen.buttons(VIEWPORT);
        let action = screen.update(&click_at((connect.rect.center_x(), connect.rect.y + 1.0)), VIEWPORT);
        match action {
            Action::Connect { host, username } => {
                assert_eq!(host, "localhost");
                assert_eq!(username, "Steve");
            }
            _ => panic!("expected Connect"),
        }
    }

    #[test]
    fn multiplayer_back_button_returns_to_menu() {
        let mut screen = MultiplayerScreen::new(VIEWPORT, "Steve");
        let [_connect, back] = screen.buttons(VIEWPORT);
        let action = screen.update(&click_at((back.rect.center_x(), back.rect.y + 1.0)), VIEWPORT);
        assert!(matches!(action, Action::BackToMenu));
    }

    fn sample_hits(n: usize) -> Vec<SearchHit> {
        (0..n)
            .map(|i| SearchHit {
                project_id: format!("id{i}"),
                slug: format!("pack{i}"),
                title: format!("Pack {i}"),
                description: String::new(),
                downloads: i as u64,
                icon_url: None,
            })
            .collect()
    }

    #[test]
    fn selecting_a_result_reports_its_index() {
        let mut screen = TexturePackPickerScreen::new();
        screen.status = PickerStatus::Loaded(sample_hits(3));
        let rects = button_column(VIEWPORT, 3, 100.0);
        let target = rects[1];
        let action = screen.update(&click_at((target.center_x(), target.y + 1.0)), VIEWPORT);
        match action {
            Action::SelectTexturePack { index } => assert_eq!(index, 1),
            _ => panic!("expected SelectTexturePack"),
        }
    }

    #[test]
    fn picker_back_button_works_regardless_of_status() {
        let mut screen = TexturePackPickerScreen::new();
        let back = TexturePackPickerScreen::back_button(VIEWPORT);
        let action = screen.update(&click_at((back.rect.center_x(), back.rect.y + 1.0)), VIEWPORT);
        assert!(matches!(action, Action::BackToMenu));
    }

    #[test]
    fn drawing_every_picker_status_does_not_panic() {
        let font = crate::font::Font::load_regular(16.0);
        let mut painter = Painter::new(&font);
        for status in [
            PickerStatus::Loading,
            PickerStatus::Loaded(sample_hits(2)),
            PickerStatus::Loaded(Vec::new()),
            PickerStatus::Downloading { title: "Foo".into() },
            PickerStatus::PackReady { title: "Foo".into(), coverage_percent: 42.0 },
            PickerStatus::Error("boom".into()),
        ] {
            let screen = TexturePackPickerScreen { status };
            screen.draw(&mut painter, VIEWPORT, (0.0, 0.0));
        }
    }

    #[test]
    fn hud_drawing_does_not_panic() {
        let font = crate::font::Font::load_regular(16.0);
        let mut painter = Painter::new(&font);
        draw_hud(&mut painter, VIEWPORT);
        assert!(!painter.list.quads.is_empty());
    }
}
