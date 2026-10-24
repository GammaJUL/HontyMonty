//! Case opening (unlimited, free) with a CS-style reel, plus the skin inventory.

use bevy::prelude::*;

use super::widgets::*;
use crate::audio::{PlaySfx, Sound};
use crate::cases::*;
use crate::profile::Profile;
use crate::skin_textures::SkinTextures;
use crate::states::AppState;

const CARD_W: f32 = 110.0;
const CARD_GAP: f32 = 6.0;
const REEL_W: f32 = 600.0;
const REEL_LEN: usize = 60;
const WINNER_AT: usize = 52;
const SPIN_TIME: f32 = 6.0;
const PAGE_SIZE: usize = 12;

#[derive(Component, Clone, Copy)]
enum InvButton {
    Open,
    QuickOpen,
    Card(u64),
    PrevPage,
    NextPage,
    EquipResult,
    Filter(Option<Rarity>),
    DeleteMode,
    Back,
}

#[derive(Component)]
struct HintText;

/// Inserted by the dev helper to start one spin automatically.
#[derive(Resource)]
pub struct DevAutoOpen;

#[derive(Component)]
struct Reel;

#[derive(Component)]
struct ReelStrip;

#[derive(Component)]
struct ResultPanel;

#[derive(Component)]
struct Grid;

#[derive(Component)]
struct PageText;

#[derive(Component)]
struct StatsText;

struct Spin {
    item: OwnedSkin,
    target: f32,
    elapsed: f32,
    last_tick: i32,
}

#[derive(Resource, Default)]
struct CaseUi {
    spin: Option<Spin>,
    last: Option<OwnedSkin>,
    page: usize,
    dirty: bool,
    rng: Option<fastrand::Rng>,
    filter: Option<Rarity>,
    /// Clicking an item deletes it instead of equipping it.
    delete_mode: bool,
}

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Inventory), spawn)
        .add_systems(
            Update,
            (handle, animate_spin, rebuild_grid, rebuild_result)
                .chain()
                .run_if(in_state(AppState::Inventory)),
        );
}

fn spawn(mut commands: Commands) {
    commands.insert_resource(CaseUi {
        dirty: true,
        rng: Some(fastrand::Rng::new()),
        ..default()
    });
    commands
        .spawn((
            DespawnOnExit(AppState::Inventory),
            Node {
                padding: UiRect::all(px(30)),
                column_gap: px(20),
                ..screen_root()
            },
        ))
        .with_children(|root| {
            root.spawn(panel(px(REEL_W + 44.0))).with_children(|p| {
                title(p, CASE_NAME);
                label(p, "Unlimited free openings. Same odds as a CS2 case:", 16.0, TEXT_DIM);
                for r in Rarity::ALL {
                    p.spawn(row()).with_children(|rw| {
                        rw.spawn((
                            Node {
                                width: px(14),
                                height: px(14),
                                border_radius: BorderRadius::all(px(3)),
                                ..default()
                            },
                            BackgroundColor(rgb(r.color())),
                        ));
                        let count = skins_of(r).count();
                        label(rw, format!("{:<20}", r.name()), 16.0, rgb(r.color()));
                        label(rw, format!("{:.2}%   ({count} items)", r.odds()), 16.0, TEXT_DIM);
                    });
                }
                label(p, "StatTrak chance: 10%   |   Wear: Factory New -> Battle-Scarred", 14.0, TEXT_DIM);
                // The reel.
                p.spawn((
                    Reel,
                    Node {
                        width: px(REEL_W),
                        height: px(130),
                        overflow: Overflow::clip(),
                        margin: UiRect::vertical(px(10)),
                        border: UiRect::all(px(1)),
                        border_radius: BorderRadius::all(px(6)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                    BorderColor::all(BORDER),
                ))
                .with_children(|reel| {
                    reel.spawn((
                        ReelStrip,
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(0),
                            top: px(8),
                            flex_direction: FlexDirection::Row,
                            column_gap: px(CARD_GAP),
                            ..default()
                        },
                    ));
                    reel.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(REEL_W / 2.0 - 1.0),
                            top: px(0),
                            width: px(3),
                            height: percent(100),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(1.0, 0.85, 0.2)),
                        GlobalZIndex(5),
                    ));
                });
                p.spawn(row()).with_children(|r| {
                    button_sized(r, "OPEN CASE", InvButton::Open, px(260), 50.0, 22.0);
                    button_sized(r, "Quick Open", InvButton::QuickOpen, px(160), 50.0, 18.0);
                });
                p.spawn((
                    ResultPanel,
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: px(4),
                        min_height: px(110),
                        ..default()
                    },
                ));
                button(p, "Back", InvButton::Back, px(120));
            });
            root.spawn(panel(px(560))).with_children(|p| {
                title(p, "Inventory");
                p.spawn((StatsText, Text::new(""), font(15.0), TextColor(TEXT_DIM)));
                p.spawn(row()).with_children(|r| {
                    button_sized(r, "All", InvButton::Filter(None), px(52), 28.0, 13.0);
                    for rarity in Rarity::ALL {
                        let short = match rarity {
                            Rarity::MilSpec => "Blue",
                            Rarity::Restricted => "Purple",
                            Rarity::Classified => "Pink",
                            Rarity::Covert => "Red",
                            Rarity::RareSpecial => "Gold",
                        };
                        let b = button_sized(r, short, InvButton::Filter(Some(rarity)), px(66), 28.0, 13.0);
                        r.commands().entity(b).insert(BorderColor::all(rgb(rarity.color())));
                    }
                    button_sized(r, "Delete mode", InvButton::DeleteMode, px(132), 28.0, 13.0);
                });
                p.spawn((HintText, Text::new(""), font(14.0), TextColor(TEXT_DIM)));
                p.spawn((
                    Grid,
                    Node {
                        display: Display::Grid,
                        grid_template_columns: RepeatedGridTrack::flex(3, 1.0),
                        row_gap: px(8),
                        column_gap: px(8),
                        min_height: px(480),
                        ..default()
                    },
                ));
                p.spawn(row()).with_children(|r| {
                    button_sized(r, "<", InvButton::PrevPage, px(44), 36.0, 18.0);
                    r.spawn((PageText, Text::new(""), font(16.0), TextColor(TEXT)));
                    button_sized(r, ">", InvButton::NextPage, px(44), 36.0, 18.0);
                });
            });
        });
}

fn start_spin(
    commands: &mut Commands,
    ui: &mut CaseUi,
    profile: &mut Profile,
    strip: Entity,
    images: &mut Assets<Image>,
    textures: &mut SkinTextures,
) {
    let mut rng = ui.rng.take().unwrap_or_default();
    let uid = profile.next_uid;
    profile.next_uid += 1;
    let item = open_case(&mut rng, uid);
    // Filler items are rolled with the same odds so the reel looks honest.
    let mut reel: Vec<(&'static SkinDef, f32)> = (0..REEL_LEN)
        .map(|_| {
            let r = roll_rarity(&mut rng);
            (random_skin_of(&mut rng, r), 0.1)
        })
        .collect();
    reel[WINNER_AT] = (item.def(), item.float);
    commands.entity(strip).despawn_children();
    commands.entity(strip).with_children(|s| {
        for (def, float) in reel {
            spawn_card(s, def, float, images, textures, true);
        }
    });
    let jitter = (rng.f32() - 0.5) * CARD_W * 0.8;
    let target = WINNER_AT as f32 * (CARD_W + CARD_GAP) + CARD_W / 2.0 - REEL_W / 2.0 + jitter;
    ui.rng = Some(rng);
    ui.spin = Some(Spin {
        item,
        target,
        elapsed: 0.0,
        last_tick: -1,
    });
}

fn spawn_card(
    p: &mut ChildSpawnerCommands,
    def: &SkinDef,
    float: f32,
    images: &mut Assets<Image>,
    textures: &mut SkinTextures,
    hide_rare: bool,
) -> Entity {
    let color = rgb(def.rarity.color());
    let mystery = hide_rare && def.rarity == Rarity::RareSpecial;
    p.spawn((
        Node {
            width: px(CARD_W),
            height: px(112),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            padding: UiRect::all(px(4)),
            border: UiRect::bottom(px(4)),
            border_radius: BorderRadius::all(px(4)),
            flex_shrink: 0.0,
            ..default()
        },
        BackgroundColor(Color::srgba(0.12, 0.13, 0.16, 1.0)),
        BorderColor::all(color),
    ))
    .with_children(|c| {
        if mystery {
            c.spawn((Text::new("*"), font(46.0), TextColor(color)));
            label(c, "Rare Special Item", 12.0, color);
        } else {
            c.spawn((
                ImageNode {
                    image_mode: NodeImageMode::Stretch,
                    ..ImageNode::new(textures.get(images, def, float, 0))
                },
                Node {
                    width: px(CARD_W - 16.0),
                    height: px(48),
                    border_radius: BorderRadius::all(px(3)),
                    ..default()
                },
            ));
            let weapon = if def.weapon == crate::weapons::WeaponId::Knife {
                format!("* {}", def.knife.name())
            } else {
                def.weapon.def().name.to_string()
            };
            label(c, weapon, 12.0, TEXT_DIM);
            label(c, def.name, 13.0, TEXT);
        }
    })
    .id()
}

#[allow(clippy::too_many_arguments)]
fn handle(
    mut commands: Commands,
    q: Clicked<InvButton>,
    mut ui: ResMut<CaseUi>,
    mut profile: ResMut<Profile>,
    strip: Query<Entity, With<ReelStrip>>,
    mut images: ResMut<Assets<Image>>,
    mut textures: ResMut<SkinTextures>,
    mut next: ResMut<NextState<AppState>>,
    mut sfx: MessageWriter<PlaySfx>,
    auto: Option<Res<DevAutoOpen>>,
) {
    let auto_open = auto.is_some().then_some(InvButton::Open);
    if auto_open.is_some() {
        commands.remove_resource::<DevAutoOpen>();
    }
    for action in clicked(&q).chain(auto_open.as_ref()) {
        match *action {
            InvButton::Open => {
                if ui.spin.is_none()
                    && let Ok(strip) = strip.single()
                {
                    start_spin(&mut commands, &mut ui, &mut profile, strip, &mut images, &mut textures);
                }
            }
            InvButton::QuickOpen => {
                if ui.spin.is_none() {
                    let mut rng = ui.rng.take().unwrap_or_default();
                    let uid = profile.next_uid;
                    profile.next_uid += 1;
                    let item = open_case(&mut rng, uid);
                    ui.rng = Some(rng);
                    sfx.write(PlaySfx::ui(Sound::Reveal(item.def().rarity)));
                    finish(&mut ui, &mut profile, item);
                }
            }
            InvButton::Card(uid) => {
                if ui.delete_mode {
                    profile.inventory.retain(|s| s.uid != uid);
                    profile.equipped.retain(|_, equipped| *equipped != uid);
                    profile.save();
                } else {
                    toggle_equip(&mut profile, uid);
                }
                ui.dirty = true;
            }
            InvButton::Filter(f) => {
                ui.filter = f;
                ui.page = 0;
                ui.dirty = true;
            }
            InvButton::DeleteMode => {
                ui.delete_mode = !ui.delete_mode;
                ui.dirty = true;
            }
            InvButton::EquipResult => {
                if let Some(item) = &ui.last {
                    let uid = item.uid;
                    let w = item.def().weapon;
                    profile.equipped.insert(w, uid);
                    profile.save();
                    ui.dirty = true;
                }
            }
            InvButton::PrevPage => {
                ui.page = ui.page.saturating_sub(1);
                ui.dirty = true;
            }
            InvButton::NextPage => {
                let pages = profile.inventory.len().div_ceil(PAGE_SIZE).max(1);
                ui.page = (ui.page + 1).min(pages - 1);
                ui.dirty = true;
            }
            InvButton::Back => {
                profile.save();
                next.set(AppState::MainMenu);
            }
        }
    }
}

fn toggle_equip(profile: &mut Profile, uid: u64) {
    let Some(item) = profile.inventory.iter().find(|s| s.uid == uid) else { return };
    let w = item.def().weapon;
    if profile.equipped.get(&w) == Some(&uid) {
        profile.equipped.remove(&w);
    } else {
        profile.equipped.insert(w, uid);
    }
    profile.save();
}

fn finish(ui: &mut CaseUi, profile: &mut Profile, item: OwnedSkin) {
    profile.inventory.push(item.clone());
    profile.cases_opened += 1;
    profile.save();
    ui.last = Some(item);
    ui.dirty = true;
}

fn animate_spin(
    time: Res<Time>,
    mut ui: ResMut<CaseUi>,
    mut profile: ResMut<Profile>,
    mut strip: Query<&mut Node, With<ReelStrip>>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    let Some(spin) = ui.spin.as_mut() else { return };
    spin.elapsed += time.delta_secs();
    let t = (spin.elapsed / SPIN_TIME).min(1.0);
    // Ease-out quint for that slow CS crawl at the end.
    let eased = 1.0 - (1.0 - t).powi(5);
    let offset = spin.target * eased;
    if let Ok(mut node) = strip.single_mut() {
        node.left = px(-offset);
    }
    let under_marker = ((offset + REEL_W / 2.0) / (CARD_W + CARD_GAP)).floor() as i32;
    if under_marker != spin.last_tick {
        spin.last_tick = under_marker;
        sfx.write(PlaySfx {
            volume: 0.6,
            ..PlaySfx::ui(Sound::Tick)
        });
    }
    if t >= 1.0 {
        let spin = ui.spin.take().unwrap();
        sfx.write(PlaySfx::ui(Sound::Reveal(spin.item.def().rarity)));
        finish(&mut ui, &mut profile, spin.item);
    }
}

fn rebuild_result(
    mut commands: Commands,
    ui: Res<CaseUi>,
    panel: Query<Entity, With<ResultPanel>>,
    mut shown: Local<Option<u64>>,
) {
    let Some(item) = &ui.last else { return };
    if *shown == Some(item.uid) {
        return;
    }
    *shown = Some(item.uid);
    let Ok(panel) = panel.single() else { return };
    commands.entity(panel).despawn_children();
    let def = item.def();
    let color = rgb(def.rarity.color());
    commands.entity(panel).with_children(|p| {
        label(p, format!("You unboxed: {}", item.display_name()), 22.0, color);
        label(
            p,
            format!(
                "{}  |  {}  |  Float {:.6}  |  Pattern #{}",
                def.rarity.name(),
                item.wear().name(),
                item.float,
                item.pattern_seed
            ),
            15.0,
            TEXT_DIM,
        );
        button_sized(p, "Equip", InvButton::EquipResult, px(120), 34.0, 16.0);
    });
}

#[allow(clippy::too_many_arguments)]
fn rebuild_grid(
    mut commands: Commands,
    mut ui: ResMut<CaseUi>,
    profile: Res<Profile>,
    grid: Query<Entity, With<Grid>>,
    mut page_text: Query<&mut Text, (With<PageText>, Without<StatsText>, Without<HintText>)>,
    mut stats: Query<&mut Text, (With<StatsText>, Without<PageText>, Without<HintText>)>,
    mut images: ResMut<Assets<Image>>,
    mut textures: ResMut<SkinTextures>,
    buttons: Query<(Entity, &InvButton)>,
    mut hint: Query<&mut Text, (With<HintText>, Without<PageText>, Without<StatsText>)>,
) {
    if !ui.dirty {
        return;
    }
    ui.dirty = false;
    for (e, b) in &buttons {
        let on = match b {
            InvButton::Filter(f) => *f == ui.filter,
            InvButton::DeleteMode => ui.delete_mode,
            _ => continue,
        };
        if on {
            commands.entity(e).insert(Selected);
        } else {
            commands.entity(e).remove::<Selected>();
        }
    }
    if let Ok(mut t) = hint.single_mut() {
        t.0 = if ui.delete_mode {
            "DELETE MODE: clicking an item deletes it forever.".into()
        } else {
            "Click an item to equip / unequip it.".into()
        };
    }
    let Ok(grid) = grid.single() else { return };
    let filter = ui.filter;
    let mut items: Vec<&OwnedSkin> = profile
        .inventory
        .iter()
        .filter(|s| filter.is_none_or(|r| s.def().rarity == r))
        .collect();
    items.sort_by(|a, b| b.def().rarity.cmp(&a.def().rarity).then(b.uid.cmp(&a.uid)));
    let pages = items.len().div_ceil(PAGE_SIZE).max(1);
    ui.page = ui.page.min(pages - 1);
    if let Ok(mut t) = page_text.single_mut() {
        t.0 = format!("Page {} / {}", ui.page + 1, pages);
    }
    if let Ok(mut t) = stats.single_mut() {
        let counts: Vec<String> = Rarity::ALL
            .iter()
            .map(|r| {
                let n = profile.inventory.iter().filter(|s| s.def().rarity == *r).count();
                format!("{}: {n}", r.name())
            })
            .collect();
        t.0 = format!("{} items | {} cases opened\n{}", profile.inventory.len(), profile.cases_opened, counts.join("  |  "));
    }
    commands.entity(grid).despawn_children();
    let page: Vec<OwnedSkin> = items
        .into_iter()
        .skip(ui.page * PAGE_SIZE)
        .take(PAGE_SIZE)
        .cloned()
        .collect();
    commands.entity(grid).with_children(|g| {
        if page.is_empty() {
            g.spawn((
                Text::new("Nothing here yet - open some cases!"),
                font(16.0),
                TextColor(TEXT_DIM),
                Node {
                    grid_column: GridPlacement::span(3),
                    ..default()
                },
            ));
        }
        for item in page {
            let def = item.def();
            let equipped = profile.equipped.get(&def.weapon) == Some(&item.uid);
            let color = rgb(def.rarity.color());
            let tex = textures.get(&mut images, def, item.float, item.pattern_seed);
            let mut e = g.spawn((
                UiButton,
                InvButton::Card(item.uid),
                Node {
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(px(6)),
                    border: UiRect::new(px(1), px(1), px(1), px(4)),
                    border_radius: BorderRadius::all(px(4)),
                    row_gap: px(2),
                    ..default()
                },
                BackgroundColor(BG_BUTTON),
                BorderColor {
                    bottom: color,
                    ..BorderColor::all(if equipped { ACCENT } else { BORDER })
                },
            ));
            e.with_children(|c| {
                c.spawn((
                    ImageNode {
                        image_mode: NodeImageMode::Stretch,
                        ..ImageNode::new(tex)
                    },
                    Node {
                        width: percent(100),
                        height: px(46),
                        border_radius: BorderRadius::all(px(3)),
                        ..default()
                    },
                ));
                let st = if item.stattrak { "ST " } else { "" };
                let weapon = if def.weapon == crate::weapons::WeaponId::Knife {
                    format!("* {}", def.knife.name())
                } else {
                    def.weapon.def().name.to_string()
                };
                label(c, format!("{st}{weapon}"), 13.0, TEXT_DIM);
                label(c, def.name, 15.0, color);
                label(
                    c,
                    format!(
                        "{} | {:.3}{}{}",
                        item.wear().name(),
                        item.float,
                        if item.stattrak { format!(" | {} kills", item.kills) } else { String::new() },
                        if equipped { "  [equipped]" } else { "" }
                    ),
                    12.0,
                    TEXT_DIM,
                );
            });
        }
    });
}
