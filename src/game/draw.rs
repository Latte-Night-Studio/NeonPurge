//! Rendering: world, lighting, HUD, menus.

use std::f32::consts::{FRAC_PI_4, SQRT_2};

use super::*;
use crate::canvas::{lerp_color, rgb, Canvas};
use crate::lang;
use crate::math::Vec2;
use crate::ui::{self, Action, Rect};

impl Game {
    pub fn draw(&mut self, c: &mut Canvas) {
        c.copy_from(&self.bg);

        if self.state == State::Title {
            self.draw_title(c);
            if self.page == MenuPage::Settings {
                self.draw_settings(c);
            }
            return;
        }

        let cam = self.camera_offset();
        self.draw_world(c, cam);
        self.draw_hud(c);
        match self.state {
            State::Paused if self.page == MenuPage::Main => self.draw_pause(c),
            State::Dead if self.page == MenuPage::Main => self.draw_dead(c),
            _ => {}
        }
        if self.page == MenuPage::Settings {
            self.draw_settings(c);
        } else if self.state == State::Playing {
            // In the arena the crosshair is the pointer; menus use the real
            // cursor instead.
            self.draw_crosshair(c);
        }
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

        for p in &self.projectiles {
            let pos = p.pos + cam;
            let dir = p.vel.norm();
            match p.kind {
                ProjectileKind::Rocket => {
                    c.line(pos - dir * 18.0, pos, p.r * 2.4, p.color, 0.6);
                    c.glow(pos, 26.0, p.color, 0.85);
                    c.circle(pos, p.r, p.color, 1.0);
                    c.circle(pos, p.r * 0.45, C_WHITE, 1.0);
                }
                ProjectileKind::Grenade => {
                    let fuse = (p.life / p.max_life).clamp(0.0, 1.0);
                    let blink_rate = 7.0 + (1.0 - fuse) * 30.0;
                    c.glow(pos, 18.0, p.color, 0.45);
                    c.circle(pos, p.r, rgb(10, 20, 14), 0.95);
                    c.ring(pos, p.r, 2.0, p.color, 0.95);
                    let blink = if (p.life * blink_rate).sin() > 0.0 {
                        1.0
                    } else {
                        0.35
                    };
                    c.circle(pos, p.r * 0.48, p.color, blink);
                    c.line(
                        pos + Vec2::from_angle(p.spin) * (p.r * 0.4),
                        pos + Vec2::from_angle(p.spin) * (p.r * 1.9),
                        1.5,
                        p.color,
                        0.7,
                    );
                }
            }
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

        if e.spawn <= 0.0 && !e.alerted {
            // Faint vision cone so the player can read who has noticed them.
            let spread = 0.6;
            let reach = 110.0;
            let a = 0.05 + e.awareness * 0.22;
            let look = Vec2::from_angle(e.aim);
            let p1 = pos + Vec2::from_angle(e.aim - spread) * (r + reach);
            let p2 = pos + Vec2::from_angle(e.aim + spread) * (r + reach);
            c.triangle(pos, p1, p2, color, a * 0.5);
            c.line(pos + look * (r + 3.0), p1, 1.0, color, a);
            c.line(pos + look * (r + 3.0), p2, 1.0, color, a);
            if e.awareness > 0.04 {
                c.ring(
                    pos,
                    r + 6.0 + (1.0 - e.awareness) * 10.0,
                    1.5,
                    C_AMMO,
                    e.awareness * 0.55,
                );
            }
        }

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
        if e.alerted && e.spawn <= 0.0 {
            let marker = pos + Vec2::new(0.0, -r - 20.0);
            c.line(
                marker,
                marker + Vec2::new(0.0, 5.5),
                2.0,
                C_GRUNT,
                0.85,
            );
            c.circle(marker + Vec2::new(0.0, 9.0), 1.6, C_GRUNT, 0.85);
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

        let def = WEAPONS[p.weapon];
        if p.slash > 0.0 && def.kind == WeaponKind::Melee {
            let frac = (p.slash / 0.18).clamp(0.0, 1.0);
            let radius = (p.r + def.melee_range * 0.85) * (0.62 + 0.38 * frac);
            let arc = def.melee_arc;
            let steps = 10;
            let mut prev = pos + Vec2::from_angle(p.slash_angle - arc) * (p.r + 4.0);
            for i in 1..=steps {
                let a = p.slash_angle - arc
                    + 2.0 * arc * i as f32 / steps as f32;
                let point = pos + Vec2::from_angle(a) * radius;
                c.line(prev, point, 4.0, rgb(8, 28, 30), frac * 0.7);
                c.line(prev, point, 2.0, def.color, frac);
                prev = point;
            }
            let tip = pos + Vec2::from_angle(p.slash_angle) * radius;
            c.glow(tip, 20.0, def.color, frac * 0.5);
        }
        if p.hurt_flash > 0.0 {
            c.circle(pos, p.r + 6.0, rgb(255, 60, 80), p.hurt_flash * 0.5);
        }
    }

    // -- HUD ----------------------------------------------------------------

    fn draw_hud(&self, c: &mut Canvas) {
        let text = lang::text(self.settings.lang);
        c.text_shadow(
            &format!("{} {:02}", text.wave, self.wave),
            48,
            40,
            3,
            C_WHITE,
            1.0,
        );
        c.text_shadow(
            &format!("{} {:06}", text.score, self.score),
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
            c.text_shadow(
                &format!("{} X{:.1}", text.combo, mult),
                48,
                108,
                2,
                col,
                0.95,
            );
            c.rect(48, 132, 110, 4, rgb(30, 34, 44));
            let frac = (self.combo_t / 2.5).clamp(0.0, 1.0);
            c.rect(48, 132, (110.0 * frac) as i32, 4, col);
        }

        let right = format!("{} {:06}", text.best, self.high_score.max(self.score));
        let rw = c.text_width(&right, 2);
        c.text_shadow(&right, W - 48 - rw, 40, 2, C_DIM, 0.9);
        let kills = format!("{} {}", text.kills, self.kills);
        let kw = c.text_width(&kills, 2);
        c.text_shadow(&kills, W - 48 - kw, 74, 2, C_DIM, 0.75);

        if self.sound_muted {
            let off = text.sound_off;
            let ow = c.text_width(off, 1);
            c.text_shadow(off, W - 48 - ow, 106, 1, rgb(255, 160, 90), 0.8);
        }

        if self.banner_t > 0.0 && !self.banner.is_empty() {
            let a = (self.banner_t * 2.0).min(1.0);
            c.text_center_shadow(&self.banner, W / 2, 142, 5, C_WHITE, a);
        }

        if self.run_time < 14.0 {
            let a = (1.0 - self.run_time / 14.0).clamp(0.0, 1.0) * 0.75;
            let hint = if cfg!(target_os = "android") {
                text.hint_touch
            } else {
                text.hint
            };
            c.text_center(hint, W / 2, 196, 2, rgb(140, 165, 190), a);
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
            &format!(
                "{} {:.0}/{:.0}",
                text.hp,
                self.player.hp.max(0.0),
                self.player.max_hp
            ),
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
        c.text(text.dash, bx + 160, dy - 2, 1, C_DIM, 0.8);

        // Weapon panel.
        let w = self.player.weapon;
        let def = WEAPONS[w];
        let name = text.weapons[w];
        let role = text.roles[w];
        let nw = c.text_width(name, 2);
        c.text_shadow(name, W - 48 - nw, H - 178, 2, C_WHITE, 0.9);
        let rw = c.text_width(role, 1);
        c.text(role, W - 48 - rw, H - 160, 1, def.color, 0.9);

        let melee = def.kind == WeaponKind::Melee;
        let reloading = self.player.reload[w] > 0.0;
        let ammo_text = if melee {
            text.ready.to_string()
        } else if reloading {
            text.reloading.to_string()
        } else {
            format!("{} / {}", self.player.ammo[w], def.mag)
        };
        let ammo_col = if melee {
            def.color
        } else if reloading {
            rgb(255, 190, 80)
        } else if self.player.ammo[w] == 0 {
            rgb(255, 80, 100)
        } else {
            C_WHITE
        };
        let aw = c.text_width(&ammo_text, 4);
        c.text_shadow(&ammo_text, W - 48 - aw, H - 136, 4, ammo_col, 1.0);

        c.rect(W - 268, H - 96, 220, 6, rgb(18, 22, 30));
        if melee {
            let pulse = 0.55 + 0.45 * (self.time * 3.0).sin();
            c.rect(W - 268, H - 96, 220, 6, lerp_color(rgb(20, 40, 40), def.color, pulse));
        } else if reloading {
            let frac = (1.0 - self.player.reload[w] / def.reload_time).clamp(0.0, 1.0);
            c.rect(W - 268, H - 96, (220.0 * frac) as i32, 6, rgb(255, 190, 80));
        } else {
            let frac = self.player.ammo[w] as f32 / def.mag.max(1) as f32;
            c.rect(
                W - 268,
                H - 96,
                (220.0 * frac) as i32,
                6,
                lerp_color(rgb(255, 90, 110), C_ACCENT, frac),
            );
        }

        // Weapon slots, one per category. The number is the selection key.
        let slot_w = 26;
        let slot_gap = 4;
        let total_w = WEAPON_COUNT as i32 * (slot_w + slot_gap) - slot_gap;
        let sx0 = W - 48 - total_w;
        let sy = H - 70;
        for (i, slot) in WEAPONS.iter().enumerate() {
            let sx = sx0 + i as i32 * (slot_w + slot_gap);
            let active = i == w;
            c.rect(
                sx,
                sy,
                slot_w,
                18,
                if active { rgb(30, 46, 58) } else { rgb(16, 20, 28) },
            );
            if active {
                c.rect_outline(sx, sy, slot_w, 18, 1, slot.color, 0.95);
            }
            c.rect_blend(
                sx + 3,
                sy + 14,
                slot_w - 6,
                2,
                slot.color,
                if active { 0.9 } else { 0.35 },
            );
            let label = format!("{}", i + 1);
            let lw = c.text_width(&label, 1);
            c.text(
                &label,
                sx + (slot_w - lw) / 2,
                sy + 4,
                1,
                if active { C_WHITE } else { rgb(100, 112, 130) },
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

    /// One menu button, shared by every screen. `hovered` brightens it,
    /// `pressed` sinks it a couple of pixels and `selected` marks the active
    /// value in the settings page.
    fn draw_button(
        &self,
        c: &mut Canvas,
        rect: Rect,
        label: &str,
        scale: i32,
        hovered: bool,
        pressed: bool,
        selected: bool,
    ) {
        let sink = if pressed { 2 } else { 0 };
        let x = rect.x;
        let y = rect.y + sink;

        if hovered && !pressed {
            c.glow(
                Vec2::new((x + rect.w / 2) as f32, (y + rect.h / 2) as f32),
                rect.w as f32 * 0.6,
                C_ACCENT,
                0.10,
            );
        }

        let base = if selected {
            rgb(16, 52, 48)
        } else if pressed {
            rgb(28, 40, 58)
        } else if hovered {
            rgb(22, 34, 52)
        } else {
            rgb(11, 16, 27)
        };
        c.rect_blend(x, y, rect.w, rect.h, rgb(0, 0, 0), 0.35);
        c.rect_blend(x, y, rect.w, rect.h, base, 0.94);

        let edge = if selected {
            C_PLAYER
        } else if hovered {
            C_ACCENT
        } else {
            rgb(42, 62, 92)
        };
        let edge_alpha = if selected || hovered { 1.0 } else { 0.7 };
        c.rect_outline(x, y, rect.w, rect.h, 2, edge, edge_alpha);
        if selected {
            c.rect(x, y + 7, 4, rect.h - 14, C_PLAYER);
        }
        if hovered {
            c.rect_blend(x + 2, y + 2, rect.w - 4, 1, C_WHITE, 0.18);
        }

        let color = if selected || hovered { C_WHITE } else { rgb(176, 198, 220) };
        let tw = c.text_width(label, scale);
        let th = 7 * scale;
        c.text_shadow(
            label,
            x + (rect.w - tw) / 2,
            y + (rect.h - th) / 2,
            scale,
            color,
            1.0,
        );

        if pressed {
            c.rect_outline(x, y, rect.w, rect.h, 1, rgb(0, 0, 0), 0.35);
        }
    }

    fn hovered_pressed(&self, rect: Rect) -> (bool, bool) {
        let hovered = rect.contains(self.mouse);
        (hovered, hovered && self.mouse_down)
    }

    fn draw_title(&self, c: &mut Canvas) {
        let text = lang::text(self.settings.lang);
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
        let scale = 10;
        let tw = c.text_width(title, scale);
        let tx = (W - tw) / 2;
        let ty = 70;
        c.text(title, tx - 5, ty, scale, rgb(255, 0, 110), 0.55);
        c.text(title, tx + 5, ty, scale, rgb(0, 255, 230), 0.55);
        c.text_shadow(title, tx, ty, scale, C_WHITE, 1.0);

        c.text_center(text.subtitle, W / 2, 156, 2, C_DIM, 0.95);
        c.rect(W / 2 - 320, 188, 640, 1, rgb(40, 60, 90));

        // Controls, left column. The touch build swaps the keyboard list for
        // its on-screen stick and button layout.
        let controls = if cfg!(target_os = "android") {
            &text.controls_touch
        } else {
            &text.controls
        };
        let controls_y = 252;
        for (i, (keys, action)) in controls.iter().enumerate() {
            let y = controls_y + i as i32 * 34;
            c.text(keys, 110, y, 2, C_ACCENT, 0.85);
            let w = c.text_width(keys, 2);
            c.text(action, 110 + w + 28, y, 2, rgb(170, 192, 215), 0.9);
        }

        // Buttons, right column.
        let buttons = ui::title_buttons();
        for (rect, action) in buttons.iter() {
            let (hovered, pressed) = self.hovered_pressed(*rect);
            let label = match action {
                Action::Start => text.play,
                Action::OpenSettings => text.settings,
                Action::Quit => text.quit,
                _ => "",
            };
            self.draw_button(c, *rect, label, 3, hovered, pressed, false);
        }

        let hint_y = buttons[2].0.y + buttons[2].0.h + 34;
        let start_hint = if cfg!(target_os = "android") {
            text.start_hint_touch
        } else {
            text.start_hint
        };
        c.text_center(start_hint, buttons[0].0.cx(), hint_y, 2, rgb(110, 130, 155), 0.8);

        let best = format!("{} {:06}", text.best, self.high_score);
        let bw = c.text_width(&best, 2);
        c.text_shadow(&best, W - 48 - bw, 40, 2, C_DIM, 0.85);
        let footer = if cfg!(target_os = "android") {
            text.footer_touch
        } else {
            text.esc_hint
        };
        c.text_center(footer, W / 2, H - 46, 1, rgb(85, 100, 120), 0.65);
    }

    fn draw_pause(&self, c: &mut Canvas) {
        let text = lang::text(self.settings.lang);
        c.overlay(rgb(4, 6, 14), 0.72);
        c.text_center_shadow(text.paused, W / 2, 190, 7, C_WHITE, 1.0);

        let buttons = ui::pause_buttons();
        for (rect, action) in buttons.iter() {
            let (hovered, pressed) = self.hovered_pressed(*rect);
            let label = match action {
                Action::Resume => text.resume,
                Action::OpenSettings => text.settings,
                Action::Restart => text.restart,
                Action::MainMenu => text.main_menu,
                _ => "",
            };
            self.draw_button(c, *rect, label, 3, hovered, pressed, false);
        }
        let resume_hint = if cfg!(target_os = "android") {
            text.resume_hint_touch
        } else {
            text.esc_resume
        };
        c.text_center(
            resume_hint,
            W / 2,
            buttons[3].0.y + buttons[3].0.h + 26,
            1,
            rgb(120, 140, 165),
            0.8,
        );
    }

    fn draw_dead(&self, c: &mut Canvas) {
        let text = lang::text(self.settings.lang);
        let a = (self.death_t * 2.5).min(0.7);
        c.overlay(rgb(30, 0, 10), a);
        c.glow(Vec2::new(W as f32 * 0.5, 250.0), 360.0, rgb(255, 40, 80), 0.22);

        let pulse = 0.6 + 0.4 * (self.time * 4.0).sin();
        c.text_center_shadow(text.died, W / 2, 165, 8, rgb(255, 80, 100), 1.0);
        c.text_center(
            &format!("{}  {:06}", text.score, self.score),
            W / 2,
            268,
            3,
            C_WHITE,
            0.95,
        );
        c.text_center(
            &format!("{}  {}     {}  {}", text.wave, self.wave, text.kills, self.kills),
            W / 2,
            312,
            2,
            rgb(205, 160, 175),
            0.9,
        );

        if self.new_record {
            c.text_center(text.new_record, W / 2, 350, 3, rgb(255, 220, 90), pulse);
        } else {
            c.text_center(
                &format!("{}  {:06}", text.best, self.high_score),
                W / 2,
                350,
                2,
                C_DIM,
                0.85,
            );
        }

        let buttons = ui::dead_buttons();
        for (rect, action) in buttons.iter() {
            let (hovered, pressed) = self.hovered_pressed(*rect);
            let label = match action {
                Action::Restart => text.retry,
                Action::OpenSettings => text.settings,
                Action::MainMenu => text.main_menu,
                _ => "",
            };
            self.draw_button(c, *rect, label, 3, hovered, pressed, false);
        }
        let menu_hint = if cfg!(target_os = "android") {
            text.menu_hint_touch
        } else {
            text.esc_menu
        };
        c.text_center(
            menu_hint,
            W / 2,
            buttons[2].0.y + buttons[2].0.h + 25,
            1,
            rgb(130, 150, 170),
            0.8,
        );
    }

    fn draw_settings(&self, c: &mut Canvas) {
        let text = lang::text(self.settings.lang);
        c.overlay(rgb(3, 5, 12), 0.78);

        let panel = Rect::new(140, 70, 1000, 580);
        c.rect_blend(panel.x, panel.y, panel.w, panel.h, rgb(8, 12, 22), 0.94);
        c.rect_outline(panel.x, panel.y, panel.w, panel.h, 2, rgb(45, 70, 105), 0.9);
        c.rect(panel.x, panel.y, panel.w, 3, C_ACCENT);
        c.text_center_shadow(text.settings, W / 2, panel.y + 34, 5, C_WHITE, 1.0);

        // Display mode.
        if !cfg!(target_os = "android") {
            c.text_center(text.display_mode, W / 2, 140, 2, C_DIM, 0.95);
            for (rect, action) in ui::settings_display_buttons().iter() {
                let (hovered, pressed) = self.hovered_pressed(*rect);
                let (label, selected) = match action {
                    Action::Display(mode) => {
                        let label = match mode {
                            DisplayMode::Windowed => text.windowed,
                            DisplayMode::Borderless => text.borderless,
                            DisplayMode::Fullscreen => text.fullscreen,
                        };
                        (label, *mode == self.settings.display)
                    }
                    _ => ("", false),
                };
                self.draw_button(c, *rect, label, 2, hovered, pressed, selected);
            }

            // Resolution.
            c.text_center(text.resolution, W / 2, 252, 2, C_DIM, 0.95);
            for (rect, action) in ui::settings_resolution_buttons().iter() {
                let (hovered, pressed) = self.hovered_pressed(*rect);
                let (label, selected) = match action {
                    Action::Resolution(resolution) => {
                        (resolution.label(), *resolution == self.settings.resolution)
                    }
                    _ => (String::new(), false),
                };
                self.draw_button(c, *rect, &label, 2, hovered, pressed, selected);
            }
            c.text_center(text.resolution_note, W / 2, 338, 1, rgb(100, 120, 145), 0.8);
        }

        // Language.
        let lang_y = if cfg!(target_os = "android") { 258 } else { 358 };
        c.text_center(text.language, W / 2, lang_y, 2, C_DIM, 0.95);
        for (rect, action) in ui::settings_language_buttons().iter() {
            let (hovered, pressed) = self.hovered_pressed(*rect);
            let (label, selected) = match action {
                Action::Language(lang) => {
                    let label = match lang {
                        Lang::Uk => text.lang_uk,
                        Lang::En => text.lang_en,
                    };
                    (label, *lang == self.settings.lang)
                }
                _ => ("", false),
            };
            self.draw_button(c, *rect, label, 2, hovered, pressed, selected);
        }

        // Sound.
        let sound_y = if cfg!(target_os = "android") { 392 } else { 466 };
        c.text_center(text.sound, W / 2, sound_y, 2, C_DIM, 0.95);
        for (rect, action) in ui::settings_sound_buttons().iter() {
            let (hovered, pressed) = self.hovered_pressed(*rect);
            let (label, selected) = match action {
                Action::Sound(on) => {
                    let label = if *on { text.on } else { text.off };
                    (label, *on == self.settings.sound)
                }
                _ => ("", false),
            };
            self.draw_button(c, *rect, label, 2, hovered, pressed, selected);
        }

        let (back, _) = ui::settings_back_button();
        let (hovered, pressed) = self.hovered_pressed(back);
        self.draw_button(c, back, text.back, 3, hovered, pressed, false);
        let back_hint = if cfg!(target_os = "android") {
            text.back_hint_touch
        } else {
            text.esc_back
        };
        c.text_center(
            back_hint,
            W / 2,
            back.y + back.h + 14,
            1,
            rgb(100, 120, 145),
            0.75,
        );
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

