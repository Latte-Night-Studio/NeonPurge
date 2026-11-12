//! A tiny software rasterizer. The whole game is drawn into a `Vec<u32>`
//! buffer that is handed to the window every frame.

use crate::font;
use crate::math::Vec2;

/// Pack 8-bit channels into minifb's 0RGB pixel format.
pub const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

#[inline]
fn channels(c: u32) -> (f32, f32, f32) {
    (
        ((c >> 16) & 0xff) as f32,
        ((c >> 8) & 0xff) as f32,
        (c & 0xff) as f32,
    )
}

pub fn lerp_color(a: u32, b: u32, t: f32) -> u32 {
    let t = t.clamp(0.0, 1.0);
    let (ar, ag, ab) = channels(a);
    let (br, bg, bb) = channels(b);
    rgb(
        (ar + (br - ar) * t) as u8,
        (ag + (bg - ag) * t) as u8,
        (ab + (bb - ab) * t) as u8,
    )
}

#[derive(Clone)]
pub struct Canvas {
    pub w: i32,
    pub h: i32,
    pub px: Vec<u32>,
}

impl Canvas {
    pub fn new(w: i32, h: i32) -> Self {
        Self {
            w,
            h,
            px: vec![0; (w.max(1) * h.max(1)) as usize],
        }
    }

    #[inline]
    fn index(&self, x: i32, y: i32) -> Option<usize> {
        if x >= 0 && y >= 0 && x < self.w && y < self.h {
            Some((y * self.w + x) as usize)
        } else {
            None
        }
    }

    pub fn copy_from(&mut self, src: &[u32]) {
        let n = self.px.len().min(src.len());
        self.px[..n].copy_from_slice(&src[..n]);
    }

    /// Straight alpha blend of one pixel.
    #[inline]
    pub fn blend(&mut self, x: i32, y: i32, color: u32, alpha: f32) {
        if alpha <= 0.0 {
            return;
        }
        let Some(i) = self.index(x, y) else { return };
        if alpha >= 1.0 {
            self.px[i] = color;
            return;
        }
        let d = self.px[i];
        let (dr, dg, db) = channels(d);
        let (sr, sg, sb) = channels(color);
        self.px[i] = rgb(
            (dr + (sr - dr) * alpha) as u8,
            (dg + (sg - dg) * alpha) as u8,
            (db + (sb - db) * alpha) as u8,
        );
    }

    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: u32) {
        let x0 = x.max(0);
        let y0 = y.max(0);
        let x1 = (x.saturating_add(w)).min(self.w);
        let y1 = (y.saturating_add(h)).min(self.h);
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        for yy in y0..y1 {
            let row = (yy * self.w) as usize;
            self.px[row + x0 as usize..row + x1 as usize].fill(color);
        }
    }

    pub fn rect_blend(&mut self, x: i32, y: i32, w: i32, h: i32, color: u32, alpha: f32) {
        if alpha <= 0.0 {
            return;
        }
        if alpha >= 1.0 {
            self.rect(x, y, w, h, color);
            return;
        }
        let x0 = x.max(0);
        let y0 = y.max(0);
        let x1 = (x.saturating_add(w)).min(self.w);
        let y1 = (y.saturating_add(h)).min(self.h);
        for yy in y0..y1 {
            for xx in x0..x1 {
                self.blend(xx, yy, color, alpha);
            }
        }
    }

    pub fn rect_outline(
        &mut self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        t: i32,
        color: u32,
        alpha: f32,
    ) {
        self.rect_blend(x, y, w, t, color, alpha);
        self.rect_blend(x, y + h - t, w, t, color, alpha);
        self.rect_blend(x, y, t, h, color, alpha);
        self.rect_blend(x + w - t, y, t, h, color, alpha);
    }

    pub fn circle(&mut self, center: Vec2, radius: f32, color: u32, alpha: f32) {
        if !center.is_finite() || radius <= 0.0 || alpha <= 0.0 {
            return;
        }
        let x0 = (center.x - radius - 1.0).floor() as i32;
        let y0 = (center.y - radius - 1.0).floor() as i32;
        let x1 = (center.x + radius + 1.0).ceil() as i32;
        let y1 = (center.y + radius + 1.0).ceil() as i32;
        for y in y0..=y1 {
            let dy = y as f32 + 0.5 - center.y;
            for x in x0..=x1 {
                let dx = x as f32 + 0.5 - center.x;
                let d = (dx * dx + dy * dy).sqrt();
                let coverage = (radius - d + 0.5).clamp(0.0, 1.0);
                if coverage > 0.0 {
                    self.blend(x, y, color, alpha * coverage);
                }
            }
        }
    }

    pub fn ring(
        &mut self,
        center: Vec2,
        radius: f32,
        thickness: f32,
        color: u32,
        alpha: f32,
    ) {
        if !center.is_finite() || radius <= 0.0 || alpha <= 0.0 {
            return;
        }
        let outer = radius + thickness * 0.5;
        let inner = (radius - thickness * 0.5).max(0.0);
        let x0 = (center.x - outer - 1.0).floor() as i32;
        let y0 = (center.y - outer - 1.0).floor() as i32;
        let x1 = (center.x + outer + 1.0).ceil() as i32;
        let y1 = (center.y + outer + 1.0).ceil() as i32;
        for y in y0..=y1 {
            let dy = y as f32 + 0.5 - center.y;
            for x in x0..=x1 {
                let dx = x as f32 + 0.5 - center.x;
                let d = (dx * dx + dy * dy).sqrt();
                let cov_outer = (outer - d + 0.5).clamp(0.0, 1.0);
                let cov_inner = (d - inner + 0.5).clamp(0.0, 1.0);
                let cov = cov_outer.min(cov_inner);
                if cov > 0.0 {
                    self.blend(x, y, color, alpha * cov);
                }
            }
        }
    }

    /// Soft additive light blob.
    pub fn glow(&mut self, center: Vec2, radius: f32, color: u32, intensity: f32) {
        if !center.is_finite() || radius <= 0.0 || intensity <= 0.0 {
            return;
        }
        let x0 = (center.x - radius).floor() as i32;
        let y0 = (center.y - radius).floor() as i32;
        let x1 = (center.x + radius).ceil() as i32;
        let y1 = (center.y + radius).ceil() as i32;
        let (sr, sg, sb) = channels(color);
        let inv = 1.0 / radius;
        for y in y0..=y1 {
            let dy = y as f32 + 0.5 - center.y;
            for x in x0..=x1 {
                let dx = x as f32 + 0.5 - center.x;
                let d = (dx * dx + dy * dy).sqrt();
                if d >= radius {
                    continue;
                }
                let t = 1.0 - d * inv;
                let a = intensity * t * t;
                let Some(i) = self.index(x, y) else { continue };
                let p = self.px[i];
                let (dr, dg, db) = channels(p);
                self.px[i] = rgb(
                    (dr + sr * a).min(255.0) as u8,
                    (dg + sg * a).min(255.0) as u8,
                    (db + sb * a).min(255.0) as u8,
                );
            }
        }
    }

    pub fn line(&mut self, a: Vec2, b: Vec2, width: f32, color: u32, alpha: f32) {
        if !a.is_finite() || !b.is_finite() || alpha <= 0.0 || width <= 0.0 {
            return;
        }
        let half = width * 0.5;
        if half <= 0.5 {
            // Fast path for hairlines.
            let steps = ((b - a).len().ceil() as i32).clamp(1, 4096);
            for i in 0..=steps {
                let p = a.lerp(b, i as f32 / steps as f32);
                self.blend(p.x as i32, p.y as i32, color, alpha);
            }
            return;
        }
        let ab = b - a;
        let len2 = ab.len_sq();
        if len2 < 1e-6 {
            self.circle(a, half, color, alpha);
            return;
        }
        let pad = half + 1.0;
        let x0 = (a.x.min(b.x) - pad).floor() as i32;
        let y0 = (a.y.min(b.y) - pad).floor() as i32;
        let x1 = (a.x.max(b.x) + pad).ceil() as i32;
        let y1 = (a.y.max(b.y) + pad).ceil() as i32;
        for y in y0..=y1 {
            for x in x0..=x1 {
                let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
                let d = p.dist(a + ab * t);
                let cov = (half - d + 0.5).clamp(0.0, 1.0);
                if cov > 0.0 {
                    self.blend(x, y, color, alpha * cov);
                }
            }
        }
    }

    pub fn triangle(&mut self, a: Vec2, b: Vec2, c: Vec2, color: u32, alpha: f32) {
        if !a.is_finite() || !b.is_finite() || !c.is_finite() || alpha <= 0.0 {
            return;
        }
        let area = (b - a).cross(c - a);
        if area.abs() < 1e-4 {
            return;
        }
        let x0 = a.x.min(b.x).min(c.x).floor() as i32;
        let y0 = a.y.min(b.y).min(c.y).floor() as i32;
        let x1 = a.x.max(b.x).max(c.x).ceil() as i32;
        let y1 = a.y.max(b.y).max(c.y).ceil() as i32;
        for y in y0..=y1 {
            for x in x0..=x1 {
                let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                let w0 = (b - a).cross(p - a) / area;
                let w1 = (c - b).cross(p - b) / area;
                let w2 = (a - c).cross(p - c) / area;
                if w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0 {
                    self.blend(x, y, color, alpha);
                }
            }
        }
    }

    /// Regular polygon fan around `center`.
    pub fn polygon(
        &mut self,
        center: Vec2,
        radius: f32,
        sides: u32,
        rotation: f32,
        color: u32,
        alpha: f32,
    ) {
        if !center.is_finite() || radius <= 0.0 || alpha <= 0.0 {
            return;
        }
        let n = sides.max(3);
        let mut pts = Vec::with_capacity(n as usize);
        for i in 0..n {
            let a = rotation + i as f32 * std::f32::consts::TAU / n as f32;
            pts.push(center + Vec2::from_angle(a) * radius);
        }
        for i in 0..n {
            self.triangle(center, pts[i as usize], pts[((i + 1) % n) as usize], color, alpha);
        }
    }

    pub fn text_width(&self, s: &str, scale: i32) -> i32 {
        let scale = scale.max(1);
        let n = s.chars().count() as i32;
        if n == 0 {
            0
        } else {
            n * font::ADVANCE * scale - scale
        }
    }

    pub fn text(&mut self, s: &str, x: i32, y: i32, scale: i32, color: u32, alpha: f32) {
        if alpha <= 0.0 {
            return;
        }
        let scale = scale.max(1);
        let mut cx = x;
        for ch in s.chars() {
            let g = font::glyph(ch.to_ascii_uppercase());
            for (row, bits) in g.iter().enumerate() {
                for col in 0..font::GLYPH_W {
                    if bits & (1 << (font::GLYPH_W - 1 - col)) != 0 {
                        self.rect_blend(
                            cx + col * scale,
                            y + row as i32 * scale,
                            scale,
                            scale,
                            color,
                            alpha,
                        );
                    }
                }
            }
            cx += font::ADVANCE * scale;
        }
    }

    pub fn text_shadow(
        &mut self,
        s: &str,
        x: i32,
        y: i32,
        scale: i32,
        color: u32,
        alpha: f32,
    ) {
        let off = scale.max(1);
        self.text(s, x + off, y + off, scale, rgb(0, 0, 0), alpha * 0.55);
        self.text(s, x, y, scale, color, alpha);
    }

    pub fn text_center(
        &mut self,
        s: &str,
        cx: i32,
        y: i32,
        scale: i32,
        color: u32,
        alpha: f32,
    ) {
        let w = self.text_width(s, scale);
        self.text(s, cx - w / 2, y, scale, color, alpha);
    }

    pub fn text_center_shadow(
        &mut self,
        s: &str,
        cx: i32,
        y: i32,
        scale: i32,
        color: u32,
        alpha: f32,
    ) {
        let w = self.text_width(s, scale);
        self.text_shadow(s, cx - w / 2, y, scale, color, alpha);
    }

    /// Tint the whole screen.
    pub fn overlay(&mut self, color: u32, alpha: f32) {
        self.rect_blend(0, 0, self.w, self.h, color, alpha);
    }

    /// Radial darkening map (0 at the center, 1 in the corners).
    pub fn vignette(&mut self, tint: u32, strength: f32, map: &[f32]) {
        if strength <= 0.0 {
            return;
        }
        let (tr, tg, tb) = channels(tint);
        for i in 0..self.px.len() {
            let a = strength * map.get(i).copied().unwrap_or(0.0);
            if a <= 0.001 {
                continue;
            }
            let a = a.min(1.0);
            let p = self.px[i];
            let (pr, pg, pb) = channels(p);
            self.px[i] = rgb(
                (pr + (tr - pr) * a) as u8,
                (pg + (tg - pg) * a) as u8,
                (pb + (tb - pb) * a) as u8,
            );
        }
    }
}
