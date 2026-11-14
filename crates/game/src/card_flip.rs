//! A card looked over outside a fight turns over on the right mouse button, its upgraded self on
//! the other side, to see what an upgrade would do. And the right button never presses anything:
//! the buttons underneath take any mouse button for a click, and a card would be bought by it.

use crate::art::Art;
use crate::audio::{Mixer, Sfx};
use crate::battle::card_face;
use crate::l10n::L10n;
use crate::run_ui::CardHints;
use crate::session::Db;
use crate::theme::Fonts;
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::Button;
use nav_core::{Card, CardDef, Form};

/// Seconds of a whole turn over.
const TURN: f32 = 0.28;

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (only_left_presses, start_turning, turn));
}

#[derive(Component)]
pub struct CardFlip {
    card: Card,
    form: Form,
    /// Seconds into a turn under way, and the width it is shown at.
    turning: Option<(f32, f32)>,
    /// Which side it comes to at the turn's middle has been put on.
    swapped: bool,
}

impl CardFlip {
    pub fn new(card: Card, form: Form) -> CardFlip {
        CardFlip { card, form, turning: None, swapped: false }
    }
}

/// Whether the upgrade changes anything to show.
pub fn upgrades(def: &CardDef, form: Form, l10n: &L10n) -> bool {
    let (now, up) = (def.view(false, form), def.view(true, form));
    now.cost != up.cost || now.drowse != up.drowse || l10n.card_text(&def.id, def, &now) != l10n.card_text(&def.id, def, &up)
}

/// A press with the right button is not a press of the button under it.
fn only_left_presses(mouse: Res<ButtonInput<MouseButton>>, pressed: Query<Entity, (With<Button>, With<bevy::ui::Pressed>)>, mut commands: Commands) {
    if mouse.pressed(MouseButton::Right) && !mouse.pressed(MouseButton::Left) {
        for e in &pressed {
            commands.entity(e).remove::<bevy::ui::Pressed>();
        }
    }
}

fn start_turning(mouse: Res<ButtonInput<MouseButton>>, mut cards: Query<(&mut CardFlip, &Hovered, &UiTransform)>, sfx: Res<Sfx>, mixer: Res<Mixer>, mut commands: Commands) {
    if !mouse.just_pressed(MouseButton::Right) {
        return;
    }
    for (mut flip, hovered, tf) in &mut cards {
        if hovered.get() && flip.turning.is_none() {
            flip.turning = Some((0.0, tf.scale.x));
            flip.swapped = false;
            sfx.play(&mut commands, "card_draw", &mixer, 0.7);
        }
    }
}

/// How wide a card is `p` of the way through its turn: edge on at the middle.
fn width_at(p: f32) -> f32 {
    (p * std::f32::consts::PI).cos().abs()
}

#[allow(clippy::too_many_arguments)]
fn turn(
    time: Res<Time>,
    mut commands: Commands,
    mut cards: Query<(Entity, &mut CardFlip, &mut UiTransform, Option<&mut CardHints>)>,
    db: Res<Db>,
    l10n: Res<L10n>,
    fonts: Res<Fonts>,
    art: Res<Art>,
    assets: Res<AssetServer>,
) {
    for (entity, mut flip, mut tf, hints) in &mut cards {
        let Some((t, width)) = flip.turning else { continue };
        let t = t + time.delta_secs();
        let p = (t / TURN).min(1.0);
        tf.scale.x = width * width_at(p);
        if p >= 0.5 && !flip.swapped {
            flip.swapped = true;
            flip.card.upgraded = !flip.card.upgraded;
            let (card, form) = (flip.card.clone(), flip.form);
            if let Some(def) = db.0.cards.get(&card.def) {
                commands.entity(entity).despawn_children().with_children(|c| card_face(c, &card, def, form, false, &l10n, &fonts, &art, &assets));
                // what it tells on hover goes with the side it shows
                if let Some(mut hints) = hints {
                    // its panel was among the things cleared off the card
                    hints.open = None;
                    hints.lines = l10n.card_glossary(&def.view(card.upgraded, form));
                    hints.lines.push((l10n.t("card-flip-hint"), String::new()));
                }
            }
        }
        flip.turning = if p >= 1.0 {
            tf.scale.x = width;
            None
        } else {
            Some((t, width))
        };
    }
}

#[cfg(test)]
mod tests {
    use super::width_at;

    #[test]
    fn a_card_turns_edge_on_at_the_middle_and_comes_back_whole() {
        assert!((width_at(0.0) - 1.0).abs() < 1e-5);
        assert!(width_at(0.5) < 1e-5);
        assert!((width_at(1.0) - 1.0).abs() < 1e-5);
    }
}
