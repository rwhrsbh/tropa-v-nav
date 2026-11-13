//! Escape in a run opens a small menu over it: back to the run, out to the main menu (the run is
//! kept: alone it waits under "Continue", in a party the others carry on and the host keeps it
//! saved), the settings laid over it, or out of the game.

use crate::l10n::L10n;
use crate::menu::menu_button;
use crate::net::Net;
use crate::theme::{self, Fonts};
use crate::{AppState, Stage};
use bevy::prelude::*;
use bevy::ui_widgets::{observe, Activate};

pub fn plugin(app: &mut App) {
    app.add_systems(Update, toggle.run_if(resource_exists::<Net>))
        .add_systems(OnExit(AppState::Map), close)
        .add_systems(OnExit(AppState::Battle), close)
        .add_systems(OnExit(AppState::Reward), close)
        .add_systems(OnExit(AppState::Rest), close)
        .add_systems(OnExit(AppState::Shop), close)
        .add_systems(OnExit(AppState::Omen), close);
}

#[derive(Component)]
pub(crate) struct PauseMenu;

fn in_run(state: AppState) -> bool {
    matches!(state, AppState::Map | AppState::Battle | AppState::Reward | AppState::Rest | AppState::Shop | AppState::Omen)
}

fn close(mut commands: Commands, open: Query<Entity, Or<(With<PauseMenu>, With<crate::menu::SettingsOverlay>)>>) {
    for e in &open {
        commands.entity(e).despawn();
    }
}

#[allow(clippy::too_many_arguments)]
fn toggle(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<AppState>>,
    open: Query<Entity, With<PauseMenu>>,
    piles: Query<(), Or<(With<crate::battle::piles::PileView>, With<crate::run_ui::MapPeek>)>>,
    settings: Query<Entity, With<crate::menu::SettingsOverlay>>,
    stage: Single<Entity, With<Stage>>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
    net: Res<Net>,
) {
    if !keys.just_pressed(KeyCode::Escape) || !in_run(*state.get()) {
        return;
    }
    // out of the settings, back to the pause menu under them
    if !settings.is_empty() {
        for e in &settings {
            commands.entity(e).despawn();
        }
        return;
    }
    if !open.is_empty() {
        for e in &open {
            commands.entity(e).despawn();
        }
        return;
    }
    // Escape closes an open pile, or the map looked at in a fight, first
    if !piles.is_empty() {
        return;
    }
    let party = net.node.lobby.players.len() > 1 && net.locals.is_empty();
    let hint = l10n.t(if party { "pause-leave-party-hint" } else { "pause-leave-solo-hint" });
    commands
        .spawn((
            PauseMenu,
            Node {
                position_type: PositionType::Absolute,
                width: px(crate::STAGE_W),
                height: px(crate::STAGE_H),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: px(18),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.02, 0.03, 0.88)),
            // over everything a run screen lays on top, the lot's rings and the map's marks too
            GlobalZIndex(1000),
            ChildOf(*stage),
        ))
        .with_children(|p| {
            p.spawn(theme::text(l10n.t("pause-title"), &fonts.title, 72.0, theme::BONE));
            p.spawn((
                menu_button(l10n.t("pause-resume"), &fonts),
                observe(|_: On<Activate>, mut commands: Commands, open: Query<Entity, With<PauseMenu>>| {
                    for e in &open {
                        commands.entity(e).despawn();
                    }
                }),
            ));
            p.spawn((
                menu_button(l10n.t("menu-settings"), &fonts),
                observe(|_: On<Activate>, mut commands: Commands| commands.run_system_cached(crate::menu::open_settings_overlay)),
            ));
            p.spawn((
                menu_button(l10n.t("pause-to-menu"), &fonts),
                observe(|_: On<Activate>, mut commands: Commands, net: Option<Res<Net>>, steam: Option<Res<crate::steam::Steam>>, open: Query<Entity, With<PauseMenu>>, mut next: ResMut<NextState<AppState>>| {
                    for e in &open {
                        commands.entity(e).despawn();
                    }
                    if let Some(net) = net {
                        info!("left the run for the main menu");
                        crate::net::leave(&mut commands, steam.as_deref(), &net);
                    }
                    commands.remove_resource::<crate::session::Session>();
                    next.set(AppState::Menu);
                }),
            ));
            p.spawn((theme::text(hint, &fonts.body, 22.0, theme::DIM), Node { max_width: px(760), ..default() }, TextLayout::justify(Justify::Center)));
            p.spawn((
                menu_button(l10n.t("pause-quit"), &fonts),
                observe(|_: On<Activate>, mut exit: MessageWriter<AppExit>| {
                    exit.write(AppExit::Success);
                }),
            ));
        });
}
