//! Shape cues: a solid square, rectangle, circle, oval or star, drawn on the
//! processor at the output's size (see `show::ShapeCue`). The picture is
//! transparent but for the shape, with soft edges, and the cue's geometry
//! moves and scales it like any other picture.

use crate::media::video::RgbaImage;
use crate::show::{ShapeCue, ShapeKind};

/// The fewest and most points a star can have.
pub const MIN_POINTS: u8 = 3;
pub const MAX_POINTS: u8 = 12;

/// How far in a star's valleys are, as a share of its points' distance.
const STAR_INNER: f32 = 0.42;

/// The outline of the shape, in pixels, around the point (0, 0).
enum Outline {
    /// Half the width and half the height.
    Box(f32, f32),
    Ellipse(f32, f32),
    /// The corners, in order.
    Polygon(Vec<(f32, f32)>),
}

impl Outline {
    fn of(cue: &ShapeCue, output_h: f32) -> Outline {
        let height = cue.height.clamp(0.0, 4.0) * output_h;
        let width = if cue.shape.has_width() {
            cue.width.clamp(0.0, 4.0) * output_h
        } else {
            height
        };
        let (hw, hh) = (width / 2.0, height / 2.0);
        match cue.shape {
            ShapeKind::Square | ShapeKind::Rectangle => Outline::Box(hw, hh),
            ShapeKind::Circle | ShapeKind::Oval => Outline::Ellipse(hw, hh),
            ShapeKind::Star => {
                let n = usize::from(cue.points.clamp(MIN_POINTS, MAX_POINTS));
                let (outer, inner) = (hh, hh * STAR_INNER);
                let corners = (0..n * 2)
                    .map(|k| {
                        let r = if k % 2 == 0 { outer } else { inner };
                        // The first point is straight up.
                        let a = -std::f32::consts::FRAC_PI_2
                            + k as f32 * std::f32::consts::PI / n as f32;
                        (r * a.cos(), r * a.sin())
                    })
                    .collect();
                Outline::Polygon(corners)
            }
        }
    }

    /// Half the size of the box around it.
    fn extent(&self) -> (f32, f32) {
        match self {
            Outline::Box(a, b) | Outline::Ellipse(a, b) => (*a, *b),
            Outline::Polygon(p) => p.iter().fold((0.0, 0.0), |(x, y), (px, py)| {
                (f32::max(x, px.abs()), f32::max(y, py.abs()))
            }),
        }
    }

    fn contains(&self, x: f32, y: f32) -> bool {
        match self {
            Outline::Box(a, b) => x.abs() <= *a && y.abs() <= *b,
            Outline::Ellipse(a, b) => {
                *a > 0.0 && *b > 0.0 && (x / a) * (x / a) + (y / b) * (y / b) <= 1.0
            }
            Outline::Polygon(p) => {
                // Even-odd: count the edges a ray to the right crosses.
                let mut inside = false;
                let mut j = p.len() - 1;
                for i in 0..p.len() {
                    let ((xi, yi), (xj, yj)) = (p[i], p[j]);
                    if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
                        inside = !inside;
                    }
                    j = i;
                }
                inside
            }
        }
    }

    /// How much of the pixel with its corner at (x, y) the shape covers, 0 to 1.
    fn coverage(&self, x: f32, y: f32) -> f32 {
        let at = |dx: f32, dy: f32| self.contains(x + dx, y + dy);
        let corners = [at(0.0, 0.0), at(1.0, 0.0), at(0.0, 1.0), at(1.0, 1.0)];
        let centre = at(0.5, 0.5);
        if centre && corners.iter().all(|&c| c) {
            return 1.0;
        }
        if !centre && corners.iter().all(|&c| !c) {
            return 0.0;
        }
        // On the edge: count a finer grid of points.
        const N: u32 = 6;
        let mut hit = 0;
        for j in 0..N {
            for i in 0..N {
                if at((i as f32 + 0.5) / N as f32, (j as f32 + 0.5) / N as f32) {
                    hit += 1;
                }
            }
        }
        hit as f32 / (N * N) as f32
    }
}

/// Draw the shape in the middle of a picture of `size` pixels.
pub fn render(cue: &ShapeCue, size: (u32, u32)) -> RgbaImage {
    let (w, h) = (size.0.max(2), size.1.max(2));
    let mut img = RgbaImage {
        width: w,
        height: h,
        data: vec![0; (w * h * 4) as usize],
    };
    let outline = Outline::of(cue, h as f32);
    let (ex, ey) = outline.extent();
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    let x0 = ((cx - ex).floor() - 1.0).max(0.0) as u32;
    let x1 = (((cx + ex).ceil() + 1.0) as u32).min(w);
    let y0 = ((cy - ey).floor() - 1.0).max(0.0) as u32;
    let y1 = (((cy + ey).ceil() + 1.0) as u32).min(h);
    let alpha = f32::from(cue.color[3]) / 255.0;
    for y in y0..y1 {
        for x in x0..x1 {
            let a = outline.coverage(x as f32 - cx, y as f32 - cy) * alpha;
            if a > 0.0 {
                let i = ((y * w + x) * 4) as usize;
                img.data[i..i + 3].copy_from_slice(&cue.color[..3]);
                img.data[i + 3] = (a * 255.0).round() as u8;
            }
        }
    }
    img
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: (u32, u32) = (640, 360);

    fn cue(shape: ShapeKind) -> ShapeCue {
        let mut c = ShapeCue::new(None);
        c.shape = shape;
        c.color = [200, 30, 60, 255];
        c
    }

    fn alpha(img: &RgbaImage, x: u32, y: u32) -> u8 {
        img.data[((y * img.width + x) * 4 + 3) as usize]
    }

    /// Bounding box of the pixels that are more than half there.
    fn ink(img: &RgbaImage) -> (u32, u32, u32, u32) {
        let mut b = (u32::MAX, u32::MAX, 0, 0);
        for y in 0..img.height {
            for x in 0..img.width {
                if alpha(img, x, y) > 127 {
                    b = (b.0.min(x), b.1.min(y), b.2.max(x), b.3.max(y));
                }
            }
        }
        b
    }

    #[test]
    fn a_square_is_as_wide_as_it_is_tall_and_a_rectangle_has_its_own_width() {
        // 0.4 of a 360 pixel height is 144 pixels.
        let sq = render(&cue(ShapeKind::Square), SIZE);
        assert_eq!((sq.width, sq.height), SIZE);
        let (x0, y0, x1, y1) = ink(&sq);
        assert_eq!((x1 - x0 + 1, y1 - y0 + 1), (144, 144));
        // In the middle, and the colour is the cue's, solid.
        assert_eq!((x0 + x1).div_ceil(2), SIZE.0 / 2);
        assert_eq!((y0 + y1).div_ceil(2), SIZE.1 / 2);
        let i = ((180 * SIZE.0 + 320) * 4) as usize;
        assert_eq!(&sq.data[i..i + 4], &[200, 30, 60, 255]);
        assert_eq!(alpha(&sq, 0, 0), 0, "the corners are clear");
        // A rectangle: 0.6 of the height wide.
        let rc = render(&cue(ShapeKind::Rectangle), SIZE);
        let (x0, y0, x1, y1) = ink(&rc);
        assert_eq!((x1 - x0 + 1, y1 - y0 + 1), (216, 144));
        // Its width does not change a square.
        let mut wide = cue(ShapeKind::Square);
        wide.width = 3.0;
        let (x0, _, x1, _) = ink(&render(&wide, SIZE));
        assert_eq!(x1 - x0 + 1, 144);
    }

    #[test]
    fn a_circle_is_round_and_an_oval_is_stretched() {
        let c = render(&cue(ShapeKind::Circle), SIZE);
        let (x0, y0, x1, y1) = ink(&c);
        assert_eq!((x1 - x0 + 1, y1 - y0 + 1), (144, 144));
        // The corner of its box is empty, the edge midpoints are not.
        assert_eq!(alpha(&c, x0, y0), 0);
        assert_eq!(alpha(&c, x1, y1), 0);
        assert!(alpha(&c, (x0 + x1) / 2, y0 + 1) > 100);
        assert!(alpha(&c, x0 + 1, (y0 + y1) / 2) > 100);
        // The edge is soft: some pixels are partly covered.
        let soft = c.data.chunks(4).filter(|p| p[3] > 10 && p[3] < 245).count();
        assert!(soft > 100, "{soft} soft pixels");
        // An area of about pi r squared.
        let area: f32 = c.data.chunks(4).map(|p| f32::from(p[3]) / 255.0).sum();
        let want = std::f32::consts::PI * 72.0 * 72.0;
        assert!((area - want).abs() < want * 0.01, "{area} vs {want}");
        let o = render(&cue(ShapeKind::Oval), SIZE);
        let (x0, y0, x1, y1) = ink(&o);
        assert_eq!((x1 - x0 + 1, y1 - y0 + 1), (216, 144));
        assert_eq!(alpha(&o, x0, y0), 0);
    }

    #[test]
    fn a_star_has_its_points_up_and_hollows_between_them() {
        let s = render(&cue(ShapeKind::Star), SIZE);
        let (cx, cy) = (320u32, 180u32);
        // The middle is in, the top point is in, and the way between two
        // points at the same distance out is not.
        assert!(alpha(&s, cx, cy) > 250);
        assert!(alpha(&s, cx, cy - 66) > 250, "the top point");
        let a = 36.0_f32.to_radians();
        let (hx, hy) = (cx as f32 - 66.0 * a.sin(), cy as f32 - 66.0 * a.cos());
        assert_eq!(alpha(&s, hx as u32, hy as u32), 0, "between the points");
        // Six points make a different star; the number is kept in range.
        let mut six = cue(ShapeKind::Star);
        six.points = 6;
        assert!(alpha(&render(&six, SIZE), cx, cy - 66) > 250);
        six.points = 200;
        assert!(render(&six, SIZE).data.iter().any(|&b| b > 0));
        six.points = 0;
        assert!(render(&six, SIZE).data.iter().any(|&b| b > 0));
    }

    #[test]
    fn colour_alpha_and_size_are_kept_and_nothing_overflows_a_small_picture() {
        let mut c = cue(ShapeKind::Square);
        c.color[3] = 128;
        let img = render(&c, SIZE);
        assert!((127..=129).contains(&alpha(&img, 320, 180)));
        // Bigger than the picture: it fills it, and nothing is written outside.
        c.color[3] = 255;
        c.height = 4.0;
        let img = render(&c, (64, 36));
        assert!(img.data.chunks(4).all(|p| p[3] == 255));
        // Nothing at all.
        c.height = 0.0;
        assert!(render(&c, SIZE).data.iter().all(|&b| b == 0));
        // A tiny picture does not panic.
        let _ = render(&cue(ShapeKind::Star), (1, 1));
    }
}
