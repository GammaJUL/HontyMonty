//! Buy menu: guns, grenades and armor. The server validates every purchase.

use bevy::prelude::*;

use super::{GameSet, GameUi, InGameEntity};
use crate::keybinds::{Action, Controls};
use crate::net::protocol::Cmd;
use crate::net::NetClient;
use crate::ui::settings_menu::SettingsOpen;
use crate::ui::widgets::*;
use crate::weapons::{shop_categories, ShopItem, Slot};

#[derive(Component)]
struct BuyRoot;

#[derive(Component, Clone, Copy)]
enum BuyButton {
    Item(ShopItem),
    Close,
}

#[derive(Component)]
struct BuyHeader;

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (toggle, show, handle, refresh).chain().in_set(GameSet::Input));
}

fn toggle(
    controls: Controls,
    client: Option<Res<NetClient>>,
    open: Res<SettingsOpen>,
    mut ui: ResMut<GameUi>,
) {
    let can_buy = client
        .as_ref()
        .and_then(|c| Some((c.me()?, c.snapshot.as_ref()?)))
        .is_some_and(|(me, s)| me.alive && s.buy_time_left > 0.0);
    if controls.just_pressed(Action::BuyMenu) && !ui.pause_open && !open.0 {
        ui.buy_open = !ui.buy_open && can_buy;
    }
    if ui.buy_open && !can_buy {
        ui.buy_open = false;
    }
}

fn show(mut commands: Commands, ui: Res<GameUi>, root: Query<Entity, With<BuyRoot>>) {
    let exists = !root.is_empty();
    if ui.buy_open && !exists {
        commands
            .spawn((
                InGameEntity,
                BuyRoot,
                GlobalZIndex(50),
                Node {
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..screen_root()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
            ))
            .with_children(|root| {
                root.spawn(panel(px(1000))).with_children(|p| {
                    p.spawn(Node {
                        justify_content: JustifyContent::SpaceBetween,
                        ..row()
                    })
                    .with_children(|r| {
                        r.spawn((BuyHeader, Text::new("Buy Menu"), font(28.0), TextColor(ACCENT)));
                        button(r, "Close", BuyButton::Close, px(100));
                    });
                    p.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: px(14),
                        ..default()
                    })
                    .with_children(|cols| {
                        for (name, items) in shop_categories() {
                            cols.spawn(Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: px(6),
                                width: px(180),
                                ..default()
                            })
                            .with_children(|col| {
                                label(col, name, 18.0, TEXT_DIM);
                                for item in items {
                                    button_sized(
                                        col,
                                        format!("{}\n${}", item.name(), item.price()),
                                        BuyButton::Item(item),
                                        percent(100),
                                        52.0,
                                        15.0,
                                    );
                                }
                            });
                        }
                    });
                    label(p, "Buying a primary replaces your current one. Max 4 grenades (2 flashbangs).", 14.0, TEXT_DIM);
                });
            });
    } else if !ui.buy_open && exists {
        for e in &root {
            commands.entity(e).despawn();
        }
    }
}

fn handle(q: Clicked<BuyButton>, client: Option<ResMut<NetClient>>, mut ui: ResMut<GameUi>) {
    let Some(mut c) = client else { return };
    for b in clicked(&q) {
        match *b {
            BuyButton::Item(item) => c.send_cmd(Cmd::Buy(item)),
            BuyButton::Close => ui.buy_open = false,
        }
    }
}

/// Dims items you can't afford or already own.
fn refresh(
    mut commands: Commands,
    client: Option<Res<NetClient>>,
    buttons: Query<(Entity, &BuyButton, Has<Disabled>)>,
    mut header: Query<&mut Text, With<BuyHeader>>,
) {
    let Some(me) = client.as_ref().and_then(|c| c.me()) else { return };
    let snap = client.as_ref().and_then(|c| c.snapshot.as_ref());
    if let Ok(mut t) = header.single_mut() {
        let left = snap.map(|s| s.buy_time_left).unwrap_or(0.0);
        let time = if left > 9000.0 { String::new() } else { format!("  |  {left:.0}s left") };
        let s = format!("Buy Menu  |  ${}{time}", me.money);
        if t.0 != s {
            t.0 = s;
        }
    }
    for (e, b, disabled) in &buttons {
        let BuyButton::Item(item) = *b else { continue };
        let owned = match item {
            ShopItem::Weapon(w) => match w.def().slot {
                Slot::Primary => me.primary == Some(w),
                Slot::Secondary => me.secondary == Some(w),
                Slot::Melee => true,
            },
            ShopItem::Kevlar => me.armor >= 100,
            ShopItem::KevlarHelmet => me.armor >= 100 && me.helmet,
            ShopItem::Grenade(_) => false,
        };
        let price = match item {
            ShopItem::KevlarHelmet if me.armor >= 100 => 350,
            _ => item.price(),
        };
        let off = owned || me.money < price;
        if off != disabled {
            if off {
                commands.entity(e).insert(Disabled);
            } else {
                commands.entity(e).remove::<Disabled>();
            }
        }
    }
}
