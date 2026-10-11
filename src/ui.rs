//! HUD and overlay text. Plain ASCII: Bevy's built-in font has no fancy glyphs.

use bevy::prelude::*;

use crate::game::{GameState, Stats};
use crate::realm::{Realm, RealmKind};

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_ui).add_systems(Update, (update_ui, update_realm_ui));
    }
}

#[derive(Component, Clone, Copy)]
enum Hud {
    Score,
    Hi,
    Wave,
    Lives,
    Banner,
    /// Red warning text before a flip.
    Warn,
    /// Arrival banner after a flip.
    Arrival,
    /// Countdown to resurfacing while underground.
    Surfacing,
}

/// Pulsing frame around the screen while the ground rumbles.
#[derive(Component)]
struct WarnBorder;

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
    // Realm messages: warning, arrival, and the countdown home.
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(24.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(10.0),
            ..default()
        },
        children![
            (Text::new(""), TextFont { font_size: 30.0, ..default() }, TextColor(Color::NONE), Hud::Warn),
            (Text::new(""), TextFont { font_size: 40.0, ..default() }, TextColor(Color::NONE), Hud::Arrival),
        ],
    ));
    commands.spawn((
        Node { position_type: PositionType::Absolute, top: Val::Px(40.0), width: Val::Percent(100.0), justify_content: JustifyContent::Center, ..default() },
        children![(Text::new(""), TextFont { font_size: 15.0, ..default() }, TextColor(Color::srgb(1.0, 0.54, 0.24)), Hud::Surfacing)],
    ));
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            border: UiRect::all(Val::Px(14.0)),
            ..default()
        },
        BorderColor::all(Color::NONE),
        WarnBorder,
    ));
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
            Hud::Warn | Hud::Arrival | Hud::Surfacing => continue,
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

/// Warning before a flip, the arrival banner, and the countdown home. All driven by the realm clock.
fn update_realm_ui(
    time: Res<Time>,
    realm: Res<Realm>,
    state: Res<State<GameState>>,
    mut texts: Query<(&Hud, &mut Text, &mut TextColor)>,
    mut border: Query<&mut BorderColor, With<WarnBorder>>,
) {
    let playing = *state.get() == GameState::Playing;
    let pulse = (time.elapsed_secs() * 10.0).sin() * 0.5 + 0.5;
    let cave = realm.kind == RealmKind::Cave;
    for (hud, mut text, mut color) in &mut texts {
        let (s, c) = match hud {
            Hud::Warn if playing && realm.warning() => (
                if cave { "RETURNING TO THE SURFACE" } else { "THE GATE IS OPENING" }.to_string(),
                if cave { Color::srgba(1.0, 0.54, 0.24, 0.55 + 0.45 * pulse) } else { Color::srgba(1.0, 0.16, 0.24, 0.55 + 0.45 * pulse) },
            ),
            Hud::Arrival if playing && realm.banner > 0.0 && !realm.flipping() => (
                if cave { "THE UNDERGROUND" } else { "THE SURFACE" }.to_string(),
                if cave { Color::srgba(0.25, 0.94, 0.82, realm.banner.min(1.0)) } else { Color::srgba(0.95, 0.92, 0.85, realm.banner.min(1.0)) },
            ),
            Hud::Surfacing if playing && cave && !realm.flipping() => (
                format!("SURFACING IN {}s", realm.timer.max(0.0).ceil() as u32),
                Color::srgb(1.0, 0.54, 0.24),
            ),
            Hud::Warn | Hud::Arrival | Hud::Surfacing => (String::new(), Color::NONE),
            _ => continue,
        };
        if text.0 != s {
            text.0 = s;
        }
        color.0 = c;
    }
    for mut b in &mut border {
        *b = if playing && realm.warning() {
            let a = 0.25 + 0.5 * pulse;
            BorderColor::all(if cave { Color::srgba(1.0, 0.54, 0.24, a) } else { Color::srgba(1.0, 0.16, 0.24, a) })
        } else {
            BorderColor::all(Color::NONE)
        };
    }
}
