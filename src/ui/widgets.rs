//! Shared UI building blocks: themed buttons, labels, panels and text inputs.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::input::ButtonState;
use bevy::prelude::*;

use bevy::ui::{ComputedNode, FocusPolicy, UiGlobalTransform, UiStack};

use crate::audio::{PlaySfx, Sound};
use crate::render_scale::UiCursor;

pub const BG_PANEL: Color = Color::srgba(0.06, 0.07, 0.09, 0.88);
pub const BG_BUTTON: Color = Color::srgba(0.16, 0.18, 0.22, 0.95);
pub const BG_HOVER: Color = Color::srgba(0.26, 0.29, 0.35, 0.98);
pub const BG_PRESSED: Color = Color::srgba(0.36, 0.40, 0.48, 1.0);
pub const BG_SELECTED: Color = Color::srgba(0.85, 0.55, 0.15, 0.95);
pub const ACCENT: Color = Color::srgb(0.95, 0.65, 0.2);
pub const TEXT: Color = Color::srgb(0.92, 0.93, 0.95);
pub const TEXT_DIM: Color = Color::srgb(0.6, 0.63, 0.68);
pub const BORDER: Color = Color::srgba(1.0, 1.0, 1.0, 0.08);

/// Clickable UI node; its [`Interaction`] is kept up to date by [`update_interactions`].
#[derive(Component, Default)]
#[require(Node, Interaction, FocusPolicy::Block)]
pub struct UiButton;

/// Hover/press state of a [`UiButton`]. Computed here instead of by bevy_ui so it also
/// works when the UI is rendered into the low-resolution texture (see `render_scale`).
#[derive(Component, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Interaction {
    #[default]
    None,
    Hovered,
    Pressed,
}

/// Finds the topmost button under the cursor, like bevy_ui's focus system.
fn update_interactions(
    cursor: Res<UiCursor>,
    mouse: Res<ButtonInput<MouseButton>>,
    stack: Res<UiStack>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform, &InheritedVisibility, Option<&FocusPolicy>, Has<UiButton>)>,
    mut buttons: Query<(Entity, &mut Interaction)>,
) {
    let mut hovered = None;
    if let Some(point) = cursor.0 {
        for &e in stack.uinodes.iter().rev() {
            let Ok((node, transform, visible, policy, is_button)) = nodes.get(e) else { continue };
            if !visible.get() || !node.contains_point(*transform, point) {
                continue;
            }
            if is_button {
                hovered = Some(e);
                break;
            }
            if matches!(policy, Some(FocusPolicy::Block)) {
                break;
            }
        }
    }
    for (e, mut interaction) in &mut buttons {
        let next = if *interaction == Interaction::Pressed && mouse.pressed(MouseButton::Left) {
            Interaction::Pressed
        } else if Some(e) == hovered {
            if mouse.just_pressed(MouseButton::Left) { Interaction::Pressed } else { Interaction::Hovered }
        } else {
            Interaction::None
        };
        interaction.set_if_neq(next);
    }
}

/// Marks a button as the currently selected option (drawn highlighted).
#[derive(Component)]
pub struct Selected;

/// Marks a button as unusable (drawn dim, presses ignored by screens).
#[derive(Component)]
pub struct Disabled;

pub fn rgb(c: [f32; 3]) -> Color {
    Color::srgb(c[0], c[1], c[2])
}

pub fn font(size: f32) -> TextFont {
    TextFont::from_font_size(size)
}

pub fn label(p: &mut ChildSpawnerCommands, text: impl Into<String>, size: f32, color: Color) -> Entity {
    p.spawn((Text::new(text), font(size), TextColor(color))).id()
}

pub fn title(p: &mut ChildSpawnerCommands, text: impl Into<String>) {
    p.spawn((
        Text::new(text),
        font(34.0),
        TextColor(ACCENT),
        Node {
            margin: UiRect::bottom(px(12)),
            ..default()
        },
    ));
}

pub fn button_node(width: Val, height: f32) -> Node {
    Node {
        width,
        height: px(height),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        padding: UiRect::horizontal(px(12)),
        border: UiRect::all(px(1)),
        border_radius: BorderRadius::all(px(4)),
        ..default()
    }
}

/// Spawns a themed button carrying `action`.
pub fn button<A: Component>(
    p: &mut ChildSpawnerCommands,
    text: impl Into<String>,
    action: A,
    width: Val,
) -> Entity {
    button_sized(p, text, action, width, 40.0, 18.0)
}

pub fn button_sized<A: Component>(
    p: &mut ChildSpawnerCommands,
    text: impl Into<String>,
    action: A,
    width: Val,
    height: f32,
    font_size: f32,
) -> Entity {
    p.spawn((
        UiButton,
        action,
        button_node(width, height),
        BackgroundColor(BG_BUTTON),
        BorderColor::all(BORDER),
    ))
    .with_children(|b| {
        b.spawn((Text::new(text), font(font_size), TextColor(TEXT)));
    })
    .id()
}

pub fn panel(width: Val) -> (Node, BackgroundColor, BorderColor) {
    (
        Node {
            width,
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(px(20)),
            row_gap: px(8),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(8)),
            ..default()
        },
        BackgroundColor(BG_PANEL),
        BorderColor::all(BORDER),
    )
}

/// Scrolls with the mouse wheel while hovered. Used by [`scroll_panel`].
#[derive(Component, Default)]
pub struct Scrollable;

/// Like [`panel`] but never taller than its parent; extra content scrolls (handy at low resolutions).
pub fn scroll_panel(width: Val) -> (Node, BackgroundColor, BorderColor, Scrollable, ScrollPosition) {
    let (mut node, bg, border) = panel(width);
    node.max_height = percent(100);
    node.overflow = Overflow::scroll_y();
    (node, bg, border, Scrollable, ScrollPosition::default())
}

fn scroll_wheel(
    mut wheel: MessageReader<MouseWheel>,
    cursor: Res<UiCursor>,
    mut q: Query<(&ComputedNode, &UiGlobalTransform, &mut ScrollPosition), With<Scrollable>>,
) {
    let dy: f32 = wheel
        .read()
        .map(|w| match w.unit {
            MouseScrollUnit::Line => w.y * 40.0,
            MouseScrollUnit::Pixel => w.y,
        })
        .sum();
    let Some(point) = cursor.0 else { return };
    if dy == 0.0 {
        return;
    }
    for (node, transform, mut scroll) in &mut q {
        if node.contains_point(*transform, point) {
            let max = (node.content_size.y - node.size.y).max(0.0) * node.inverse_scale_factor;
            scroll.y = (scroll.y - dy).clamp(0.0, max);
        }
    }
}

pub fn row() -> Node {
    Node {
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        column_gap: px(8),
        ..default()
    }
}

/// Full-screen transparent root used by menu screens.
pub fn screen_root() -> Node {
    Node {
        width: percent(100),
        height: percent(100),
        position_type: PositionType::Absolute,
        ..default()
    }
}

fn style_buttons(
    mut q: Query<
        (&Interaction, &mut BackgroundColor, Has<Selected>, Has<Disabled>),
        (With<UiButton>, Without<TextInput>),
    >,
) {
    for (interaction, mut bg, selected, disabled) in &mut q {
        let target = if disabled {
            Color::srgba(0.12, 0.12, 0.14, 0.7)
        } else if selected {
            match interaction {
                Interaction::Pressed => BG_PRESSED,
                _ => BG_SELECTED,
            }
        } else {
            match interaction {
                Interaction::Pressed => BG_PRESSED,
                Interaction::Hovered => BG_HOVER,
                Interaction::None => BG_BUTTON,
            }
        };
        if bg.0 != target {
            bg.0 = target;
        }
    }
}

fn button_sounds(q: Query<&Interaction, (Changed<Interaction>, With<UiButton>)>, mut sfx: MessageWriter<PlaySfx>) {
    for i in &q {
        match i {
            Interaction::Pressed => {
                sfx.write(PlaySfx::ui(Sound::Click));
            }
            Interaction::Hovered => {
                sfx.write(PlaySfx {
                    volume: 0.5,
                    ..PlaySfx::ui(Sound::Hover)
                });
            }
            Interaction::None => {}
        }
    }
}

/// Helper used by screens: returns actions of buttons clicked this frame.
pub fn clicked<'a, A: Component>(
    q: &'a Query<(&Interaction, &A), (Changed<Interaction>, With<UiButton>, Without<Disabled>)>,
) -> impl Iterator<Item = &'a A> + 'a {
    q.iter()
        .filter(|(i, _)| **i == Interaction::Pressed)
        .map(|(_, a)| a)
}

/// Query type for `clicked`.
pub type Clicked<'w, 's, A> =
    Query<'w, 's, (&'static Interaction, &'static A), (Changed<Interaction>, With<UiButton>, Without<Disabled>)>;

// ---------------------------------------------------------------- text input

#[derive(Component)]
pub struct TextInput {
    pub value: String,
    pub placeholder: String,
    pub masked: bool,
    pub max_len: usize,
    pub focused: bool,
}

#[derive(Component)]
struct TextInputLabel;

pub fn text_input<A: Component>(
    p: &mut ChildSpawnerCommands,
    value: &str,
    placeholder: &str,
    masked: bool,
    max_len: usize,
    tag: A,
) -> Entity {
    p.spawn((
        UiButton,
        tag,
        TextInput {
            value: value.to_string(),
            placeholder: placeholder.to_string(),
            masked,
            max_len,
            focused: false,
        },
        Node {
            width: percent(100),
            height: px(38),
            align_items: AlignItems::Center,
            padding: UiRect::horizontal(px(10)),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(4)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.02, 0.02, 0.03, 0.9)),
        BorderColor::all(BORDER),
    ))
    .with_children(|b| {
        b.spawn((TextInputLabel, Text::new(""), font(18.0), TextColor(TEXT)));
    })
    .id()
}

fn focus_text_inputs(
    mouse: Res<ButtonInput<MouseButton>>,
    mut q: Query<(Entity, &Interaction, &mut TextInput)>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let clicked = q
        .iter()
        .find(|(_, i, _)| **i == Interaction::Pressed)
        .map(|(e, _, _)| e);
    for (e, _, mut input) in &mut q {
        let focus = Some(e) == clicked;
        if input.focused != focus {
            input.focused = focus;
        }
    }
}

fn type_into_inputs(mut keys: MessageReader<KeyboardInput>, mut q: Query<&mut TextInput>) {
    let events: Vec<_> = keys.read().filter(|k| k.state == ButtonState::Pressed).cloned().collect();
    if events.is_empty() {
        return;
    }
    for mut input in &mut q {
        if !input.focused {
            continue;
        }
        for ev in &events {
            match &ev.logical_key {
                Key::Backspace => {
                    input.value.pop();
                }
                Key::Enter | Key::Escape | Key::Tab => input.focused = false,
                Key::Space => {
                    if input.value.chars().count() < input.max_len {
                        input.value.push(' ');
                    }
                }
                Key::Character(s) => {
                    for c in s.chars() {
                        if !c.is_control() && input.value.chars().count() < input.max_len {
                            input.value.push(c);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

fn render_text_inputs(
    time: Res<Time>,
    inputs: Query<(&TextInput, &Children, &mut BorderColor)>,
    mut labels: Query<(&mut Text, &mut TextColor), With<TextInputLabel>>,
) {
    let caret = (time.elapsed_secs() * 2.0) as u32 % 2 == 0;
    for (input, children, mut border) in inputs {
        let want_border = if input.focused { ACCENT } else { BORDER };
        if border.top != want_border {
            *border = BorderColor::all(want_border);
        }
        for child in children.iter() {
            let Ok((mut text, mut color)) = labels.get_mut(child) else { continue };
            let shown = if input.value.is_empty() && !input.focused {
                color.0 = TEXT_DIM;
                input.placeholder.clone()
            } else {
                color.0 = TEXT;
                let v = if input.masked {
                    "*".repeat(input.value.chars().count())
                } else {
                    input.value.clone()
                };
                if input.focused && caret { format!("{v}|") } else { v }
            };
            if text.0 != shown {
                text.0 = shown;
            }
        }
    }
}

pub struct WidgetsPlugin;

impl Plugin for WidgetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, update_interactions.after(bevy::input::InputSystems))
            .add_systems(
            Update,
            (
                scroll_wheel,
                style_buttons,
                button_sounds,
                focus_text_inputs,
                type_into_inputs,
                render_text_inputs,
            )
                .chain(),
        );
    }
}
