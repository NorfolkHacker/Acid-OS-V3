//! Fixed-point 3D maths, meshes and built-in shapes. Integer only.
//!
//! Angles are 0-255 per full turn (always taken `& 255`); `SIN` holds
//! 4096 * sin. Faces wind counter-clockwise when viewed from outside.

use alloc::vec::Vec;

pub use crate::sin_table::SIN;

pub const NO_INDEX: u16 = u16::MAX;
pub const MESH_POINTS_MAX: usize = 512;
pub const MESH_FACES_MAX: usize = 1024;
pub const BUILTIN_NAMES: [&str; 5] = ["cube", "pyramid", "octahedron", "sphere", "torus"];

pub fn sin(a: i32) -> i32 {
    SIN[(a & 255) as usize]
}

pub fn cos(a: i32) -> i32 {
    SIN[((a + 64) & 255) as usize]
}

/// Rotate about X, then Y, then Z.
pub fn rotate(p: (i32, i32, i32), rx: i32, ry: i32, rz: i32) -> (i32, i32, i32) {
    let (mut x, mut y, mut z) = (p.0 as i64, p.1 as i64, p.2 as i64);
    let (s, c) = (sin(rx) as i64, cos(rx) as i64);
    (y, z) = ((y * c - z * s) >> 12, (y * s + z * c) >> 12);
    let (s, c) = (sin(ry) as i64, cos(ry) as i64);
    (x, z) = ((x * c + z * s) >> 12, (-x * s + z * c) >> 12);
    let (s, c) = (sin(rz) as i64, cos(rz) as i64);
    (x, y) = ((x * c - y * s) >> 12, (x * s + y * c) >> 12);
    (x as i32, y as i32, z as i32)
}

/// Perspective-project an already rotated point; `None` when it is behind
/// the camera. `size` 64 = one model unit per pixel at z 0; y goes up.
pub fn project(p: (i32, i32, i32), cx: i32, cy: i32, size: i32) -> Option<(i32, i32)> {
    let size = size as i64;
    let zc = p.2 as i64 * size / 64 + 512;
    if zc <= 16 {
        return None;
    }
    let sx = p.0 as i64 * size * 512 / (64 * zc);
    let sy = p.1 as i64 * size * 512 / (64 * zc);
    Some((cx + sx as i32, cy - sy as i32))
}

#[allow(dead_code)] // used by the tests now; the renderer needs it next
pub(crate) fn sub(a: (i32, i32, i32), b: (i32, i32, i32)) -> (i64, i64, i64) {
    ((a.0 - b.0) as i64, (a.1 - b.1) as i64, (a.2 - b.2) as i64)
}

#[allow(dead_code)] // used by the tests now; the renderer needs it next
pub(crate) fn cross(a: (i64, i64, i64), b: (i64, i64, i64)) -> (i64, i64, i64) {
    (a.1 * b.2 - a.2 * b.1, a.2 * b.0 - a.0 * b.2, a.0 * b.1 - a.1 * b.0)
}

#[derive(Debug, PartialEq, Eq)]
pub enum MeshError {
    Bad,
    TooBig,
}

#[derive(Debug, Clone)]
pub struct Mesh {
    pub points: Vec<(i32, i32, i32)>,
    /// Three or four point indices; the fourth is `NO_INDEX` for a triangle.
    pub faces: Vec<[u16; 4]>,
    /// Unique unordered point pairs, in first-seen order.
    pub edges: Vec<(u16, u16)>,
}

impl Mesh {
    pub fn new(points: Vec<(i32, i32, i32)>, faces: Vec<[u16; 4]>) -> Result<Mesh, MeshError> {
        if points.len() < 3 {
            return Err(MeshError::Bad);
        }
        if points.len() > MESH_POINTS_MAX || faces.len() > MESH_FACES_MAX {
            return Err(MeshError::TooBig);
        }
        let mut edges: Vec<(u16, u16)> = Vec::new();
        for f in &faces {
            let n = if f[3] == NO_INDEX { 3 } else { 4 };
            for i in 0..n {
                if f[i] as usize >= points.len() || f[..i].contains(&f[i]) {
                    return Err(MeshError::Bad);
                }
            }
            for i in 0..n {
                let (a, b) = (f[i], f[(i + 1) % n]);
                let e = (a.min(b), a.max(b));
                if !edges.contains(&e) {
                    edges.push(e);
                }
            }
        }
        Ok(Mesh { points, faces, edges })
    }
}

pub fn builtin(name: &str) -> Option<Mesh> {
    let (points, faces) = match name {
        "cube" => cube(),
        "pyramid" => pyramid(),
        "octahedron" => octahedron(),
        "sphere" => sphere(),
        "torus" => torus(),
        _ => return None,
    };
    Mesh::new(points, faces).ok()
}

type Shape = (Vec<(i32, i32, i32)>, Vec<[u16; 4]>);

fn tri(a: u16, b: u16, c: u16) -> [u16; 4] {
    [a, b, c, NO_INDEX]
}

fn cube() -> Shape {
    // Point index = x | y << 1 | z << 2, each bit 1 for +100.
    let points = (0..8)
        .map(|i| {
            let s = |bit: i32| if i >> bit & 1 == 1 { 100 } else { -100 };
            (s(0), s(1), s(2))
        })
        .collect();
    let faces = alloc::vec![
        [4, 5, 7, 6], [0, 2, 3, 1], [1, 3, 7, 5], [0, 4, 6, 2], [2, 6, 7, 3], [0, 1, 5, 4],
    ];
    (points, faces)
}

fn pyramid() -> Shape {
    let points = alloc::vec![(-100, -70, -100), (100, -70, -100), (100, -70, 100), (-100, -70, 100), (0, 100, 0)];
    let mut faces: Vec<[u16; 4]> = (0..4).map(|i| tri((i + 1) % 4, i, 4)).collect();
    faces.push([0, 1, 2, 3]);
    (points, faces)
}

fn octahedron() -> Shape {
    let points = alloc::vec![(100, 0, 0), (-100, 0, 0), (0, 100, 0), (0, -100, 0), (0, 0, 100), (0, 0, -100)];
    let mut faces = Vec::new();
    for k in 0..8u16 {
        let (x, y, z) = (k & 1, k >> 1 & 1, k >> 2 & 1);
        let (px, py, pz) = (x, 2 + y, 4 + z);
        // An odd number of negative axes flips the handedness.
        if (x + y + z) % 2 == 0 {
            faces.push(tri(px, py, pz));
        } else {
            faces.push(tri(px, pz, py));
        }
    }
    (points, faces)
}

fn sphere() -> Shape {
    let mut points = alloc::vec![(0, 100, 0), (0, -100, 0)];
    for i in 1..=5i32 {
        let lat = i * 128 / 6;
        for j in 0..8i32 {
            let lon = j * 32;
            let x = (100 * sin(lat) as i64 * cos(lon) as i64) >> 24;
            let y = (100 * cos(lat) as i64) >> 12;
            let z = (100 * sin(lat) as i64 * sin(lon) as i64) >> 24;
            points.push((x as i32, y as i32, z as i32));
        }
    }
    let ring = |i: u16, j: u16| 2 + (i - 1) * 8 + j % 8;
    let mut faces = Vec::new();
    for j in 0..8 {
        faces.push(tri(0, ring(1, j + 1), ring(1, j)));
    }
    for i in 1..5 {
        for j in 0..8 {
            faces.push([ring(i, j), ring(i, j + 1), ring(i + 1, j + 1), ring(i + 1, j)]);
        }
    }
    for j in 0..8 {
        faces.push(tri(1, ring(5, j), ring(5, j + 1)));
    }
    (points, faces)
}

fn torus() -> Shape {
    let mut points = Vec::new();
    for i in 0..6i32 {
        let v = i * 256 / 6;
        for j in 0..12i32 {
            let u = j * 256 / 12;
            let r = 70 * 4096 + 30 * cos(v) as i64;
            let x = (r * cos(u) as i64) >> 24;
            let y = (30 * sin(v) as i64) >> 12;
            let z = (r * sin(u) as i64) >> 24;
            points.push((x as i32, y as i32, z as i32));
        }
    }
    let at = |i: u16, j: u16| (i % 6) * 12 + j % 12;
    let mut faces = Vec::new();
    for i in 0..6 {
        for j in 0..12 {
            faces.push([at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)]);
        }
    }
    (points, faces)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sine_table_has_its_landmarks() {
        assert_eq!((sin(0), sin(64), sin(128), sin(192)), (0, 4096, 0, -4096));
        assert_eq!((cos(0), cos(64)), (4096, 0));
        for a in 0..256 { assert_eq!(sin(a), -sin((256 - a) & 255), "odd symmetry at {a}"); }
    }

    #[test]
    fn a_quarter_turn_about_z_moves_x_to_y() {
        let (x, y, z) = rotate((100, 0, 0), 0, 0, 64);
        assert!(x.abs() <= 1 && (y - 100).abs() <= 1 && z == 0, "got {x},{y},{z}");
        assert_eq!(rotate((10, 20, 30), 0, 0, 0), (10, 20, 30));
        let (x, y, z) = rotate((0, 100, 0), 64, 0, 0);
        assert!(x == 0 && y.abs() <= 1 && (z - 100).abs() <= 1, "x-axis turn takes y to z: {x},{y},{z}");
    }

    #[test]
    fn projection_centres_the_origin_and_hides_points_behind_the_camera() {
        assert_eq!(project((0, 0, 0), 120, 100, 64), Some((120, 100)));
        assert_eq!(project((10, 10, 0), 120, 100, 64), Some((130, 90)), "64 = one unit per pixel at z 0; y goes up");
        assert_eq!(project((0, 0, -600), 120, 100, 64), None, "behind the camera");
    }

    #[test]
    fn mesh_new_validates_its_input() {
        let tri = vec![(0, 0, 0), (10, 0, 0), (0, 10, 0)];
        assert!(Mesh::new(tri.clone(), vec![[0, 1, 2, NO_INDEX]]).is_ok());
        assert!(matches!(Mesh::new(tri[..2].to_vec(), vec![]), Err(MeshError::Bad)), "fewer than 3 points");
        assert!(matches!(Mesh::new(tri.clone(), vec![[0, 1, 3, NO_INDEX]]), Err(MeshError::Bad)), "index out of range");
        assert!(matches!(Mesh::new(tri.clone(), vec![[0, 1, 1, NO_INDEX]]), Err(MeshError::Bad)), "repeated index");
        assert!(matches!(Mesh::new(vec![(0, 0, 0); MESH_POINTS_MAX + 1], vec![]), Err(MeshError::TooBig)));
        assert!(matches!(Mesh::new(tri, vec![[0, 1, 2, NO_INDEX]; MESH_FACES_MAX + 1]), Err(MeshError::TooBig)));
    }

    #[test]
    fn edges_are_unique() {
        let cube = builtin("cube").unwrap();
        assert_eq!(cube.edges.len(), 12);
    }

    #[test]
    fn builtins_have_their_counts() {
        let counts: Vec<(usize, usize)> = BUILTIN_NAMES.iter().map(|n| { let m = builtin(n).unwrap(); (m.points.len(), m.faces.len()) }).collect();
        assert_eq!(counts, [(8, 6), (5, 5), (6, 8), (42, 48), (72, 72)]);
        assert!(builtin("teapot").is_none());
    }

    fn isqrt(n: i64) -> i64 {
        let mut r = 0;
        while (r + 1) * (r + 1) <= n { r += 1; }
        r
    }

    #[test]
    fn builtin_faces_wind_outward() {
        // Convex shapes: every face's normal points away from the origin.
        for name in ["cube", "pyramid", "octahedron", "sphere"] {
            let m = builtin(name).unwrap();
            for f in &m.faces {
                let (a, b, c) = (m.points[f[0] as usize], m.points[f[1] as usize], m.points[f[2] as usize]);
                let n = cross(sub(b, a), sub(c, a));
                let centre = (a.0 as i64 + b.0 as i64 + c.0 as i64, a.1 as i64 + b.1 as i64 + c.1 as i64, a.2 as i64 + b.2 as i64 + c.2 as i64);
                assert!(n.0 * centre.0 + n.1 * centre.1 + n.2 * centre.2 > 0, "{name} face {f:?} winds inward");
            }
        }
        // Torus: each face's normal points away from the tube's centre circle
        // (radius 70, y = 0). Work in units of 1/4 so the quad centre is exact.
        let m = builtin("torus").unwrap();
        for f in &m.faces {
            let p: Vec<(i32, i32, i32)> = f.iter().map(|&i| m.points[i as usize]).collect();
            let n = cross(sub(p[1], p[0]), sub(p[2], p[0]));
            let c4 = (
                p.iter().map(|q| q.0 as i64).sum::<i64>(),
                p.iter().map(|q| q.1 as i64).sum::<i64>(),
                p.iter().map(|q| q.2 as i64).sum::<i64>(),
            );
            let len = isqrt(c4.0 * c4.0 + c4.2 * c4.2).max(1);
            let ring4 = (c4.0 * 280 / len, 0, c4.2 * 280 / len);
            let out = (c4.0 - ring4.0, c4.1 - ring4.1, c4.2 - ring4.2);
            assert!(n.0 * out.0 + n.1 * out.1 + n.2 * out.2 > 0, "torus face {f:?} winds inward");
        }
    }
}
