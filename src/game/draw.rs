//! Rendering: world, lighting, HUD, menus.

use std::f32::consts::{FRAC_PI_4, SQRT_2};

use super::*;
use crate::canvas::{lerp_color, rgb, Canvas};
use crate::math::Vec2;

impl Game {
    pub fn draw(&mut self, c: &mut Canvas) {
        c.copy_from(&self.bg);

        if self.state == State::Title {
            self.draw_title(c);
            return;
        }

        let cam = self.camera_offset();
        self.draw_world(c, cam);
        self.draw_hud(c);
        match self.state {
            State::Paused => self.draw_pause(c),
            State::Dead => self.draw_dead(c),
            _ => {}
        }
        self.draw_crosshair(c);
        self.draw_screen_effects(c);
    }

    fn camera_offset(&mut self) -> Vec2 {
        if self.shake <= 0.05 {
            return Vec2::ZERO;
        }
        let s = self.shake.min(18.0);
        Vec2::new(self.rng.range(-s, s), self.rng.range(-s, s))
    }

    // -- world --------------------------------------------------------------

    fn draw_world(&self, c: &mut Canvas, cam: Vec2) {
        for s in &self.shockwaves {
            let a = (s.life / s.max_life).clamp(0.0, 1.0);
            c.ring(s.pos + cam, s.r, s.width, s.color, a * 0.75);
        }

        for p in &self.pickups {
            let fade = (p.life / 1.2).clamp(0.0, 1.0);
            let bob = (p.phase * 3.0).sin() * 3.0;
            let pos = p.pos + cam + Vec2::new(0.0, bob);
            let (color, label) = match p.kind {
                PickupKind::Health => (C_HEALTH, "+"),
                PickupKind::Ammo => (C_AMMO, "A"),
            };
            c.glow(pos, 30.0, color, 0.55 * fade);
            c.circle(pos, 10.0, rgb(8, 16, 16), 0.9 * fade);
            c.ring(pos, 10.0, 2.0, color, fade);
            if label == "+" {
                c.line(pos + Vec2::new(-4.5, 0.0), pos + Vec2::new(4.5, 0.0), 2.5, color, fade);
                c.line(pos + Vec2::new(0.0, -4.5), pos + Vec2::new(0.0, 4.5), 2.5, color, fade);
            } else {
                c.line(pos + Vec2::new(0.0, -5.0), pos + Vec2::new(0.0, 4.0), 4.0, color, fade);
                c.circle(pos + Vec2::new(0.0, -5.0), 2.0, color, fade);
            }
        }

        for e in &self.enemies {
            self.draw_enemy(c, e, cam);
        }

        self.draw_player(c, cam);

        for b in &self.bullets {
            let pos = b.pos + cam;
            let dir = b.vel.norm();
            let trail = if b.from_player { 20.0 } else { 13.0 };
            c.line(pos - dir * trail, pos, b.r * 2.0, b.color, 0.45);
            c.circle(pos, b.r * 1.9, b.color, 0.4);
            c.circle(pos, b.r, rgb(255, 255, 255), 0.9);
        }

        for p in &self.particles {
            let t = (p.life / p.max_life).clamp(0.0, 1.0);
            let pos = p.pos + cam;
            if p.additive {
                c.glow(pos, p.size * 3.2, p.color, t * 1.5);
            }
            c.circle(pos, p.size * (0.35 + 0.65 * t), p.color, t * 0.85);
        }

        for p in &self.popups {
            let a = (p.life / p.max_life).clamp(0.0, 1.0);
            c.text_center_shadow(
                &p.text,
                (p.pos.x + cam.x) as i32,
                (p.pos.y + cam.y) as i32,
                p.scale,
                p.color,
                a,
            );
        }
    }

    fn draw_enemy(&self, c: &mut Canvas, e: &Enemy, cam: Vec2) {
        let pos = e.pos + cam;
        let grow = if e.spawn > 0.0 {
            1.0 - (e.spawn / 0.5).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let r = e.r * (0.55 + 0.45 * grow);
        let alpha = 0.35 + 0.65 * grow;
        let color = e.kind.color();

        c.glow(pos, r * 2.9, color, 0.28 * alpha);

        match e.kind {
            EnemyKind::Grunt => {
                c.polygon(pos, r, 6, e.phase * 0.7, color, alpha);
                c.polygon(pos, r * 0.58, 6, -e.phase * 1.1, rgb(24, 8, 22), alpha);
                c.circle(pos, r * 0.2, lerp_color(color, C_WHITE, 0.6), alpha);
            }
            EnemyKind::Runner => {
                let aim = Vec2::from_angle(e.aim);
                c.triangle(
                    pos + aim * r * 1.4,
                    pos + aim.rotated(2.4) * r,
                    pos + aim.rotated(-2.4) * r,
                    color,
                    alpha,
                );
                c.triangle(
                    pos + aim * r * 0.85,
                    pos + aim.rotated(2.4) * r * 0.5,
                    pos + aim.rotated(-2.4) * r * 0.5,
                    rgb(34, 14, 6),
                    alpha,
                );
            }
            EnemyKind::Shooter => {
                c.polygon(pos, r, 4, FRAC_PI_4 + e.phase * 0.8, color, alpha);
                c.ring(pos, r * 0.72, 2.0, lerp_color(color, C_WHITE, 0.35), alpha);
                if e.fire_cd > 0.0 && e.fire_cd < 0.45 {
                    let charge = 1.0 - e.fire_cd / 0.45;
                    let tip = pos + Vec2::from_angle(e.aim) * (r + 4.0);
                    c.glow(tip, 12.0 + 18.0 * charge, C_WHITE, 0.9 * charge);
                }
            }
            EnemyKind::Tank => {
                c.polygon(pos, r, 6, e.phase * 0.35, color, alpha);
                c.polygon(pos, r * 0.74, 6, -e.phase * 0.5, rgb(28, 16, 44), alpha);
                c.ring(pos, r * 0.5, 3.0, lerp_color(color, C_WHITE, 0.45), alpha);
                c.circle(pos, r * 0.15, C_WHITE, alpha);
            }
        }

        if e.hit_flash > 0.0 {
            c.circle(pos, r + 2.0, C_WHITE, (e.hit_flash * 5.0).min(0.85));
        }
        if e.spawn > 0.0 {
            c.ring(pos, r + 34.0 * e.spawn, 2.0, color, (e.spawn * 1.6).min(1.0));
        }
        if e.hp < e.max_hp && e.spawn <= 0.0 {
            let bw = (e.r * 2.0) as i32;
            let bx = (pos.x - e.r) as i32;
            let by = (pos.y - e.r - 10.0) as i32;
            c.rect(bx, by, bw, 4, rgb(8, 8, 14));
            let fw = (bw as f32 * (e.hp / e.max_hp).clamp(0.0, 1.0)) as i32;
            c.rect(bx, by, fw, 4, color);
        }
    }

    fn draw_player(&self, c: &mut Canvas, cam: Vec2) {
        let p = &self.player;
        let pos = p.pos + cam;
        let aim = Vec2::from_angle(p.aim);
        let a = if p.iframes > 0.0 { 0.6 } else { 1.0 };

        c.circle(pos + Vec2::new(0.0, 4.0), p.r * 1.05, rgb(0, 0, 0), 0.35);
        c.glow(pos, p.r * 3.5, C_PLAYER, 0.3 * a);

        if p.dash_cd > 0.0 {
            let frac = (1.0 - p.dash_cd / DASH_COOLDOWN).clamp(0.0, 1.0);
            c.ring(pos, p.r + 9.0, 2.0, C_ACCENT, 0.08 + 0.3 * frac);
        } else {
            let pulse = 0.18 + 0.1 * (self.time * 4.0).sin();
            c.ring(pos, p.r + 9.0, 1.5, C_ACCENT, pulse);
        }

        c.line(
            pos + aim * (p.r * 0.2),
            pos + aim * (p.r + 16.0),
            8.0,
            rgb(8, 28, 30),
            a,
        );
        c.line(
            pos + aim * (p.r * 0.2),
            pos + aim * (p.r + 16.0),
            4.0,
            rgb(160, 235, 220),
            a,
        );

        c.circle(pos, p.r + 2.0, rgb(6, 18, 18), 0.9 * a);
        c.circle(pos, p.r, C_PLAYER, a);
        c.circle(pos, p.r * 0.66, C_PLAYER_DARK, a);
        c.triangle(
            pos + aim * (p.r * 0.95),
            pos + aim.rotated(0.9) * (p.r * 0.35),
            pos + aim.rotated(-0.9) * (p.r * 0.35),
            rgb(220, 255, 245),
            a * 0.9,
        );

        if p.muzzle > 0.0 {
            let m = pos + aim * (p.r + 18.0);
            c.glow(m, 26.0, rgb(255, 240, 190), p.muzzle * 7.0);
            c.circle(m, 4.0, rgb(255, 255, 230), (p.muzzle * 14.0).min(1.0));
        }
        if p.hurt_flash > 0.0 {
            c.circle(pos, p.r + 6.0, rgb(255, 60, 80), p.hurt_flash * 0.5);
        }
    }

    // -- HUD ----------------------------------------------------------------

    fn draw_hud(&self, c: &mut Canvas) {
        c.text_shadow(
            &format!("WAVE {:02}", self.wave),
            48,
            40,
            3,
            C_WHITE,
            1.0,
        );
        c.text_shadow(
            &format!("SCORE {:06}", self.score),
            48,
            78,
            2,
            C_ACCENT,
            0.9,
        );
        if self.combo >= 2 {
            let mult = self.multiplier();
            let col = lerp_color(
                rgb(255, 210, 90),
                rgb(255, 80, 120),
                ((mult - 1.0) / 4.0).clamp(0.0, 1.0),
            );
            c.text_shadow(&format!("COMBO X{:.1}", mult), 48, 108, 2, col, 0.95);
            c.rect(48, 132, 110, 4, rgb(30, 34, 44));
            let frac = (self.combo_t / 2.5).clamp(0.0, 1.0);
            c.rect(48, 132, (110.0 * frac) as i32, 4, col);
        }

        let right = format!("BEST {:06}", self.high_score.max(self.score));
        let rw = c.text_width(&right, 2);
        c.text_shadow(&right, W - 48 - rw, 40, 2, C_DIM, 0.9);
        let kills = format!("KILLS {}", self.kills);
        let kw = c.text_width(&kills, 2);
        c.text_shadow(&kills, W - 48 - kw, 74, 2, C_DIM, 0.75);

        if self.banner_t > 0.0 && !self.banner.is_empty() {
            let a = (self.banner_t * 2.0).min(1.0);
            c.text_center_shadow(&self.banner, W / 2, 142, 5, C_WHITE, a);
        }

        if self.run_time < 14.0 {
            let a = (1.0 - self.run_time / 14.0).clamp(0.0, 1.0) * 0.75;
            c.text_center(
                "WASD MOVE   LMB FIRE   SHIFT DASH   R RELOAD   Q WEAPON   P PAUSE",
                W / 2,
                196,
                2,
                rgb(140, 165, 190),
                a,
            );
        }

        // Health.
        let hp_frac = (self.player.hp / self.player.max_hp).clamp(0.0, 1.0);
        let hp_col = if hp_frac > 0.55 {
            rgb(80, 240, 170)
        } else if hp_frac > 0.28 {
            rgb(255, 200, 70)
        } else {
            rgb(255, 70, 90)
        };
        let bx = 48;
        let by = H - 92;
        let bw = 300;
        let bh = 18;
        c.rect(bx - 3, by - 3, bw + 6, bh + 6, rgb(10, 14, 20));
        c.rect(bx, by, bw, bh, rgb(22, 26, 34));
        c.rect(bx, by, (bw as f32 * hp_frac) as i32, bh, hp_col);
        c.rect_blend(bx, by, (bw as f32 * hp_frac) as i32, 5, rgb(255, 255, 255), 0.22);
        c.rect_outline(bx - 3, by - 3, bw + 6, bh + 6, 1, rgb(70, 90, 110), 0.6);
        c.text_shadow(
            &format!("HP {:.0}/{:.0}", self.player.hp.max(0.0), self.player.max_hp),
            bx + bw + 16,
            by + 2,
            2,
            hp_col,
            0.95,
        );

        // Dash charge.
        let dy = H - 58;
        let dash_frac = (1.0 - self.player.dash_cd / DASH_COOLDOWN).clamp(0.0, 1.0);
        c.rect(bx, dy, 150, 7, rgb(18, 22, 30));
        let dash_col = if dash_frac >= 1.0 {
            C_ACCENT
        } else {
            rgb(50, 90, 120)
        };
        c.rect(bx, dy, (150.0 * dash_frac) as i32, 7, dash_col);
        c.text("DASH", bx + 160, dy - 2, 1, C_DIM, 0.8);

        // Weapon panel.
        let w = self.player.weapon;
        let def = WEAPONS[w];
        let name = def.name.to_string();
        let nw = c.text_width(&name, 2);
        c.text_shadow(&name, W - 48 - nw, H - 152, 2, C_WHITE, 0.85);

        let reloading = self.player.reload[w] > 0.0;
        let ammo_text = if reloading {
            "RELOADING".to_string()
        } else {
            format!("{} / {}", self.player.ammo[w], def.mag)
        };
        let ammo_col = if reloading {
            rgb(255, 190, 80)
        } else if self.player.ammo[w] == 0 {
            rgb(255, 80, 100)
        } else {
            C_WHITE
        };
        let aw = c.text_width(&ammo_text, 4);
        c.text_shadow(&ammo_text, W - 48 - aw, H - 128, 4, ammo_col, 1.0);

        c.rect(W - 268, H - 82, 220, 6, rgb(18, 22, 30));
        if reloading {
            let frac = (1.0 - self.player.reload[w] / def.reload_time).clamp(0.0, 1.0);
            c.rect(W - 268, H - 82, (220.0 * frac) as i32, 6, rgb(255, 190, 80));
        } else {
            let frac = self.player.ammo[w] as f32 / def.mag as f32;
            c.rect(
                W - 268,
                H - 82,
                (220.0 * frac) as i32,
                6,
                lerp_color(rgb(255, 90, 110), C_ACCENT, frac),
            );
        }

        // Weapon slots.
        for (i, slot) in WEAPONS.iter().enumerate() {
            let sx = W - 48 - 2 * 34 + i as i32 * 34;
            let sy = H - 58;
            let active = i == w;
            c.rect(sx, sy, 28, 16, if active { rgb(30, 46, 58) } else { rgb(16, 20, 28) });
            if active {
                c.rect_outline(sx, sy, 28, 16, 1, def.color, 0.9);
            }
            let label = format!("{}", slot.mag);
            let lw = c.text_width(&label, 1);
            c.text(
                &label,
                sx + (28 - lw) / 2,
                sy + 5,
                1,
                if active { C_WHITE } else { rgb(90, 100, 115) },
                0.9,
            );
        }
    }

    fn draw_crosshair(&self, c: &mut Canvas) {
        if self.state != State::Playing {
            return;
        }
        let m = self.mouse;
        if !m.is_finite() {
            return;
        }
        let kick = (self.player.muzzle * 90.0).min(10.0);
        let gap = 5.0 + kick;
        let len = 9.0;
        let color = rgb(210, 255, 245);
        for dir in [
            Vec2::new(1.0, 0.0),
            Vec2::new(-1.0, 0.0),
            Vec2::new(0.0, 1.0),
            Vec2::new(0.0, -1.0),
        ] {
            c.line(m + dir * gap, m + dir * (gap + len), 2.0, color, 0.85);
        }
        c.circle(m, 1.7, color, 1.0);
    }

    // -- screens ------------------------------------------------------------

    fn draw_title(&self, c: &mut Canvas) {
        let t = self.time;
        let center = Vec2::new(W as f32 * 0.5, H as f32 * 0.5);

        for i in 0..6 {
            let r = 150.0 + i as f32 * 88.0 + (t * 0.6 + i as f32).sin() * 10.0;
            c.ring(
                center + Vec2::new(0.0, -40.0),
                r,
                2.0,
                C_ACCENT,
                0.04 + 0.012 * i as f32,
            );
        }

        for i in 0..70 {
            let seed = i as f32 * 37.7;
            let x = ((seed * 13.0 + t * (10.0 + (i % 7) as f32 * 3.0)) % (W as f32 + 120.0)) - 60.0;
            let y = ((seed * 7.3 + t * (6.0 + (i % 5) as f32 * 4.0)) % (H as f32 + 120.0)) - 60.0;
            let col = if i % 2 == 0 { C_PLAYER } else { C_GRUNT };
            c.circle(Vec2::new(x, y), 1.6, col, 0.22);
        }

        c.polygon(
            center + Vec2::new(-450.0, 20.0),
            64.0,
            6,
            t * 0.3,
            C_GRUNT,
            0.10,
        );
        c.polygon(
            center + Vec2::new(452.0, 10.0),
            46.0,
            3,
            -t * 0.4,
            C_SHOOTER,
            0.12,
        );

        let title = "NEON PURGE";
        let scale = 12;
        let tw = c.text_width(title, scale);
        let tx = (W - tw) / 2;
        let ty = 150;
        c.text(title, tx - 5, ty, scale, rgb(255, 0, 110), 0.55);
        c.text(title, tx + 5, ty, scale, rgb(0, 255, 230), 0.55);
        c.text_shadow(title, tx, ty, scale, C_WHITE, 1.0);

        c.text_center("ARENA SHOOTER", W / 2, ty + 112, 2, C_DIM, 0.95);
        c.rect(W / 2 - 260, 302, 520, 1, rgb(40, 60, 90));

        let pulse = 0.55 + 0.45 * (t * 3.2).sin();
        c.text_center_shadow("PRESS ENTER TO DEPLOY", W / 2, 338, 4, C_PLAYER, pulse);

        let lines = [
            "WASD / ARROWS   MOVE",
            "MOUSE           AIM",
            "LEFT MOUSE      FIRE",
            "SHIFT / SPACE   DASH",
            "R               RELOAD",
            "Q               SWAP WEAPON",
            "P / ESC         PAUSE",
        ];
        let start_y = 414;
        for (i, line) in lines.iter().enumerate() {
            c.text_center(line, W / 2, start_y + i as i32 * 24, 2, rgb(150, 175, 200), 0.8);
        }

        let best = format!("BEST {:06}", self.high_score);
        let bw = c.text_width(&best, 2);
        c.text_shadow(&best, W - 48 - bw, 40, 2, C_DIM, 0.85);
        c.text_center("ESC QUITS", W / 2, H - 46, 1, rgb(85, 100, 120), 0.65);
    }

    fn draw_pause(&self, c: &mut Canvas) {
        c.overlay(rgb(4, 6, 14), 0.72);
        c.text_center_shadow("PAUSED", W / 2, 230, 8, C_WHITE, 1.0);
        c.text_center("P / ESC    RESUME", W / 2, 370, 3, rgb(170, 200, 220), 0.9);
        c.text_center("R          RESTART", W / 2, 424, 3, rgb(170, 200, 220), 0.9);
        c.text_center("ENTER      MAIN MENU", W / 2, 478, 2, C_DIM, 0.85);
    }

    fn draw_dead(&self, c: &mut Canvas) {
        let a = (self.death_t * 2.5).min(0.7);
        c.overlay(rgb(30, 0, 10), a);
        c.glow(Vec2::new(W as f32 * 0.5, 250.0), 360.0, rgb(255, 40, 80), 0.22);

        let pulse = 0.6 + 0.4 * (self.time * 4.0).sin();
        c.text_center_shadow("YOU DIED", W / 2, 200, 10, rgb(255, 80, 100), 1.0);
        c.text_center(&format!("SCORE  {:06}", self.score), W / 2, 336, 3, C_WHITE, 0.95);
        c.text_center(
            &format!("WAVE  {}     KILLS  {}", self.wave, self.kills),
            W / 2,
            380,
            2,
            rgb(205, 160, 175),
            0.9,
        );

        if self.new_record {
            c.text_center("NEW HIGH SCORE", W / 2, 424, 3, rgb(255, 220, 90), pulse);
        } else {
            c.text_center(&format!("BEST  {:06}", self.high_score), W / 2, 424, 2, C_DIM, 0.85);
        }

        c.text_center("R / ENTER   RETRY", W / 2, 502, 3, C_PLAYER, pulse);
        c.text_center("ESC         MAIN MENU", W / 2, 552, 2, rgb(130, 150, 170), 0.8);
    }

    fn draw_screen_effects(&self, c: &mut Canvas) {
        if self.flash > 0.01 {
            c.overlay(rgb(255, 255, 255), (self.flash * 0.5).min(0.65));
        }
        if self.damage_tint > 0.01 {
            c.overlay(rgb(255, 30, 40), (self.damage_tint * 0.16).min(0.3));
            c.vignette(rgb(255, 20, 40), (self.damage_tint * 0.5).min(1.0), &self.vig);
        }
        if self.state == State::Playing && self.player.hp / self.player.max_hp < 0.3 {
            let low = 1.0 - (self.player.hp / self.player.max_hp / 0.3);
            let pulse = 0.5 + 0.5 * (self.time * 6.0).sin();
            let strength = low.clamp(0.0, 1.0) * 0.35 * (0.55 + 0.45 * pulse);
            c.vignette(rgb(255, 20, 40), strength, &self.vig);
        }
    }
}

// ---------------------------------------------------------------------------
// Static imagery, built once at startup.
// ---------------------------------------------------------------------------

pub(super) fn build_background() -> Vec<u32> {
    let mut c = Canvas::new(W, H);
    let cx = W as f32 * 0.5;
    let cy = H as f32 * 0.5;

    for y in 0..H {
        for x in 0..W {
            let dx = (x as f32 - cx) / cx;
            let dy = (y as f32 - cy) / cy;
            let d = (dx * dx + dy * dy).sqrt().min(1.0);
            let col = lerp_color(rgb(15, 18, 32), rgb(5, 6, 12), d);
            c.px[(y * W + x) as usize] = col;
        }
    }

    c.glow(Vec2::new(cx, cy), 540.0, rgb(18, 36, 82), 0.12);

    let m = ARENA_MARGIN as i32;
    let mut x = m;
    while x <= W - m {
        c.rect_blend(x, m, 1, H - 2 * m, rgb(32, 48, 76), 0.32);
        x += 64;
    }
    let mut y = m;
    while y <= H - m {
        c.rect_blend(m, y, W - 2 * m, 1, rgb(32, 48, 76), 0.32);
        y += 64;
    }

    let mut gx = m;
    while gx <= W - m {
        let mut gy = m;
        while gy <= H - m {
            c.circle(Vec2::new(gx as f32, gy as f32), 1.3, rgb(55, 85, 125), 0.4);
            gy += 64;
        }
        gx += 64;
    }

    c.rect_outline(m, m, W - 2 * m, H - 2 * m, 2, rgb(60, 90, 130), 0.55);
    c.rect_outline(
        m - 4,
        m - 4,
        W - 2 * m + 8,
        H - 2 * m + 8,
        1,
        rgb(30, 50, 80),
        0.5,
    );

    let bl = 48;
    let t = 4;
    let accent = rgb(90, 220, 255);
    c.rect_blend(m - 2, m - 2, bl, t, accent, 0.55);
    c.rect_blend(m - 2, m - 2, t, bl, accent, 0.55);
    c.rect_blend(W - m - bl + 2, m - 2, bl, t, accent, 0.55);
    c.rect_blend(W - m - 2, m - 2, t, bl, accent, 0.55);
    c.rect_blend(m - 2, H - m - 2, bl, t, accent, 0.55);
    c.rect_blend(m - 2, H - m - bl + 2, t, bl, accent, 0.55);
    c.rect_blend(W - m - bl + 2, H - m - 2, bl, t, accent, 0.55);
    c.rect_blend(W - m - 2, H - m - bl + 2, t, bl, accent, 0.55);

    let mut sy = 0;
    while sy < H {
        c.rect_blend(0, sy, W, 1, rgb(0, 0, 0), 0.08);
        sy += 3;
    }

    c.px
}

pub(super) fn build_vignette(w: i32, h: i32) -> Vec<f32> {
    let mut map = vec![0.0f32; (w * h) as usize];
    let cx = w as f32 * 0.5;
    let cy = h as f32 * 0.5;
    for y in 0..h {
        for x in 0..w {
            let dx = (x as f32 - cx) / cx;
            let dy = (y as f32 - cy) / cy;
            let d = (dx * dx + dy * dy).sqrt() / SQRT_2;
            map[(y * w + x) as usize] = (d * d * 1.7).min(1.0);
        }
    }
    map
}
