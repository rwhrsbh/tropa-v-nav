//! Palette and fonts from the art bible (design doc, section 9).

use bevy::prelude::*;

pub const SOOT: Color = Color::srgb(0.078, 0.067, 0.059);
pub const PEAT: Color = Color::srgb(0.169, 0.141, 0.125);
pub const BONE: Color = Color::srgb(0.851, 0.812, 0.722);
pub const RUST_BLOOD: Color = Color::srgb(0.478, 0.118, 0.078);
pub const FRESH_BLOOD: Color = Color::srgb(0.659, 0.149, 0.106);
pub const SWAMP: Color = Color::srgb(0.243, 0.290, 0.180);
pub const ROTLIGHT: Color = Color::srgb(0.608, 0.722, 0.416);
pub const CANDLE: Color = Color::srgb(0.847, 0.604, 0.235);
pub const NAV_BLUE: Color = Color::srgb(0.290, 0.416, 0.549);
pub const FROST: Color = Color::srgb(0.725, 0.800, 0.847);
pub const DIM: Color = Color::srgb(0.55, 0.52, 0.46);

/// Hero frame and player colour.
pub fn hero_color(hero: &str) -> Color {
    match hero {
        "bogatyr" => Color::srgb(0.60, 0.36, 0.20),
        "vorozheya" => Color::srgb(0.30, 0.42, 0.24),
        "volhv" => Color::srgb(0.45, 0.66, 0.82),
        "volkolak" => Color::srgb(0.62, 0.10, 0.12),
        "znaharka" => Color::srgb(0.85, 0.62, 0.22),
        "skomoroh" => Color::srgb(0.48, 0.36, 0.56),
        "emelya" => Color::srgb(0.78, 0.74, 0.62),
        _ => DIM,
    }
}

/// A player's colour on the map's parchment: their hero's own, a little deeper so it reads on the
/// pale paper. What they draw there is in it, and their token is framed in it.
pub fn map_color(hero: &str) -> Color {
    match hero {
        // Yemelya's pale linen would vanish on parchment: the same hue, much darker
        "emelya" => Color::srgb(0.50, 0.45, 0.30),
        _ => hero_color(hero).darker(0.08),
    }
}

#[derive(Resource, Clone)]
pub struct Fonts {
    pub title: Handle<Font>,
    pub body: Handle<Font>,
    pub num: Handle<Font>,
}

pub fn load_fonts(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(Fonts {
        title: assets.load("fonts/NavDisplay-Regular.ttf"),
        body: assets.load("fonts/PT_Serif-Web-Regular.ttf"),
        num: assets.load("fonts/AlegreyaSC-Bold.ttf"),
    });
}

pub fn text(s: impl Into<String>, font: &Handle<Font>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(s),
        TextFont { font: font.clone().into(), font_size: FontSize::Px(size), ..default() },
        TextColor(color),
        Pickable::IGNORE,
    )
}

/// A wood-and-iron panel: the stock container for HUD pieces.
pub fn panel(node: Node) -> impl Bundle {
    panel_edged(node, Color::srgb(0.32, 0.26, 0.20))
}

/// A panel with its border in a chosen colour.
pub fn panel_edged(node: Node, edge: Color) -> impl Bundle {
    (node, BackgroundColor(PEAT.with_alpha(0.92)), BorderColor::all(edge))
}
