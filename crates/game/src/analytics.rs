//! In-game run telemetry, analytics aggregator, and Google Sheets dispatcher.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::l10n::L10n;
use crate::theme::{self, Fonts};
use crate::AppState;
use bevy::ui_widgets::{observe, Activate};

/// Telemetry payload sent for each completed run.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunTelemetry {
    pub version: String,
    pub run_id: u64,
    pub timestamp: u64,
    pub won: bool,
    pub circle: u8,
    pub act: u8,
    pub floor: u32,
    pub heroes: Vec<String>,
    pub final_deck: Vec<String>,
    pub relics: Vec<String>,
    pub cards_picked: BTreeMap<String, u32>,
    pub cards_played: BTreeMap<String, u32>,
    pub enemies_summary: BTreeMap<String, EnemyTelemetry>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EnemyTelemetry {
    pub damage_dealt: u32,
    pub rounds: u32,
    pub hero_kills: u32,
}

/// Aggregated card metrics calculated across recorded runs.
#[derive(Clone, Debug, Default)]
pub struct CardMetric {
    pub in_deck_count: u32,
    pub wins: u32,
    pub win_rate: f32,
    pub played_count: u32,
    pub is_imba: bool,
    pub is_weak: bool,
}

/// Aggregated enemy metrics calculated across recorded runs.
#[derive(Clone, Debug, Default)]
pub struct EnemyMetric {
    pub encounters: u32,
    pub total_damage: u32,
    pub avg_damage: f32,
    pub hero_kills: u32,
    pub avg_rounds: f32,
    pub is_deadly: bool,
    pub is_punching_bag: bool,
}

/// Summary report aggregated from local analytics history.
#[derive(Clone, Debug, Default)]
pub struct AnalyticsReport {
    pub total_runs: u32,
    pub total_wins: u32,
    pub win_rate: f32,
    pub cards: BTreeMap<String, CardMetric>,
    pub enemies: BTreeMap<String, EnemyMetric>,
}

pub fn analytics_file_path() -> Option<PathBuf> {
    crate::platform::user_dir().map(|d| d.join("profiles").join("analytics.jsonl"))
}

pub fn save_local(telemetry: &RunTelemetry) {
    let Some(path) = analytics_file_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(line) = serde_json::to_string(telemetry) {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
            let _ = writeln!(f, "{line}");
        }
    }
}

pub fn load_history() -> Vec<RunTelemetry> {
    let Some(path) = analytics_file_path() else { return Vec::new() };
    let Ok(content) = std::fs::read_to_string(&path) else { return Vec::new() };
    content.lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

pub fn compute_report() -> AnalyticsReport {
    let history = load_history();
    let total_runs = history.len() as u32;
    let total_wins = history.iter().filter(|r| r.won).count() as u32;
    let win_rate = if total_runs > 0 { (total_wins as f32 / total_runs as f32) * 100.0 } else { 0.0 };

    let mut cards: BTreeMap<String, CardMetric> = BTreeMap::new();
    let mut enemies: BTreeMap<String, EnemyMetric> = BTreeMap::new();

    for run in &history {
        for card in &run.final_deck {
            let m = cards.entry(card.clone()).or_default();
            m.in_deck_count += 1;
            if run.won {
                m.wins += 1;
            }
        }
        for (card, count) in &run.cards_played {
            let m = cards.entry(card.clone()).or_default();
            m.played_count += *count;
        }
        for (enemy, tel) in &run.enemies_summary {
            let e = enemies.entry(enemy.clone()).or_default();
            e.encounters += 1;
            e.total_damage += tel.damage_dealt;
            e.hero_kills += tel.hero_kills;
            e.avg_rounds += tel.rounds as f32;
        }
    }

    for (_, m) in &mut cards {
        if m.in_deck_count > 0 {
            m.win_rate = (m.wins as f32 / m.in_deck_count as f32) * 100.0;
        }
        if m.in_deck_count >= 3 && m.win_rate >= 65.0 {
            m.is_imba = true;
        } else if m.in_deck_count >= 3 && m.win_rate <= 35.0 {
            m.is_weak = true;
        }
    }

    for (_, e) in &mut enemies {
        if e.encounters > 0 {
            e.avg_damage = e.total_damage as f32 / e.encounters as f32;
            e.avg_rounds /= e.encounters as f32;
        }
        if e.hero_kills > 0 || e.avg_damage >= 35.0 {
            e.is_deadly = true;
        } else if e.avg_damage < 8.0 && e.avg_rounds < 2.0 {
            e.is_punching_bag = true;
        }
    }

    AnalyticsReport { total_runs, total_wins, win_rate, cards, enemies }
}

/// Asynchronously sends the telemetry payload to the remote HTTP endpoint (e.g., Google Sheets Web App).
pub fn send_telemetry_async(url: String, telemetry: RunTelemetry) {
    if url.trim().is_empty() {
        return;
    }
    std::thread::Builder::new()
        .name("telemetry_sender".into())
        .spawn(move || {
            let agent = ureq::AgentBuilder::new()
                .timeout(std::time::Duration::from_secs(5))
                .build();
            match agent.post(&url).send_json(&telemetry) {
                Ok(resp) => {
                    bevy::log::info!("Telemetry sent successfully: HTTP {}", resp.status());
                }
                Err(e) => {
                    bevy::log::warn!("Telemetry send failed (non-critical): {e}");
                }
            }
        })
        .ok();
}

/// Helper to record and dispatch telemetry when a run finishes.
pub fn record_run(run: &nav_core::Run, config_url: Option<&str>, won: bool) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let heroes: Vec<String> = run.players.iter().map(|p| p.hero.clone()).collect();
    let mut final_deck: Vec<String> = Vec::new();
    let mut relics: Vec<String> = Vec::new();
    for p in &run.players {
        for (card, upgraded) in &p.deck {
            final_deck.push(if *upgraded { format!("{card}+") } else { card.clone() });
        }
        relics.extend(p.relics.clone());
    }

    let tel = RunTelemetry {
        version: env!("CARGO_PKG_VERSION").into(),
        run_id: run.seed,
        timestamp: now,
        won,
        circle: run.circle,
        act: run.act,
        floor: run.floors_cleared,
        heroes,
        final_deck,
        relics,
        cards_picked: BTreeMap::new(),
        cards_played: BTreeMap::new(),
        enemies_summary: BTreeMap::new(),
    };

    save_local(&tel);

    let url = std::env::var("TROPA_TELEMETRY_URL")
        .ok()
        .or_else(|| config_url.map(|s| s.to_string()));

    if let Some(u) = url {
        send_telemetry_async(u, tel);
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Analytics), enter_analytics);
}

fn enter_analytics(
    mut commands: Commands,
    stage: Single<Entity, With<crate::Stage>>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
    art: Res<crate::art::Art>,
    assets: Res<AssetServer>,
) {
    let report = compute_report();
    commands.spawn((DespawnOnExit(AppState::Analytics), crate::menu::backdrop(&art, &assets, "bg/chronicle", 0.6), ChildOf(*stage)));
    commands.spawn(crate::menu::screen(AppState::Analytics, *stage)).with_children(|p| {
        p.spawn(theme::text("Аналитика и Баланс", &fonts.title, 56.0, theme::BONE));
        let summary = format!("Всего забегов: {}  ·  Побед: {} ({:.1}%)", report.total_runs, report.total_wins, report.win_rate);
        p.spawn(theme::text(summary, &fonts.body, 22.0, theme::CANDLE));

        p.spawn((
            Node {
                width: px(1700),
                height: px(720),
                flex_direction: FlexDirection::Row,
                column_gap: px(24),
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|cols| {
            // Left Column: Cards Meta
            cols.spawn((
                Node {
                    width: px(820),
                    height: px(700),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(px(16)),
                    border: UiRect::all(px(2)),
                    row_gap: px(8),
                    ..default()
                },
                BackgroundColor(theme::SOOT.with_alpha(0.85)),
                BorderColor::all(Color::srgb(0.35, 0.28, 0.20)),
            ))
            .with_children(|col| {
                col.spawn(theme::text("Мета Карт (Винрейт / Имба / Слабые)", &fonts.title, 26.0, theme::CANDLE));
                let mut sorted_cards: Vec<(&String, &CardMetric)> = report.cards.iter().collect();
                sorted_cards.sort_by(|a, b| b.1.win_rate.partial_cmp(&a.1.win_rate).unwrap_or(std::cmp::Ordering::Equal));
                for (id, m) in sorted_cards.into_iter().take(12) {
                    let badge = if m.is_imba {
                        " [🌟 ИМБА]"
                    } else if m.is_weak {
                        " [💀 СЛАБАЯ]"
                    } else {
                        ""
                    };
                    let color = if m.is_imba {
                        Color::srgb(0.4, 0.9, 0.4)
                    } else if m.is_weak {
                        theme::FRESH_BLOOD
                    } else {
                        theme::BONE
                    };
                    let line = format!("{id}{badge} · В колодах: {} · Винрейт: {:.0}%", m.in_deck_count, m.win_rate);
                    col.spawn(theme::text(line, &fonts.body, 17.0, color));
                }
            });

            // Right Column: Enemy Danger Balance
            cols.spawn((
                Node {
                    width: px(820),
                    height: px(700),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(px(16)),
                    border: UiRect::all(px(2)),
                    row_gap: px(8),
                    ..default()
                },
                BackgroundColor(theme::SOOT.with_alpha(0.85)),
                BorderColor::all(Color::srgb(0.35, 0.28, 0.20)),
            ))
            .with_children(|col| {
                col.spawn(theme::text("Баланс Врагов (Опасность / Смертоносность)", &fonts.title, 26.0, theme::CANDLE));
                let mut sorted_enemies: Vec<(&String, &EnemyMetric)> = report.enemies.iter().collect();
                sorted_enemies.sort_by(|a, b| b.1.avg_damage.partial_cmp(&a.1.avg_damage).unwrap_or(std::cmp::Ordering::Equal));
                for (id, e) in sorted_enemies.into_iter().take(12) {
                    let badge = if e.is_deadly {
                        " [☠️ ГРОЗА]"
                    } else if e.is_punching_bag {
                        " [🥊 ГРУША]"
                    } else {
                        ""
                    };
                    let color = if e.is_deadly {
                        theme::FRESH_BLOOD
                    } else if e.is_punching_bag {
                        Color::srgb(0.8, 0.8, 0.3)
                    } else {
                        theme::BONE
                    };
                    let line = format!("{id}{badge} · Встреч: {} · Ср. урон: {:.0} HP · Убийств: {}", e.encounters, e.avg_damage, e.hero_kills);
                    col.spawn(theme::text(line, &fonts.body, 17.0, color));
                }
            });
        });

        p.spawn((
            crate::menu::menu_button(l10n.t("coop-back"), &fonts),
            observe(|_: On<Activate>, mut next: ResMut<NextState<AppState>>| next.set(AppState::Chronicle)),
        ));
    });
}
