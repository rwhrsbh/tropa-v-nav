//! A card asked the player to pick a card out of a pile (an ally's hand, draw pile or discard,
//! or their own): the pile is spread out over the stage, and the card clicked is the one taken.
//! Nothing else can be done until it is.

use super::*;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, show_pick.run_if(in_state(AppState::Battle)).run_if(resource_exists::<Session>));
}

/// The spread-out pile to pick from, and which pick it was opened for.
#[derive(Component)]
pub(crate) struct PickView(u64);

/// A card in the spread that can be picked.
#[derive(Component)]
struct PickCard(u32);

#[allow(clippy::too_many_arguments)]
fn show_pick(
    mut commands: Commands,
    session: Res<Session>,
    db: Res<Db>,
    l10n: Res<L10n>,
    fonts: Res<Fonts>,
    art: Res<Art>,
    assets: Res<AssetServer>,
    layer: Single<Entity, With<FloatLayer>>,
    open: Query<(Entity, &PickView)>,
) {
    let h = session.my_hero();
    let key = h.picking.as_ref().map(|p| {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        p.hash(&mut hasher);
        hasher.finish()
    });
    // the view stays while the same pick waits; a new pick or none takes it away
    let mut shown = false;
    for (e, v) in &open {
        if Some(v.0) == key {
            shown = true;
        } else {
            commands.entity(e).despawn();
        }
    }
    let (Some(p), Some(key), false) = (h.picking.clone(), key, shown) else { return };
    let can = nav_core::craft::pickable_cards(&db.0, &session.truth, &p);
    let Some(owner) = session.truth.hero(p.owner) else { return };
    let pile = match p.pile {
        nav_core::Pile::Hand => &owner.hand,
        nav_core::Pile::Draw => &owner.draw,
        nav_core::Pile::Discard => &owner.discard,
    };
    let mut cards: Vec<Card> = pile.iter().filter(|c| can.contains(&c.uid)).cloned().collect();
    // a draw pile is shown by name, so its order stays secret, unless only its top is looked at
    if p.pile == nav_core::Pile::Draw && p.top == 0 {
        cards.sort_by_key(|c| (l10n.card_name(&c.def, c.upgraded), c.uid));
    } else if p.pile == nav_core::Pile::Draw {
        cards.reverse();
    }
    let k = if cards.len() <= 10 { 0.78 } else if cards.len() <= 24 { 0.62 } else { 0.48 };
    let who = l10n.t(&format!("hero-{}", owner.def));
    let title = l10n.tf(&format!("battle-pick-{:?}-{:?}", p.then, p.pile), &[("who", who.into())]);
    let root = commands
        .spawn((
            PickView(key),
            Node {
                position_type: PositionType::Absolute,
                width: px(STAGE_W),
                height: px(STAGE_H),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(px(60), px(40)),
                row_gap: px(18),
                ..default()
            },
            BackgroundColor(Color::BLACK.with_alpha(0.86)),
            GlobalZIndex(455),
            ChildOf(*layer),
        ))
        .id();
    commands.spawn((theme::text(title, &fonts.title, 40.0, theme::CANDLE), ChildOf(root)));
    let row = commands
        .spawn((
            Node { flex_wrap: FlexWrap::Wrap, justify_content: JustifyContent::Center, column_gap: px(12), row_gap: px(12), max_width: px(STAGE_W - 120.0), ..default() },
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();
    for card in &cards {
        let slot = commands.spawn((Node { width: px(CARD_W * k), height: px(CARD_H * k), ..default() }, Pickable::IGNORE, ChildOf(row))).id();
        let at = Vec2::new(-CARD_W * (1.0 - k) / 2.0, -CARD_H * (1.0 - k) / 2.0);
        let form = owner.form;
        let Some(e) = crate::l10n::with_ranges(|| spawn_card(&mut commands, slot, card, form, Some(at), false, (), &db.0, &l10n, &fonts, &art, &assets)) else { continue };
        commands
            .entity(e)
            .insert((UiTransform { scale: Vec2::splat(k), ..default() }, PickCard(card.uid), Hovered::default(), Button, Btn(crate::battle::card_frame(db.0.cards.get(&card.def).unwrap()))))
            .observe(|ev: On<PointerClick>, cards: Query<&PickCard>, mut outbox: ResMut<Outbox>| {
                if let Ok(c) = cards.get(ev.event_target()) {
                    outbox.0.push(CombatAction::Pick { card: c.0 });
                }
            });
        if let Some(def) = db.0.cards.get(&card.def) {
            commands.entity(e).insert(crate::run_ui::CardHints::new(l10n.card_glossary(&def.view(card.upgraded, form))));
        }
    }
}
