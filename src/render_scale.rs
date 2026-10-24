//! Render resolution (like CS), applied to the whole game: 3D, UI, text and menus.
//!
//! With a custom resolution the main camera renders the world *and* the UI into a
//! texture of that size. A small 2D camera then shows that texture full-screen,
//! either stretched (e.g. 4:3 stretched) or with black bars. "Native" renders
//! straight to the window. Because the UI no longer lives in the window, mouse
//! hit-testing is done by `ui::widgets` using [`UiCursor`].

use bevy::camera::visibility::RenderLayers;
use bevy::camera::{ImageRenderTarget, RenderTarget};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::Msaa;
use bevy::window::{PrimaryWindow, WindowRef};

use crate::settings::Settings;

/// The camera that shows the world and the UI (menu showroom or first-person view).
#[derive(Component)]
pub struct MainCamera;

/// Mouse position in the UI's physical pixels (window or low-res texture), if over it.
#[derive(Resource, Default)]
pub struct UiCursor(pub Option<Vec2>);

/// The texture everything renders into while a custom resolution is active.
#[derive(Resource)]
struct ScaledView {
    image: Handle<Image>,
    size: UVec2,
}

/// Shows the scaled texture in the window.
#[derive(Component)]
struct PresentCamera;

#[derive(Component)]
struct PresentSprite;

/// Render layer used only for presenting the texture.
const PRESENT_LAYER: usize = 31;

pub struct RenderScalePlugin;

impl Plugin for RenderScalePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiCursor>()
            .add_systems(PreUpdate, update_ui_cursor.after(bevy::input::InputSystems))
            .add_systems(PostUpdate, apply_render_resolution);
    }
}

/// Aspect ratio of the 3D view (used for the field of view).
pub fn view_aspect(settings: &Settings, window: &Window) -> f32 {
    match settings.render_resolution {
        Some((w, h)) => w as f32 / h as f32,
        None => window.width() / window.height().max(1.0),
    }
}

/// Where the texture is drawn in the window (logical pixels): (offset, size).
fn presented_rect(settings: &Settings, window: &Window, size: UVec2) -> (Vec2, Vec2) {
    let win = Vec2::new(window.width(), window.height().max(1.0));
    let view_aspect = size.x as f32 / size.y as f32;
    let shown = if settings.stretched {
        win
    } else if view_aspect > win.x / win.y {
        Vec2::new(win.x, win.x / view_aspect)
    } else {
        Vec2::new(win.y * view_aspect, win.y)
    };
    ((win - shown) / 2.0, shown)
}

fn update_ui_cursor(
    settings: Res<Settings>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut cursor: ResMut<UiCursor>,
) {
    cursor.0 = match settings.render_resolution {
        None => window.physical_cursor_position(),
        Some((w, h)) => window.cursor_position().and_then(|c| {
            let size = UVec2::new(w, h);
            let (offset, shown) = presented_rect(&settings, &window, size);
            let p = (c - offset) / shown * size.as_vec2();
            (p.cmpge(Vec2::ZERO).all() && p.cmplt(size.as_vec2()).all()).then_some(p)
        }),
    };
}

#[allow(clippy::too_many_arguments)]
fn apply_render_resolution(
    mut commands: Commands,
    settings: Res<Settings>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut images: ResMut<Assets<Image>>,
    scaled: Option<Res<ScaledView>>,
    cams: Query<(Entity, Option<&RenderTarget>, Has<IsDefaultUiCamera>), With<MainCamera>>,
    present_cams: Query<Entity, With<PresentCamera>>,
    mut sprites: Query<(Entity, &mut Sprite), With<PresentSprite>>,
) {
    let Some((w, h)) = settings.render_resolution else {
        // Native: render straight to the window and drop the extra pieces.
        for (cam, target, _) in &cams {
            if !matches!(target, None | Some(RenderTarget::Window(_))) {
                commands.entity(cam).insert(RenderTarget::Window(WindowRef::Primary));
            }
        }
        for e in present_cams.iter().chain(sprites.iter().map(|(e, _)| e)) {
            commands.entity(e).despawn();
        }
        if scaled.is_some() {
            commands.remove_resource::<ScaledView>();
        }
        return;
    };

    let size = UVec2::new(w, h);
    let image = match &scaled {
        Some(s) if s.size == size => s.image.clone(),
        _ => {
            let handle = images.add(Image::new_target_texture(w, h, TextureFormat::Rgba8UnormSrgb, None));
            commands.insert_resource(ScaledView {
                image: handle.clone(),
                size,
            });
            handle
        }
    };
    // Same DPI scaling as the window so the UI keeps its usual proportions.
    let scale_factor = window.scale_factor();
    for (cam, target, is_ui_camera) in &cams {
        let up_to_date = matches!(target,
            Some(RenderTarget::Image(t)) if t.handle == image && t.scale_factor == scale_factor);
        if !up_to_date {
            commands.entity(cam).insert(RenderTarget::Image(ImageRenderTarget {
                handle: image.clone(),
                scale_factor,
            }));
        }
        if !is_ui_camera {
            commands.entity(cam).insert(IsDefaultUiCamera);
        }
    }

    if present_cams.is_empty() {
        commands.spawn((
            PresentCamera,
            Camera2d,
            Camera {
                order: 1,
                clear_color: ClearColorConfig::Custom(Color::BLACK),
                ..default()
            },
            Msaa::Off,
            RenderLayers::layer(PRESENT_LAYER),
        ));
    }

    let (_, shown) = presented_rect(&settings, &window, size);
    match sprites.single_mut() {
        Ok((_, mut sprite)) => {
            if sprite.image != image {
                sprite.image = image.clone();
            }
            if sprite.custom_size != Some(shown) {
                sprite.custom_size = Some(shown);
            }
        }
        Err(_) => {
            commands.spawn((
                PresentSprite,
                Sprite {
                    image,
                    custom_size: Some(shown),
                    ..default()
                },
                RenderLayers::layer(PRESENT_LAYER),
            ));
        }
    }
}
