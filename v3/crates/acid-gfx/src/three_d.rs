//! Fixed-point 3D maths, meshes and built-in shapes. Integer only.
//!
//! Angles are 0-255 per full turn (always taken `& 255`); `SIN` holds
//! 4096 * sin. Faces wind counter-clockwise when viewed from outside.

use alloc::vec::Vec;

use crate::Canvas;

pub use crate::sin_table::SIN;

/// Largest coordinate magnitude a mesh point may have.
pub const COORD_MAX: i32 = 32767;
pub const NO_INDEX: u16 = u16::MAX;
pub const MESH_POINTS_MAX: usize = 512;
pub const MESH_FACES_MAX: usize = 1024;
pub const BUILTIN_NAMES: [&str; 5] = ["cube", "pyramid", "octahedron", "sphere", "torus"];

pub fn sin(a: i32) -> i32 {
    SIN[(a & 255) as usize]
}

pub fn cos(a: i32) -> i32 {
    SIN[(a.wrapping_add(64) & 255) as usize]
}

/// Rotate about X, then Y, then Z. Angles are any i32. Results are meaningful
/// only for coordinates within +-COORD_MAX (Mesh::new enforces it); anything
/// else cannot overflow i64 and the outputs saturate to the i32 range.
pub fn rotate(p: (i32, i32, i32), rx: i32, ry: i32, rz: i32) -> (i32, i32, i32) {
    let (mut x, mut y, mut z) = (p.0 as i64, p.1 as i64, p.2 as i64);
    let (s, c) = (sin(rx) as i64, cos(rx) as i64);
    (y, z) = ((y * c - z * s) >> 12, (y * s + z * c) >> 12);
    let (s, c) = (sin(ry) as i64, cos(ry) as i64);
    (x, z) = ((x * c + z * s) >> 12, (-x * s + z * c) >> 12);
    let (s, c) = (sin(rz) as i64, cos(rz) as i64);
    (x, y) = ((x * c - y * s) >> 12, (x * s + y * c) >> 12);
    let sat = |v: i64| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
    (sat(x), sat(y), sat(z))
}

/// Perspective-project an already rotated point, with the camera 512 model
/// units in front of the origin: `zc = z + 512`, and the screen point is
/// `(cx + x*size*512/(64*zc), cy - y*size*512/(64*zc))`. `size` scales only
/// the image, never the depth, so the perspective is the same at every size;
/// 64 = one model unit per pixel at z 0. Y goes up. `None` when the point is
/// behind the camera (`zc <= 16`) or `size <= 0`; the screen point saturates
/// to +-2^30.
pub fn project(p: (i32, i32, i32), cx: i32, cy: i32, size: i32) -> Option<(i32, i32)> {
    if size <= 0 {
        return None;
    }
    let size = size as i128;
    let zc = p.2 as i128 + 512;
    if zc <= 16 {
        return None;
    }
    let lim = 1i128 << 30;
    let sx = (cx as i128 + p.0 as i128 * size * 512 / (64 * zc)).clamp(-lim, lim);
    let sy = (cy as i128 - p.1 as i128 * size * 512 / (64 * zc)).clamp(-lim, lim);
    Some((sx as i32, sy as i32))
}

pub(crate) fn sub(a: (i32, i32, i32), b: (i32, i32, i32)) -> (i64, i64, i64) {
    (a.0 as i64 - b.0 as i64, a.1 as i64 - b.1 as i64, a.2 as i64 - b.2 as i64)
}

pub(crate) fn cross(a: (i64, i64, i64), b: (i64, i64, i64)) -> (i64, i64, i64) {
    (a.1 * b.2 - a.2 * b.1, a.2 * b.0 - a.0 * b.2, a.0 * b.1 - a.1 * b.0)
}

#[derive(Debug, PartialEq, Eq)]
pub enum MeshError {
    Bad,
    TooBig,
}

/// A validated mesh. `Mesh::new` and `builtin` are the only constructors,
/// so every point is within +-COORD_MAX and every index is in range; the
/// fields are private so a hand-built mesh cannot skip that:
///
/// ```compile_fail,E0451
/// let m = acid_gfx::three_d::Mesh { points: vec![], faces: vec![], edges: vec![] };
/// ```
///
/// ```compile_fail,E0616
/// let m = acid_gfx::three_d::builtin("cube").unwrap();
/// let _ = m.points;
/// ```
#[derive(Debug, Clone)]
pub struct Mesh {
    points: Vec<(i32, i32, i32)>,
    faces: Vec<[u16; 4]>,
    edges: Vec<(u16, u16)>,
}

impl Mesh {
    pub fn points(&self) -> &[(i32, i32, i32)] {
        &self.points
    }

    /// Three or four point indices; the fourth is `NO_INDEX` for a triangle.
    pub fn faces(&self) -> &[[u16; 4]] {
        &self.faces
    }

    /// Unique unordered point pairs, in first-seen order.
    pub fn edges(&self) -> &[(u16, u16)] {
        &self.edges
    }

    pub fn new(points: Vec<(i32, i32, i32)>, faces: Vec<[u16; 4]>) -> Result<Mesh, MeshError> {
        if points.len() < 3 {
            return Err(MeshError::Bad);
        }
        if points.len() > MESH_POINTS_MAX || faces.len() > MESH_FACES_MAX {
            return Err(MeshError::TooBig);
        }
        if points.iter().any(|p| [p.0, p.1, p.2].iter().any(|c| c.unsigned_abs() > COORD_MAX as u32)) {
            return Err(MeshError::Bad);
        }
        // Every edge as (low, high, first-seen order); sorting groups the
        // duplicates, so deduplicating is O(E log E), not O(E^2).
        let mut all: Vec<(u16, u16, u32)> = Vec::new();
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
                all.push((e.0, e.1, all.len() as u32));
            }
        }
        all.sort_unstable();
        all.dedup_by_key(|e| (e.0, e.1));
        all.sort_unstable_by_key(|e| e.2);
        let edges = all.into_iter().map(|e| (e.0, e.1)).collect();
        Ok(Mesh { points, faces, edges })
    }
}

/// `(points, faces)` of a built-in, without building it; None for an unknown
/// name. A test keeps it equal to the built meshes.
pub fn builtin_counts(name: &str) -> Option<(usize, usize)> {
    Some(match name {
        "cube" => (8, 6),
        "pyramid" => (5, 5),
        "octahedron" => (6, 8),
        "sphere" => (42, 48),
        "torus" => (72, 72),
        _ => return None,
    })
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

/// Light direction in Q12, toward the upper-left front (unit length).
const LIGHT: (i64, i64, i64) = (-2365, 2365, -2365);

/// Floor square root.
fn isqrt_u128(n: u128) -> u128 {
    if n < 2 {
        return n;
    }
    // Newton's method from an over-estimate converges down to the floor.
    let mut x = 1u128 << (128 - n.leading_zeros()).div_ceil(2);
    loop {
        let y = (x + n / x) / 2;
        if y >= x {
            return x;
        }
        x = y;
    }
}

/// Rotate then project every point. Projected points are `None` behind the
/// camera; the rotated points give normals and depth.
fn prepare(m: &Mesh, cx: i32, cy: i32, size: i32, rx: i32, ry: i32, rz: i32) -> (Vec<Option<(i32, i32)>>, Vec<(i32, i32, i32)>) {
    let rot: Vec<(i32, i32, i32)> = m.points().iter().map(|&p| rotate(p, rx, ry, rz)).collect();
    let proj = rot.iter().map(|&p| project(p, cx, cy, size)).collect();
    (proj, rot)
}

struct Tri {
    p: [(i32, i32); 3],
    /// Brightness out of 256.
    light: i64,
}

/// The triangles to fill, farthest first: every one with all three points
/// in front of the camera, facing it, and with a non-zero normal.
fn visible_tris(m: &Mesh, proj: &[Option<(i32, i32)>], rot: &[(i32, i32, i32)]) -> Vec<Tri> {
    let mut tris: Vec<(i64, Tri)> = Vec::new();
    for f in m.faces() {
        let halves: &[[u16; 3]] = if f[3] == NO_INDEX { &[[f[0], f[1], f[2]]] } else { &[[f[0], f[1], f[2]], [f[0], f[2], f[3]]] };
        for t in halves {
            let [ia, ib, ic] = t.map(|i| i as usize);
            let (Some(a), Some(b), Some(c)) = (proj[ia], proj[ib], proj[ic]) else { continue };
            // Culling: faces wind counter-clockwise seen from outside, and
            // screen y grows downward, so a face toward the camera has a
            // positive signed area (b - a) x (c - a) on screen. Projected
            // coordinates reach +-2^30, so the products need i128.
            let area = (b.0 as i128 - a.0 as i128) * (c.1 as i128 - a.1 as i128)
                - (b.1 as i128 - a.1 as i128) * (c.0 as i128 - a.0 as i128);
            if area <= 0 {
                continue;
            }
            let (ra, rb, rc) = (rot[ia], rot[ib], rot[ic]);
            // Mesh::new bounds points to +-32767, so rotated ones stay within
            // about +-56800 and the i64 cross product cannot overflow.
            let n = cross(sub(rb, ra), sub(rc, ra));
            let (nx, ny, nz) = (n.0 as i128, n.1 as i128, n.2 as i128);
            let len = isqrt_u128((nx * nx + ny * ny + nz * nz) as u128) as i128;
            if len == 0 {
                continue;
            }
            let dot = (nx * LIGHT.0 as i128 + ny * LIGHT.1 as i128 + nz * LIGHT.2 as i128).max(0);
            let light = (64 + 192 * dot / (len * 4096)).min(256) as i64;
            let z = ra.2 as i64 + rb.2 as i64 + rc.2 as i64;
            tris.push((z, Tri { p: [a, b, c], light }));
        }
    }
    // Stable, so equal depths keep face order.
    tris.sort_by(|x, y| y.0.cmp(&x.0));
    tris.into_iter().map(|(_, t)| t).collect()
}

fn drawn_edges<'a>(m: &'a Mesh, proj: &'a [Option<(i32, i32)>]) -> impl Iterator<Item = ((i32, i32), (i32, i32))> + 'a {
    m.edges().iter().filter_map(|&(a, b)| Some((proj[a as usize]?, proj[b as usize]?)))
}

fn scale(color: u32, light: i64) -> u32 {
    let ch = |shift: u32| ((((color >> shift) & 0xFF) as i64 * light) >> 8) as u32;
    ch(16) << 16 | ch(8) << 8 | ch(0)
}

fn brighten(color: u32) -> u32 {
    let ch = |shift: u32| {
        let c = (color >> shift) & 0xFF;
        c + (255 - c) / 2
    };
    ch(16) << 16 | ch(8) << 8 | ch(0)
}

/// Draw a mesh: mode 0 wire, 1 solid (culled, lit, painter-sorted), 2 solid
/// then the edges in `color` brightened half way to white. Any other mode
/// draws nothing.
#[allow(clippy::too_many_arguments)]
pub fn draw_mesh(c: &mut Canvas, m: &Mesh, cx: i32, cy: i32, size: i32, rx: i32, ry: i32, rz: i32, mode: i32, color: u32) {
    if !(0..=2).contains(&mode) {
        return;
    }
    let (proj, rot) = prepare(m, cx, cy, size, rx, ry, rz);
    if mode >= 1 {
        for t in visible_tris(m, &proj, &rot) {
            let [a, b, d] = t.p;
            c.fill_triangle(a.0, a.1, b.0, b.1, d.0, d.1, scale(color, t.light));
        }
    }
    if mode != 1 {
        let line = if mode == 2 { brighten(color) } else { color };
        for (a, b) in drawn_edges(m, &proj) {
            c.draw_line(a.0, a.1, b.0, b.1, line);
        }
    }
}

/// What `draw_mesh` with the same arguments costs, in pixels (callers scale
/// by bytes per pixel): 64 per point and 64 per face (in every mode, since
/// the host projects and culls them regardless), plus each drawn edge's
/// `max(|dx|, |dy|) + 1` capped at the screen's area, plus each drawn
/// triangle's bounding box with its width and height each capped at the
/// screen's, wherever the box sits (like a single triangle's fuel), so a
/// triangle off the screen is paid for too.
#[allow(clippy::too_many_arguments)]
pub fn mesh_cost(m: &Mesh, cx: i32, cy: i32, size: i32, rx: i32, ry: i32, rz: i32, mode: i32, screen_w: i32, screen_h: i32) -> u64 {
    let mut cost = 64 * (m.points().len() + m.faces().len()) as u64;
    if !(0..=2).contains(&mode) {
        return cost;
    }
    let (w, h) = (screen_w.max(0) as i64, screen_h.max(0) as i64);
    let area = (w * h) as u64;
    let (proj, rot) = prepare(m, cx, cy, size, rx, ry, rz);
    if mode >= 1 {
        for t in visible_tris(m, &proj, &rot) {
            let xs = t.p.map(|q| q.0 as i64);
            let ys = t.p.map(|q| q.1 as i64);
            let bw = (xs[0].max(xs[1]).max(xs[2]) - xs[0].min(xs[1]).min(xs[2]) + 1).min(w);
            let bh = (ys[0].max(ys[1]).max(ys[2]) - ys[0].min(ys[1]).min(ys[2]) + 1).min(h);
            cost = cost.saturating_add((bw * bh) as u64);
        }
    }
    if mode != 1 {
        for (a, b) in drawn_edges(m, &proj) {
            let d = (a.0 as i64 - b.0 as i64).abs().max((a.1 as i64 - b.1 as i64).abs());
            cost = cost.saturating_add(((d + 1) as u64).min(area));
        }
    }
    cost
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

    /// The old quadratic dedupe, kept as the reference for `Mesh::new`.
    fn naive_edges(faces: &[[u16; 4]]) -> Vec<(u16, u16)> {
        let mut edges: Vec<(u16, u16)> = Vec::new();
        for f in faces {
            let n = if f[3] == NO_INDEX { 3 } else { 4 };
            for i in 0..n {
                let (a, b) = (f[i], f[(i + 1) % n]);
                let e = (a.min(b), a.max(b));
                if !edges.contains(&e) {
                    edges.push(e);
                }
            }
        }
        edges
    }

    #[test]
    fn the_fast_edge_dedupe_matches_the_naive_one_exactly() {
        let pts = |n: usize| -> Vec<(i32, i32, i32)> { (0..n as i32).map(|i| (i, 0, 0)).collect() };
        // Heavy sharing: a strip of triangles sharing edges, 1024 faces.
        let strip: Vec<[u16; 4]> = (0..MESH_FACES_MAX).map(|i| [(i % 509) as u16, (i % 509 + 1) as u16, (i % 509 + 2) as u16, NO_INDEX]).collect();
        let m = Mesh::new(pts(MESH_POINTS_MAX), strip.clone()).unwrap();
        assert_eq!(m.edges().len(), 1019);
        assert_eq!(m.edges(), naive_edges(&strip));
        // Reversed pairs: each triangle again with its winding reversed.
        let mut both = Vec::new();
        for i in 0..200u16 {
            both.push([i, i + 1, i + 2, NO_INDEX]);
            both.push([i + 2, i + 1, i, NO_INDEX]);
        }
        let m = Mesh::new(pts(MESH_POINTS_MAX), both.clone()).unwrap();
        assert_eq!(m.edges(), naive_edges(&both));
        // Pseudo-random quads and triangles from a fixed LCG.
        let mut x = 12345u32;
        let mut next = |m: u32| {
            x = x.wrapping_mul(1664525).wrapping_add(1013904223);
            (x >> 8) % m
        };
        for n_pts in [4usize, 7, 40, 300] {
            let mut faces = Vec::new();
            while faces.len() < 300 {
                let mut f = [0u16; 4];
                let quad = next(2) == 1 && n_pts > 3;
                let k = if quad { 4 } else { 3 };
                let mut ok = true;
                for i in 0..k {
                    f[i] = next(n_pts as u32) as u16;
                    if f[..i].contains(&f[i]) {
                        ok = false;
                    }
                }
                if !quad {
                    f[3] = NO_INDEX;
                }
                if ok {
                    faces.push(f);
                }
            }
            let m = Mesh::new(pts(n_pts), faces.clone()).unwrap();
            assert_eq!(m.edges(), naive_edges(&faces), "{n_pts} points");
        }
        for n in BUILTIN_NAMES {
            let m = builtin(n).unwrap();
            assert_eq!(m.edges(), naive_edges(m.faces()), "{n}");
        }
    }

    #[test]
    fn edges_are_unique() {
        let cube = builtin("cube").unwrap();
        assert_eq!(cube.edges().len(), 12);
    }

    #[test]
    fn builtins_have_their_counts() {
        let counts: Vec<(usize, usize)> = BUILTIN_NAMES.iter().map(|n| { let m = builtin(n).unwrap(); (m.points().len(), m.faces().len()) }).collect();
        assert_eq!(counts, [(8, 6), (5, 5), (6, 8), (42, 48), (72, 72)]);
        for (n, c) in BUILTIN_NAMES.iter().zip(&counts) {
            assert_eq!(builtin_counts(n), Some(*c), "{n}");
        }
        assert!(builtin("teapot").is_none());
        assert_eq!(builtin_counts("teapot"), None);
    }

    #[test]
    fn mesh_new_rejects_far_points_and_bad_fourth_slots() {
        let quad = vec![(0, 0, 0), (10, 0, 0), (10, 10, 0), (0, 10, 0)];
        assert!(matches!(Mesh::new(vec![(40000, 0, 0), (0, 0, 0), (0, 1, 0)], vec![]), Err(MeshError::Bad)));
        assert!(matches!(Mesh::new(vec![(i32::MIN, 0, 0), (0, 0, 0), (0, 1, 0)], vec![]), Err(MeshError::Bad)));
        assert!(Mesh::new(vec![(32767, -32767, 0), (0, 0, 0), (0, 1, 0)], vec![]).is_ok());
        assert!(matches!(Mesh::new(quad.clone(), vec![[0, 1, 2, 2]]), Err(MeshError::Bad)), "repeated 4th");
        assert!(matches!(Mesh::new(quad.clone(), vec![[0, 1, 2, 4]]), Err(MeshError::Bad)), "out-of-range 4th");
        assert!(Mesh::new(quad, vec![[0, 1, 2, NO_INDEX]]).is_ok());
    }

    #[test]
    fn mesh_new_checks_caps_before_indices() {
        let mut faces = vec![[0, 1, 9, NO_INDEX]; MESH_FACES_MAX + 1];
        faces[0] = [0, 1, 9, NO_INDEX];
        let tri = vec![(0, 0, 0), (10, 0, 0), (0, 10, 0)];
        assert!(matches!(Mesh::new(tri, faces), Err(MeshError::TooBig)));
    }

    #[test]
    fn edges_of_a_mixed_mesh_dedupe_opposite_directions() {
        let pts = vec![(0, 0, 0), (10, 0, 0), (10, 10, 0), (0, 10, 0), (5, 20, 0)];
        // quad 0-1-2-3 plus triangle 3-2-4 sharing edge 2-3 backwards.
        let m = Mesh::new(pts, vec![[0, 1, 2, 3], [3, 2, 4, NO_INDEX]]).unwrap();
        assert_eq!(m.edges(), vec![(0, 1), (1, 2), (2, 3), (0, 3), (2, 4), (3, 4)]);
    }

    #[test]
    fn project_is_safe_for_any_size_and_point() {
        assert_eq!(project((0, 0, 0), 0, 0, 0), None);
        assert_eq!(project((0, 0, 0), 0, 0, -5), None);
        for &s in &[1, 64, i32::MAX] {
            for &c in &[i32::MIN, -1, 0, 1, i32::MAX] {
                if let Some((x, y)) = project((c, c, c), c, c, s) {
                    assert!(x.abs() <= 1 << 30 && y.abs() <= 1 << 30);
                }
            }
        }
        assert_eq!(project((i32::MAX, 0, 0), 0, 0, i32::MAX), Some((1 << 30, 0)));
    }

    #[test]
    fn sin_and_cos_accept_any_angle() {
        assert_eq!(sin(i32::MAX), SIN[255]);
        assert_eq!(cos(i32::MAX), SIN[(255 + 64) & 255]);
        assert_eq!(cos(i32::MIN), SIN[64]);
        assert_eq!(sin(i32::MIN), 0);
        assert_eq!(sub((i32::MAX, i32::MIN, 0), (-1, i32::MAX, 0)).0, i32::MAX as i64 + 1);
    }

    #[test]
    fn rotate_never_panics() {
        let (x, y, z) = rotate((i32::MAX, i32::MIN, 7), i32::MAX, i32::MIN, 3);
        let _ = (x, y, z);
        assert_eq!(rotate((i32::MAX, 0, 0), 0, 0, 0), (i32::MAX, 0, 0));
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
            for f in m.faces() {
                let (a, b, c) = (m.points()[f[0] as usize], m.points()[f[1] as usize], m.points()[f[2] as usize]);
                let n = cross(sub(b, a), sub(c, a));
                let centre = (a.0 as i64 + b.0 as i64 + c.0 as i64, a.1 as i64 + b.1 as i64 + c.1 as i64, a.2 as i64 + b.2 as i64 + c.2 as i64);
                assert!(n.0 * centre.0 + n.1 * centre.1 + n.2 * centre.2 > 0, "{name} face {f:?} winds inward");
            }
        }
        // Torus: each face's normal points away from the tube's centre circle
        // (radius 70, y = 0). Work in units of 1/4 so the quad centre is exact.
        let m = builtin("torus").unwrap();
        for f in m.faces() {
            let p: Vec<(i32, i32, i32)> = f.iter().map(|&i| m.points()[i as usize]).collect();
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

    // ---- Task 3: drawing meshes ----

    use crate::rgb565;

    fn colours(c: &Canvas) -> Vec<u16> {
        let mut v: Vec<u16> = c.pixels().iter().copied().filter(|&p| p != 0).collect();
        v.sort();
        v.dedup();
        v
    }

    fn quad_mesh(q: [(i32, i32, i32); 4]) -> Mesh {
        Mesh::new(q.to_vec(), vec![[0, 1, 2, 3]]).unwrap()
    }

    /// Wound so its outward normal (Task 2's convention) points at the camera (-z).
    fn facing_quad(z: i32, r: i32) -> [(i32, i32, i32); 4] {
        [(-r, -r, z), (-r, r, z), (r, r, z), (r, -r, z)]
    }

    #[test]
    fn a_wire_cube_draws_12_edges() {
        let m = builtin("cube").unwrap();
        let (cx, cy, size, rx, ry, rz) = (60, 60, 24, 20, 30, 0);
        let mut c = Canvas::new(120, 120);
        draw_mesh(&mut c, &m, cx, cy, size, rx, ry, rz, 0, 0xFFFFFF);
        assert_eq!(colours(&c), [rgb565(0xFFFFFF)], "every lit pixel is the colour");
        let lit = |x: i32, y: i32| (-1..=1).any(|dy| (-1..=1).any(|dx| c.pixel(x + dx, y + dy) == Some(rgb565(0xFFFFFF))));
        assert_eq!(m.edges().len(), 12);
        for &(a, b) in m.edges() {
            let pa = project(rotate(m.points()[a as usize], rx, ry, rz), cx, cy, size).unwrap();
            let pb = project(rotate(m.points()[b as usize], rx, ry, rz), cx, cy, size).unwrap();
            let (mx, my) = ((pa.0 + pb.0) / 2, (pa.1 + pb.1) / 2);
            assert!((0..120).contains(&mx) && (0..120).contains(&my), "midpoint on canvas");
            assert!(lit(mx, my), "edge {a}-{b} midpoint ({mx},{my}) not drawn");
        }
    }

    #[test]
    fn a_solid_cube_shows_at_most_three_faces_lit_differently() {
        let m = builtin("cube").unwrap();
        let mut c = Canvas::new(120, 120);
        draw_mesh(&mut c, &m, 60, 60, 24, 20, 30, 0, 1, 0xFFFFFF);
        let n = colours(&c).len();
        assert!((2..=3).contains(&n), "three faces show, not all lit the same: {n} colours");
    }

    #[test]
    fn front_face_drawn_back_face_culled() {
        let m = quad_mesh(facing_quad(-50, 20));
        let mut c = Canvas::new(120, 120);
        draw_mesh(&mut c, &m, 60, 60, 64, 0, 0, 0, 1, 0xFFFFFF);
        assert!(c.pixel(60, 60).unwrap() != 0, "a face toward the camera is drawn");
        let mut c = Canvas::new(120, 120);
        draw_mesh(&mut c, &m, 60, 60, 64, 0, 128, 0, 1, 0xFFFFFF);
        assert!(colours(&c).is_empty(), "turned half round it faces away and is culled");
    }

    #[test]
    fn nearer_faces_are_drawn_over_farther_ones() {
        // One colour per mesh, so the two quads are told apart by their
        // lighting: the far one is tilted (still facing the camera).
        let near = facing_quad(-50, 20);
        let far = [(-30, -30, 30), (-30, 30, 30), (30, 30, 70), (30, -30, 70)];
        let alone = |q| {
            let mut c = Canvas::new(120, 120);
            draw_mesh(&mut c, &quad_mesh(q), 60, 60, 64, 0, 0, 0, 1, 0xFF0000);
            c.pixel(60, 60).unwrap()
        };
        let (near_c, far_c) = (alone(near), alone(far));
        assert!(near_c != 0 && far_c != 0 && near_c != far_c, "{near_c:x} vs {far_c:x}");
        // The near face is listed first, so face order alone would paint it under.
        let mut pts = near.to_vec();
        pts.extend_from_slice(&far);
        let m = Mesh::new(pts, vec![[0, 1, 2, 3], [4, 5, 6, 7]]).unwrap();
        let mut c = Canvas::new(120, 120);
        draw_mesh(&mut c, &m, 60, 60, 64, 0, 0, 0, 1, 0xFF0000);
        assert_eq!(c.pixel(60, 60).unwrap(), near_c, "the overlap shows the nearer face");
        assert_eq!(colours(&c), { let mut v = vec![near_c, far_c]; v.sort(); v }, "the far face shows round the near one");
    }

    #[test]
    fn faces_behind_the_camera_are_skipped() {
        let m = Mesh::new(vec![(-20, -20, -800), (-20, 20, -800), (20, 20, -800)], vec![[0, 1, 2, NO_INDEX]]).unwrap();
        for mode in 0..3 {
            let mut c = Canvas::new(120, 120);
            draw_mesh(&mut c, &m, 60, 60, 64, 0, 0, 0, mode, 0xFFFFFF);
            assert!(colours(&c).is_empty(), "mode {mode} drew a face behind the camera");
        }
    }

    #[test]
    fn modes_2_and_unknown() {
        let m = builtin("cube").unwrap();
        let draw = |mode| {
            let mut c = Canvas::new(120, 120);
            draw_mesh(&mut c, &m, 60, 60, 24, 20, 30, 0, mode, 0x804020);
            c
        };
        let bright = rgb565(0xBF9F8F); // each channel half way to 255
        let (solid, both) = (draw(1), draw(2));
        assert!(both.pixels().contains(&bright), "edges in the brightened colour");
        assert!(!solid.pixels().contains(&bright));
        for (s, b) in solid.pixels().iter().zip(both.pixels()) {
            assert!(b == s || *b == bright, "mode 2 is mode 1 plus brightened edges");
        }
        assert!(colours(&draw(7)).is_empty(), "an unknown mode draws nothing");
        assert!(colours(&draw(-1)).is_empty());
    }

    #[test]
    fn mesh_cost_counts_points_lines_and_fills() {
        let m = builtin("cube").unwrap();
        let (cx, cy, size, rx, ry, rz, w, h) = (60, 60, 24, 20, 30, 0, 120, 120);
        let pts: Vec<(i32, i32)> = m.points().iter().map(|&p| project(rotate(p, rx, ry, rz), cx, cy, size).unwrap()).collect();
        let base = 64 * (8 + 6) as u64;
        let lines: u64 = m.edges().iter().map(|&(a, b)| {
            let (p, q) = (pts[a as usize], pts[b as usize]);
            ((p.0 - q.0).abs().max((p.1 - q.1).abs()) + 1) as u64
        }).sum();
        // Visible triangles found in 3D, independently of the renderer: the
        // normal points back toward the eye at z = -512, whatever the size.
        let eye = (0, 0, -512);
        let mut fills = 0u64;
        let mut visible = 0;
        for f in m.faces() {
            for t in [[f[0], f[1], f[2]], [f[0], f[2], f[3]]] {
                let r: Vec<_> = t.iter().map(|&i| rotate(m.points()[i as usize], rx, ry, rz)).collect();
                let n = cross(sub(r[1], r[0]), sub(r[2], r[0]));
                let v = sub(r[0], eye);
                if n.0 * v.0 + n.1 * v.1 + n.2 * v.2 >= 0 { continue; }
                visible += 1;
                let p: Vec<_> = t.iter().map(|&i| pts[i as usize]).collect();
                // The box's size, each side capped at the screen's.
                let bw = (p.iter().map(|q| q.0).max().unwrap() - p.iter().map(|q| q.0).min().unwrap() + 1).min(w);
                let bh = (p.iter().map(|q| q.1).max().unwrap() - p.iter().map(|q| q.1).min().unwrap() + 1).min(h);
                fills += (bw * bh) as u64;
            }
        }
        assert_eq!(visible, 6, "three faces, two triangles each");
        assert_eq!(mesh_cost(&m, cx, cy, size, rx, ry, rz, 0, w, h), base + lines);
        assert_eq!(mesh_cost(&m, cx, cy, size, rx, ry, rz, 1, w, h), base + fills);
        assert_eq!(mesh_cost(&m, cx, cy, size, rx, ry, rz, 2, w, h), base + fills + lines);
        assert_eq!(mesh_cost(&m, cx, cy, size, rx, ry, rz, 7, w, h), base);
        // A 4x4 screen caps each line at 16 pixels, and the cap does bind.
        let line_len = |&(a, b): &(u16, u16)| {
            let (p, q) = (pts[a as usize], pts[b as usize]);
            ((p.0 - q.0).abs().max((p.1 - q.1).abs()) + 1) as u64
        };
        assert!(m.edges().iter().any(|e| line_len(e) > 16));
        let small = mesh_cost(&m, cx, cy, size, rx, ry, rz, 0, 4, 4);
        assert_eq!(small, base + m.edges().iter().map(|e| line_len(e).min(16)).sum::<u64>());
        // Same answer whatever canvas the mesh is then drawn on.
        for (cw, ch) in [(1, 1), (120, 120), (640, 480)] {
            let mut c = Canvas::new(cw, ch);
            draw_mesh(&mut c, &m, cx, cy, size, rx, ry, rz, 2, 0xFFFFFF);
            assert_eq!(mesh_cost(&m, cx, cy, size, rx, ry, rz, 2, w, h), base + fills + lines);
        }
    }

    #[test]
    fn off_canvas_triangles_are_still_paid_for_and_draw_nothing() {
        // 1024 copies of one facing quad, far off the left edge: every
        // triangle spans every row of the screen but no column of it.
        let m = Mesh::new(facing_quad(-50, 20).to_vec(), vec![[0, 1, 2, 3]; MESH_FACES_MAX]).unwrap();
        let (cx, cy, size, w, h) = (-2_000_000, 240, 6400, 640, 480);
        let (proj, _) = prepare(&m, cx, cy, size, 0, 0, 0);
        let ys: Vec<i64> = proj.iter().map(|p| p.unwrap().1 as i64).collect();
        let rows = (ys.iter().max().unwrap() - ys.iter().min().unwrap() + 1).min(h as i64) as u64;
        assert_eq!(rows, h as u64, "the triangles span every row");
        let cost = mesh_cost(&m, cx, cy, size, 0, 0, 0, 1, w, h);
        assert!(cost >= 1024 * 2 * rows, "cost {cost} < {}", 1024 * 2 * rows);
        let mut c = Canvas::new(w, h);
        draw_mesh(&mut c, &m, cx, cy, size, 0, 0, 0, 1, 0xFFFFFF);
        assert!(colours(&c).is_empty(), "nothing is drawn on the canvas");
    }

    #[test]
    fn the_largest_valid_mesh_draws_and_costs_at_any_size() {
        // Every corner at +-32767, the most Mesh::new allows, with the faces
        // and edges of the cube; debug builds trap any i64 overflow.
        let cube = builtin("cube").unwrap();
        let pts: Vec<(i32, i32, i32)> = cube.points().iter().map(|p| (p.0.signum() * COORD_MAX, p.1.signum() * COORD_MAX, p.2.signum() * COORD_MAX)).collect();
        let m = Mesh::new(pts, cube.faces().to_vec()).unwrap();
        for (rx, ry, rz) in [(0, 0, 0), (32, 32, 32), (20, 30, 0), (128, 64, 200)] {
            for mode in 0..3 {
                let mut c = Canvas::new(64, 64);
                draw_mesh(&mut c, &m, 32, 32, i32::MAX, rx, ry, rz, mode, 0xFFFFFF);
                assert!(mesh_cost(&m, 32, 32, i32::MAX, rx, ry, rz, mode, 64, 64) >= 64 * 8);
            }
        }
    }

    #[test]
    fn the_projection_keeps_its_perspective_at_any_size() {
        let p = (50, 50, -100);
        let (x1, y1) = project(p, 0, 0, 64).unwrap();
        let (x10, y10) = project(p, 0, 0, 640).expect("size never moves a point behind the camera");
        assert!((x10 - 10 * x1).abs() <= 1 && (y10 - 10 * y1).abs() <= 1, "size 64 ({x1},{y1}), size 640 ({x10},{y10})");
        assert_eq!((x1, y1), (62, -62), "50 * 512 / 412 pixels off centre");
    }

    #[test]
    fn a_big_cube_is_never_behind_the_camera() {
        let m = builtin("cube").unwrap();
        // (32, 32, 0) and (32, 224, 0) turn a corner straight at the camera.
        for (rx, ry, rz) in [(0, 0, 0), (20, 30, 0), (32, 32, 0), (32, 224, 0), (96, 160, 40), (128, 0, 0)] {
            let (proj, _) = prepare(&m, 120, 100, 400, rx, ry, rz);
            assert!(proj.iter().all(|p| p.is_some()), "pose ({rx},{ry},{rz}): {proj:?}");
        }
    }

    #[test]
    fn isqrt_is_the_floor_root() {
        for n in 0..2000u128 {
            let r = isqrt_u128(n);
            assert!(r * r <= n && (r + 1) * (r + 1) > n, "isqrt({n}) = {r}");
        }
        assert_eq!(isqrt_u128(u128::MAX), u64::MAX as u128);
        assert_eq!(isqrt_u128((1u128 << 70) * 3), 59_512_812_588); // Python math.isqrt(3 << 70)
    }

    #[test]
    fn drawing_and_costing_never_panic() {
        let far = Mesh::new(vec![(32767, -32767, 32767), (-32767, 32767, -32767), (32767, 32767, -32767), (-32767, -32767, 32767)], vec![[0, 1, 2, 3], [3, 2, 1, NO_INDEX]]).unwrap();
        for &size in &[i32::MIN, 0, 1, 64, 1 << 20, i32::MAX] {
            for &(cx, cy) in &[(i32::MIN, i32::MAX), (0, 0), (i32::MAX, i32::MIN)] {
                for mode in 0..3 {
                    let mut c = Canvas::new(16, 16);
                    draw_mesh(&mut c, &far, cx, cy, size, 37, 200, 91, mode, 0xFFFFFF);
                    let cost = mesh_cost(&far, cx, cy, size, 37, 200, 91, mode, i32::MAX, i32::MAX);
                    assert!(cost >= 64 * 4);
                    mesh_cost(&far, cx, cy, size, 37, 200, 91, mode, -5, -5);
                }
            }
        }
    }
}
