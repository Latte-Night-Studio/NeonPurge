//! Per-frame simulation systems: enemies, bullets, pickups, waves, damage.

use super::*;
use crate::canvas::rgb;
use crate::math::Vec2;
use std::f32::consts::TAU;

impl Game {
    pub(super) fn update_enemies(&mut self, dt: f32) {
        let player_pos = self.player.pos;
        let player_r = self.player.r;
        let player_alive = self.player.hp > 0.0 && self.state == State::Playing;
        let mut shouts: Vec<Vec2> = Vec::new();

        for e in self.enemies.iter_mut() {
            e.spawn = (e.spawn - dt).max(0.0);
            e.hit_flash = (e.hit_flash - dt).max(0.0);
            e.touch_cd = (e.touch_cd - dt).max(0.0);
            e.fire_cd -= dt;
            e.phase += dt;

            e.wander_t -= dt;
            if e.wander_t <= 0.0 {
                e.wander = self.rng.range(-1.5, 1.5);
                e.wander_t = self.rng.range(0.9, 2.4);
            }

            let to_p = player_pos - e.pos;
            let dist = to_p.len().max(0.001);
            let toward = to_p / dist;
            let was_alerted = e.alerted;
            let patrol_angle = toward.angle() + e.wander;

            // Unaware enemies patrol toward the player's part of the arena
            // with a random offset, so they can be flanked for a silent
            // takedown but still keep the fight moving.
            if !e.alerted && e.spawn <= 0.0 && player_alive {
                let facing = Vec2::from_angle(patrol_angle);
                let in_cone = dist < 360.0 && facing.dot(toward) > 0.6;
                let close = dist < 95.0;
                if in_cone || close {
                    let rate = if close { 3.2 } else { 1.4 };
                    e.awareness = (e.awareness + dt * rate).min(1.0);
                } else {
                    e.awareness = (e.awareness - dt * 0.7).max(0.0);
                }
                if e.awareness >= 1.0 {
                    e.alerted = true;
                }
            }

            if e.alerted {
                e.awareness = 1.0;
                e.aim = toward.angle();
            } else {
                e.aim = patrol_angle;
            }
            if e.alerted && !was_alerted {
                shouts.push(e.pos);
            }

            let desired = if !e.alerted {
                Vec2::from_angle(patrol_angle) * e.speed * 0.55
            } else {
                match e.kind {
                    EnemyKind::Grunt | EnemyKind::Tank => toward * e.speed,
                    EnemyKind::Runner => {
                        let wobble = toward.perp() * (e.phase * 7.0).sin() * 0.55;
                        (toward + wobble).norm() * e.speed
                    }
                    EnemyKind::Shooter => {
                        let radial = if dist > 330.0 {
                            1.0
                        } else if dist < 250.0 {
                            -1.0
                        } else {
                            0.0
                        };
                        let tangent = toward.perp() * e.strafe * 0.9;
                        (toward * radial + tangent).norm() * e.speed
                    }
                }
            };
            let desired = if e.spawn > 0.0 { Vec2::ZERO } else { desired };
            e.vel = e.vel.damp(desired, 4.5, dt);
            e.pos += e.vel * dt;

            let min = ARENA_MARGIN + e.r;
            let max_x = W as f32 - ARENA_MARGIN - e.r;
            let max_y = H as f32 - ARENA_MARGIN - e.r;
            e.pos.x = e.pos.x.clamp(min, max_x);
            e.pos.y = e.pos.y.clamp(min, max_y);
        }

        for pos in shouts {
            self.alert_enemies(pos, 260.0);
        }

        // Soft separation so enemies do not stack into a single blob.
        let n = self.enemies.len();
        for i in 0..n {
            for j in (i + 1)..n {
                let delta = self.enemies[j].pos - self.enemies[i].pos;
                let min_dist = self.enemies[i].r + self.enemies[j].r;
                let d2 = delta.len_sq();
                if d2 < min_dist * min_dist && d2 > 1e-6 {
                    let d = d2.sqrt();
                    let push = delta / d * ((min_dist - d) * 0.5);
                    self.enemies[i].pos -= push;
                    self.enemies[j].pos += push;
                }
            }
        }

        let mut contact = 0.0;
        let mut shots: Vec<(usize, f32)> = Vec::new();

        for i in 0..self.enemies.len() {
            let (er, epos, kind, touch_cd, aim, spawn, fire_cd, alerted) = {
                let e = &self.enemies[i];
                (
                    e.r,
                    e.pos,
                    e.kind,
                    e.touch_cd,
                    e.aim,
                    e.spawn,
                    e.fire_cd,
                    e.alerted,
                )
            };
            if spawn > 0.0 {
                continue;
            }
            if touch_cd <= 0.0 && epos.dist(player_pos) < er + player_r + 2.0 {
                self.enemies[i].touch_cd = 0.7;
                if !self.enemies[i].alerted {
                    self.enemies[i].alerted = true;
                    self.enemies[i].awareness = 1.0;
                    self.alert_enemies(epos, 260.0);
                }
                contact += kind.contact_damage();
                let away = (epos - player_pos).norm();
                self.enemies[i].vel += away * 220.0;
                self.enemies[i].pos += away * 4.0;
                self.spark(epos.lerp(player_pos, 0.5), rgb(255, 90, 90), 6, 160.0);
            }
            if kind == EnemyKind::Shooter && fire_cd <= 0.0 && alerted {
                shots.push((i, aim));
            }
        }

        if contact > 0.0 {
            self.hurt_player(contact);
        }

        for (i, aim) in shots {
            if i >= self.enemies.len() {
                continue;
            }
            self.enemies[i].fire_cd = self.rng.range(1.5, 2.3);
            let angle = aim + self.rng.range(-0.07, 0.07);
            let pos = self.enemies[i].pos + Vec2::from_angle(angle) * (self.enemies[i].r + 6.0);
            let bullet = Bullet {
                pos,
                vel: Vec2::from_angle(angle) * 340.0,
                r: 5.0,
                dmg: 9.0,
                life: 5.0,
                color: C_SHOOTER,
                from_player: false,
                pierce: 0,
            };
            self.bullets.push(bullet);
            self.spark(pos, C_SHOOTER, 4, 90.0);
            self.play_at(Sfx::EnemyShot, pos, 0.42);
        }
    }

    /// Wakes every enemy within `radius` of a loud event (gunshot, blast or
    /// scream). Stealth kills deliberately skip this.
    pub(super) fn alert_enemies(&mut self, pos: Vec2, radius: f32) {
        let r2 = radius * radius;
        for e in self.enemies.iter_mut() {
            if e.spawn > 0.0 {
                continue;
            }
            if e.pos.dist_sq(pos) <= r2 {
                e.alerted = true;
                e.awareness = 1.0;
            }
        }
    }

    pub(super) fn update_bullets(&mut self, dt: f32) {
        for b in self.bullets.iter_mut() {
            b.pos += b.vel * dt;
            b.life -= dt;
        }

        let min = ARENA_MARGIN;
        let max_x = W as f32 - ARENA_MARGIN;
        let max_y = H as f32 - ARENA_MARGIN;
        let mut wall_hits: Vec<(Vec2, u32)> = Vec::new();
        let mut enemy_hits: Vec<(usize, f32, Vec2, Vec2, u32)> = Vec::new();
        let mut player_hit = 0.0f32;

        let player_pos = self.player.pos;
        let player_r = self.player.r;

        for i in 0..self.bullets.len() {
            let b = self.bullets[i];
            if b.life <= 0.0 {
                continue;
            }
            if b.pos.x < min || b.pos.x > max_x || b.pos.y < min || b.pos.y > max_y {
                self.bullets[i].life = -1.0;
                wall_hits.push((b.pos, b.color));
                continue;
            }
            if b.from_player {
                // Pick the nearest enemy so a piercing sniper round travels
                // through a line of targets in the expected order.
                let mut nearest: Option<(usize, f32)> = None;
                for ei in 0..self.enemies.len() {
                    let e = &self.enemies[ei];
                    if e.spawn > 0.0 || e.hp <= 0.0 {
                        continue;
                    }
                    let rr = e.r + b.r;
                    let d2 = e.pos.dist_sq(b.pos);
                    if d2 < rr * rr && nearest.map_or(true, |(_, best)| d2 < best) {
                        nearest = Some((ei, d2));
                    }
                }
                if let Some((ei, _)) = nearest {
                    let dir = b.vel.norm();
                    let (e_pos, e_r) = {
                        let e = &self.enemies[ei];
                        (e.pos, e.r)
                    };
                    enemy_hits.push((ei, b.dmg, dir, b.pos, b.color));
                    if b.pierce > 0 {
                        self.bullets[i].pierce -= 1;
                        // Step past the target so the same enemy is not hit
                        // again on the next frame.
                        self.bullets[i].pos = e_pos + dir * (e_r + b.r + 1.0);
                    } else {
                        self.bullets[i].life = -1.0;
                    }
                }
            } else if self.player.iframes <= 0.0 && player_hit == 0.0 {
                let rr = player_r + b.r;
                if b.pos.dist_sq(player_pos) < rr * rr {
                    self.bullets[i].life = -1.0;
                    player_hit += b.dmg;
                }
            }
        }

        let hit_pos = enemy_hits.first().map(|hit| hit.3);
        let mut alerts: Vec<Vec2> = Vec::new();
        for (ei, dmg, dir, pos, color) in enemy_hits {
            if ei >= self.enemies.len() || self.enemies[ei].hp <= 0.0 {
                continue;
            }
            {
                let e = &mut self.enemies[ei];
                e.hp -= dmg;
                e.hit_flash = 0.15;
                e.alerted = true;
                e.awareness = 1.0;
                e.vel += dir * (2600.0 / e.r.max(12.0));
            }
            alerts.push(pos);
            self.spark(pos, color, 4, 150.0);
        }
        for pos in alerts {
            self.alert_enemies(pos, 300.0);
        }

        if let Some(pos) = hit_pos {
            if self.hit_sfx_cd <= 0.0 {
                self.play_at(Sfx::Hit, pos, 0.30);
                self.hit_sfx_cd = 0.035;
            }
        }

        let dead: Vec<usize> = self
            .enemies
            .iter()
            .enumerate()
            .filter(|(_, e)| e.hp <= 0.0)
            .map(|(i, _)| i)
            .collect();
        for &i in dead.iter().rev() {
            self.remove_enemy(i, true, false);
        }

        if player_hit > 0.0 {
            self.hurt_player(player_hit);
        }

        for (pos, color) in wall_hits {
            self.spark(pos, color, 4, 130.0);
        }

        self.bullets.retain(|b| b.life > 0.0);
    }

    /// Rockets detonate on impact; grenades bounce, drag and cook off when
    /// their fuse runs out.
    pub(super) fn update_projectiles(&mut self, dt: f32) {
        let mut blasts: Vec<(Vec2, Blast)> = Vec::new();

        for p in self.projectiles.iter_mut() {
            p.life -= dt;
            p.vel *= 1.0 / (1.0 + p.drag * dt);
            p.pos += p.vel * dt;
            p.spin += dt * 7.0;

            let mut detonate = p.life <= 0.0;
            let min = ARENA_MARGIN + p.r;
            let max_x = W as f32 - ARENA_MARGIN - p.r;
            let max_y = H as f32 - ARENA_MARGIN - p.r;
            if p.pos.x < min || p.pos.x > max_x || p.pos.y < min || p.pos.y > max_y {
                if p.bounce {
                    if p.pos.x < min {
                        p.pos.x = min;
                        p.vel.x = p.vel.x.abs() * 0.55;
                    } else if p.pos.x > max_x {
                        p.pos.x = max_x;
                        p.vel.x = -p.vel.x.abs() * 0.55;
                    }
                    if p.pos.y < min {
                        p.pos.y = min;
                        p.vel.y = p.vel.y.abs() * 0.55;
                    } else if p.pos.y > max_y {
                        p.pos.y = max_y;
                        p.vel.y = -p.vel.y.abs() * 0.55;
                    }
                    p.vel *= 0.80;
                } else {
                    detonate = true;
                }
            }

            if !detonate && p.kind == ProjectileKind::Rocket {
                for e in &self.enemies {
                    if e.spawn > 0.0 || e.hp <= 0.0 {
                        continue;
                    }
                    let rr = e.r + p.r;
                    if e.pos.dist_sq(p.pos) < rr * rr {
                        detonate = true;
                        break;
                    }
                }
            }

            if detonate {
                blasts.push((p.pos, p.blast));
                p.life = -1.0;
            }
        }

        self.projectiles.retain(|p| p.life > 0.0);
        for (pos, blast) in blasts {
            self.explode(pos, blast);
        }
    }

    /// Area damage used by both launcher rockets and thrown grenades.
    pub(super) fn explode(&mut self, pos: Vec2, blast: Blast) {
        self.shockwaves.push(Shockwave {
            pos,
            r: blast.radius * 0.25,
            speed: blast.radius * 2.4,
            life: 0.55,
            max_life: 0.55,
            width: 5.0,
            color: blast.color,
        });
        self.spawn_burst(pos, blast.color, 34, 480.0, 3.4);
        self.spawn_burst(pos, C_WHITE, 12, 560.0, 2.2);
        self.flash = self.flash.max(0.34);
        self.shake = self.shake.max(19.0);
        self.hit_stop = self.hit_stop.max(0.055);
        self.play_centered(Sfx::Explosion, 0.92);
        self.alert_enemies(pos, blast.radius * 2.8);

        let mut dead: Vec<usize> = Vec::new();
        for i in 0..self.enemies.len() {
            let e = &self.enemies[i];
            if e.spawn > 0.0 || e.hp <= 0.0 {
                continue;
            }
            let delta = e.pos - pos;
            let d = delta.len();
            let reach = blast.radius + e.r;
            if d <= reach {
                let falloff = (1.0 - d / reach).max(0.22);
                let dmg = blast.damage * falloff;
                let push = delta.norm() * (1100.0 / e.r.max(12.0));
                let e = &mut self.enemies[i];
                e.hp -= dmg;
                e.hit_flash = 0.22;
                e.alerted = true;
                e.awareness = 1.0;
                e.vel += push;
                if e.hp <= 0.0 {
                    dead.push(i);
                }
            }
        }
        for &i in dead.iter().rev() {
            self.remove_enemy(i, true, false);
        }

        let pd = self.player.pos.dist(pos);
        let reach = blast.radius + self.player.r;
        if pd <= reach {
            let falloff = (1.0 - pd / reach).max(0.35);
            self.hurt_player(blast.self_damage * falloff);
        }
    }

    pub(super) fn update_pickups(&mut self, dt: f32) {
        let player_pos = self.player.pos;
        let player_r = self.player.r;
        let text = crate::lang::text(self.settings.lang);

        for i in 0..self.pickups.len() {
            self.pickups[i].life -= dt;
            self.pickups[i].phase += dt;

            let pos = self.pickups[i].pos;
            let dist = pos.dist(player_pos);
            if dist < 170.0 && dist > 0.001 {
                let dir = (player_pos - pos) / dist;
                let pull = 60.0 + 340.0 * (1.0 - dist / 170.0);
                self.pickups[i].pos += dir * pull * dt;
            }

            if dist < player_r + 13.0 && self.pickups[i].life > 0.0 {
                self.pickups[i].life = -1.0;
                match self.pickups[i].kind {
                    PickupKind::Health => {
                        self.player.hp = (self.player.hp + 30.0).min(self.player.max_hp);
                        self.popup(pos, text.heal.to_string(), C_HEALTH, 2);
                        self.spark(pos, C_HEALTH, 10, 180.0);
                        self.play_at(Sfx::PickupHealth, pos, 0.55);
                    }
                    PickupKind::Ammo => {
                        for w in 0..WEAPONS.len() {
                            self.player.ammo[w] = WEAPONS[w].mag;
                            self.player.reload[w] = 0.0;
                            self.player.cooldown[w] = 0.0;
                        }
                        self.popup(pos, text.ammo.to_string(), C_AMMO, 2);
                        self.spark(pos, C_AMMO, 10, 180.0);
                        self.play_at(Sfx::PickupAmmo, pos, 0.55);
                    }
                }
            }
        }

        self.pickups.retain(|p| p.life > 0.0);
    }

    pub(super) fn update_particles(&mut self, dt: f32) {
        for p in self.particles.iter_mut() {
            p.pos += p.vel * dt;
            p.vel *= 1.0 / (1.0 + p.drag * dt);
            p.life -= dt;
        }
        self.particles.retain(|p| p.life > 0.0);
    }

    pub(super) fn update_shockwaves(&mut self, dt: f32) {
        for s in self.shockwaves.iter_mut() {
            s.r += s.speed * dt;
            s.life -= dt;
        }
        self.shockwaves.retain(|s| s.life > 0.0);
    }

    pub(super) fn update_popups(&mut self, dt: f32) {
        for p in self.popups.iter_mut() {
            p.pos += p.vel * dt;
            p.vel *= 1.0 / (1.0 + 2.5 * dt);
            p.life -= dt;
        }
        self.popups.retain(|p| p.life > 0.0);
    }

    pub(super) fn update_waves(&mut self, dt: f32) {
        if self.wave_break > 0.0 {
            self.wave_break -= dt;
            if self.wave_break <= 0.0 {
                self.wave_break = 0.0;
                self.start_wave();
            }
            return;
        }

        if !self.spawn_queue.is_empty() {
            self.spawn_timer -= dt;
            if self.spawn_timer <= 0.0 {
                if let Some(kind) = self.spawn_queue.pop() {
                    self.spawn_enemy(kind);
                }
                self.spawn_timer = (0.45 - self.wave as f32 * 0.012).max(0.15);
            }
            return;
        }

        if self.enemies.is_empty() {
            self.wave_break = 3.0;
            let bonus = 50 + self.wave * 20;
            self.score += bonus;
            self.player.hp = (self.player.hp + 18.0).min(self.player.max_hp);
            let text = crate::lang::text(self.settings.lang);
            self.banner = format!("{} {} {}", text.wave, self.wave, text.cleared);
            self.banner_t = 2.6;
            self.popup(
                self.player.pos + Vec2::new(0.0, -42.0),
                format!("{} +{}", text.bonus, bonus),
                C_ACCENT,
                2,
            );
            self.spark(self.player.pos, C_PLAYER, 20, 240.0);
            self.shockwaves.push(Shockwave {
                pos: self.player.pos,
                r: 20.0,
                speed: 380.0,
                life: 0.6,
                max_life: 0.6,
                width: 3.0,
                color: C_PLAYER,
            });
            self.play_centered(Sfx::WaveClear, 0.65);
        }
    }

    pub(super) fn hurt_player(&mut self, dmg: f32) {
        if self.player.iframes > 0.0 || self.state != State::Playing {
            return;
        }
        self.player.hp -= dmg;
        self.player.iframes = 0.55;
        self.player.hurt_flash = 0.45;
        self.damage_tint = 1.0;
        self.shake = self.shake.max(10.0);
        self.hit_stop = self.hit_stop.max(0.05);
        self.combo = 0;
        self.combo_t = 0.0;
        self.spawn_burst(self.player.pos, rgb(255, 70, 90), 14, 240.0, 2.4);
        self.play_centered(Sfx::Hurt, 0.75);
    }

    pub(super) fn remove_enemy(&mut self, idx: usize, credit: bool, quiet: bool) {
        if idx >= self.enemies.len() {
            return;
        }
        let e = self.enemies.remove(idx);
        let color = e.kind.color();
        let is_tank = e.kind == EnemyKind::Tank;
        let (_, _, _, base_score) = e.kind.base();
        let mut gain = if credit {
            (base_score as f32 * self.multiplier()).round().max(1.0) as u32
        } else {
            0
        };
        if quiet {
            // Silent takedowns are worth more: they are harder to set up and
            // keep the rest of the room asleep.
            gain = (gain as f32 * 1.5).round() as u32;
        }

        if credit {
            self.kills += 1;
            self.score += gain;
            self.combo += 1;
            self.combo_t = 2.5;
        }
        self.shake = self.shake.max(if quiet { 1.5 } else { 3.0 + e.r * 0.12 });
        self.hit_stop = self.hit_stop.max(if quiet { 0.015 } else { 0.03 });

        let count = if quiet { 8 } else { (14.0 + e.r) as usize };
        self.spawn_burst(e.pos, color, count, if quiet { 120.0 } else { 280.0 }, e.r * 0.16 + 1.2);
        if !quiet {
            self.spawn_burst(e.pos, C_WHITE, 6, 360.0, 1.8);
        }
        self.shockwaves.push(Shockwave {
            pos: e.pos,
            r: e.r * 0.7,
            speed: if quiet { 180.0 } else { 420.0 },
            life: if quiet { 0.22 } else { 0.35 },
            max_life: if quiet { 0.22 } else { 0.35 },
            width: if quiet { 1.5 } else { 3.0 },
            color,
        });

        if quiet {
            let text = crate::lang::text(self.settings.lang);
            self.popup(e.pos, format!("{} +{}", text.silent, gain), color, 2);
            self.play_at(Sfx::Stealth, e.pos, 0.55);
        } else {
            self.popup(e.pos, format!("+{}", gain), color, 2);
            self.play_at(
                if is_tank { Sfx::TankBoom } else { Sfx::Kill },
                e.pos,
                if is_tank { 0.85 } else { 0.50 },
            );
        }

        if self.rng.chance(0.14) {
            let phase = self.rng.range(0.0, TAU);
            self.pickups.push(Pickup {
                pos: e.pos,
                kind: PickupKind::Health,
                life: 14.0,
                phase,
            });
        }
        if self.rng.chance(0.20) {
            let phase = self.rng.range(0.0, TAU);
            self.pickups.push(Pickup {
                pos: e.pos + Vec2::new(16.0, 0.0),
                kind: PickupKind::Ammo,
                life: 14.0,
                phase,
            });
        }

        if is_tank {
            self.shake = self.shake.max(11.0);
            // A tank that dies quietly does not spray shrapnel; the knife
            // player keeps the element of surprise.
            if !quiet {
                for i in 0..8 {
                    let angle = i as f32 * TAU / 8.0 + self.rng.range(-0.16, 0.16);
                    let bullet = Bullet {
                        pos: e.pos,
                        vel: Vec2::from_angle(angle) * 260.0,
                        r: 5.0,
                        dmg: 8.0,
                        life: 5.0,
                        color: C_TANK,
                        from_player: false,
                        pierce: 0,
                    };
                    self.bullets.push(bullet);
                }
            }
        }

        if !quiet {
            self.alert_enemies(e.pos, 420.0);
        }
    }

    pub(super) fn game_over(&mut self) {
        if self.state == State::Dead {
            return;
        }
        self.state = State::Dead;
        self.death_t = 0.0;
        self.flash = 1.0;
        self.shake = 22.0;
        self.hit_stop = 0.0;
        self.damage_tint = 1.0;
        self.play_centered(Sfx::GameOver, 0.90);

        if self.score > self.high_score {
            self.high_score = self.score;
            self.new_record = true;
            save_high_score(self.high_score);
        }

        let pos = self.player.pos;
        self.spawn_burst(pos, C_PLAYER, 40, 420.0, 3.0);
        self.spawn_burst(pos, C_WHITE, 18, 520.0, 2.0);
        self.shockwaves.push(Shockwave {
            pos,
            r: 18.0,
            speed: 520.0,
            life: 0.8,
            max_life: 0.8,
            width: 4.0,
            color: C_PLAYER,
        });
    }
}
