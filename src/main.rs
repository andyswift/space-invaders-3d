//! Space Invaders: The Upside Down Odyssey, in 3D (Rust + Bevy + WebAssembly).

mod data;
mod game;
mod ui;

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Space Invaders 3D".into(),
                canvas: Some("#game".into()),
                fit_canvas_to_parent: true,
                prevent_default_event_handling: true,
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.02, 0.01, 0.04)))
        .insert_resource(Time::<Fixed>::from_hz(60.0))
        .add_plugins((game::GamePlugin, ui::UiPlugin))
        .run();
}
