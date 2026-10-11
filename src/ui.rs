//! HUD and overlay text. Plain ASCII: Bevy's built-in font has no fancy glyphs.

use bevy::prelude::*;

use crate::game::{GameState, Stats};

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_ui).add_systems(Update, update_ui);
    }
}

#[derive(Component, Clone, Copy)]
enum Hud {
    Score,
    Hi,
    Wave,
    Lives,
    Banner,
}

/// The dimmed panel behind the centre message (menu, pause, game over).
#[derive(Component)]
struct BannerRoot;

const CREAM: Color = Color::srgb(0.95, 0.92, 0.85);

fn label(hud: Hud, size: f32, color: Color) -> impl Bundle {
    (Text::new(""), TextFont { font_size: size, ..default() }, TextColor(color), hud)
}

fn setup_ui(mut commands: Commands) {
    // Top bar.
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            top: Val::Px(0.0),
            left: Val::Px(0.0),
            width: Val::Percent(100.0),
            padding: UiRect::all(Val::Px(14.0)),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        })
        .with_children(|p| {
            p.spawn(label(Hud::Score, 24.0, CREAM));
            p.spawn(label(Hud::Wave, 18.0, Color::srgb(0.35, 0.84, 1.0)));
            p.spawn(label(Hud::Hi, 24.0, Color::srgb(1.0, 0.35, 0.94)));
        });
    // Lives, bottom left.
    commands.spawn((
        Node { position_type: PositionType::Absolute, bottom: Val::Px(14.0), left: Val::Px(14.0), ..default() },
        children![label(Hud::Lives, 20.0, CREAM)],
    ));
    // Centre banner: a dimmed panel around the message (menu, pause, game over).
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            padding: UiRect::all(Val::Px(16.0)),
            ..default()
        },
        BannerRoot,
        children![(
            Node { padding: UiRect::all(Val::Px(26.0)), max_width: Val::Percent(100.0), ..default() },
            BackgroundColor(Color::srgba(0.02, 0.01, 0.04, 0.72)),
            children![(
                Text::new(""),
                TextFont { font_size: 32.0, ..default() },
                TextColor(CREAM),
                TextLayout::new_with_justify(Justify::Center),
                Hud::Banner,
            )],
        )],
    ));
}

fn update_ui(
    stats: Res<Stats>,
    state: Res<State<GameState>>,
    mut q: Query<(&Hud, &mut Text)>,
    mut banner: Query<&mut Visibility, With<BannerRoot>>,
) {
    for mut v in &mut banner {
        *v = if *state.get() == GameState::Playing { Visibility::Hidden } else { Visibility::Inherited };
    }
    for (hud, mut text) in &mut q {
        let s = match hud {
            Hud::Score => format!("SCORE {:06}", stats.score),
            Hud::Hi => format!("HI {:06}", stats.hi),
            Hud::Wave => format!("WAVE {:02}", stats.level),
            Hud::Lives => format!("LIVES {}", stats.lives),
            Hud::Banner => match state.get() {
                GameState::Menu => "SPACE INVADERS 3D\n\nPress ENTER or SPACE to start\n\nARROWS move   SPACE fire   P pause".to_string(),
                GameState::Over => format!("GAME OVER\n\nScore {}\n\nPress ENTER or SPACE to play again", stats.score),
                GameState::Paused => "PAUSED".to_string(),
                GameState::Playing => String::new(),
            },
        };
        if text.0 != s {
            text.0 = s;
        }
    }
}
