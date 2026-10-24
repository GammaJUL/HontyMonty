//! Character customization: gender, skin, hair, eyes, clothing, height.

use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;

use super::widgets::*;
use crate::menu_scene::TurntableSpin;
use crate::profile::*;
use crate::states::AppState;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Field {
    Gender,
    SkinTone,
    HairStyle,
    HairColor,
    EyeColor,
    Shirt,
    Pants,
    Height,
    FacialHair,
    Accessory,
}

const FIELDS: [(Field, &str); 10] = [
    (Field::Gender, "Body"),
    (Field::SkinTone, "Skin Tone"),
    (Field::HairStyle, "Hair Style"),
    (Field::HairColor, "Hair Color"),
    (Field::EyeColor, "Eye Color"),
    (Field::FacialHair, "Facial Hair"),
    (Field::Accessory, "Accessory"),
    (Field::Shirt, "Shirt"),
    (Field::Pants, "Pants"),
    (Field::Height, "Height"),
];

#[derive(Component, Clone, Copy)]
enum CustomizeButton {
    Step(Field, i32),
    Randomize,
    Reset,
    Done,
}

#[derive(Component)]
struct ValueText(Field);

#[derive(Component)]
struct Swatch(Field);

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Customize), spawn)
        .add_systems(
            Update,
            (handle, refresh, drag_rotate).run_if(in_state(AppState::Customize)),
        );
}

fn spawn(mut commands: Commands) {
    commands
        .spawn((
            DespawnOnExit(AppState::Customize),
            Node {
                padding: UiRect::all(px(40)),
                align_items: AlignItems::Center,
                ..screen_root()
            },
        ))
        .with_children(|root| {
            root.spawn(panel(px(460))).with_children(|p| {
                title(p, "Customize Character");
                for (field, name) in FIELDS {
                    p.spawn(Node {
                        justify_content: JustifyContent::SpaceBetween,
                        ..row()
                    })
                    .with_children(|r| {
                        r.spawn((Text::new(name), font(18.0), TextColor(TEXT_DIM), Node {
                            width: px(120),
                            ..default()
                        }));
                        button_sized(r, "<", CustomizeButton::Step(field, -1), px(36), 34.0, 18.0);
                        r.spawn(Node {
                            width: px(190),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            column_gap: px(8),
                            ..default()
                        })
                        .with_children(|v| {
                            v.spawn((
                                Swatch(field),
                                Node {
                                    width: px(16),
                                    height: px(16),
                                    border_radius: BorderRadius::all(px(8)),
                                    ..default()
                                },
                                BackgroundColor(Color::NONE),
                            ));
                            v.spawn((ValueText(field), Text::new(""), font(18.0), TextColor(TEXT)));
                        });
                        button_sized(r, ">", CustomizeButton::Step(field, 1), px(36), 34.0, 18.0);
                    });
                }
                p.spawn(Node {
                    margin: UiRect::top(px(12)),
                    ..row()
                })
                .with_children(|r| {
                    button(r, "Randomize", CustomizeButton::Randomize, px(130));
                    button(r, "Reset", CustomizeButton::Reset, px(100));
                    button(r, "Save & Back", CustomizeButton::Done, px(160));
                });
                label(p, "Drag with the right mouse button to rotate", 14.0, TEXT_DIM);
            });
        });
}

fn step(i: usize, d: i32, n: usize) -> usize {
    ((i as i32 + d).rem_euclid(n as i32)) as usize
}

fn handle(
    q: Clicked<CustomizeButton>,
    mut profile: ResMut<Profile>,
    mut next: ResMut<NextState<AppState>>,
) {
    for action in clicked(&q) {
        let a = &mut profile.appearance;
        match *action {
            CustomizeButton::Step(field, d) => match field {
                Field::Gender => {
                    a.gender = if a.gender == Gender::Male { Gender::Female } else { Gender::Male };
                }
                Field::SkinTone => a.skin_tone = step(a.skin_tone, d, SKIN_TONES.len()),
                Field::HairStyle => {
                    let i = HairStyle::ALL.iter().position(|h| *h == a.hair_style).unwrap_or(0);
                    a.hair_style = HairStyle::ALL[step(i, d, HairStyle::ALL.len())];
                }
                Field::HairColor => a.hair_color = step(a.hair_color, d, HAIR_COLORS.len()),
                Field::EyeColor => a.eye_color = step(a.eye_color, d, EYE_COLORS.len()),
                Field::Shirt => a.shirt_color = step(a.shirt_color, d, CLOTHING_COLORS.len()),
                Field::Pants => a.pants_color = step(a.pants_color, d, CLOTHING_COLORS.len()),
                Field::Height => a.height = (a.height + d as f32 * 0.02).clamp(0.9, 1.1),
                Field::FacialHair => {
                    let i = FacialHair::ALL.iter().position(|h| *h == a.facial_hair).unwrap_or(0);
                    a.facial_hair = FacialHair::ALL[step(i, d, FacialHair::ALL.len())];
                }
                Field::Accessory => {
                    let i = Accessory::ALL.iter().position(|h| *h == a.accessory).unwrap_or(0);
                    a.accessory = Accessory::ALL[step(i, d, Accessory::ALL.len())];
                }
            },
            CustomizeButton::Randomize => {
                *a = Appearance {
                    gender: if fastrand::bool() { Gender::Male } else { Gender::Female },
                    skin_tone: fastrand::usize(..SKIN_TONES.len()),
                    hair_style: HairStyle::ALL[fastrand::usize(..HairStyle::ALL.len())],
                    hair_color: fastrand::usize(..HAIR_COLORS.len()),
                    eye_color: fastrand::usize(..EYE_COLORS.len()),
                    shirt_color: fastrand::usize(..CLOTHING_COLORS.len()),
                    pants_color: fastrand::usize(..CLOTHING_COLORS.len()),
                    height: 0.94 + fastrand::f32() * 0.12,
                    facial_hair: FacialHair::ALL[fastrand::usize(..FacialHair::ALL.len())],
                    accessory: Accessory::ALL[fastrand::usize(..Accessory::ALL.len())],
                };
            }
            CustomizeButton::Reset => *a = Appearance::default(),
            CustomizeButton::Done => {
                profile.save();
                next.set(AppState::MainMenu);
            }
        }
    }
}

fn refresh(
    profile: Res<Profile>,
    mut texts: Query<(&ValueText, &mut Text)>,
    mut swatches: Query<(&Swatch, &mut BackgroundColor)>,
) {
    let a = &profile.appearance;
    for (ValueText(field), mut text) in &mut texts {
        let value = match field {
            Field::Gender => match a.gender {
                Gender::Male => "Man".to_string(),
                Gender::Female => "Woman".to_string(),
            },
            Field::SkinTone => SKIN_TONES[a.skin_tone % SKIN_TONES.len()].0.to_string(),
            Field::HairStyle => a.hair_style.name().to_string(),
            Field::HairColor => HAIR_COLORS[a.hair_color % HAIR_COLORS.len()].0.to_string(),
            Field::EyeColor => EYE_COLORS[a.eye_color % EYE_COLORS.len()].0.to_string(),
            Field::Shirt => CLOTHING_COLORS[a.shirt_color % CLOTHING_COLORS.len()].0.to_string(),
            Field::Pants => CLOTHING_COLORS[a.pants_color % CLOTHING_COLORS.len()].0.to_string(),
            Field::Height => format!("{:.0} cm", 180.0 * a.height),
            Field::FacialHair => a.facial_hair.name().to_string(),
            Field::Accessory => a.accessory.name().to_string(),
        };
        if text.0 != value {
            text.0 = value;
        }
    }
    for (Swatch(field), mut bg) in &mut swatches {
        let c = match field {
            Field::SkinTone => pal(SKIN_TONES, a.skin_tone),
            Field::HairColor => pal(HAIR_COLORS, a.hair_color),
            Field::EyeColor => pal(EYE_COLORS, a.eye_color),
            Field::Shirt => pal(CLOTHING_COLORS, a.shirt_color),
            Field::Pants => pal(CLOTHING_COLORS, a.pants_color),
            _ => Color::NONE,
        };
        bg.0 = c;
    }
}

fn drag_rotate(
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut spin: ResMut<TurntableSpin>,
) {
    if mouse.pressed(MouseButton::Right) {
        spin.0 += motion.delta.x * 0.01;
    }
    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
        spin.0 -= time.delta_secs() * 2.0;
    }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
        spin.0 += time.delta_secs() * 2.0;
    }
}
