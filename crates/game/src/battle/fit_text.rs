//! A card's rules text is measured once it is laid out and shrunk until it stays on the card, so
//! no card and no language has to be fitted by hand.

use bevy::prelude::*;

/// Height left for the rules text under the name, the picture and the type line.
pub(super) const ROOM: f32 = 90.0;
/// Smallest size still worth reading: a text too long for this is reported, not hidden.
const SMALLEST: f32 = 9.0;

#[derive(Component)]
pub(super) struct FitText;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, fit);
}

/// The size that should bring a text this high into the room. Lines grow with the size both ways,
/// taller and wrapping sooner, so the height goes with its square.
fn fitted(size: f32, height: f32) -> f32 {
    (size * (ROOM / height).sqrt()).min(size - 0.25).max(SMALLEST)
}

fn fit(mut commands: Commands, mut texts: Query<(Entity, &ComputedNode, &mut TextFont, &Text), With<FitText>>) {
    for (entity, node, mut font, text) in &mut texts {
        // the layout comes after the first frame
        let height = node.size().y * node.inverse_scale_factor();
        if height <= 0.0 {
            continue;
        }
        let FontSize::Px(size) = font.font_size else { continue };
        if height <= ROOM {
            continue;
        }
        if size <= SMALLEST {
            warn!("card text does not fit at the smallest size ({height:.0} high): {}", text.0);
            commands.entity(entity).remove::<FitText>();
            continue;
        }
        font.font_size = FontSize::Px(fitted(size, height));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_text_twice_too_high_loses_about_a_third_of_its_size() {
        let s = fitted(18.0, ROOM * 2.0);
        assert!((s - 18.0 / 2f32.sqrt()).abs() < 0.01, "{s}");
    }

    #[test]
    fn a_text_barely_over_still_shrinks_and_none_goes_below_the_smallest() {
        assert!(fitted(18.0, ROOM + 0.1) <= 17.75);
        assert_eq!(fitted(10.0, ROOM * 9.0), SMALLEST);
    }
}
