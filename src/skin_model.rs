//! The Oxide player model, posed and software-rendered for the skin editor preview.
//!
//! Box layout, walking poses and the cloak follow the Oxide client
//! (`visuals/entity_mesh` and `visuals/model_box` in hydra-client) so the preview
//! matches what the game draws, including the `classic_anim` loose-arm walk.
use glam::{EulerRot, Quat, Vec3};
use std::f32::consts::PI;

/// Player textures are edited on a fixed 64-pixel-wide layout.
pub(crate) const SIZE: usize = 64;

/// A 64 by 64 RGBA texture; legacy skins and cloaks use only the top 32 rows.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Texture {
    pub pixels: Vec<[u8; 4]>,
    pub height: usize,
}

impl Texture {
    pub fn new(height: usize) -> Self {
        Self {
            pixels: vec![[0; 4]; SIZE * SIZE],
            height,
        }
    }
    pub fn get(&self, x: usize, y: usize) -> [u8; 4] {
        self.pixels[y * SIZE + x]
    }
    pub fn set(&mut self, x: usize, y: usize, color: [u8; 4]) {
        self.pixels[y * SIZE + x] = color;
    }
    pub fn fill(&mut self, [x, y, w, h]: [usize; 4], color: [u8; 4]) {
        for y in y..y + h {
            for x in x..x + w {
                self.set(x, y, color);
            }
        }
    }
    /// The visible rows as tightly packed RGBA bytes.
    pub fn rgba(&self) -> Vec<u8> {
        self.pixels[..SIZE * self.height]
            .iter()
            .flatten()
            .copied()
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Walk {
    /// Beta 1.7.3's ordinary gait.
    Normal,
    /// The Oxide `classic_anim` perk: loose arms, head sway and body bounce.
    Classic,
}

/// Limb swing per tick at walking speed (EntityLiving's limb amount settles near this).
pub(crate) const WALK_AMOUNT: f32 = 0.86;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Motion {
    pub walk: Walk,
    /// Accumulated limb swing, in the game's units.
    pub phase: f32,
    /// 0 standing still, [`WALK_AMOUNT`] walking.
    pub amount: f32,
    /// Ticks the model has existed, for the idle arm sway.
    pub age: f32,
}

struct Pose {
    arms: [[f32; 3]; 2],
    arm_pivots: [[f32; 3]; 2],
    legs: [[f32; 3]; 2],
    leg_pivots: [[f32; 3]; 2],
    head_y: f32,
    head: [f32; 2],
    body_y: f32,
}

fn pose(motion: Motion) -> Pose {
    let Motion {
        walk,
        phase,
        amount,
        age,
    } = motion;
    let mut pose = match walk {
        Walk::Normal => {
            let gait = [0., PI].map(|offset| (phase * 0.6662 + offset).cos() * 1.4 * amount);
            Pose {
                arms: [[gait[1] / 1.4, 0., 0.], [gait[0] / 1.4, 0., 0.]],
                arm_pivots: [[-5., 2., 0.], [5., 2., 0.]],
                legs: [[gait[0], 0., 0.], [gait[1], 0., 0.]],
                leg_pivots: [[-2., 12., 0.], [2., 12., 0.]],
                head_y: 0.,
                head: [0.; 2],
                body_y: 0.,
            }
        }
        // ModelBiped.applyClassicAnimation.
        Walk::Classic => {
            let q = phase / 3. * 2.;
            let bounce = -((q * 0.6662).sin() * 3. * amount).abs();
            Pose {
                arms: [
                    [
                        (q * 0.6662 + PI).cos() * 2. * amount,
                        0.,
                        ((q * 0.2312).sin() + 1.) * amount,
                    ],
                    [
                        (q * 0.6662).cos() * 2. * amount,
                        0.,
                        ((q * 0.2812).sin() - 1.) * amount,
                    ],
                ],
                arm_pivots: [[-5., 2. + bounce, 0.], [5., 2. + bounce, 0.]],
                legs: [
                    [(q * 0.6662).sin() * 1.4 * amount, 0., 0.],
                    [(q * 0.6662 + PI).sin() * 1.4 * amount, 0., 0.],
                ],
                leg_pivots: [[-2., 12. + bounce, 0.], [2., 12. + bounce, 0.]],
                head_y: 0.5 + bounce,
                head: [
                    (q * 0.1).sin() * 0.8 / 1.5 * amount,
                    (q * 0.23).sin() * amount,
                ],
                body_y: bounce,
            }
        }
    };
    // The idle arm sway; the classic walk scales it up and drops it once moving.
    let idle = match walk {
        Walk::Normal => 1.,
        Walk::Classic if (amount * 10. + 0.5).floor() <= 1. => 1.25,
        Walk::Classic => 0.,
    };
    for (arm, side) in pose.arms.iter_mut().zip([1., -1.]) {
        arm[0] += (age * 0.067).sin() * 0.05 * idle * side;
        arm[2] += ((age * 0.09).cos() * 0.05 + 0.05) * idle * side;
    }
    pose
}

#[derive(Clone, Copy)]
enum Source {
    Skin,
    Cape,
}

/// A ModelRenderer box in Java model space (Y down, 24 at the feet).
#[derive(Clone)]
struct Part {
    pivot: [f32; 3],
    min: [f32; 3],
    size: [f32; 3],
    uv: [usize; 2],
    rotation: Quat,
    inflation: f32,
    mirror: bool,
    source: Source,
}

impl Part {
    fn new(pivot: [f32; 3], min: [f32; 3], size: [f32; 3], uv: [usize; 2]) -> Self {
        Self {
            pivot,
            min,
            size,
            uv,
            rotation: Quat::IDENTITY,
            inflation: 0.,
            mirror: false,
            source: Source::Skin,
        }
    }
    fn rotate(mut self, [x, y, z]: [f32; 3]) -> Self {
        self.rotation = Quat::from_euler(EulerRot::ZYX, -z, y, -x);
        self
    }
    fn inflate(mut self, inflation: f32) -> Self {
        self.inflation = inflation;
        self
    }
}

/// The texture rectangles `[u, v, w, h]` of a box's faces, in ModelRenderer's
/// order (+X, -X, -Y, +Y, -Z, +Z).
fn face_rectangles([u, v]: [usize; 2], size: [f32; 3]) -> [[usize; 4]; 6] {
    let [w, h, d] = size.map(|v| v as usize);
    [
        [u + d + w, v + d, d, h],
        [u, v + d, d, h],
        [u + d, v, w, d],
        [u + d + w, v, w, d],
        [u + d, v + d, w, h],
        [u + d + w + d, v + d, w, h],
    ]
}

pub(crate) struct Options {
    pub slim: bool,
    /// Draw the hat and, on 64 by 64 skins, the jacket, sleeve and trouser layers.
    pub overlay: bool,
}

fn parts(skin: &Texture, cape: bool, options: &Options, motion: Motion) -> Vec<Part> {
    let modern = skin.height == SIZE;
    let pose = pose(motion);
    let head = [pose.head[0], pose.head[1], 0.];
    let mut base = vec![
        Part::new([0., pose.head_y, 0.], [-4., -8., -4.], [8.; 3], [0, 0]).rotate(head),
        Part::new(
            [0., pose.body_y, 0.],
            [-4., 0., -2.],
            [8., 12., 4.],
            [16, 16],
        ),
    ];
    let mut overlays = Vec::new();
    if options.overlay {
        overlays.push(
            Part::new([0., pose.head_y, 0.], [-4., -8., -4.], [8.; 3], [32, 0])
                .rotate(head)
                .inflate(0.5),
        );
        if modern {
            overlays.push(
                Part::new(
                    [0., pose.body_y, 0.],
                    [-4., 0., -2.],
                    [8., 12., 4.],
                    [16, 32],
                )
                .inflate(0.25),
            );
        }
    }
    let arm = if options.slim { 3. } else { 4. };
    for i in 0..2 {
        let left = i == 1;
        let limbs = [
            (
                Part::new(
                    pose.arm_pivots[i],
                    [if left { -1. } else { 1. - arm }, -2., -2.],
                    [arm, 12., 4.],
                    if modern && left { [32, 48] } else { [40, 16] },
                )
                .rotate(pose.arms[i]),
                if left { [48, 48] } else { [40, 32] },
            ),
            (
                Part::new(
                    pose.leg_pivots[i],
                    [-2., 0., -2.],
                    [4., 12., 4.],
                    if modern && left { [16, 48] } else { [0, 16] },
                )
                .rotate(pose.legs[i]),
                if left { [0, 48] } else { [0, 32] },
            ),
        ];
        for (mut part, overlay_uv) in limbs {
            // Legacy skins reuse the right limbs, mirrored.
            part.mirror = left && !modern;
            if modern && options.overlay {
                let mut overlay = part.clone().inflate(0.25);
                overlay.uv = overlay_uv;
                overlays.push(overlay);
            }
            base.push(part);
        }
    }
    if cape {
        // RenderPlayer: the cloak leans back further the faster the player moves.
        let tilt = (6. + motion.amount * 30.).to_radians();
        let pivot = match motion.walk {
            Walk::Normal => [0., 0., 2.],
            Walk::Classic => [0., pose.body_y + 1., 2.1],
        };
        let mut cloak = Part::new(pivot, [-5., 0., -1.], [10., 16., 1.], [0, 0]);
        cloak.rotation = Quat::from_rotation_x(-tilt) * Quat::from_rotation_y(PI);
        cloak.source = Source::Cape;
        base.push(cloak);
    }
    // Translucent outer layers blend over whatever was drawn before them.
    base.extend(overlays);
    base
}

struct Quad {
    /// Model space, Y up, feet at 0.
    corners: [Vec3; 4],
    /// Texture coordinates in pixels.
    uv: [[f32; 2]; 4],
    rect: [usize; 4],
    outward: Vec3,
    source: Source,
}

fn quads(part: &Part) -> [Quad; 6] {
    let size = Vec3::from_array(part.size);
    let pivot = Vec3::new(part.pivot[0], 24. - part.pivot[1], part.pivot[2]);
    let place = |java: Vec3| {
        let local = pivot + part.rotation * Vec3::new(java.x, -java.y, java.z);
        // RenderLiving flips X as well as Y.
        Vec3::new(-local.x, local.y, local.z)
    };
    let min = Vec3::from_array(part.min);
    let corners = [
        [0., 0., 0.],
        [1., 0., 0.],
        [1., 1., 0.],
        [0., 1., 0.],
        [0., 0., 1.],
        [1., 0., 1.],
        [1., 1., 1.],
        [0., 1., 1.],
    ]
    .map(|corner| {
        let mut corner = Vec3::from_array(corner);
        if part.mirror {
            corner.x = 1. - corner.x;
        }
        place(
            min - Vec3::splat(part.inflation) + corner * (size + Vec3::splat(part.inflation * 2.)),
        )
    });
    let center = place(min + size / 2.);
    let indices = [
        [5, 1, 2, 6],
        [0, 4, 7, 3],
        [5, 4, 0, 1],
        [2, 3, 7, 6],
        [1, 0, 3, 2],
        [4, 5, 6, 7],
    ];
    let rectangles = face_rectangles(part.uv, part.size);
    std::array::from_fn(|face| {
        let rect = rectangles[face];
        let [u, v, w, h] = rect.map(|value| value as f32);
        let mut corners = indices[face].map(|index| corners[index]);
        let mut uv = [[u + w, v], [u, v], [u, v + h], [u + w, v + h]];
        if part.mirror {
            corners.reverse();
            uv.reverse();
        }
        let middle = corners.iter().sum::<Vec3>() / 4.;
        let normal = (corners[1] - corners[0])
            .cross(corners[2] - corners[0])
            .normalize_or_zero();
        Quad {
            corners,
            uv,
            rect,
            outward: if normal.dot(middle - center) < 0. {
                -normal
            } else {
                normal
            },
            source: part.source,
        }
    })
}

/// RenderHelper's two directional lights plus ambient, applied in view space.
fn light(normal: Vec3) -> f32 {
    let a = Vec3::new(0.2, 1., -0.7).normalize();
    let b = Vec3::new(-0.2, 1., 0.7).normalize();
    (0.4 + 0.6 * (normal.dot(a).max(0.) + normal.dot(b).max(0.))).min(1.)
}

pub(crate) struct Scene<'a> {
    pub skin: &'a Texture,
    pub cape: Option<&'a Texture>,
    pub options: Options,
    pub motion: Motion,
    /// Turn around the vertical axis, then tip toward the viewer, in radians.
    pub yaw: f32,
    pub pitch: f32,
}

const CAMERA_DISTANCE: f32 = 110.;
/// Model units kept in view: the player is 32 tall and swings its arms wide.
const VIEW_HEIGHT: f32 = 42.;
const VIEW_WIDTH: f32 = 30.;

/// A square RGBA tile repeated behind the model.
pub(crate) struct Backdrop<'a> {
    pub tile: &'a [u8],
    pub size: usize,
    /// Output pixels per tile pixel.
    pub zoom: f32,
}

/// Draws the scene over the tiled backdrop and returns opaque RGBA bytes.
pub(crate) fn render(
    scene: &Scene<'_>,
    width: usize,
    height: usize,
    backdrop: &Backdrop<'_>,
) -> Vec<u8> {
    let mut color = Vec::with_capacity(width * height * 4);
    let wrap = |pixel: usize| (pixel as f32 / backdrop.zoom) as usize % backdrop.size;
    let columns: Vec<usize> = (0..width).map(wrap).collect();
    let mut row = Vec::with_capacity(width * 4);
    let mut drawn = usize::MAX;
    for y in 0..height {
        let line = wrap(y);
        if line != drawn {
            drawn = line;
            row.clear();
            let source = &backdrop.tile[line * backdrop.size * 4..][..backdrop.size * 4];
            for &column in &columns {
                row.extend_from_slice(&source[column * 4..column * 4 + 3]);
                row.push(255);
            }
        }
        color.extend_from_slice(&row);
    }
    // Nearer surfaces have the larger reciprocal distance.
    let mut depth = vec![0_f32; width * height];
    let view = Quat::from_rotation_x(scene.pitch) * Quat::from_rotation_y(scene.yaw);
    let scale = (height as f32 / VIEW_HEIGHT).min(width as f32 / VIEW_WIDTH);
    let center = Vec3::new(0., 16., 0.);
    // The camera sits in front of the model (its face looks along -Z).
    let camera = Vec3::new(0., 0., CAMERA_DISTANCE);
    for part in parts(
        scene.skin,
        scene.cape.is_some(),
        &scene.options,
        scene.motion,
    ) {
        for quad in quads(&part) {
            let texture = match quad.source {
                Source::Skin => scene.skin,
                Source::Cape => scene.cape.unwrap_or(scene.skin),
            };
            let corners = quad.corners.map(|corner| view * (corner - center) + camera);
            let outward = view * quad.outward;
            // Solid boxes hide their far sides; outer layers show theirs through gaps.
            if part.inflation == 0. && outward.dot(corners[0]) >= 0. {
                continue;
            }
            let [u, v, w, h] = quad.rect;
            if (v..v + h).all(|y| (u..u + w).all(|x| texture.get(x, y)[3] == 0)) {
                continue;
            }
            let points = corners.map(|p| {
                let w = CAMERA_DISTANCE / p.z;
                [
                    width as f32 / 2. - p.x * w * scale,
                    height as f32 / 2. - p.y * w * scale,
                    1. / p.z,
                ]
            });
            let shade = (light(outward) * 256.) as u32;
            for triangle in [[0, 1, 2], [0, 2, 3]] {
                fill_triangle(
                    &mut color,
                    &mut depth,
                    [width, height],
                    triangle.map(|i| points[i]),
                    triangle.map(|i| quad.uv[i]),
                    quad.rect,
                    texture,
                    shade,
                );
            }
        }
    }
    color
}

#[allow(clippy::too_many_arguments)]
fn fill_triangle(
    color: &mut [u8],
    depth: &mut [f32],
    [width, height]: [usize; 2],
    points: [[f32; 3]; 3],
    uv: [[f32; 2]; 3],
    rect: [usize; 4],
    texture: &Texture,
    shade: u32,
) {
    let [a, b, c] = points;
    let area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
    if area.abs() < 1e-6 {
        return;
    }
    let bound = |axis: usize, limit: usize| {
        let low = a[axis].min(b[axis]).min(c[axis]);
        let high = a[axis].max(b[axis]).max(c[axis]);
        (
            (low.floor().max(0.) as usize).min(limit),
            (high.ceil().max(0.) as usize).min(limit),
        )
    };
    let (x0, x1) = bound(0, width);
    let (y0, y1) = bound(1, height);
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    // Each barycentric weight is affine in screen space: `[per x, per y, at the origin]`.
    let weight = |p: [f32; 3], q: [f32; 3]| {
        let (dx, dy) = ((p[1] - q[1]) / area, (q[0] - p[0]) / area);
        [dx, dy, -dx * p[0] - dy * p[1]]
    };
    let weights = [weight(b, c), weight(c, a), weight(a, b)];
    // So is anything interpolated with them; perspective-correct texturing
    // interpolates the coordinates divided by distance.
    let plane = |values: [f32; 3]| -> [f32; 3] {
        std::array::from_fn(|term| (0..3).map(|i| weights[i][term] * values[i]).sum())
    };
    let inverse = plane([a[2], b[2], c[2]]);
    let u_plane = plane([uv[0][0] * a[2], uv[1][0] * b[2], uv[2][0] * c[2]]);
    let v_plane = plane([uv[0][1] * a[2], uv[1][1] * b[2], uv[2][1] * c[2]]);
    let at = |plane: [f32; 3], x: f32, y: f32| plane[0] * x + plane[1] * y + plane[2];
    let [u0, v0, w, h] = rect;
    let (u1, v1) = ((u0 + w - 1) as f32, (v0 + h - 1) as f32);
    let (u0, v0) = (u0 as f32, v0 as f32);
    for y in y0..y1 {
        let (px, py) = (x0 as f32 + 0.5, y as f32 + 0.5);
        let mut edge = weights.map(|plane| at(plane, px, py));
        let mut near = at(inverse, px, py);
        let mut u = at(u_plane, px, py);
        let mut v = at(v_plane, px, py);
        for index in y * width + x0..y * width + x1 {
            if edge[0] >= 0. && edge[1] >= 0. && edge[2] >= 0. && near > depth[index] {
                let texel = texture.pixels
                    [(v / near).clamp(v0, v1) as usize * SIZE + (u / near).clamp(u0, u1) as usize];
                let alpha = u32::from(texel[3]);
                if alpha != 0 {
                    let lit = alpha * shade;
                    let pixel = &mut color[index * 4..index * 4 + 3];
                    for channel in 0..3 {
                        pixel[channel] = ((u32::from(texel[channel]) * lit
                            + u32::from(pixel[channel]) * (255 - alpha) * 256)
                            / (255 * 256)) as u8;
                    }
                    if alpha >= 128 {
                        depth[index] = near;
                    }
                }
            }
            edge[0] += weights[0][0];
            edge[1] += weights[1][0];
            edge[2] += weights[2][0];
            near += inverse[0];
            u += u_plane[0];
            v += v_plane[0];
        }
    }
}

/// Outlines of each body part's unwrapped faces, for the paint grid.
pub(crate) fn skin_guides(modern: bool) -> Vec<[usize; 4]> {
    let mut boxes = vec![
        ([0, 0], [8., 8., 8.]),
        ([32, 0], [8., 8., 8.]),
        ([16, 16], [8., 12., 4.]),
        ([40, 16], [4., 12., 4.]),
        ([0, 16], [4., 12., 4.]),
    ];
    if modern {
        boxes.extend([
            ([16, 32], [8., 12., 4.]),
            ([40, 32], [4., 12., 4.]),
            ([0, 32], [4., 12., 4.]),
            ([32, 48], [4., 12., 4.]),
            ([48, 48], [4., 12., 4.]),
            ([16, 48], [4., 12., 4.]),
            ([0, 48], [4., 12., 4.]),
        ]);
    }
    boxes
        .into_iter()
        .flat_map(|(uv, size)| face_rectangles(uv, size))
        .collect()
}

pub(crate) fn cape_guides() -> Vec<[usize; 4]> {
    face_rectangles([0, 0], [10., 16., 1.]).to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn painted() -> Texture {
        let mut skin = Texture::new(SIZE);
        skin.fill([0, 0, SIZE, SIZE], [200, 60, 40, 255]);
        skin
    }

    fn scene(skin: &Texture, walk: Walk, phase: f32, amount: f32) -> Scene<'_> {
        Scene {
            skin,
            cape: None,
            options: Options {
                slim: false,
                overlay: true,
            },
            motion: Motion {
                walk,
                phase,
                amount,
                age: 0.,
            },
            yaw: 0.5,
            pitch: 0.15,
        }
    }

    const SKY: Backdrop<'static> = Backdrop {
        tile: &[0, 0, 255, 255],
        size: 1,
        zoom: 1.,
    };

    #[test]
    fn model_is_drawn_centered_over_the_background() {
        let skin = painted();
        let image = render(&scene(&skin, Walk::Normal, 0., 0.), 120, 160, &SKY);
        assert_eq!(image.len(), 120 * 160 * 4);
        assert_eq!(image[..4], [0, 0, 255, 255]);
        let middle = (80 * 120 + 60) * 4;
        assert!(image[middle] > 60 && image[middle + 2] < 100);
    }

    #[test]
    fn walks_differ_only_while_moving() {
        let skin = painted();
        let draw = |walk, amount| render(&scene(&skin, walk, 4., amount), 90, 120, &SKY);
        assert!(draw(Walk::Normal, WALK_AMOUNT) != draw(Walk::Classic, WALK_AMOUNT));
        assert!(draw(Walk::Normal, WALK_AMOUNT) != draw(Walk::Normal, 0.));
    }

    #[test]
    fn classic_walk_matches_the_client_reference_pose() {
        // hydra-walking.csv: phase 4, amount 1 gives q = 8/3.
        let pose = pose(Motion {
            walk: Walk::Classic,
            phase: 4.,
            amount: 1.,
            age: 0.,
        });
        let q = 8. / 3. * 0.6662_f32;
        assert!((pose.body_y + (q.sin() * 3.).abs()).abs() < 1e-5);
        assert!((pose.legs[0][0] - q.sin() * 1.4).abs() < 1e-5);
        assert!((pose.arms[1][0] - q.cos() * 2.).abs() < 1e-5);
        assert_eq!(pose.head_y, 0.5 + pose.body_y);
    }

    #[test]
    fn guides_stay_inside_the_texture() {
        for [x, y, w, h] in skin_guides(true).into_iter().chain(cape_guides()) {
            assert!(x + w <= SIZE && y + h <= SIZE);
        }
        assert!(
            skin_guides(false)
                .iter()
                .all(|rect| rect[1] + rect[3] <= 32)
        );
    }
}
