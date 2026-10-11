//! The two worlds (surface and underground) and the flip between them: procedural creatures, the cave,
//! the roll-over transition, and the warning rumble that comes first.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use bevy::prelude::*;

use crate::game::{self, Art, GameState, Respawn, Rng};

const SURFACE_FOR: (f32, f32) = (26.0, 34.0);
const CAVE_FOR: (f32, f32) = (18.0, 24.0);
const WARN_TIME: f32 = 2.0;
const FLIP_TIME: f32 = 0.9;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RealmKind {
    Surface,
    Cave,
}

impl RealmKind {
    pub fn idx(self) -> usize {
        self as usize
    }
    fn other(self) -> RealmKind {
        match self {
            RealmKind::Surface => RealmKind::Cave,
            RealmKind::Cave => RealmKind::Surface,
        }
    }
}

#[derive(Resource)]
pub struct Realm {
    pub kind: RealmKind,
    pub timer: f32,
    pub len: f32,
    /// 0 = not flipping, otherwise progress 0..1 of the roll-over.
    pub flip: f32,
    pub warned: bool,
    /// Counts down after arriving, to show the "THE UNDERGROUND" banner.
    pub banner: f32,
}

impl Realm {
    pub fn fresh(rng: &mut Rng) -> Self {
        let len = rng.range(SURFACE_FOR.0, SURFACE_FOR.1);
        Realm { kind: RealmKind::Surface, timer: len, len, flip: 0.0, warned: false, banner: 0.0 }
    }
    pub fn flipping(&self) -> bool {
        self.flip > 0.0
    }
    pub fn warning(&self) -> bool {
        self.warned && !self.flipping()
    }
    fn arm(&mut self, rng: &mut Rng) {
        let (a, b) = if self.kind == RealmKind::Cave { CAVE_FOR } else { SURFACE_FOR };
        self.len = rng.range(a, b);
        self.timer = self.len;
        self.warned = false;
    }
}

#[derive(Component)]
pub struct SurfaceEnv;
#[derive(Component)]
pub struct CaveEnv;
/// Lets the camera sit still while the warning shake plays on top of it.
#[derive(Resource, Default)]
pub struct CamBase(pub Transform);

pub struct RealmPlugin;

impl Plugin for RealmPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CamBase>()
            .add_systems(Startup, spawn_env.after(game::setup_art))
            .add_systems(Update, (env_flip, apply_theme, camera_shake).chain().after(game::fit_camera))
            .add_systems(Update, debug_keys)
            .add_systems(
                FixedUpdate,
                realm_clock.run_if(in_state(GameState::Playing)).run_if(|r: Res<Respawn>| r.0 <= 0.0),
            );
    }
}

/// Developer shortcut (debug builds only): F brings the next flip forward.
fn debug_keys(keys: Res<ButtonInput<KeyCode>>, mut realm: ResMut<Realm>) {
    #[cfg(debug_assertions)]
    if keys.just_pressed(KeyCode::KeyF) && !realm.flipping() {
        info!("[realm] F pressed: timer {:.1} -> {:.1}", realm.timer, realm.timer.min(WARN_TIME + 0.2));
        realm.timer = realm.timer.min(WARN_TIME + 0.2);
    }
    #[cfg(not(debug_assertions))]
    let _ = (&keys, &mut realm);
}

fn realm_clock(time: Res<Time>, mut realm: ResMut<Realm>, mut rng: ResMut<Rng>) {
    let dt = time.delta_secs();
    realm.banner = (realm.banner - dt).max(0.0);
    if realm.flipping() {
        let before = realm.flip;
        realm.flip += dt / FLIP_TIME;
        if before < 0.5 && realm.flip >= 0.5 {
            realm.kind = realm.kind.other();
            info!("[realm] swapped to {:?}", realm.kind);
            crate::sfx("swap", 0.0);
        }
        if realm.flip >= 1.0 {
            realm.flip = 0.0;
            realm.banner = 1.8;
            realm.arm(&mut rng);
        }
        return;
    }
    realm.timer -= dt;
    if realm.timer <= WARN_TIME && !realm.warned {
        realm.warned = true;
        crate::sfx("rumble", 0.0);
    }
    if realm.timer <= 0.0 {
        info!("[realm] flip starts from {:?}", realm.kind);
        realm.flip = 0.0001;
        realm.warned = false;
        crate::sfx("flip", 0.0);
    }
}

fn ease(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Roll both environments over the table's long axis. The leaving world turns away by theta while the
/// arriving one turns in from a half-turn behind it; the active world swaps at the midpoint, when both
/// are edge-on and the change cannot be seen.
fn env_flip(
    realm: Res<Realm>,
    mut surface: Query<(&mut Transform, &mut Visibility), (With<SurfaceEnv>, Without<CaveEnv>)>,
    mut cave: Query<(&mut Transform, &mut Visibility), (With<CaveEnv>, Without<SurfaceEnv>)>,
) {
    let (s_angle, c_angle) = if realm.flipping() {
        let theta = PI * ease(realm.flip);
        let origin = if realm.flip < 0.5 { realm.kind } else { realm.kind.other() };
        match origin {
            RealmKind::Surface => (theta, PI + theta),
            RealmKind::Cave => (PI + theta, theta),
        }
    } else {
        match realm.kind {
            RealmKind::Surface => (0.0, PI),
            RealmKind::Cave => (PI, 0.0),
        }
    };
    let in_cave = realm.kind == RealmKind::Cave;
    for (mut t, mut v) in &mut surface {
        t.rotation = Quat::from_rotation_z(s_angle);
        *v = if in_cave { Visibility::Hidden } else { Visibility::Inherited };
    }
    for (mut t, mut v) in &mut cave {
        t.rotation = Quat::from_rotation_z(c_angle);
        *v = if in_cave { Visibility::Inherited } else { Visibility::Hidden };
    }
}

/// Re-skin lighting, fog and every living thing whenever the active world changes.
#[allow(clippy::type_complexity)]
fn apply_theme(
    realm: Res<Realm>,
    mut art: ResMut<Art>,
    mut last: Local<Option<RealmKind>>,
    mut clear: ResMut<ClearColor>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut sun: Query<&mut DirectionalLight>,
    mut fog: Query<&mut DistanceFog>,
    mut things: Query<(
        &mut MeshMaterial3d<StandardMaterial>,
        Option<&game::Invader>,
        Has<game::Player>,
        Has<game::ShieldVoxel>,
        Has<game::PlayerBullet>,
        Has<game::Bomb>,
        Has<game::Ufo>,
    )>,
) {
    if *last == Some(realm.kind) {
        return;
    }
    *last = Some(realm.kind);
    art.cur = realm.kind.idx();
    let cave = realm.kind == RealmKind::Cave;
    clear.0 = if cave { Color::srgb(0.02, 0.012, 0.05) } else { Color::srgb(0.02, 0.01, 0.04) };
    ambient.color = if cave { Color::srgb(0.35, 0.28, 0.6) } else { Color::srgb(0.5, 0.25, 0.4) };
    ambient.brightness = if cave { 260.0 } else { 220.0 };
    for mut s in &mut sun {
        s.color = if cave { Color::srgb(0.7, 0.75, 1.0) } else { Color::srgb(1.0, 0.85, 0.9) };
        s.illuminance = if cave { 3200.0 } else { 5500.0 };
    }
    for mut f in &mut fog {
        f.color = if cave { Color::srgb(0.04, 0.025, 0.09) } else { Color::srgb(0.05, 0.01, 0.06) };
    }
    let set = art.cur();
    for (mut m, inv, player, voxel, bullet, bomb, ufo) in &mut things {
        let h = if let Some(i) = inv {
            Some(set.inv_mat[i.kind_index()].clone())
        } else if player {
            Some(set.ship_mat.clone())
        } else if voxel {
            Some(set.voxel_mat.clone())
        } else if bullet {
            Some(set.bullet_mat.clone())
        } else if bomb {
            Some(set.bomb_mat.clone())
        } else if ufo {
            Some(set.ufo_mat.clone())
        } else {
            None
        };
        if let Some(h) = h {
            m.0 = h;
        }
    }
}

/// Camera rumble: a slow warning tremble, and a heavier shake while the world turns over.
fn camera_shake(
    time: Res<Time>,
    realm: Res<Realm>,
    base: Res<CamBase>,
    mut rng: ResMut<Rng>,
    mut cam: Query<&mut Transform, With<game::MainCamera>>,
) {
    let Ok(mut t) = cam.single_mut() else { return };
    let amp = if realm.flipping() {
        0.16 * (PI * realm.flip).sin()
    } else if realm.warning() {
        0.05 + 0.05 * (time.elapsed_secs() * 40.0).sin().abs()
    } else {
        0.0
    };
    let mut tf = base.0;
    tf.translation += Vec3::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)) * amp;
    *t = tf;
}

// ---------------------------------------------------------------------------------------------
// Procedural meshes
// ---------------------------------------------------------------------------------------------

fn merge_all(parts: Vec<Mesh>) -> Mesh {
    let mut it = parts.into_iter();
    let mut out = it.next().expect("at least one part");
    for m in it {
        out.merge(&m).expect("primitive meshes share a layout");
    }
    out
}

fn ellipsoid(radii: Vec3, at: Vec3, rot: Quat) -> Mesh {
    Mesh::from(Sphere::new(1.0).mesh().uv(14, 9)).scaled_by(radii).rotated_by(rot).translated_by(at)
}

fn limb(radius: f32, length: f32, from: Vec3, rot: Quat) -> Mesh {
    // A capsule that starts at `from` and extends along the rotated +Y axis.
    let c = Mesh::from(Capsule3d::new(radius, length).mesh().latitudes(4).longitudes(8));
    c.translated_by(Vec3::new(0.0, length / 2.0 + radius, 0.0)).rotated_by(rot).translated_by(from)
}

/// Underground creatures: real 3D bodies, two walk frames each. Squid -> spore jelly, crab -> demogorgon, octopus -> spider.
pub fn cave_creature(kind: usize, frame: usize) -> Mesh {
    let f = if frame == 0 { 1.0 } else { -1.0 };
    match kind {
        // Spore jelly: a glowing dome with trailing tentacles.
        0 => {
            let mut parts = vec![ellipsoid(Vec3::new(0.5, 0.42, 0.45), Vec3::new(0.0, 0.8, 0.0), Quat::IDENTITY)];
            for i in 0..4 {
                let x = -0.33 + i as f32 * 0.22;
                let sway = f * (if i % 2 == 0 { 0.3 } else { -0.3 });
                parts.push(limb(0.055, 0.55, Vec3::new(x, 0.62, 0.0), Quat::from_rotation_z(PI + sway * 0.5)));
            }
            merge_all(parts)
        }
        // Demogorgon: five petals around a dark mouth.
        1 => {
            let mut parts = vec![ellipsoid(Vec3::new(0.2, 0.2, 0.16), Vec3::new(0.0, 0.7, 0.0), Quat::IDENTITY)];
            for i in 0..5 {
                let a = i as f32 / 5.0 * TAU + 0.12 * f;
                let dir = Vec3::new(-a.sin(), a.cos(), 0.0);
                parts.push(ellipsoid(
                    Vec3::new(0.16, 0.5 + 0.05 * f, 0.12),
                    Vec3::new(0.0, 0.7, 0.0) + dir * (0.42 + 0.03 * f),
                    Quat::from_rotation_z(a),
                ));
            }
            merge_all(parts)
        }
        // Spider: a bulb body on six jointed legs.
        _ => {
            let mut parts = vec![ellipsoid(Vec3::new(0.38, 0.3, 0.34), Vec3::new(0.0, 0.62, 0.0), Quat::IDENTITY)];
            for s in [-1.0_f32, 1.0] {
                for i in 0..3 {
                    let spread = (i as f32 - 1.0) * 0.2;
                    let flex = if (i + (frame as i32 as usize)) % 2 == 0 { 0.2 } else { -0.2 };
                    parts.push(limb(
                        0.05,
                        0.62,
                        Vec3::new(s * 0.3, 0.64, spread),
                        Quat::from_rotation_z(-s * (1.15 + flex)) * Quat::from_rotation_y(spread * 0.6),
                    ));
                }
            }
            merge_all(parts)
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Environments
// ---------------------------------------------------------------------------------------------

fn emissive(color: Color, strength: f32) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        emissive: LinearRgba::from(color) * strength,
        unlit: false,
        ..default()
    }
}

fn spawn_env(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rng: ResMut<Rng>,
) {
    // ---------------- surface ----------------
    let surface = commands
        .spawn((SurfaceEnv, Transform::default(), Visibility::Inherited, InheritedVisibility::default()))
        .id();
    let table = commands
        .spawn((
            Mesh3d(meshes.add(Plane3d::default().mesh().size(26.0, 42.0))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.10, 0.03, 0.08),
                perceptual_roughness: 0.35,
                metallic: 0.2,
                double_sided: true,
                cull_mode: None,
                ..default()
            })),
            Transform::from_xyz(0.0, 0.0, -7.0),
        ))
        .id();
    let grid_mat = materials.add(emissive(Color::srgb(1.0, 0.16, 0.24), 1.6));
    let mut kids = vec![table];
    for i in -6..=6 {
        kids.push(
            commands
                .spawn((
                    Mesh3d(meshes.add(Cuboid::new(0.04, 0.02, 40.0))),
                    MeshMaterial3d(grid_mat.clone()),
                    Transform::from_xyz(i as f32 * 2.0, 0.012, -7.0),
                ))
                .id(),
        );
    }
    for j in 0..21 {
        kids.push(
            commands
                .spawn((
                    Mesh3d(meshes.add(Cuboid::new(24.0, 0.02, 0.04))),
                    MeshMaterial3d(grid_mat.clone()),
                    Transform::from_xyz(0.0, 0.012, -26.0 + j as f32 * 2.0),
                ))
                .id(),
        );
    }
    // A few stars on the far horizon.
    let star = meshes.add(Cuboid::new(0.12, 0.12, 0.12));
    let star_mat = materials.add(emissive(Color::srgb(1.0, 0.95, 0.85), 3.0));
    for _ in 0..70 {
        kids.push(
            commands
                .spawn((
                    Mesh3d(star.clone()),
                    MeshMaterial3d(star_mat.clone()),
                    Transform::from_xyz(rng.range(-34.0, 34.0), rng.range(3.0, 22.0), -rng.range(30.0, 36.0)),
                ))
                .id(),
        );
    }
    commands.entity(surface).add_children(&kids);

    // ---------------- cave ----------------
    let cave = commands
        .spawn((
            CaveEnv,
            Transform::from_rotation(Quat::from_rotation_z(PI)),
            Visibility::Hidden,
            InheritedVisibility::default(),
        ))
        .id();
    let rock = materials.add(StandardMaterial {
        base_color: Color::srgb(0.13, 0.09, 0.2),
        perceptual_roughness: 0.9,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    let rock_dark = materials.add(StandardMaterial {
        base_color: Color::srgb(0.06, 0.04, 0.1),
        perceptual_roughness: 1.0,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    let mut kids = vec![commands
        .spawn((
            Mesh3d(meshes.add(Plane3d::default().mesh().size(26.0, 42.0))),
            MeshMaterial3d(rock_dark.clone()),
            Transform::from_xyz(0.0, 0.0, -7.0),
        ))
        .id()];
    // Jagged walls down both sides, and boulders on the floor.
    let block = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    for side in [-1.0_f32, 1.0] {
        for i in 0..26 {
            let h = rng.range(2.0, 8.0);
            let w = rng.range(1.6, 3.4);
            kids.push(
                commands
                    .spawn((
                        Mesh3d(block.clone()),
                        MeshMaterial3d(rock.clone()),
                        Transform::from_xyz(side * rng.range(12.2, 14.5), h / 2.0 - 0.3, -27.0 + i as f32 * 1.6)
                            .with_rotation(Quat::from_rotation_y(rng.range(-0.6, 0.6)) * Quat::from_rotation_z(rng.range(-0.15, 0.15)))
                            .with_scale(Vec3::new(w, h, rng.range(1.4, 3.0))),
                    ))
                    .id(),
            );
        }
    }
    for _ in 0..14 {
        let s = rng.range(0.4, 1.0);
        let side = if rng.f32() < 0.5 { -1.0 } else { 1.0 };
        kids.push(
            commands
                .spawn((
                    Mesh3d(block.clone()),
                    MeshMaterial3d(rock.clone()),
                    Transform::from_xyz(side * rng.range(10.4, 11.8), s * 0.3, rng.range(-24.0, 8.0))
                        .with_rotation(Quat::from_euler(EulerRot::XYZ, rng.range(0.0, 1.0), rng.range(0.0, 1.0), rng.range(0.0, 1.0)))
                        .with_scale(Vec3::splat(s)),
                ))
                .id(),
        );
    }
    // The gate back to the surface, glowing at the far end.
    let ember = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.54, 0.24),
        emissive: LinearRgba::rgb(1.0, 0.45, 0.12) * 5.0,
        ..default()
    });
    let glow_disc = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.7, 0.4),
        emissive: LinearRgba::rgb(1.0, 0.5, 0.2) * 1.6,
        unlit: true,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    kids.push(
        commands
            .spawn((
                Mesh3d(meshes.add(Torus::new(2.1, 2.5))),
                MeshMaterial3d(ember),
                Transform::from_xyz(0.0, 2.0, -26.5).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
            ))
            .id(),
    );
    kids.push(
        commands
            .spawn((
                Mesh3d(meshes.add(Circle::new(2.1))),
                MeshMaterial3d(glow_disc),
                Transform::from_xyz(0.0, 2.0, -26.6),
            ))
            .id(),
    );
    // Stalactites hanging in the distance, and glowing fungus on the floor.
    let spike = meshes.add(Cone { radius: 0.55, height: 3.0 }.mesh().resolution(7));
    for _ in 0..16 {
        let l = rng.range(0.7, 1.7);
        kids.push(
            commands
                .spawn((
                    Mesh3d(spike.clone()),
                    MeshMaterial3d(rock.clone()),
                    Transform::from_xyz(rng.range(-12.0, 12.0), 9.0, rng.range(-26.0, -8.0))
                        .with_rotation(Quat::from_rotation_x(PI))
                        .with_scale(Vec3::new(l, l * 1.3, l)),
                ))
                .id(),
        );
    }
    let bulb = meshes.add(Sphere::new(0.16).mesh().uv(8, 6));
    let colors = [
        Color::srgb(0.25, 0.94, 0.82),
        Color::srgb(0.43, 1.0, 0.82),
        Color::srgb(0.71, 0.55, 1.0),
        Color::srgb(0.62, 1.0, 0.35),
    ];
    let fungus: Vec<Handle<StandardMaterial>> = colors.iter().map(|c| materials.add(emissive(*c, 4.0))).collect();
    for i in 0..70 {
        let side = if rng.f32() < 0.5 { -1.0 } else { 1.0 };
        let x = side * rng.range(9.6, 12.0);
        kids.push(
            commands
                .spawn((
                    Mesh3d(bulb.clone()),
                    MeshMaterial3d(fungus[i % fungus.len()].clone()),
                    Transform::from_xyz(x, 0.12, rng.range(-24.0, 9.0)).with_scale(Vec3::splat(rng.range(0.6, 1.6))),
                ))
                .id(),
        );
    }
    commands.entity(cave).add_children(&kids);
    // Cold light from the cave's own glow.
    for (x, z, c) in [(-8.0, -2.0, Color::srgb(0.25, 0.94, 0.82)), (8.0, -8.0, Color::srgb(0.71, 0.55, 1.0)), (0.0, -23.0, Color::srgb(1.0, 0.55, 0.25))] {
        let l = commands
            .spawn((
                PointLight { color: c, intensity: 260_000.0, range: 26.0, ..default() },
                Transform::from_xyz(x, 3.0, z),
            ))
            .id();
        commands.entity(cave).add_child(l);
    }
}
