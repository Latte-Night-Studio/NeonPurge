//! Tiny 2D vector math used everywhere in the game.

use std::ops::{Add, AddAssign, Div, Mul, MulAssign, Neg, Sub, SubAssign};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn len(self) -> f32 {
        self.len_sq().sqrt()
    }

    pub fn len_sq(self) -> f32 {
        self.x * self.x + self.y * self.y
    }

    /// Unit vector, or zero for (near) zero-length vectors.
    pub fn norm(self) -> Self {
        let l = self.len();
        if l > 1e-6 {
            self / l
        } else {
            Self::ZERO
        }
    }

    pub fn angle(self) -> f32 {
        self.y.atan2(self.x)
    }

    pub fn dist(self, other: Self) -> f32 {
        (self - other).len()
    }

    pub fn dist_sq(self, other: Self) -> f32 {
        (self - other).len_sq()
    }

    pub fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y
    }

    pub fn cross(self, other: Self) -> f32 {
        self.x * other.y - self.y * other.x
    }

    pub fn perp(self) -> Self {
        Self::new(-self.y, self.x)
    }

    pub fn from_angle(a: f32) -> Self {
        Self::new(a.cos(), a.sin())
    }

    pub fn rotated(self, a: f32) -> Self {
        let (s, c) = a.sin_cos();
        Self::new(self.x * c - self.y * s, self.x * s + self.y * c)
    }

    pub fn lerp(self, other: Self, t: f32) -> Self {
        self + (other - self) * t
    }

    pub fn clamp_len(self, max: f32) -> Self {
        let l = self.len();
        if l > max && l > 1e-6 {
            self * (max / l)
        } else {
            self
        }
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }

    /// Frame-rate independent smoothing towards `target`.
    pub fn damp(self, target: Self, rate: f32, dt: f32) -> Self {
        let t = 1.0 - (-rate * dt).exp();
        self.lerp(target, t.clamp(0.0, 1.0))
    }
}

impl Add for Vec2 {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Self::new(self.x + o.x, self.y + o.y)
    }
}

impl Sub for Vec2 {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y)
    }
}

impl Mul<f32> for Vec2 {
    type Output = Self;
    fn mul(self, s: f32) -> Self {
        Self::new(self.x * s, self.y * s)
    }
}

impl Div<f32> for Vec2 {
    type Output = Self;
    fn div(self, s: f32) -> Self {
        if s.abs() > 1e-6 {
            Self::new(self.x / s, self.y / s)
        } else {
            Self::ZERO
        }
    }
}

impl Neg for Vec2 {
    type Output = Self;
    fn neg(self) -> Self {
        Self::new(-self.x, -self.y)
    }
}

impl AddAssign for Vec2 {
    fn add_assign(&mut self, o: Self) {
        self.x += o.x;
        self.y += o.y;
    }
}

impl SubAssign for Vec2 {
    fn sub_assign(&mut self, o: Self) {
        self.x -= o.x;
        self.y -= o.y;
    }
}

impl MulAssign<f32> for Vec2 {
    fn mul_assign(&mut self, s: f32) {
        self.x *= s;
        self.y *= s;
    }
}
