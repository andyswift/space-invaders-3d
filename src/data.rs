//! Pixel art for the classic surface sprites, and a builder that extrudes it into real 3D voxel meshes.

use bevy::prelude::*;

/// Two walk frames per creature, `#` = filled voxel. Row 0 is the top of the sprite.
pub const SQUID: [[&str; 8]; 2] = [
    ["...##...", "..####..", ".######.", "##.##.##", "########", "..#..#..", ".#.##.#.", "#.#..#.#"],
    ["...##...", "..####..", ".######.", "##.##.##", "########", ".#.##.#.", "#......#", ".#....#."],
];
pub const CRAB: [[&str; 8]; 2] = [
    ["..#.....#..", "...#...#...", "..#######..", ".##.###.##.", "###########", "#.#######.#", "#.#.....#.#", "...##.##..."],
    ["..#.....#..", "#..#...#..#", "#.#######.#", "###.###.###", "###########", ".#########.", "..#.....#..", ".#.......#."],
];
pub const OCTOPUS: [[&str; 8]; 2] = [
    ["....####....", ".##########.", "############", "###..##..###", "############", "...##..##...", "..##.##.##..", "##........##"],
    ["....####....", ".##########.", "############", "###..##..###", "############", "..###..###..", ".##..##..##.", "..##....##.."],
];
pub const SHIP: [&str; 6] = [".....#.....", "....###....", "....###....", ".#########.", "###########", "###########"];
pub const UFO: [&str; 7] = [
    ".....######.....",
    "...##########...",
    "..############..",
    ".##.##.##.##.##.",
    "################",
    "..###..##..###..",
    "...#........#...",
];

/// Extrude a bitmap into one merged mesh, standing upright: x = column, y = height, thickness along z.
/// The sprite is centred on x and sits on y = 0, facing +z (towards the player).
pub fn voxels(rows: &[&str], vox: f32, depth: f32) -> Mesh {
    let h = rows.len();
    let mut out: Option<Mesh> = None;
    for (r, row) in rows.iter().enumerate() {
        let w = row.chars().count();
        for (c, ch) in row.chars().enumerate() {
            if ch != '#' {
                continue;
            }
            let x = (c as f32 - (w as f32 - 1.0) / 2.0) * vox;
            let y = ((h - 1 - r) as f32 + 0.5) * vox;
            let cube = Mesh::from(Cuboid::new(vox * 0.92, vox * 0.92, depth))
                .translated_by(Vec3::new(x, y, 0.0));
            match out.as_mut() {
                Some(m) => m.merge(&cube).expect("voxel meshes share a layout"),
                None => out = Some(cube),
            }
        }
    }
    out.expect("bitmap has at least one voxel")
}

/// The shield bunker: 20 x 14 cells, corners cut, arch at the player's side. `true` = solid.
pub fn shield_cells() -> Vec<(usize, usize)> {
    const COLS: usize = 20;
    const ROWS: usize = 14;
    let mut cells = Vec::new();
    for r in 0..ROWS {
        for c in 0..COLS {
            let cut_corner = (c as i32) < 4 - 2 * r as i32 || (c as i32) >= COLS as i32 - (4 - 2 * r as i32);
            let cut_arch = (r >= 10 && (7..13).contains(&c)) || (r == 9 && (8..12).contains(&c));
            if !cut_corner && !cut_arch {
                cells.push((c, r));
            }
        }
    }
    cells
}
