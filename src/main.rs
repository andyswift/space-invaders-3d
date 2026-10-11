//! Space Invaders: The Upside Down Odyssey, in 3D (Rust + Bevy + WebAssembly).

mod data;
mod game;
mod realm;
mod ui;

use bevy::prelude::*;

/// Sound effects are synthesised by a few lines of WebAudio in index.html (no audio files, tiny download).
#[cfg(target_arch = "wasm32")]
mod web_audio {
    use wasm_bindgen::prelude::*;
    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_name = sfx)]
        pub fn sfx(name: &str, arg: f32);
        #[wasm_bindgen(js_name = hi_get)]
        pub fn hi_get() -> f64;
        #[wasm_bindgen(js_name = hi_set)]
        pub fn hi_set(v: f64);
    }
}

pub fn sfx(name: &str, arg: f32) {
    #[cfg(target_arch = "wasm32")]
    web_audio::sfx(name, arg);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (name, arg);
}

/// The saved high score (browser local storage; 0 where unavailable).
pub fn hi_get() -> u32 {
    #[cfg(target_arch = "wasm32")]
    return web_audio::hi_get() as u32;
    #[cfg(not(target_arch = "wasm32"))]
    0
}

pub fn hi_set(v: u32) {
    #[cfg(target_arch = "wasm32")]
    web_audio::hi_set(v as f64);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = v;
}

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
        .add_plugins((game::GamePlugin, realm::RealmPlugin, ui::UiPlugin))
        .run();
}
