//! Core game state: entities, the run/wave flow and player controls.

mod draw;
mod sim;

use std::f32::consts::TAU;

use crate::canvas::{lerp_color, rgb};
use crate::math::Vec2;

pub const W: i32 = 1280;
pub const H: i32 = 720;

pub(super) const ARENA_MARGIN: f32 = 44.0;
pub(super) const PLAYER_R: f32 = 14.0;
pub(super) const DASH_COOLDOWN: f32 = 1.0;

pub(super) const C_PLAYER: u32 = rgb(80, 240, 200);
pub(super) const C_PLAYER_DARK: u32 = rgb(14, 48, 46);
pub(super) const C_ACCENT: u32 = rgb(120, 220, 255);
pub(super) const C_WHITE: u32 = rgb(238, 246, 255);
pub(super) const C_DIM: u32 = rgb(120, 140, 165);
pub(super) const C_GRUNT: u32 = rgb(255, 62, 130);
pub(super) const C_RUNNER: u32 = rgb(255, 160, 60);
pub(super) const C_SHOOTER: u32 = rgb(80, 220, 255);
pub(super) const C_TANK: u32 = rgb(176, 112, 255);
pub(super) const C_HEALTH: u32 = rgb(90, 255, 150);
pub(super) const C_AMMO: u32 = rgb(255, 210, 90);

// ---------------------------------------------------------------------------
// Random numbers
// ---------------------------------------------------------------------------

/// Tiny xorshift generator: no external crates, good enough for a shooter.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        (x >> 32) as u32
    }

    fn unit(&mut self) -> f32 {
        self.next_u32() as f32 / 4_294_967_296.0
    }

    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.unit()
    }

    fn range_i(&mut self, a: i32, b: i32) -> i32 {
        if b <= a {
            a
        } else {
            a + (self.next_u32() % (b - a) as u32) as i32
        }
    }

    fn chance(&mut self, p: f32) -> bool {
        self.unit() < p
    }
}

// ---------------------------------------------------------------------------
// Input
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    pub mouse: Vec2,
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub shoot: bool,
    pub dash: bool,
    pub reload: bool,
    pub swap: bool,
    pub press_start: bool,
    pub press_pause: bool,
    pub press_restart: bool,
}

// ---------------------------------------------------------------------------
// Entities
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Title,
    Playing,
    Paused,
    Dead,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum EnemyKind {
    Grunt,
    Runner,
    Shooter,
    Tank,
}

impl EnemyKind {
    pub(super) fn base(self) -> (f32, f32, f32, u32) {
        // (hp, radius, speed, score)
        match self {
            EnemyKind::Grunt => (34.0, 15.0, 92.0, 10),
            EnemyKind::Runner => (20.0, 11.0, 178.0, 16),
            EnemyKind::Shooter => (42.0, 16.0, 82.0, 26),
            EnemyKind::Tank => (190.0, 27.0, 56.0, 70),
        }
    }

    pub(super) fn color(self) -> u32 {
        match self {
            EnemyKind::Grunt => C_GRUNT,
            EnemyKind::Runner => C_RUNNER,
            EnemyKind::Shooter => C_SHOOTER,
            EnemyKind::Tank => C_TANK,
        }
    }

    pub(super) fn contact_damage(self) -> f32 {
        match self {
            EnemyKind::Grunt => 11.0,
            EnemyKind::Runner => 8.0,
            EnemyKind::Shooter => 9.0,
            EnemyKind::Tank => 22.0,
        }
    }
}

pub(super) struct Enemy {
    pub kind: EnemyKind,
    pub pos: Vec2,
    pub vel: Vec2,
    pub r: f32,
    pub hp: f32,
    pub max_hp: f32,
    pub speed: f32,
    pub hit_flash: f32,
    pub fire_cd: f32,
    pub touch_cd: f32,
    pub phase: f32,
    pub aim: f32,
    pub spawn: f32,
    pub strafe: f32,
}

#[derive(Clone, Copy)]
pub(super) struct Bullet {
    pub pos: Vec2,
    pub vel: Vec2,
    pub r: f32,
    pub dmg: f32,
    pub life: f32,
    pub color: u32,
    pub from_player: bool,
}

pub(super) struct Particle {
    pub pos: Vec2,
    pub vel: Vec2,
    pub life: f32,
    pub max_life: f32,
    pub size: f32,
    pub color: u32,
    pub drag: f32,
    pub additive: bool,
}

pub(super) struct Shockwave {
    pub pos: Vec2,
    pub r: f32,
    pub speed: f32,
    pub life: f32,
    pub max_life: f32,
    pub width: f32,
    pub color: u32,
}

pub(super) struct Popup {
    pub pos: Vec2,
    pub vel: Vec2,
    pub text: String,
    pub life: f32,
    pub max_life: f32,
    pub color: u32,
    pub scale: i32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum PickupKind {
    Health,
    Ammo,
}

pub(super) struct Pickup {
    pub pos: Vec2,
    pub kind: PickupKind,
    pub life: f32,
    pub phase: f32,
}

// ---------------------------------------------------------------------------
// Weapons
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub(super) struct Weapon {
    pub name: &'static str,
    pub mag: u32,
    pub reload_time: f32,
    pub delay: f32,
    pub pellets: u32,
    pub spread: f32,
    pub damage: f32,
    pub speed: f32,
    pub bullet_r: f32,
    pub shake: f32,
    pub knock: f32,
    pub color: u32,
}

pub(super) const RIFLE: Weapon = Weapon {
    name: "RIFLE",
    mag: 30,
    reload_time: 1.05,
    delay: 0.085,
    pellets: 1,
    spread: 0.028,
    damage: 14.0,
    speed: 1250.0,
    bullet_r: 3.0,
    shake: 2.0,
    knock: 300.0,
    color: rgb(255, 224, 130),
};

pub(super) const SHOTGUN: Weapon = Weapon {
    name: "SHOTGUN",
    mag: 6,
    reload_time: 1.75,
    delay: 0.55,
    pellets: 9,
    spread: 0.30,
    damage: 11.0,
    speed: 1000.0,
    bullet_r: 2.6,
    shake: 7.0,
    knock: 950.0,
    color: rgb(255, 170, 90),
};

pub(super) const WEAPONS: [Weapon; 2] = [RIFLE, SHOTGUN];

pub(super) struct Player {
    pub pos: Vec2,
    pub vel: Vec2,
    pub r: f32,
    pub hp: f32,
    pub max_hp: f32,
    pub aim: f32,
    pub weapon: usize,
    pub ammo: [u32; 2],
    pub reload: [f32; 2],
    pub cooldown: [f32; 2],
    pub dash_cd: f32,
    pub dash_time: f32,
    pub dash_dir: Vec2,
    pub iframes: f32,
    pub muzzle: f32,
    pub hurt_flash: f32,
}

impl Player {
    fn new(pos: Vec2) -> Self {
        Self {
            pos,
            vel: Vec2::ZERO,
            r: PLAYER_R,
            hp: 100.0,
            max_hp: 100.0,
            aim: 0.0,
            weapon: 0,
            ammo: [RIFLE.mag, SHOTGUN.mag],
            reload: [0.0, 0.0],
            cooldown: [0.0, 0.0],
            dash_cd: 0.0,
            dash_time: 0.0,
            dash_dir: Vec2::new(1.0, 0.0),
            iframes: 0.0,
            muzzle: 0.0,
            hurt_flash: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Game
// ---------------------------------------------------------------------------

pub struct Game {
    pub state: State,
    pub quit: bool,

    bg: Vec<u32>,
    vig: Vec<f32>,
    rng: Rng,
    time: f32,
    run_time: f32,

    player: Player,
    enemies: Vec<Enemy>,
    bullets: Vec<Bullet>,
    particles: Vec<Particle>,
    shockwaves: Vec<Shockwave>,
    pickups: Vec<Pickup>,
    popups: Vec<Popup>,

    wave: u32,
    wave_break: f32,
    spawn_queue: Vec<EnemyKind>,
    spawn_timer: f32,

    score: u32,
    high_score: u32,
    new_record: bool,
    kills: u32,
    combo: u32,
    combo_t: f32,

    shake: f32,
    flash: f32,
    damage_tint: f32,
    hit_stop: f32,
    death_t: f32,

    banner: String,
    banner_t: f32,
    mouse: Vec2,
}

impl Game {
    pub fn new() -> Self {
        Self {
            state: State::Title,
            quit: false,
            bg: draw::build_background(),
            vig: draw::build_vignette(W, H),
            rng: Rng::new(0x9E37_79B9_7F4A_7C15),
            time: 0.0,
            run_time: 0.0,
            player: Player::new(Vec2::new(W as f32 * 0.5, H as f32 * 0.5)),
            enemies: Vec::new(),
            bullets: Vec::new(),
            particles: Vec::new(),
            shockwaves: Vec::new(),
            pickups: Vec::new(),
            popups: Vec::new(),
            wave: 0,
            wave_break: 0.0,
            spawn_queue: Vec::new(),
            spawn_timer: 0.0,
            score: 0,
            high_score: load_high_score(),
            new_record: false,
            kills: 0,
            combo: 0,
            combo_t: 0.0,
            shake: 0.0,
            flash: 0.0,
            damage_tint: 0.0,
            hit_stop: 0.0,
            death_t: 0.0,
            banner: String::new(),
            banner_t: 0.0,
            mouse: Vec2::new(W as f32 * 0.5, H as f32 * 0.5),
        }
    }

    // -- public helpers -----------------------------------------------------

    pub fn state(&self) -> State {
        self.state
    }

    pub fn nearest_enemy(&self) -> Option<Vec2> {
        let mut best: Option<(f32, Vec2)> = None;
        for e in &self.enemies {
            if e.hp <= 0.0 {
                continue;
            }
            let d = e.pos.dist_sq(self.player.pos);
            if best.map_or(true, |(bd, _)| d < bd) {
                best = Some((d, e.pos));
            }
        }
        best.map(|(_, p)| p)
    }

    pub fn stats(&self) -> (u32, u32, u32, f32) {
        (self.score, self.wave, self.kills, self.player.hp)
    }

    /// Used by the headless smoke test to exercise the death screen.
    pub fn force_death(&mut self) {
        self.player.hp = 0.0;
    }

    pub fn start_run(&mut self) {
        self.state = State::Playing;
        self.quit = false;
        self.player = Player::new(Vec2::new(W as f32 * 0.5, H as f32 * 0.5));
        self.enemies.clear();
        self.bullets.clear();
        self.particles.clear();
        self.shockwaves.clear();
        self.pickups.clear();
        self.popups.clear();
        self.spawn_queue.clear();
        self.wave = 0;
        self.wave_break = 0.0;
        self.spawn_timer = 0.0;
        self.score = 0;
        self.new_record = false;
        self.kills = 0;
        self.combo = 0;
        self.combo_t = 0.0;
        self.shake = 0.0;
        self.flash = 0.0;
        self.damage_tint = 0.0;
        self.hit_stop = 0.0;
        self.death_t = 0.0;
        self.run_time = 0.0;
        self.banner.clear();
        self.banner_t = 0.0;
        self.start_wave();
    }

    // -- frame update -------------------------------------------------------

    pub fn update(&mut self, dt: f32, input: &Input) {
        let dt = dt.clamp(0.0, 1.0 / 20.0);
        self.time += dt;
        self.mouse = input.mouse;

        // Screen effects keep fading on every screen.
        self.shake = (self.shake - dt * 26.0).max(0.0);
        self.flash = (self.flash - dt * 3.5).max(0.0);
        self.damage_tint = (self.damage_tint - dt * 3.0).max(0.0);
        if self.banner_t > 0.0 {
            self.banner_t = (self.banner_t - dt).max(0.0);
        }

        match self.state {
            State::Title => {
                if input.press_pause {
                    self.quit = true;
                }
                if input.press_start {
                    self.start_run();
                }
                return;
            }
            State::Paused => {
                if input.press_pause {
                    self.state = State::Playing;
                } else if input.press_restart {
                    self.start_run();
                } else if input.press_start {
                    self.state = State::Title;
                    self.enemies.clear();
                    self.bullets.clear();
                    self.particles.clear();
                }
                return;
            }
            State::Dead => {
                self.death_t += dt;
                self.update_particles(dt);
                self.update_shockwaves(dt);
                self.update_popups(dt);
                if self.death_t > 0.55 && (input.press_start || input.press_restart) {
                    self.start_run();
                } else if self.death_t > 0.55 && input.press_pause {
                    self.state = State::Title;
                }
                return;
            }
            State::Playing => {}
        }

        if input.press_pause {
            self.state = State::Paused;
            return;
        }

        self.run_time += dt;

        let mut sim_dt = dt;
        if self.hit_stop > 0.0 {
            self.hit_stop -= dt;
            sim_dt = dt * 0.15;
        }

        self.update_player(sim_dt, input);
        self.update_enemies(sim_dt);
        self.update_bullets(sim_dt);
        self.update_pickups(sim_dt);
        self.update_particles(sim_dt);
        self.update_shockwaves(sim_dt);
        self.update_popups(sim_dt);
        self.update_waves(sim_dt);

        if self.combo_t > 0.0 {
            self.combo_t -= dt;
            if self.combo_t <= 0.0 {
                self.combo = 0;
            }
        }

        if self.player.hp <= 0.0 {
            self.game_over();
        }
    }

    // -- player -------------------------------------------------------------

    fn update_player(&mut self, dt: f32, input: &Input) {
        if input.mouse.is_finite() {
            self.player.aim = (input.mouse - self.player.pos).angle();
        }

        self.player.iframes = (self.player.iframes - dt).max(0.0);
        self.player.muzzle = (self.player.muzzle - dt).max(0.0);
        self.player.hurt_flash = (self.player.hurt_flash - dt).max(0.0);
        self.player.dash_cd = (self.player.dash_cd - dt).max(0.0);

        for i in 0..2 {
            self.player.cooldown[i] = (self.player.cooldown[i] - dt).max(0.0);
            if self.player.reload[i] > 0.0 {
                self.player.reload[i] -= dt;
                if self.player.reload[i] <= 0.0 {
                    self.player.reload[i] = 0.0;
                    self.player.ammo[i] = WEAPONS[i].mag;
                    self.spark(self.player.pos, WEAPONS[i].color, 5, 90.0);
                }
            }
        }

        if input.swap {
            self.player.weapon = (self.player.weapon + 1) % WEAPONS.len();
            let col = WEAPONS[self.player.weapon].color;
            self.spark(self.player.pos, col, 6, 130.0);
        }

        if input.reload {
            let w = self.player.weapon;
            if self.player.reload[w] <= 0.0 && self.player.ammo[w] < WEAPONS[w].mag {
                self.player.reload[w] = WEAPONS[w].reload_time;
            }
        }

        let mut dir = Vec2::ZERO;
        if input.left {
            dir.x -= 1.0;
        }
        if input.right {
            dir.x += 1.0;
        }
        if input.up {
            dir.y -= 1.0;
        }
        if input.down {
            dir.y += 1.0;
        }
        let dir = dir.norm();

        if input.dash && self.player.dash_cd <= 0.0 && self.player.dash_time <= 0.0 {
            let d = if dir.len_sq() > 0.0 {
                dir
            } else {
                Vec2::from_angle(self.player.aim)
            };
            self.player.dash_dir = d.norm();
            self.player.dash_time = 0.15;
            self.player.dash_cd = DASH_COOLDOWN;
            self.player.iframes = self.player.iframes.max(0.28);
            self.flash = self.flash.max(0.18);
            self.shake = self.shake.max(3.0);
            let dust = self.player.pos - self.player.dash_dir * self.player.r;
            self.spawn_burst(dust, C_ACCENT, 10, 180.0, 2.2);
        }

        if self.player.dash_time > 0.0 {
            self.player.dash_time -= dt;
            self.player.vel = self.player.dash_dir * 1400.0;
            if self.rng.chance(0.85) {
                let p = self.player.pos - self.player.dash_dir * self.player.r;
                let life = self.rng.range(0.15, 0.3);
                self.particles.push(Particle {
                    pos: p,
                    vel: -self.player.dash_dir * self.rng.range(20.0, 80.0),
                    life,
                    max_life: life,
                    size: self.rng.range(1.5, 3.5),
                    color: C_PLAYER,
                    drag: 4.0,
                    additive: true,
                });
            }
        } else {
            self.player.vel += dir * 2800.0 * dt;
            self.player.vel *= 1.0 / (1.0 + 11.0 * dt);
            self.player.vel = self.player.vel.clamp_len(315.0);
        }

        self.player.pos += self.player.vel * dt;
        let r = self.player.r;
        let min = ARENA_MARGIN + r;
        let max_x = W as f32 - ARENA_MARGIN - r;
        let max_y = H as f32 - ARENA_MARGIN - r;
        if self.player.pos.x < min {
            self.player.pos.x = min;
            self.player.vel.x = self.player.vel.x.max(0.0);
        }
        if self.player.pos.x > max_x {
            self.player.pos.x = max_x;
            self.player.vel.x = self.player.vel.x.min(0.0);
        }
        if self.player.pos.y < min {
            self.player.pos.y = min;
            self.player.vel.y = self.player.vel.y.max(0.0);
        }
        if self.player.pos.y > max_y {
            self.player.pos.y = max_y;
            self.player.vel.y = self.player.vel.y.min(0.0);
        }

        let w = self.player.weapon;
        if input.shoot && self.player.dash_time <= 0.0 && self.player.reload[w] <= 0.0 {
            if self.player.ammo[w] == 0 {
                self.player.reload[w] = WEAPONS[w].reload_time;
            } else if self.player.cooldown[w] <= 0.0 {
                self.fire_weapon(w);
            }
        }

        if self.player.reload[w] > 0.0 && self.rng.chance(0.25) {
            let p = self.player.pos + Vec2::new(self.rng.range(-14.0, 14.0), 10.0);
            let life = self.rng.range(0.2, 0.45);
            self.particles.push(Particle {
                pos: p,
                vel: Vec2::new(self.rng.range(-20.0, 20.0), self.rng.range(20.0, 60.0)),
                life,
                max_life: life,
                size: 1.6,
                color: C_DIM,
                drag: 2.0,
                additive: false,
            });
        }
    }

    fn fire_weapon(&mut self, wi: usize) {
        let weapon = WEAPONS[wi];
        if self.player.ammo[wi] == 0 {
            return;
        }
        self.player.ammo[wi] -= 1;
        self.player.cooldown[wi] = weapon.delay;
        self.player.muzzle = 0.06;
        self.player.vel -= Vec2::from_angle(self.player.aim) * (weapon.knock * 0.06);
        self.shake = self.shake.max(weapon.shake);

        let dir = Vec2::from_angle(self.player.aim);
        let muzzle = self.player.pos + dir * (self.player.r + 17.0);
        for _ in 0..weapon.pellets {
            let ang = self.player.aim + self.rng.range(-weapon.spread, weapon.spread);
            let speed = weapon.speed * self.rng.range(0.94, 1.06);
            let b = Bullet {
                pos: muzzle + Vec2::from_angle(ang) * 2.0,
                vel: Vec2::from_angle(ang) * speed,
                r: weapon.bullet_r,
                dmg: weapon.damage,
                life: 1.4,
                color: weapon.color,
                from_player: true,
            };
            self.bullets.push(b);
        }
        self.spawn_burst(muzzle, weapon.color, 4 + weapon.pellets as usize, 220.0, 2.0);

        if self.player.ammo[wi] == 0 {
            self.player.reload[wi] = weapon.reload_time;
        }
    }

    // -- spawning -----------------------------------------------------------

    pub(super) fn start_wave(&mut self) {
        self.wave += 1;
        let w = self.wave;
        self.wave_break = 0.0;
        self.spawn_queue.clear();

        for _ in 0..(4 + w * 2).min(36) {
            self.spawn_queue.push(EnemyKind::Grunt);
        }
        if w >= 2 {
            for _ in 0..(w + 1).min(14) {
                self.spawn_queue.push(EnemyKind::Runner);
            }
        }
        if w >= 3 {
            for _ in 0..(w - 1).min(10) {
                self.spawn_queue.push(EnemyKind::Shooter);
            }
        }
        if w >= 5 {
            for _ in 0..((w - 3) / 2 + 1).min(5) {
                self.spawn_queue.push(EnemyKind::Tank);
            }
        }

        for i in (1..self.spawn_queue.len()).rev() {
            let j = self.rng.range_i(0, i as i32 + 1) as usize;
            self.spawn_queue.swap(i, j);
        }

        self.spawn_timer = 0.7;
        self.banner = if w % 5 == 0 {
            format!("WAVE {} - ELITE", w)
        } else {
            format!("WAVE {}", w)
        };
        self.banner_t = 2.2;
        self.flash = self.flash.max(0.15);
    }

    pub(super) fn spawn_enemy(&mut self, kind: EnemyKind) {
        let (base_hp, r, base_speed, _) = kind.base();
        let w = (self.wave.max(1) - 1) as f32;
        let hp = base_hp * (1.0 + w * 0.16);
        let speed = base_speed * (1.0 + w.min(9.0) * 0.022);

        let player_pos = self.player.pos;
        let mut pos = Vec2::new(W as f32 * 0.5, H as f32 * 0.5);
        for _ in 0..28 {
            let ang = self.rng.range(0.0, TAU);
            let dist = self.rng.range(520.0, 780.0);
            let candidate = player_pos + Vec2::from_angle(ang) * dist;
            let clamped = Vec2::new(
                candidate.x.clamp(ARENA_MARGIN + r, W as f32 - ARENA_MARGIN - r),
                candidate.y.clamp(ARENA_MARGIN + r, H as f32 - ARENA_MARGIN - r),
            );
            if clamped.dist(player_pos) > 260.0 {
                pos = clamped;
                break;
            }
        }

        let color = kind.color();
        self.enemies.push(Enemy {
            kind,
            pos,
            vel: Vec2::ZERO,
            r,
            hp,
            max_hp: hp,
            speed,
            hit_flash: 0.0,
            fire_cd: self.rng.range(0.9, 2.2),
            touch_cd: 0.0,
            phase: self.rng.range(0.0, TAU),
            aim: 0.0,
            spawn: 0.5,
            strafe: if self.rng.chance(0.5) { 1.0 } else { -1.0 },
        });
        self.shockwaves.push(Shockwave {
            pos,
            r: r + 6.0,
            speed: 260.0,
            life: 0.4,
            max_life: 0.4,
            width: 2.0,
            color,
        });
        self.spawn_burst(pos, color, 10, 150.0, 2.0);
    }

    // -- tiny effect helpers ------------------------------------------------

    pub(super) fn spawn_burst(
        &mut self,
        pos: Vec2,
        color: u32,
        count: usize,
        speed: f32,
        size: f32,
    ) {
        for _ in 0..count {
            let ang = self.rng.range(0.0, TAU);
            let sp = speed * self.rng.range(0.3, 1.0);
            let life = self.rng.range(0.18, 0.55);
            let col = lerp_color(color, C_WHITE, self.rng.range(0.0, 0.4));
            let offset = self.rng.range(0.0, 4.0);
            let p = Particle {
                pos: pos + Vec2::from_angle(ang) * offset,
                vel: Vec2::from_angle(ang) * sp,
                life,
                max_life: life,
                size: size * self.rng.range(0.6, 1.4),
                color: col,
                drag: 3.5,
                additive: self.rng.chance(0.35),
            };
            self.particles.push(p);
        }
    }

    pub(super) fn spark(&mut self, pos: Vec2, color: u32, count: usize, speed: f32) {
        self.spawn_burst(pos, color, count, speed, 1.8);
    }

    pub(super) fn popup(&mut self, pos: Vec2, text: String, color: u32, scale: i32) {
        self.popups.push(Popup {
            pos,
            vel: Vec2::new(0.0, -46.0),
            text,
            life: 0.95,
            max_life: 0.95,
            color,
            scale,
        });
    }

    pub(super) fn multiplier(&self) -> f32 {
        (1.0 + self.combo as f32 * 0.1).min(5.0)
    }
}

// ---------------------------------------------------------------------------
// High score persistence
// ---------------------------------------------------------------------------

fn high_score_path() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    Some(dir.join("neon_purge_highscore.txt"))
}

fn load_high_score() -> u32 {
    high_score_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

pub(super) fn save_high_score(value: u32) {
    if let Some(p) = high_score_path() {
        let _ = std::fs::write(p, value.to_string());
    }
}
