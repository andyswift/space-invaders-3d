//! Core game: states, the 3D table, creatures, projectiles, shields, scoring. Runs on a fixed 60 Hz clock.

use std::collections::HashSet;

use bevy::prelude::*;

use crate::data::*;
use crate::realm::{cave_creature, CamBase, Realm, RealmKind};
use crate::{hi_get, hi_set, sfx};

pub const COLS: usize = 11;
pub const ROWS: usize = 5;
const CELL_X: f32 = 1.6;
const CELL_Z: f32 = 1.45;
const START_Z: f32 = -12.5;
const HALF_W: f32 = 9.0;
const STEP_X: f32 = 0.32;
const DROP_Z: f32 = 0.85;
const PLAYER_Z: f32 = 6.4;
const PLAYER_LIMIT: f32 = 9.6;
const PLAYER_SPEED: f32 = 12.0;
const BULLET_SPEED: f32 = 26.0;
const SHIELD_Z: f32 = 2.6;
const SHIELD_CELL: f32 = 0.16;
const LOSE_Z: f32 = 5.0;
const FAR_Z: f32 = -19.0;
const CAM_TARGET: Vec3 = Vec3::new(0.0, 0.0, -4.0);
const UFO_Z: f32 = -15.5;
/// Standing sprites lean back this much so their faces turn up to the camera.
const LEAN: f32 = -0.6;
fn lean() -> Quat {
    Quat::from_rotation_x(LEAN)
}

#[derive(States, Default, Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub enum GameState {
    #[default]
    Menu,
    Playing,
    Paused,
    Over,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Squid,
    Crab,
    Octopus,
}

impl Kind {
    fn idx(self) -> usize {
        self as usize
    }
    fn points(self) -> u32 {
        match self {
            Kind::Squid => 30,
            Kind::Crab => 20,
            Kind::Octopus => 10,
        }
    }
    fn of_row(row: usize) -> Kind {
        match row {
            0 => Kind::Squid,
            1 | 2 => Kind::Crab,
            _ => Kind::Octopus,
        }
    }
}

#[derive(Resource)]
pub struct Stats {
    pub score: u32,
    pub hi: u32,
    pub lives: u32,
    pub level: u32,
}

impl Default for Stats {
    fn default() -> Self {
        Stats { score: 0, hi: 0, lives: 3, level: 1 }
    }
}

/// What the player is asking for this frame, from keyboard or touch.
#[derive(Resource, Default)]
pub struct Controls {
    pub left: bool,
    pub right: bool,
    pub fire: bool,
    /// Where a finger is steering the ship to (world x), if one is down.
    pub target_x: Option<f32>,
}

#[derive(Resource)]
pub struct Formation {
    pub dir: f32,
    pub off_x: f32,
    pub off_z: f32,
    pub timer: f32,
    pub frame: usize,
    pub alive: u32,
    pub bomb_timer: f32,
    pub ufo_timer: f32,
    pub stepped: bool,
}

impl Formation {
    fn fresh(level: u32, ufo_timer: f32) -> Self {
        Formation {
            dir: 1.0,
            off_x: 0.0,
            off_z: (level.saturating_sub(1)).min(5) as f32 * 0.3,
            timer: 0.0,
            frame: 0,
            alive: (COLS * ROWS) as u32,
            bomb_timer: 1.0,
            ufo_timer,
            stepped: false,
        }
    }
}

#[derive(Resource, Default)]
pub struct Respawn(pub f32);

#[derive(Resource)]
pub struct Rng(pub u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    pub fn f32(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + self.f32() * (b - a)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

#[derive(Component)]
pub struct Invader {
    pub col: usize,
    pub row: usize,
    pub kind: Kind,
}
impl Invader {
    pub fn kind_index(&self) -> usize {
        self.kind.idx()
    }
}

#[derive(Component)]
pub struct MainCamera;
#[derive(Component)]
pub struct Player;
#[derive(Component)]
pub struct PlayerBullet;
#[derive(Component)]
pub struct Bomb;
#[derive(Component)]
pub struct ShieldVoxel;
#[derive(Component)]
pub struct Ufo {
    vx: f32,
}
#[derive(Component)]
pub struct Particle {
    vel: Vec3,
    life: f32,
    max: f32,
}
/// Everything that belongs to one run and is cleared when a new game starts.
#[derive(Component)]
pub struct GameEntity;

/// Everything that differs between the two worlds: creature meshes and every material.
pub struct RealmArt {
    pub inv: [[Handle<Mesh>; 2]; 3],
    pub inv_mat: [Handle<StandardMaterial>; 3],
    pub ship_mat: Handle<StandardMaterial>,
    pub ufo_mat: Handle<StandardMaterial>,
    pub bullet_mat: Handle<StandardMaterial>,
    pub bomb_mat: Handle<StandardMaterial>,
    pub voxel_mat: Handle<StandardMaterial>,
    pub spark_white: Handle<StandardMaterial>,
}

#[derive(Resource)]
pub struct Art {
    sets: [RealmArt; 2],
    /// Index of the world currently shown (0 surface, 1 cave).
    pub cur: usize,
    ship: Handle<Mesh>,
    ufo: Handle<Mesh>,
    bullet: Handle<Mesh>,
    bomb: Handle<Mesh>,
    voxel: Handle<Mesh>,
    spark: Handle<Mesh>,
}

impl Art {
    pub fn cur(&self) -> &RealmArt {
        &self.sets[self.cur]
    }
}

fn glow(color: Color, strength: f32) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        emissive: LinearRgba::from(color) * strength,
        perceptual_roughness: 0.5,
        ..default()
    }
}

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>()
            .init_resource::<Stats>()
            .init_resource::<Controls>()
            .init_resource::<Respawn>()
            .insert_resource(Rng(0x2545_F491_4F6C_DD1D))
            .insert_resource(Formation::fresh(1, 15.0))
            .insert_resource(Realm { kind: RealmKind::Surface, timer: 30.0, len: 30.0, flip: 0.0, warned: false, banner: 0.0 })
            .add_systems(Startup, (load_hi, setup_art, setup_scene, new_game).chain())
            .add_systems(OnEnter(GameState::Over), save_hi)
            .add_systems(Update, pause_on_blur.run_if(in_state(GameState::Playing)))
            .add_systems(
                OnTransition { exited: GameState::Menu, entered: GameState::Playing },
                new_game,
            )
            .add_systems(
                OnTransition { exited: GameState::Over, entered: GameState::Playing },
                new_game,
            )
            .add_systems(Update, (read_input, state_input, animate_invaders, particles, fit_camera))
            .add_systems(
                FixedUpdate,
                (
                    tick_respawn,
                    (
                        move_player,
                        fire,
                        march,
                        crush_shields,
                        spawn_bombs,
                        move_projectiles,
                        collisions,
                        ufo_system,
                        level_check,
                    )
                        .chain()
                        .run_if(|r: Res<Respawn>, realm: Res<Realm>| r.0 <= 0.0 && !realm.flipping()),
                )
                    .chain()
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

pub fn setup_art(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let mut frames = |f: &[[&str; 8]; 2], vox: f32| [meshes.add(voxels(&f[0], vox, 0.5)), meshes.add(voxels(&f[1], vox, 0.5))];
    let surface_inv = [frames(&SQUID, 0.125), frames(&CRAB, 0.125), frames(&OCTOPUS, 0.125)];
    let cave_inv = [0, 1, 2].map(|k| [meshes.add(cave_creature(k, 0)), meshes.add(cave_creature(k, 1))]);

    let surface = RealmArt {
        inv: surface_inv,
        inv_mat: [
            materials.add(glow(Color::srgb(1.0, 0.35, 0.94), 2.0)),
            materials.add(glow(Color::srgb(0.35, 0.84, 1.0), 2.0)),
            materials.add(glow(Color::srgb(0.62, 1.0, 0.35), 2.0)),
        ],
        ship_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.95, 0.92, 0.85),
            emissive: LinearRgba::rgb(0.1, 0.5, 0.45),
            ..default()
        }),
        ufo_mat: materials.add(glow(Color::srgb(1.0, 0.3, 0.3), 2.5)),
        bullet_mat: materials.add(glow(Color::srgb(1.0, 0.85, 0.85), 5.0)),
        bomb_mat: materials.add(glow(Color::srgb(1.0, 0.8, 0.3), 4.0)),
        voxel_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.07, 0.65, 0.55),
            emissive: LinearRgba::rgb(0.0, 0.25, 0.2),
            perceptual_roughness: 0.6,
            ..default()
        }),
        spark_white: materials.add(glow(Color::WHITE, 4.0)),
    };
    let cave = RealmArt {
        inv: cave_inv,
        inv_mat: [
            materials.add(glow(Color::srgb(0.71, 0.55, 1.0), 1.4)),
            materials.add(glow(Color::srgb(1.0, 0.62, 0.35), 1.4)),
            materials.add(glow(Color::srgb(0.43, 1.0, 0.82), 1.4)),
        ],
        ship_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.93, 0.91, 1.0),
            emissive: LinearRgba::rgb(0.15, 0.6, 0.55),
            ..default()
        }),
        ufo_mat: materials.add(glow(Color::srgb(1.0, 0.54, 0.24), 2.5)),
        bullet_mat: materials.add(glow(Color::srgb(0.75, 1.0, 0.97), 5.0)),
        bomb_mat: materials.add(glow(Color::srgb(0.62, 1.0, 0.35), 4.0)),
        voxel_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.36, 0.28, 0.22),
            emissive: LinearRgba::rgb(0.05, 0.03, 0.02),
            perceptual_roughness: 0.9,
            ..default()
        }),
        spark_white: materials.add(glow(Color::srgb(0.85, 1.0, 0.95), 4.0)),
    };
    commands.insert_resource(Art {
        sets: [surface, cave],
        cur: 0,
        ship: meshes.add(voxels(&SHIP, 0.16, 0.8)),
        ufo: meshes.add(voxels(&UFO, 0.13, 0.5)),
        bullet: meshes.add(Cuboid::new(0.12, 0.12, 0.8)),
        bomb: meshes.add(Cuboid::new(0.16, 0.16, 0.5)),
        voxel: meshes.add(Cuboid::new(SHIELD_CELL * 0.94, 0.34, SHIELD_CELL * 0.94)),
        spark: meshes.add(Cuboid::new(0.12, 0.12, 0.12)),
    });
}

fn setup_scene(mut commands: Commands) {
    // Tilted arcade-table camera: above and behind the ship, looking down the table.
    let mut camera = commands.spawn((
        Camera3d::default(),
        MainCamera,
        Transform::from_xyz(0.0, 12.5, 15.5).looking_at(CAM_TARGET, Vec3::Y),
        DistanceFog {
            color: Color::srgb(0.05, 0.01, 0.06),
            falloff: FogFalloff::Linear { start: 16.0, end: 42.0 },
            ..default()
        },
    ));
    #[cfg(feature = "webgpu")]
    camera.insert(bevy::post_process::bloom::Bloom::NATURAL);
    let _ = &mut camera;

    commands.spawn((
        DirectionalLight { illuminance: 5500.0, color: Color::srgb(1.0, 0.85, 0.9), ..default() },
        Transform::from_xyz(-5.0, 12.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.insert_resource(GlobalAmbientLight { color: Color::srgb(0.5, 0.25, 0.4), brightness: 220.0, ..default() });
}

fn new_game(
    mut commands: Commands,
    art: Res<Art>,
    old: Query<Entity, With<GameEntity>>,
    mut stats: ResMut<Stats>,
    mut f: ResMut<Formation>,
    mut respawn: ResMut<Respawn>,
    mut rng: ResMut<Rng>,
    mut realm: ResMut<Realm>,
) {
    *realm = Realm::fresh(&mut rng);
    for e in &old {
        commands.entity(e).despawn();
    }
    stats.score = 0;
    stats.lives = 3;
    stats.level = 1;
    respawn.0 = 0.0;
    *f = Formation::fresh(1, rng.range(12.0, 20.0));
    spawn_wave(&mut commands, &art);
    spawn_shields(&mut commands, &art);
    spawn_player(&mut commands, &art);
}

fn spawn_player(commands: &mut Commands, art: &Art) {
    commands.spawn((
        Mesh3d(art.ship.clone()),
        MeshMaterial3d(art.cur().ship_mat.clone()),
        Transform::from_xyz(0.0, 0.0, PLAYER_Z).with_rotation(lean()),
        Player,
        GameEntity,
    ));
}

fn spawn_wave(commands: &mut Commands, art: &Art) {
    for row in 0..ROWS {
        let kind = Kind::of_row(row);
        for col in 0..COLS {
            commands.spawn((
                Mesh3d(art.cur().inv[kind.idx()][0].clone()),
                MeshMaterial3d(art.cur().inv_mat[kind.idx()].clone()),
                Transform::from_xyz(0.0, 0.0, START_Z).with_rotation(lean()),
                Invader { col, row, kind },
                GameEntity,
            ));
        }
    }
}

fn spawn_shields(commands: &mut Commands, art: &Art) {
    let cells = shield_cells();
    for cx in [-7.2_f32, -2.4, 2.4, 7.2] {
        for &(c, r) in &cells {
            commands.spawn((
                Mesh3d(art.voxel.clone()),
                MeshMaterial3d(art.cur().voxel_mat.clone()),
                Transform::from_xyz(
                    cx + (c as f32 - 9.5) * SHIELD_CELL,
                    0.17,
                    SHIELD_Z + (r as f32 - 6.5) * SHIELD_CELL,
                ),
                ShieldVoxel,
                GameEntity,
            ));
        }
    }
}

fn read_input(
    keys: Res<ButtonInput<KeyCode>>,
    touches: Res<Touches>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut c: ResMut<Controls>,
) {
    // Touch: the ship follows the finger and fires while it is down (one shot in flight at a time).
    c.target_x = None;
    let mut touching = false;
    if let (Some(t), Ok(w)) = (touches.iter().next(), windows.single()) {
        touching = true;
        let nx = (t.position().x / w.width().max(1.0)).clamp(0.0, 1.0);
        c.target_x = Some(((nx * 2.0 - 1.0) * 10.7).clamp(-PLAYER_LIMIT, PLAYER_LIMIT));
    }
    c.left = keys.pressed(KeyCode::ArrowLeft) || keys.pressed(KeyCode::KeyA);
    c.right = keys.pressed(KeyCode::ArrowRight) || keys.pressed(KeyCode::KeyD);
    c.fire = touching || keys.pressed(KeyCode::Space) || keys.pressed(KeyCode::ArrowUp) || keys.pressed(KeyCode::KeyW);
}

fn state_input(
    keys: Res<ButtonInput<KeyCode>>,
    touches: Res<Touches>,
    mouse: Res<ButtonInput<MouseButton>>,
    state: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
) {
    let tapped = touches.any_just_pressed() || mouse.just_pressed(MouseButton::Left);
    let start = keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space) || tapped;
    let pause = keys.just_pressed(KeyCode::KeyP) || keys.just_pressed(KeyCode::Escape);
    match state.get() {
        GameState::Menu | GameState::Over if start => next.set(GameState::Playing),
        GameState::Playing if pause => next.set(GameState::Paused),
        GameState::Paused if pause || start => next.set(GameState::Playing),
        _ => {}
    }
}

fn tick_respawn(time: Res<Time>, mut respawn: ResMut<Respawn>, mut q: Query<&mut Visibility, With<Player>>) {
    if respawn.0 > 0.0 {
        respawn.0 -= time.delta_secs();
        if respawn.0 <= 0.0 {
            for mut v in &mut q {
                *v = Visibility::Inherited;
            }
        }
    }
}

fn move_player(time: Res<Time>, c: Res<Controls>, mut q: Query<&mut Transform, With<Player>>) {
    let dir = (c.right as i32 - c.left as i32) as f32;
    for mut t in &mut q {
        let step = PLAYER_SPEED * time.delta_secs();
        let x = match c.target_x {
            // Steer towards the finger, a little faster than the keyboard speed.
            Some(target) => t.translation.x + (target - t.translation.x).clamp(-step * 1.6, step * 1.6),
            None => t.translation.x + dir * step,
        };
        t.translation.x = x.clamp(-PLAYER_LIMIT, PLAYER_LIMIT);
    }
}

fn fire(
    mut commands: Commands,
    art: Res<Art>,
    c: Res<Controls>,
    player: Query<&Transform, With<Player>>,
    bullets: Query<(), With<PlayerBullet>>,
) {
    if !c.fire || !bullets.is_empty() {
        return;
    }
    let Ok(p) = player.single() else { return };
    commands.spawn((
        Mesh3d(art.bullet.clone()),
        MeshMaterial3d(art.cur().bullet_mat.clone()),
        Transform::from_xyz(p.translation.x, 0.5, p.translation.z - 0.8),
        PlayerBullet,
        GameEntity,
    ));
    sfx("shoot", 0.0);
}

fn invader_pos(f: &Formation, inv: &Invader) -> Vec3 {
    Vec3::new(
        (inv.col as f32 - (COLS as f32 - 1.0) / 2.0) * CELL_X + f.off_x,
        0.0,
        START_Z + inv.row as f32 * CELL_Z + f.off_z,
    )
}

fn march(time: Res<Time>, stats: Res<Stats>, mut f: ResMut<Formation>, q: Query<&Invader>, mut next: ResMut<NextState<GameState>>) {
    f.stepped = false;
    f.timer += time.delta_secs();
    let ratio = f.alive as f32 / (COLS * ROWS) as f32;
    let interval = (0.04 + 0.46 * ratio) / (1.0 + 0.08 * (stats.level as f32 - 1.0));
    if f.timer < interval {
        return;
    }
    f.timer = 0.0;
    let (mut min_x, mut max_x, mut max_z) = (f32::MAX, f32::MIN, f32::MIN);
    for inv in &q {
        let p = invader_pos(&f, inv);
        min_x = min_x.min(p.x);
        max_x = max_x.max(p.x);
        max_z = max_z.max(p.z);
    }
    if min_x > max_x {
        return;
    }
    let step = f.dir * STEP_X;
    if min_x + step < -HALF_W || max_x + step > HALF_W {
        f.dir = -f.dir;
        f.off_z += DROP_Z;
    } else {
        f.off_x += step;
    }
    f.frame ^= 1;
    f.stepped = true;
    sfx("march", f.frame as f32);
    if max_z + 0.3 >= LOSE_Z {
        next.set(GameState::Over);
    }
}

fn spawn_bombs(
    mut commands: Commands,
    time: Res<Time>,
    art: Res<Art>,
    stats: Res<Stats>,
    mut f: ResMut<Formation>,
    mut rng: ResMut<Rng>,
    inv: Query<&Invader>,
    bombs: Query<(), With<Bomb>>,
) {
    f.bomb_timer -= time.delta_secs();
    if f.bomb_timer > 0.0 || bombs.iter().count() >= 3 + (stats.level as usize) / 2 {
        return;
    }
    f.bomb_timer = rng.range(0.4, 1.1) / (1.0 + 0.1 * (stats.level as f32 - 1.0));
    // The lowest living invader in a random column drops the bomb.
    let mut lowest: Vec<&Invader> = Vec::new();
    for col in 0..COLS {
        if let Some(i) = inv.iter().filter(|i| i.col == col).max_by_key(|i| i.row) {
            lowest.push(i);
        }
    }
    if lowest.is_empty() {
        return;
    }
    let pick = lowest[rng.below(lowest.len())];
    let p = invader_pos(&f, pick);
    commands.spawn((
        Mesh3d(art.bomb.clone()),
        MeshMaterial3d(art.cur().bomb_mat.clone()),
        Transform::from_xyz(p.x, 0.5, p.z + 0.5),
        Bomb,
        GameEntity,
    ));
}

fn move_projectiles(
    mut commands: Commands,
    time: Res<Time>,
    stats: Res<Stats>,
    mut bullets: Query<(Entity, &mut Transform), (With<PlayerBullet>, Without<Bomb>)>,
    mut bombs: Query<(Entity, &mut Transform), (With<Bomb>, Without<PlayerBullet>)>,
) {
    let dt = time.delta_secs();
    for (e, mut t) in &mut bullets {
        t.translation.z -= BULLET_SPEED * dt;
        if t.translation.z < FAR_Z {
            commands.entity(e).despawn();
        }
    }
    let speed = 8.0 + stats.level as f32 * 0.7;
    for (e, mut t) in &mut bombs {
        t.translation.z += speed * dt;
        if t.translation.z > 9.0 {
            commands.entity(e).despawn();
        }
    }
}

fn burst(commands: &mut Commands, art: &Art, rng: &mut Rng, at: Vec3, mat: Handle<StandardMaterial>, n: usize, speed: f32) {
    for _ in 0..n {
        let a = rng.range(0.0, std::f32::consts::TAU);
        let up = rng.range(0.5, 1.0);
        let v = rng.range(0.3, 1.0) * speed;
        commands.spawn((
            Mesh3d(art.spark.clone()),
            MeshMaterial3d(mat.clone()),
            Transform::from_translation(at),
            Particle { vel: Vec3::new(a.cos() * v, up * speed, a.sin() * v), life: rng.range(0.4, 0.9), max: 0.9 },
            GameEntity,
        ));
    }
}

fn erode(commands: &mut Commands, voxels: &Query<(Entity, &Transform), With<ShieldVoxel>>, gone: &mut HashSet<Entity>, rng: &mut Rng, x: f32, z: f32) {
    for (e, t) in voxels {
        let d = Vec2::new(t.translation.x - x, t.translation.z - z).length();
        if (d < 0.42 || (d < 0.62 && rng.f32() < 0.5)) && gone.insert(e) {
            commands.entity(e).despawn();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn collisions(
    mut commands: Commands,
    art: Res<Art>,
    f: Res<Formation>,
    mut stats: ResMut<Stats>,
    mut rng: ResMut<Rng>,
    mut respawn: ResMut<Respawn>,
    mut next: ResMut<NextState<GameState>>,
    bullets: Query<(Entity, &Transform), With<PlayerBullet>>,
    bombs: Query<(Entity, &Transform), With<Bomb>>,
    invaders: Query<(Entity, &Invader)>,
    ufos: Query<(Entity, &Transform), With<Ufo>>,
    voxels: Query<(Entity, &Transform), With<ShieldVoxel>>,
    player: Query<(Entity, &Transform), With<Player>>,
) {
    let mut gone: HashSet<Entity> = HashSet::new();

    // Player bullet vs invaders, UFO, shields.
    if let Ok((be, bt)) = bullets.single() {
        let b = bt.translation;
        let mut hit = false;
        for (e, inv) in &invaders {
            let p = invader_pos(&f, inv);
            if (p.x - b.x).abs() < 0.75 && (p.z - b.z).abs() < 0.6 {
                commands.entity(e).despawn();
                stats.score += inv.kind.points();
                stats.hi = stats.hi.max(stats.score);
                burst(&mut commands, &art, &mut rng, p + Vec3::Y * 0.4, art.cur().inv_mat[inv.kind.idx()].clone(), 14, 3.5);
                sfx("hit", 0.0);
                hit = true;
                break;
            }
        }
        if !hit {
            for (e, t) in &ufos {
                if (t.translation.x - b.x).abs() < 0.9 && (t.translation.z - b.z).abs() < 0.7 {
                    let pts = [50, 100, 150, 300][rng.below(4)];
                    stats.score += pts;
                    stats.hi = stats.hi.max(stats.score);
                    burst(&mut commands, &art, &mut rng, t.translation + Vec3::Y * 0.4, art.cur().ufo_mat.clone(), 22, 4.5);
                    sfx("ufoHit", 0.0);
                    commands.entity(e).despawn();
                    hit = true;
                    break;
                }
            }
        }
        if !hit {
            // Sweep a little ahead of the bullet so fast shots cannot tunnel through a shield.
            let mut touched = false;
            for (_, t) in &voxels {
                if (t.translation.x - b.x).abs() < 0.16 && (t.translation.z - (b.z - 0.2)).abs() < 0.55 {
                    touched = true;
                    break;
                }
            }
            if touched {
                erode(&mut commands, &voxels, &mut gone, &mut rng, b.x, b.z - 0.2);
                burst(&mut commands, &art, &mut rng, Vec3::new(b.x, 0.4, b.z), art.cur().voxel_mat.clone(), 5, 2.0);
                hit = true;
            }
        }
        if hit {
            commands.entity(be).despawn();
        }
    }

    // Bombs vs shields and the player.
    let player_pos = player.single().ok().map(|(e, t)| (e, t.translation));
    for (e, t) in &bombs {
        let b = t.translation;
        let mut touched = false;
        for (_, v) in &voxels {
            if (v.translation.x - b.x).abs() < 0.2 && (v.translation.z - (b.z + 0.25)).abs() < 0.55 {
                touched = true;
                break;
            }
        }
        if touched {
            erode(&mut commands, &voxels, &mut gone, &mut rng, b.x, b.z + 0.25);
            burst(&mut commands, &art, &mut rng, Vec3::new(b.x, 0.4, b.z), art.cur().voxel_mat.clone(), 5, 2.0);
            commands.entity(e).despawn();
            continue;
        }
        if let Some((pe, p)) = player_pos {
            if (p.x - b.x).abs() < 0.75 && (p.z - b.z).abs() < 0.55 {
                burst(&mut commands, &art, &mut rng, p + Vec3::Y * 0.4, art.cur().spark_white.clone(), 30, 5.0);
                stats.lives = stats.lives.saturating_sub(1);
                sfx("die", 0.0);
                for (be, _) in &bombs {
                    commands.entity(be).despawn();
                }
                if let Ok((be, _)) = bullets.single() {
                    commands.entity(be).despawn();
                }
                if stats.lives == 0 {
                    commands.entity(pe).despawn();
                    next.set(GameState::Over);
                } else {
                    commands.entity(pe).insert((Transform::from_xyz(0.0, 0.0, PLAYER_Z).with_rotation(lean()), Visibility::Hidden));
                    respawn.0 = 1.4;
                }
                return;
            }
        }
    }
}

fn ufo_system(
    mut commands: Commands,
    time: Res<Time>,
    art: Res<Art>,
    mut f: ResMut<Formation>,
    mut rng: ResMut<Rng>,
    mut ufos: Query<(Entity, &Ufo, &mut Transform)>,
) {
    let dt = time.delta_secs();
    f.ufo_timer -= dt;
    if f.ufo_timer <= 0.0 && ufos.is_empty() && f.alive > 8 {
        let right = rng.f32() < 0.5;
        let (x, vx) = if right { (-11.0, 4.5) } else { (11.0, -4.5) };
        commands.spawn((
            Mesh3d(art.ufo.clone()),
            MeshMaterial3d(art.cur().ufo_mat.clone()),
            Transform::from_xyz(x, 0.7, UFO_Z).with_rotation(lean()),
            Ufo { vx },
            GameEntity,
        ));
        f.ufo_timer = rng.range(15.0, 25.0);
        sfx("ufo", 0.0);
    }
    for (e, u, mut t) in &mut ufos {
        t.translation.x += u.vx * dt;
        if t.translation.x.abs() > 11.5 {
            commands.entity(e).despawn();
        }
    }
}

fn level_check(
    mut commands: Commands,
    art: Res<Art>,
    mut stats: ResMut<Stats>,
    mut f: ResMut<Formation>,
    mut rng: ResMut<Rng>,
    invaders: Query<(), With<Invader>>,
    voxels: Query<Entity, With<ShieldVoxel>>,
) {
    f.alive = invaders.iter().count() as u32;
    if f.alive > 0 {
        return;
    }
    stats.level += 1;
    stats.lives = (stats.lives + 1).min(5);
    sfx("level", 0.0);
    for v in &voxels {
        commands.entity(v).despawn();
    }
    *f = Formation::fresh(stats.level, rng.range(12.0, 20.0));
    spawn_wave(&mut commands, &art);
    spawn_shields(&mut commands, &art);
}

/// Position every creature from the formation and flip its walk frame. Cheap: 55 transforms.
fn animate_invaders(f: Res<Formation>, art: Res<Art>, mut q: Query<(&Invader, &mut Transform, &mut Mesh3d)>, time: Res<Time>) {
    for (inv, mut t, mut mesh) in &mut q {
        let p = invader_pos(&f, inv);
        t.translation = p + Vec3::Y * (0.05 + ((time.elapsed_secs() * 2.0 + inv.col as f32 * 0.5 + inv.row as f32).sin() * 0.04));
        let want = &art.cur().inv[inv.kind.idx()][f.frame];
        if mesh.0 != *want {
            mesh.0 = want.clone();
        }
    }
}

fn particles(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut Particle, &mut Transform)>) {
    let dt = time.delta_secs();
    for (e, mut p, mut t) in &mut q {
        p.life -= dt;
        if p.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        p.vel.y -= 9.0 * dt;
        t.translation += p.vel * dt;
        if t.translation.y < 0.0 {
            t.translation.y = 0.0;
            p.vel.y = p.vel.y.abs() * 0.3;
        }
        t.scale = Vec3::splat((p.life / p.max).clamp(0.05, 1.0));
    }
}

/// Invaders trample the bunkers they reach, as in the original.
fn crush_shields(mut commands: Commands, f: Res<Formation>, invaders: Query<&Invader>, voxels: Query<(Entity, &Transform), With<ShieldVoxel>>) {
    if !f.stepped {
        return;
    }
    for inv in &invaders {
        let p = invader_pos(&f, inv);
        if p.z < SHIELD_Z - 2.0 {
            continue;
        }
        for (e, t) in &voxels {
            if (t.translation.x - p.x).abs() < 0.8 && (t.translation.z - p.z).abs() < 0.7 {
                commands.entity(e).despawn();
            }
        }
    }
}

/// Fit the whole table to any screen: wide monitors, tall phones, resized windows.
/// The camera keeps a fixed look direction and backs off just far enough that every key point is in view;
/// on tall screens it also climbs higher so the table's width is used efficiently.
pub fn fit_camera(
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut cam: Query<(&mut Transform, &Projection, &mut DistanceFog), With<MainCamera>>,
    mut ui_scale: ResMut<UiScale>,
    mut base: ResMut<CamBase>,
    mut last: Local<(f32, f32)>,
) {
    let (Ok(win), Ok((mut tf, proj, mut fog))) = (windows.single(), cam.single_mut()) else { return };
    let (w, h) = (win.width(), win.height().max(1.0));
    if (w, h) == *last {
        return;
    }
    *last = (w, h);
    let aspect = w / h;
    let Projection::Perspective(p) = proj else { return };
    // HUD text scales with the screen so it reads on a phone and a 4K monitor alike.
    ui_scale.0 = (w.min(h) / 720.0).clamp(0.55, 1.4);

    // 36 degrees on wide screens, up to 68 on tall ones.
    let tall = ((1.2 - aspect) / 0.6).clamp(0.0, 1.0);
    let pitch = (36.0 + 32.0 * tall).to_radians();
    let dir = Vec3::new(0.0, pitch.sin(), pitch.cos());
    let key_points = [
        Vec3::new(-10.2, 0.0, 8.2),
        Vec3::new(10.2, 0.0, 8.2),
        Vec3::new(-10.2, 0.8, -16.5),
        Vec3::new(10.2, 0.8, -16.5),
    ];
    // Leave room at the top and bottom edges for the HUD.
    let (max_x, max_y) = (0.95, 0.86);
    let proj_m = Mat4::perspective_rh(p.fov, aspect, 0.1, 300.0);
    let mut dist = 14.0;
    while dist < 120.0 {
        let pos = CAM_TARGET + dir * dist;
        let view = Mat4::look_at_rh(pos, CAM_TARGET, Vec3::Y);
        let fits = key_points.iter().all(|k| {
            let c = proj_m * view * k.extend(1.0);
            let n = c.truncate() / c.w;
            c.w > 0.0 && n.x.abs() <= max_x && n.y.abs() <= max_y
        });
        if fits {
            break;
        }
        dist += 0.5;
    }
    *tf = Transform::from_translation(CAM_TARGET + dir * dist).looking_at(CAM_TARGET, Vec3::Y);
    base.0 = *tf;
    // Fog fades only the far end of the table, wherever the camera ended up.
    fog.falloff = FogFalloff::Linear { start: dist + 10.0, end: dist + 46.0 };
}

fn load_hi(mut stats: ResMut<Stats>) {
    stats.hi = hi_get();
}

fn save_hi(stats: Res<Stats>) {
    if stats.hi > hi_get() {
        hi_set(stats.hi);
    }
}

/// Switching tabs or apps pauses the game instead of letting the invaders keep coming.
fn pause_on_blur(mut focus: MessageReader<bevy::window::WindowFocused>, mut next: ResMut<NextState<GameState>>) {
    for e in focus.read() {
        if !e.focused {
            next.set(GameState::Paused);
        }
    }
}
