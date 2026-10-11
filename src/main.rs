//! Space Invaders: The Upside Down Odyssey, in 3D.
//! Step 1: window/canvas plumbing and a first lit 3D scene (tilted arcade table).

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
        .add_systems(Startup, setup)
        .add_systems(Update, bob)
        .run();
}

#[derive(Component)]
struct Bob(f32);

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Tilted arcade-table camera: above and behind the ship, looking down the table.
    let mut camera = commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 11.0, 13.0).looking_at(Vec3::new(0.0, 0.0, -3.0), Vec3::Y),
        DistanceFog {
            color: Color::srgb(0.05, 0.01, 0.06),
            falloff: FogFalloff::Linear { start: 14.0, end: 38.0 },
            ..default()
        },
    ));
    #[cfg(feature = "webgpu")]
    camera.insert(bevy::post_process::bloom::Bloom::NATURAL);
    let _ = &mut camera;

    commands.spawn((
        DirectionalLight { illuminance: 6000.0, ..default() },
        Transform::from_xyz(-4.0, 10.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // The table.
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(16.0, 30.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.10, 0.03, 0.08),
            perceptual_roughness: 0.35,
            metallic: 0.2,
            ..default()
        })),
        Transform::from_xyz(0.0, 0.0, -6.0),
    ));

    // Placeholder formation: three rows of glowing creatures.
    let rows = [
        (Color::srgb(1.0, 0.35, 0.94), -9.0),
        (Color::srgb(0.35, 0.84, 1.0), -7.6),
        (Color::srgb(0.62, 1.0, 0.35), -6.2),
    ];
    let cube = meshes.add(Cuboid::new(0.8, 0.5, 0.8));
    for (r, (color, z)) in rows.iter().enumerate() {
        let mat = materials.add(StandardMaterial {
            base_color: *color,
            emissive: LinearRgba::from(*color) * 2.5,
            ..default()
        });
        for c in 0..9 {
            commands.spawn((
                Mesh3d(cube.clone()),
                MeshMaterial3d(mat.clone()),
                Transform::from_xyz(-5.6 + c as f32 * 1.4, 0.6, *z),
                Bob(r as f32 * 0.7 + c as f32 * 0.35),
            ));
        }
    }

    // Placeholder ship.
    commands.spawn((
        Mesh3d(meshes.add(Cone { radius: 0.6, height: 1.4 }.mesh().resolution(4))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.95, 0.92, 0.85),
            emissive: LinearRgba::rgb(0.2, 0.9, 0.8),
            ..default()
        })),
        Transform::from_xyz(0.0, 0.7, 5.0).with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    ));

    commands.spawn((
        Text::new("SPACE INVADERS 3D - step 1"),
        TextFont { font_size: 22.0, ..default() },
        TextColor(Color::srgb(0.95, 0.92, 0.85)),
        Node { position_type: PositionType::Absolute, top: Val::Px(12.0), left: Val::Px(14.0), ..default() },
    ));
}

fn bob(time: Res<Time>, mut q: Query<(&Bob, &mut Transform)>) {
    for (b, mut t) in &mut q {
        t.translation.y = 0.6 + (time.elapsed_secs() * 2.0 + b.0).sin() * 0.12;
    }
}
