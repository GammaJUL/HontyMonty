//! In-game HUD: crosshair, vitals, ammo, timer/score, kill feed, messages,
//! scope/flash/damage overlays and the scoreboard.

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use super::combat::{Arsenal, Held};
use super::effects::ScreenEffects;
use super::player::LocalPlayer;
use super::{GameSet, GameUi, InGameEntity};
use crate::game_mode::{Phase, Team, FFA_KILL_LIMIT};
use crate::keybinds::{Action, Controls};
use crate::net::NetClient;
use crate::settings::Settings;
use crate::states::AppState;
use crate::ui::settings_menu::SettingsOpen;
use crate::ui::spawn_crosshair_lines;
use crate::ui::widgets::*;
use crate::weapons::{ScopeStyle, WeaponId};

#[derive(Resource, Default)]
pub struct HudFeed {
    kills: Vec<(String, f32, bool)>,
    infos: Vec<(String, f32)>,
    center: Option<(String, f32)>,
    pub killed_by: Option<String>,
    now: f32,
}

impl HudFeed {
    pub fn kill(&mut self, text: String, highlight: bool) {
        self.kills.push((text, self.now + 6.0, highlight));
        if self.kills.len() > 6 {
            self.kills.remove(0);
        }
    }
    pub fn info(&mut self, text: String) {
        self.infos.push((text, self.now + 6.0));
        if self.infos.len() > 6 {
            self.infos.remove(0);
        }
    }
    pub fn center(&mut self, text: String, secs: f32) {
        self.center = Some((text, self.now + secs));
    }
}

#[derive(Resource)]
struct ScopeImages {
    sniper: Handle<Image>,
    rifle: Handle<Image>,
}

#[derive(Component)]
enum HudText {
    Top,
    Vitals,
    Money,
    Ammo,
    Weapon,
    Grenades,
    KillFeed,
    Info,
    Center,
    Bottom,
}

#[derive(Component)]
struct CrosshairRoot;

#[derive(Component)]
struct HitMarker;

#[derive(Component)]
enum Overlay {
    Flash,
    Damage,
}

#[derive(Component)]
struct ScopeOverlay(ScopeStyle);

#[derive(Component)]
struct Scoreboard;

pub fn plugin(app: &mut App) {
    app.init_resource::<HudFeed>()
        .add_systems(Startup, make_scope_images)
        .add_systems(OnEnter(AppState::InGame), spawn_hud)
        .add_systems(
            Update,
            (update_texts, update_crosshair, update_overlays, scoreboard).in_set(GameSet::Present),
        );
}

fn scope_image(inner: f32, outside_alpha: u8, lines: bool) -> Image {
    let n = 512u32;
    let mut data = Vec::with_capacity((n * n * 4) as usize);
    for y in 0..n {
        for x in 0..n {
            let dx = x as f32 / n as f32 - 0.5;
            let dy = y as f32 / n as f32 - 0.5;
            let r = (dx * dx + dy * dy).sqrt();
            let edge = ((r - inner) / 0.006).clamp(0.0, 1.0);
            let mut a = (edge * outside_alpha as f32) as u8;
            if lines && (dx.abs() < 0.0012 || dy.abs() < 0.0012) && r < inner {
                a = 255;
            }
            if !lines && r < 0.004 {
                // Red dot for rifle scopes.
                data.extend_from_slice(&[255, 30, 30, 255]);
                continue;
            }
            data.extend_from_slice(&[0, 0, 0, a]);
        }
    }
    Image::new(
        Extent3d {
            width: n,
            height: n,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

fn make_scope_images(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    commands.insert_resource(ScopeImages {
        sniper: images.add(scope_image(0.49, 255, true)),
        rifle: images.add(scope_image(0.3, 200, false)),
    });
}

fn text(kind: HudText, size: f32, node: Node) -> impl Bundle {
    (kind, Text::new(""), font(size), TextColor(TEXT), node, TextShadow::default())
}

fn abs(left: Val, top: Val, right: Val, bottom: Val) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left,
        top,
        right,
        bottom,
        ..default()
    }
}

fn spawn_hud(mut commands: Commands, scopes: Res<ScopeImages>, mut feed: ResMut<HudFeed>) {
    *feed = HudFeed::default();
    commands
        .spawn((InGameEntity, screen_root()))
        .with_children(|root| {
            // Scope overlays (behind everything else in the HUD).
            for (style, img) in [(ScopeStyle::Sniper, &scopes.sniper), (ScopeStyle::Rifle, &scopes.rifle)] {
                root.spawn((
                    ScopeOverlay(style),
                    Visibility::Hidden,
                    Node {
                        width: percent(100),
                        height: percent(100),
                        position_type: PositionType::Absolute,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                ))
                .with_children(|s| {
                    let bar = Color::srgba(0.0, 0.0, 0.0, if style == ScopeStyle::Sniper { 1.0 } else { 0.78 });
                    s.spawn((Node { flex_grow: 1.0, height: percent(100), ..default() }, BackgroundColor(bar)));
                    s.spawn((
                        ImageNode::new(img.clone()),
                        Node {
                            height: percent(100),
                            aspect_ratio: Some(1.0),
                            ..default()
                        },
                    ));
                    s.spawn((Node { flex_grow: 1.0, height: percent(100), ..default() }, BackgroundColor(bar)));
                });
            }
            root.spawn((
                Overlay::Damage,
                Node {
                    width: percent(100),
                    height: percent(100),
                    position_type: PositionType::Absolute,
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ));
            root.spawn((
                CrosshairRoot,
                abs(percent(50), percent(50), Val::Auto, Val::Auto),
            ));
            root.spawn((
                HitMarker,
                Text::new("X"),
                font(34.0),
                TextColor(Color::NONE),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(50.0),
                    top: Val::Percent(50.0),
                    margin: UiRect::new(px(-9), px(0), px(-22), px(0)),
                    ..default()
                },
            ));
            root.spawn(text(HudText::Top, 26.0, Node {
                position_type: PositionType::Absolute,
                top: px(10),
                width: percent(100),
                justify_content: JustifyContent::Center,
                ..default()
            }))
            .insert(TextLayout::justify(Justify::Center));
            root.spawn(text(HudText::Center, 34.0, Node {
                position_type: PositionType::Absolute,
                top: percent(22),
                width: percent(100),
                ..default()
            }))
            .insert((TextLayout::justify(Justify::Center), TextColor(ACCENT)));
            root.spawn(text(HudText::Bottom, 20.0, Node {
                position_type: PositionType::Absolute,
                bottom: px(110),
                width: percent(100),
                ..default()
            }))
            .insert(TextLayout::justify(Justify::Center));
            root.spawn(text(HudText::Vitals, 30.0, abs(px(24), Val::Auto, Val::Auto, px(20))));
            root.spawn(text(HudText::Money, 26.0, abs(px(24), Val::Auto, Val::Auto, px(62))))
                .insert(TextColor(Color::srgb(0.5, 1.0, 0.5)));
            root.spawn(text(HudText::Ammo, 34.0, abs(Val::Auto, Val::Auto, px(24), px(20))));
            root.spawn(text(HudText::Weapon, 20.0, abs(Val::Auto, Val::Auto, px(24), px(64))));
            root.spawn(text(HudText::Grenades, 16.0, abs(Val::Auto, Val::Auto, px(24), px(92))))
                .insert(TextColor(TEXT_DIM));
            root.spawn(text(HudText::KillFeed, 16.0, abs(Val::Auto, px(28), px(12), Val::Auto)))
                .insert(TextLayout::justify(Justify::Right));
            root.spawn(text(HudText::Info, 15.0, abs(px(16), percent(40), Val::Auto, Val::Auto)))
                .insert(TextColor(TEXT_DIM));
            root.spawn((
                Overlay::Flash,
                Node {
                    width: percent(100),
                    height: percent(100),
                    position_type: PositionType::Absolute,
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ));
            root.spawn((
                Scoreboard,
                Visibility::Hidden,
                Node {
                    position_type: PositionType::Absolute,
                    left: percent(20),
                    top: percent(12),
                    width: percent(60),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(px(16)),
                    row_gap: px(4),
                    border_radius: BorderRadius::all(px(8)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.03, 0.04, 0.06, 0.9)),
            ));
        });
}

fn fmt_time(secs: f32) -> String {
    let s = secs.max(0.0).ceil() as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

#[allow(clippy::too_many_arguments)]
fn update_texts(
    time: Res<Time>,
    client: Option<Res<NetClient>>,
    arsenal: Res<Arsenal>,
    mut feed: ResMut<HudFeed>,
    controls: Controls,
    mut q: Query<(&HudText, &mut Text)>,
) {
    feed.now = time.elapsed_secs();
    let now = feed.now;
    feed.kills.retain(|k| k.1 > now);
    feed.infos.retain(|k| k.1 > now);
    if feed.center.as_ref().is_some_and(|c| c.1 <= now) {
        feed.center = None;
    }
    let Some(c) = client else { return };
    let Some(snap) = &c.snapshot else { return };
    let Some(me) = c.me() else { return };
    let ffa = snap.lobby.mode.is_ffa();
    let buy_key = controls.binding_name(Action::BuyMenu);

    for (kind, mut t) in &mut q {
        let s = match kind {
            HudText::Top => {
                let clock = match snap.phase {
                    Phase::Freeze => format!("BUY  {}", fmt_time(snap.phase_time_left)),
                    Phase::Live => fmt_time(snap.phase_time_left),
                    Phase::RoundEnd => "ROUND OVER".into(),
                    Phase::MatchEnd => "MATCH OVER".into(),
                    Phase::Lobby => String::new(),
                };
                if ffa {
                    let leader = snap.players.iter().max_by_key(|p| p.kills);
                    format!(
                        "{clock}\nYou: {} / {FFA_KILL_LIMIT}   |   Leader: {} ({})",
                        me.kills,
                        leader.map(|p| p.name.as_str()).unwrap_or("-"),
                        leader.map(|p| p.kills).unwrap_or(0)
                    )
                } else {
                    let alive = |t: Team| snap.players.iter().filter(|p| p.team == t && p.alive).count();
                    format!(
                        "ALPHA  {}     {clock}     {}  BRAVO\n{} alive | first to {} | {} alive",
                        snap.score[0],
                        snap.score[1],
                        alive(Team::Alpha),
                        snap.lobby.mode.rounds_to_win(),
                        alive(Team::Bravo)
                    )
                }
            }
            HudText::Vitals => {
                let helmet = if me.helmet { "+H" } else { "" };
                format!("HP {}    AP {}{}", me.health, me.armor, helmet)
            }
            HudText::Money => format!("$ {}", me.money),
            HudText::Ammo => match arsenal.held {
                Held::Weapon(WeaponId::Knife) | Held::Grenade(_) => String::new(),
                Held::Weapon(w) => {
                    let (m, r) = arsenal.ammo.get(&w).copied().unwrap_or((0, 0));
                    if arsenal.reloading() { format!("reloading...  / {r}") } else { format!("{m} / {r}") }
                }
            },
            HudText::Weapon => match arsenal.held {
                Held::Weapon(w) => w.def().name.to_string(),
                Held::Grenade(g) => g.name().to_string(),
            },
            HudText::Grenades => arsenal
                .grenades
                .iter()
                .map(|g| g.name())
                .collect::<Vec<_>>()
                .join(" | "),
            HudText::KillFeed => feed
                .kills
                .iter()
                .map(|(s, _, hl)| if *hl { format!("> {s} <") } else { s.clone() })
                .collect::<Vec<_>>()
                .join("\n"),
            HudText::Info => feed.infos.iter().map(|(s, _)| s.clone()).collect::<Vec<_>>().join("\n"),
            HudText::Center => feed.center.as_ref().map(|c| c.0.clone()).unwrap_or_default(),
            HudText::Bottom => {
                if !me.alive && snap.phase != Phase::Lobby {
                    let reason = feed.killed_by.clone().unwrap_or_else(|| "You are dead".into());
                    let wait = if ffa { "Respawning soon..." } else { "Waiting for the next round..." };
                    format!("{reason}\n{wait}")
                } else if snap.buy_time_left > 0.0 && snap.buy_time_left < 9000.0 {
                    format!("Press {buy_key} to buy  |  {:.0}s", snap.buy_time_left)
                } else if ffa {
                    format!("Press {buy_key} to buy")
                } else {
                    String::new()
                }
            }
        };
        if t.0 != s {
            t.0 = s;
        }
    }
}

fn update_crosshair(
    mut commands: Commands,
    settings: Res<Settings>,
    arsenal: Res<Arsenal>,
    root: Query<(Entity, &mut Visibility), With<CrosshairRoot>>,
    projections: Query<&Projection, With<super::player::PlayerCamera>>,
    window: Single<&Window>,
    players: Query<&LocalPlayer>,
    mut last: Local<Option<f32>>,
) {
    let Some((root, mut vis)) = root.into_iter().next() else { return };
    let alive = players.single().is_ok_and(|p| p.alive);
    let sniper = arsenal.weapon().is_some_and(|w| w.def().scope == ScopeStyle::Sniper);
    let scoped = arsenal.scope > 0;
    let hide = !alive || sniper || scoped;
    let want = if hide { Visibility::Hidden } else { Visibility::Inherited };
    if *vis != want {
        *vis = want;
    }
    let fov = projections
        .single()
        .ok()
        .and_then(|p| match p {
            Projection::Perspective(pp) => Some(pp.fov),
            _ => None,
        })
        .unwrap_or(1.2);
    let spread_px = if settings.crosshair.dynamic {
        arsenal.spread.tan() / (fov / 2.0).tan() * window.height() / 2.0
    } else {
        0.0
    };
    let spread_px = spread_px.min(120.0);
    let key = spread_px.round();
    if *last == Some(key) && !settings.is_changed() {
        return;
    }
    *last = Some(key);
    commands.entity(root).despawn_children();
    commands.entity(root).with_children(|p| spawn_crosshair_lines(p, &settings, spread_px));
}

fn update_overlays(
    fx: Res<ScreenEffects>,
    arsenal: Res<Arsenal>,
    mut overlays: Query<(&Overlay, &mut BackgroundColor)>,
    mut scopes: Query<(&ScopeOverlay, &mut Visibility)>,
    mut hit: Query<&mut TextColor, With<HitMarker>>,
) {
    for (o, mut bg) in &mut overlays {
        bg.0 = match o {
            Overlay::Flash => {
                let a = if fx.flash_hold > 0.0 {
                    1.0
                } else if fx.flash_total_fade > 0.0 {
                    (fx.flash_fade / fx.flash_total_fade).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                Color::srgba(1.0, 1.0, 1.0, a)
            }
            Overlay::Damage => Color::srgba(0.8, 0.0, 0.0, fx.damage * 0.35),
        };
    }
    let style = arsenal.weapon().map(|w| w.def().scope).unwrap_or(ScopeStyle::None);
    for (s, mut vis) in &mut scopes {
        let show = arsenal.scope > 0 && s.0 == style;
        let want = if show { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want {
            *vis = want;
        }
    }
    for mut c in &mut hit {
        c.0 = if fx.hit_head {
            Color::srgba(1.0, 0.25, 0.25, fx.hitmarker)
        } else {
            Color::srgba(1.0, 1.0, 1.0, fx.hitmarker)
        };
    }
}

#[allow(clippy::too_many_arguments)]
fn scoreboard(
    mut commands: Commands,
    time: Res<Time>,
    controls: Controls,
    ui: Res<GameUi>,
    open: Res<SettingsOpen>,
    client: Option<Res<NetClient>>,
    mut q: Query<(Entity, &mut Visibility), With<Scoreboard>>,
    mut timer: Local<f32>,
) {
    let Ok((board, mut vis)) = q.single_mut() else { return };
    let show = controls.pressed(Action::Scoreboard) && !ui.any_open(&open);
    let want = if show { Visibility::Inherited } else { Visibility::Hidden };
    if *vis != want {
        *vis = want;
        *timer = 0.0;
    }
    if !show {
        return;
    }
    *timer -= time.delta_secs();
    if *timer > 0.0 {
        return;
    }
    *timer = 0.5;
    let Some(c) = client else { return };
    let Some(snap) = &c.snapshot else { return };
    let my_team = c.me().map(|p| p.team);
    let rounds = snap.round.max(1) as f32;
    commands.entity(board).despawn_children();
    commands.entity(board).with_children(|b| {
        label(
            b,
            format!("{}  |  {}  |  Round {}", snap.lobby.name, snap.lobby.mode.name(), snap.round),
            20.0,
            ACCENT,
        );
        let groups: Vec<(String, Vec<Team>)> = if snap.lobby.mode.is_ffa() {
            vec![("Players".into(), vec![Team::None, Team::Alpha, Team::Bravo])]
        } else {
            vec![
                (format!("ALPHA - {}", snap.score[0]), vec![Team::Alpha]),
                (format!("BRAVO - {}", snap.score[1]), vec![Team::Bravo]),
            ]
        };
        for (name, teams) in groups {
            let color = rgb(teams[0].color());
            label(b, name, 18.0, color);
            label(
                b,
                format!("{:<22}{:>6}{:>6}{:>7}{:>7}{:>9}", "Name", "K", "D", "HS%", "ADR", "Money"),
                15.0,
                TEXT_DIM,
            );
            let mut members: Vec<_> = snap.players.iter().filter(|p| teams.contains(&p.team)).collect();
            members.sort_by(|a, b| b.kills.cmp(&a.kills).then(a.deaths.cmp(&b.deaths)));
            for p in members {
                let hs = if p.kills > 0 { p.headshots * 100 / p.kills } else { 0 };
                let money = if Some(p.team) == my_team { format!("${}", p.money) } else { "-".into() };
                let mut name = p.name.clone();
                if !p.alive {
                    name = format!("[dead] {name}");
                }
                let line = format!(
                    "{:<22}{:>6}{:>6}{:>6}%{:>7.0}{:>9}",
                    name,
                    p.kills,
                    p.deaths,
                    hs,
                    p.damage as f32 / rounds,
                    money
                );
                let color = if Some(p.id) == c.my_id { ACCENT } else { TEXT };
                label(b, line, 15.0, color);
            }
        }
    });
}
