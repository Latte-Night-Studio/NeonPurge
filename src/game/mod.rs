//! Core game state: entities, the run/wave flow and player controls.

mod draw;
mod sim;

use std::f32::consts::TAU;

use crate::audio::{Sfx, SoundEvent};
use crate::canvas::{lerp_color, rgb};
use crate::lang::{self, Lang};
use crate::math::Vec2;
use crate::settings::{DisplayMode, Settings};
use crate::ui::{self, Action, Rect};

pub const W: i32 = 1280;
pub const H: i32 = 720;
const MAX_PENDING_SOUNDS: usize = 48;

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
    pub press_mute: bool,
    /// Left mouse button went down this frame (used by menu buttons).
    pub press_click: bool,
    /// Left mouse button is currently held (used for the pressed look).
    pub mouse_down: bool,
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

/// Which page is shown inside the current non-playing screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuPage {
    Main,
    Settings,
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

    sounds: Vec<SoundEvent>,
    hit_sfx_cd: f32,
    sound_muted: bool,

    settings: Settings,
    page: MenuPage,
    pending_display: Option<DisplayMode>,
    mouse_down: bool,
    click_guard: f32,

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
        let settings = Settings::load();
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
            sounds: Vec::new(),
            hit_sfx_cd: 0.0,
            sound_muted: !settings.sound,
            settings,
            page: MenuPage::Main,
            pending_display: None,
            mouse_down: false,
            click_guard: 0.0,
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

    pub fn drain_sounds(&mut self) -> impl Iterator<Item = SoundEvent> + '_ {
        self.sounds.drain(..)
    }

    pub fn sound_muted(&self) -> bool {
        self.sound_muted
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Timed result of a display-mode click; the window is rebuilt by `main`.
    pub fn take_display_request(&mut self) -> Option<DisplayMode> {
        self.pending_display.take()
    }

    /// Menus need a real cursor; the arena aims with the crosshair instead.
    pub fn cursor_visible(&self) -> bool {
        self.state != State::Playing
    }

    fn push_sound(&mut self, event: SoundEvent) {
        if self.sounds.len() < MAX_PENDING_SOUNDS {
            self.sounds.push(event);
        }
    }

    fn play_centered(&mut self, sfx: Sfx, gain: f32) {
        self.push_sound(SoundEvent::new(sfx, 0.0, gain));
    }

    fn play_at(&mut self, sfx: Sfx, pos: Vec2, gain: f32) {
        let pan = ((pos.x - self.player.pos.x) / (W as f32 * 0.5)).clamp(-1.0, 1.0);
        let falloff = (1.0 - pos.dist(self.player.pos) / 1100.0).clamp(0.20, 1.0);
        self.push_sound(SoundEvent::new(sfx, pan, gain * falloff));
    }

    // -- menus --------------------------------------------------------------

    fn open_settings(&mut self) {
        self.page = MenuPage::Settings;
        self.play_centered(Sfx::Ui, 0.6);
    }

    fn close_settings(&mut self) {
        self.page = MenuPage::Main;
        self.play_centered(Sfx::Ui, 0.6);
    }

    fn resume_game(&mut self) {
        self.state = State::Playing;
        self.page = MenuPage::Main;
        self.play_centered(Sfx::Ui, 0.7);
    }

    fn to_title(&mut self) {
        self.state = State::Title;
        self.page = MenuPage::Main;
        self.enemies.clear();
        self.bullets.clear();
        self.particles.clear();
        self.popups.clear();
        self.play_centered(Sfx::Ui, 0.7);
    }

    fn set_sound(&mut self, on: bool) {
        if self.settings.sound == on {
            return;
        }
        self.settings.sound = on;
        self.sound_muted = !on;
        self.settings.save();
        if on {
            self.play_centered(Sfx::Ui, 0.7);
        }
    }

    fn change_language(&mut self, lang: Lang) {
        if self.settings.lang == lang {
            return;
        }
        self.settings.lang = lang;
        self.settings.save();
        self.play_centered(Sfx::Ui, 0.6);
    }

    fn change_display(&mut self, mode: DisplayMode) {
        if self.settings.display == mode {
            return;
        }
        self.settings.display = mode;
        self.settings.save();
        self.pending_display = Some(mode);
        self.play_centered(Sfx::Ui, 0.6);
    }

    fn update_settings(&mut self, input: &Input) {
        if input.press_pause || hit(&[ui::settings_back_button()], Action::Back, input) {
            self.close_settings();
            return;
        }
        if let Some(action) = clicked_action(&ui::settings_display_buttons(), input) {
            if let Action::Display(mode) = action {
                self.change_display(mode);
            }
            return;
        }
        if let Some(action) = clicked_action(&ui::settings_language_buttons(), input) {
            if let Action::Language(lang) = action {
                self.change_language(lang);
            }
            return;
        }
        if let Some(action) = clicked_action(&ui::settings_sound_buttons(), input) {
            if let Action::Sound(on) = action {
                self.set_sound(on);
            }
        }
    }

    /// Used by the headless smoke test to exercise the death screen.
    pub fn force_death(&mut self) {
        self.player.hp = 0.0;
    }

    pub fn start_run(&mut self) {
        self.state = State::Playing;
        self.page = MenuPage::Main;
        self.quit = false;
        self.click_guard = 0.18;
        self.sounds.clear();
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
        self.hit_sfx_cd = 0.0;
        self.play_centered(Sfx::Start, 0.8);
        self.start_wave();
    }

    // -- frame update -------------------------------------------------------

    pub fn update(&mut self, dt: f32, input: &Input) {
        let dt = dt.clamp(0.0, 1.0 / 20.0);
        self.sounds.clear();
        self.time += dt;
        self.mouse = input.mouse;
        self.mouse_down = input.mouse_down;
        self.hit_sfx_cd = (self.hit_sfx_cd - dt).max(0.0);
        self.click_guard = (self.click_guard - dt).max(0.0);

        if input.press_mute {
            self.set_sound(!self.settings.sound);
        }

        // Screen effects keep fading on every screen.
        self.shake = (self.shake - dt * 26.0).max(0.0);
        self.flash = (self.flash - dt * 3.5).max(0.0);
        self.damage_tint = (self.damage_tint - dt * 3.0).max(0.0);
        if self.banner_t > 0.0 {
            self.banner_t = (self.banner_t - dt).max(0.0);
        }

        match self.state {
            State::Title => {
                if self.page == MenuPage::Settings {
                    self.update_settings(input);
                    return;
                }
                if input.press_pause {
                    self.quit = true;
                    return;
                }
                let buttons = ui::title_buttons();
                if input.press_start || hit(&buttons, Action::Start, input) {
                    self.start_run();
                } else if hit(&buttons, Action::OpenSettings, input) {
                    self.open_settings();
                } else if hit(&buttons, Action::Quit, input) {
                    self.quit = true;
                }
                return;
            }
            State::Paused => {
                if self.page == MenuPage::Settings {
                    self.update_settings(input);
                    return;
                }
                let buttons = ui::pause_buttons();
                if input.press_pause || hit(&buttons, Action::Resume, input) {
                    self.resume_game();
                } else if input.press_restart || hit(&buttons, Action::Restart, input) {
                    self.start_run();
                } else if input.press_start || hit(&buttons, Action::MainMenu, input) {
                    self.to_title();
                } else if hit(&buttons, Action::OpenSettings, input) {
                    self.open_settings();
                }
                return;
            }
            State::Dead => {
                self.death_t += dt;
                self.update_particles(dt);
                self.update_shockwaves(dt);
                self.update_popups(dt);
                if self.page == MenuPage::Settings {
                    self.update_settings(input);
                    return;
                }
                if self.death_t <= 0.55 {
                    return;
                }
                let buttons = ui::dead_buttons();
                if input.press_start || input.press_restart || hit(&buttons, Action::Restart, input) {
                    self.start_run();
                } else if input.press_pause || hit(&buttons, Action::MainMenu, input) {
                    self.to_title();
                } else if hit(&buttons, Action::OpenSettings, input) {
                    self.open_settings();
                }
                return;
            }
            State::Playing => {}
        }

        if input.press_pause {
            self.state = State::Paused;
            self.play_centered(Sfx::Ui, 0.7);
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
                    self.play_centered(Sfx::Reloaded, 0.55);
                }
            }
        }

        if input.swap {
            self.player.weapon = (self.player.weapon + 1) % WEAPONS.len();
            let col = WEAPONS[self.player.weapon].color;
            self.spark(self.player.pos, col, 6, 130.0);
            self.play_centered(Sfx::Swap, 0.55);
        }

        if input.reload {
            let w = self.player.weapon;
            if self.player.reload[w] <= 0.0 && self.player.ammo[w] < WEAPONS[w].mag {
                self.player.reload[w] = WEAPONS[w].reload_time;
                self.play_centered(Sfx::Reload, 0.50);
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
            self.play_centered(Sfx::Dash, 0.60);
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
        if input.shoot
            && self.click_guard <= 0.0
            && self.player.dash_time <= 0.0
            && self.player.reload[w] <= 0.0
        {
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
        self.play_centered(
            if wi == 0 { Sfx::Rifle } else { Sfx::Shotgun },
            if wi == 0 { 0.65 } else { 0.80 },
        );
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
            self.play_centered(Sfx::Reload, 0.50);
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
        let text = lang::text(self.settings.lang);
        self.banner = if w % 5 == 0 {
            format!("{} {} - {}", text.wave, w, text.elite)
        } else {
            format!("{} {}", text.wave, w)
        };
        self.banner_t = 2.2;
        self.flash = self.flash.max(0.15);
        if w > 1 {
            self.play_centered(Sfx::Wave, 0.55);
        }
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

/// True when the user clicked (this frame) on the button with `action`.
fn hit(buttons: &[(Rect, Action)], action: Action, input: &Input) -> bool {
    input.press_click
        && buttons
            .iter()
            .any(|(rect, kind)| *kind == action && rect.contains(input.mouse))
}

/// The action under the mouse on a fresh click, if any.
fn clicked_action(buttons: &[(Rect, Action)], input: &Input) -> Option<Action> {
    if !input.press_click {
        return None;
    }
    buttons
        .iter()
        .find(|(rect, _)| rect.contains(input.mouse))
        .map(|(_, action)| *action)
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

#[cfg(test)]
mod menu_tests {
    use super::*;

    fn click(game: &mut Game, rect: Rect) {
        let mouse = Vec2::new(
            rect.x as f32 + rect.w as f32 * 0.5,
            rect.y as f32 + rect.h as f32 * 0.5,
        );
        game.update(
            1.0 / 60.0,
            &Input {
                mouse,
                press_click: true,
                mouse_down: true,
                ..Input::default()
            },
        );
        game.update(
            1.0 / 60.0,
            &Input {
                mouse,
                ..Input::default()
            },
        );
    }

    #[test]
    fn mouse_buttons_drive_the_menus() {
        let mut game = Game::new();
        game.settings = Settings::default();
        game.sound_muted = false;
        assert_eq!(game.state, State::Title);

        click(&mut game, ui::title_buttons()[1].0);
        assert_eq!(game.page, MenuPage::Settings);

        click(&mut game, ui::settings_language_buttons()[1].0);
        assert_eq!(game.settings.lang, Lang::En);

        click(&mut game, ui::settings_display_buttons()[2].0);
        assert_eq!(game.settings.display, DisplayMode::Fullscreen);
        assert_eq!(game.take_display_request(), Some(DisplayMode::Fullscreen));

        click(&mut game, ui::settings_sound_buttons()[1].0);
        assert!(!game.settings.sound);
        assert!(game.sound_muted);

        click(&mut game, ui::settings_back_button().0);
        assert_eq!(game.page, MenuPage::Main);

        click(&mut game, ui::title_buttons()[0].0);
        assert_eq!(game.state, State::Playing);
        assert!(!game.cursor_visible());
    }

    #[test]
    fn clicks_outside_buttons_do_nothing() {
        let mut game = Game::new();
        game.settings = Settings::default();
        let blank = Input {
            mouse: Vec2::new(10.0, 10.0),
            press_click: true,
            mouse_down: true,
            ..Input::default()
        };
        game.update(1.0 / 60.0, &blank);
        assert_eq!(game.state, State::Title);
        assert_eq!(game.page, MenuPage::Main);
    }

    #[test]
    fn death_screen_offers_retry_and_menu() {
        let mut game = Game::new();
        game.settings = Settings::default();
        game.state = State::Dead;
        game.death_t = 1.0;
        click(&mut game, ui::dead_buttons()[0].0);
        assert_eq!(game.state, State::Playing);

        game.state = State::Dead;
        game.death_t = 1.0;
        click(&mut game, ui::dead_buttons()[2].0);
        assert_eq!(game.state, State::Title);
    }

    #[test]
    fn pause_settings_returns_to_the_pause_menu() {
        let mut game = Game::new();
        game.settings = Settings::default();
        game.start_run();
        game.state = State::Paused;
        click(&mut game, ui::pause_buttons()[1].0);
        assert_eq!(game.page, MenuPage::Settings);
        assert_eq!(game.state, State::Paused);
        click(&mut game, ui::settings_back_button().0);
        assert_eq!(game.page, MenuPage::Main);
        assert_eq!(game.state, State::Paused);
        click(&mut game, ui::pause_buttons()[0].0);
        assert_eq!(game.state, State::Playing);
    }
}
