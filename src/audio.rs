//! Procedural sound effects.
//!
//! Every sound is synthesised from scratch at runtime: there are no audio
//! files, samples or extra crates. On Windows a small background thread mixes
//! the effects and streams them through the system `waveOut` API; on other
//! platforms the game simply runs without sound.

use std::f32::consts::{PI, TAU};

pub const SAMPLE_RATE: u32 = 44_100;
const SR: f32 = SAMPLE_RATE as f32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sfx {
    Pistol,
    Smg,
    Rifle,
    Shotgun,
    Sniper,
    Rocket,
    Explosion,
    Throw,
    Melee,
    Stealth,
    Hit,
    Kill,
    TankBoom,
    Hurt,
    Dash,
    Reload,
    Reloaded,
    Swap,
    PickupHealth,
    PickupAmmo,
    Wave,
    WaveClear,
    EnemyShot,
    Start,
    GameOver,
    Ui,
}

impl Sfx {
    fn duration(self) -> f32 {
        match self {
            Sfx::Pistol => 0.08,
            Sfx::Smg => 0.06,
            Sfx::Rifle => 0.10,
            Sfx::Shotgun => 0.34,
            Sfx::Sniper => 0.45,
            Sfx::Rocket => 0.55,
            Sfx::Explosion => 0.80,
            Sfx::Throw => 0.18,
            Sfx::Melee => 0.16,
            Sfx::Stealth => 0.22,
            Sfx::Hit => 0.07,
            Sfx::Kill => 0.22,
            Sfx::TankBoom => 0.85,
            Sfx::Hurt => 0.38,
            Sfx::Dash => 0.26,
            Sfx::Reload => 0.13,
            Sfx::Reloaded => 0.16,
            Sfx::Swap => 0.10,
            Sfx::PickupHealth => 0.28,
            Sfx::PickupAmmo => 0.28,
            Sfx::Wave => 0.70,
            Sfx::WaveClear => 0.60,
            Sfx::EnemyShot => 0.16,
            Sfx::Start => 0.45,
            Sfx::GameOver => 1.50,
            Sfx::Ui => 0.07,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SoundEvent {
    pub sfx: Sfx,
    pub pan: f32,
    pub gain: f32,
}

impl SoundEvent {
    pub fn new(sfx: Sfx, pan: f32, gain: f32) -> Self {
        Self {
            sfx,
            pan: pan.clamp(-1.0, 1.0),
            gain: gain.clamp(0.0, 1.5),
        }
    }
}

/// Owns the audio device thread. Dropping it stops the thread cleanly.
pub struct Audio {
    tx: Option<std::sync::mpsc::SyncSender<Msg>>,
    thread: Option<std::thread::JoinHandle<()>>,
    muted: bool,
    seed: u32,
}

impl Audio {
    pub fn new() -> Self {
        let (tx, rx) = std::sync::mpsc::sync_channel(192);
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        match platform::start(rx, ready_tx) {
            Some(thread) => {
                let ready = ready_rx
                    .recv_timeout(std::time::Duration::from_millis(750))
                    .unwrap_or(false);
                if ready {
                    Self {
                        tx: Some(tx),
                        thread: Some(thread),
                        muted: false,
                        seed: 0x9E37_79B9,
                    }
                } else {
                    drop(tx);
                    let _ = thread.join();
                    Self {
                        tx: None,
                        thread: None,
                        muted: false,
                        seed: 0x9E37_79B9,
                    }
                }
            }
            None => Self {
                tx: None,
                thread: None,
                muted: false,
                seed: 0x9E37_79B9,
            },
        }
    }

    pub fn play(&mut self, event: SoundEvent) {
        if self.muted || event.gain <= 0.0 {
            return;
        }
        let Some(tx) = &self.tx else { return };

        self.seed = self.seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let samples = render(event.sfx, self.seed);
        let _ = tx.try_send(Msg::Play {
            samples,
            pan: event.pan,
            gain: event.gain,
        });
    }

    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
    }

    pub fn available(&self) -> bool {
        self.tx.is_some()
    }
}

impl Drop for Audio {
    fn drop(&mut self) {
        if let Some(tx) = self.tx.take() {
            let _ = tx.try_send(Msg::Quit);
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

enum Msg {
    Play {
        samples: Vec<f32>,
        pan: f32,
        gain: f32,
    },
    Quit,
}

#[inline]
fn pan_gains(pan: f32) -> (f32, f32) {
    let angle = (pan.clamp(-1.0, 1.0) + 1.0) * PI * 0.25;
    (angle.cos(), angle.sin())
}

// ---------------------------------------------------------------------------
// Tiny software synthesiser
// ---------------------------------------------------------------------------

struct Noise(u32);

impl Noise {
    fn new(seed: u32) -> Self {
        Self(seed | 1)
    }

    #[inline]
    fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x >> 8) as f32 / 8_388_608.0 - 1.0
    }
}

#[inline]
fn advance(phase: &mut f32, freq: f32) {
    *phase += TAU * freq / SR;
    if *phase >= TAU {
        *phase -= TAU;
    }
}

#[inline]
fn osc(phase: &mut f32, freq: f32) -> f32 {
    advance(phase, freq);
    (*phase).sin()
}

#[inline]
fn square(phase: &mut f32, freq: f32) -> f32 {
    advance(phase, freq);
    if *phase < PI {
        1.0
    } else {
        -1.0
    }
}

fn render(sfx: Sfx, seed: u32) -> Vec<f32> {
    let n = (sfx.duration() * SR).round() as usize;
    let mut out = vec![0.0f32; n];
    let mut noise = Noise::new(seed);

    match sfx {
        Sfx::Pistol => pistol(&mut out, &mut noise),
        Sfx::Smg => smg(&mut out, &mut noise),
        Sfx::Rifle => rifle(&mut out, &mut noise),
        Sfx::Shotgun => shotgun(&mut out, &mut noise),
        Sfx::Sniper => sniper(&mut out, &mut noise),
        Sfx::Rocket => rocket(&mut out, &mut noise),
        Sfx::Explosion => explosion(&mut out, &mut noise),
        Sfx::Throw => throw(&mut out, &mut noise),
        Sfx::Melee => melee(&mut out, &mut noise),
        Sfx::Stealth => stealth(&mut out, &mut noise),
        Sfx::Hit => hit(&mut out, &mut noise),
        Sfx::Kill => kill(&mut out, &mut noise),
        Sfx::TankBoom => tank_boom(&mut out, &mut noise),
        Sfx::Hurt => hurt(&mut out, &mut noise),
        Sfx::Dash => dash(&mut out, &mut noise),
        Sfx::Reload => reload(&mut out, &mut noise),
        Sfx::Reloaded => reloaded(&mut out, &mut noise),
        Sfx::Swap => swap(&mut out, &mut noise),
        Sfx::PickupHealth => pickup_health(&mut out),
        Sfx::PickupAmmo => pickup_ammo(&mut out, &mut noise),
        Sfx::Wave => wave(&mut out, &mut noise),
        Sfx::WaveClear => wave_clear(&mut out),
        Sfx::EnemyShot => enemy_shot(&mut out, &mut noise),
        Sfx::Start => start(&mut out),
        Sfx::GameOver => game_over(&mut out, &mut noise),
        Sfx::Ui => ui(&mut out),
    }

    // Short fades hide the seams at both ends of every buffer.
    let fade = ((SR * 0.003) as usize).min(n / 2);
    for i in 0..fade {
        let g = i as f32 / fade as f32;
        out[i] *= g;
        out[n - 1 - i] *= g;
    }
    out
}

fn pistol(out: &mut [f32], noise: &mut Noise) {
    let mut ph = 0.0;
    for (i, s) in out.iter_mut().enumerate() {
        let t = i as f32 / SR;
        let u = t / 0.08;
        let f = 110.0 + 320.0 * (1.0 - u).powf(2.4);
        let body = osc(&mut ph, f) * (-t * 34.0).exp() * 0.44;
        let crack = noise.next() * (-t * 70.0).exp() * 0.34;
        *s = (body + crack) * 0.85;
    }
}

fn smg(out: &mut [f32], noise: &mut Noise) {
    let mut ph = 0.0;
    for (i, s) in out.iter_mut().enumerate() {
        let t = i as f32 / SR;
        let u = t / 0.06;
        let f = 130.0 + 380.0 * (1.0 - u).powf(2.0);
        let body = osc(&mut ph, f) * (-t * 46.0).exp() * 0.36;
        let tick = noise.next() * (-t * 90.0).exp() * 0.26;
        *s = (body + tick) * 0.80;
    }
}

fn sniper(out: &mut [f32], noise: &mut Noise) {
    let mut ph = 0.0;
    let mut ph2 = 0.0;
    for (i, s) in out.iter_mut().enumerate() {
        let t = i as f32 / SR;
        let u = t / 0.45;
        let f = 48.0 + 160.0 * (1.0 - u).powf(2.2);
        let body = osc(&mut ph, f) * (-t * 4.5).exp() * 0.72;
        let crack = noise.next() * (-t * 16.0).exp() * 0.55;
        let ring = osc(&mut ph2, 1850.0) * (-t * 7.0).exp() * 0.10;
        *s = (body + crack + ring) * 0.82;
    }
}

fn rocket(out: &mut [f32], noise: &mut Noise) {
    let mut ph = 0.0;
    for (i, s) in out.iter_mut().enumerate() {
        let t = i as f32 / SR;
        let u = t / 0.55;
        let f = 70.0 + 210.0 * (1.0 - u).powf(1.4);
        let launch = osc(&mut ph, f) * (-t * 5.0).exp() * 0.34;
        let whoosh = noise.next() * (-t * 4.2).exp() * 0.42;
        *s = (launch + whoosh) * 0.82;
    }
}

fn explosion(out: &mut [f32], noise: &mut Noise) {
    let mut ph = 0.0;
    for (i, s) in out.iter_mut().enumerate() {
        let t = i as f32 / SR;
        let u = t / 0.80;
        let f = 22.0 + 70.0 * (1.0 - u).powf(2.0);
        let low = osc(&mut ph, f) * (-t * 2.4).exp() * 0.90;
        let blast = noise.next() * (-t * 3.2).exp() * 0.72;
        let debris = noise.next() * (-t * 9.0).exp() * 0.22;
        *s = (low + blast + debris) * 0.78;
    }
}

fn throw(out: &mut [f32], noise: &mut Noise) {
    let mut ph = 0.0;
    for (i, s) in out.iter_mut().enumerate() {
        let t = i as f32 / SR;
        let u = t / 0.18;
        let env = (PI * u).sin().powf(1.4);
        let f = 320.0 + 520.0 * u;
        let tone = osc(&mut ph, f) * env * 0.12;
        let air = noise.next() * env * 0.22;
        *s = tone + air;
    }
}

fn melee(out: &mut [f32], noise: &mut Noise) {
    let mut ph = 0.0;
    for (i, s) in out.iter_mut().enumerate() {
        let t = i as f32 / SR;
        let u = t / 0.16;
        let env = (PI * u).sin().powf(1.2);
        let f = 900.0 + 1500.0 * u;
        let blade = osc(&mut ph, f) * env * 0.10;
        let swoosh = noise.next() * env * 0.30;
        *s = blade + swoosh;
    }
}

fn stealth(out: &mut [f32], noise: &mut Noise) {
    let mut ph = 0.0;
    for (i, s) in out.iter_mut().enumerate() {
        let t = i as f32 / SR;
        let u = t / 0.22;
        let f = 90.0 + 130.0 * (1.0 - u).powf(1.5);
        let thud = osc(&mut ph, f) * (-t * 18.0).exp() * 0.30;
        let tick = if t < 0.012 {
            noise.next() * (1.0 - t / 0.012) * 0.24
        } else {
            0.0
        };
        *s = thud + tick;
    }
}

fn rifle(out: &mut [f32], noise: &mut Noise) {
    let mut ph = 0.0;
    for (i, s) in out.iter_mut().enumerate() {
        let t = i as f32 / SR;
        let u = t / 0.10;
        let f = 65.0 + 260.0 * (1.0 - u).powf(2.2);
        let body = osc(&mut ph, f) * (-t * 19.0).exp() * 0.48;
        let crack = noise.next() * (-t * 55.0).exp() * 0.42;
        let snap = if t < 0.006 {
            noise.next() * (1.0 - t / 0.006) * 0.30
        } else {
            0.0
        };
        *s = (body + crack + snap) * 0.85;
    }
}

fn shotgun(out: &mut [f32], noise: &mut Noise) {
    let mut ph = 0.0;
    for (i, s) in out.iter_mut().enumerate() {
        let t = i as f32 / SR;
        let u = t / 0.34;
        let f = 36.0 + 125.0 * (1.0 - u).powf(2.0);
        let body = osc(&mut ph, f) * (-t * 7.0).exp() * 0.62;
        let blast = noise.next() * (-t * 10.0).exp() * 0.58;
        let grit = if noise.next() > 0.45 {
            noise.next() * (-t * 25.0).exp() * 0.14
        } else {
            0.0
        };
        *s = (body + blast + grit) * 0.80;
    }
}

fn hit(out: &mut [f32], noise: &mut Noise) {
    let mut ph = 0.0;
    for (i, s) in out.iter_mut().enumerate() {
        let t = i as f32 / SR;
        let u = t / 0.07;
        let f = 500.0 + 1000.0 * (1.0 - u);
        let tone = osc(&mut ph, f) * (-t * 55.0).exp() * 0.30;
        let tick = noise.next() * (-t * 45.0).exp() * 0.26;
        *s = tone + tick;
    }
}

fn kill(out: &mut [f32], noise: &mut Noise) {
    let mut ph = 0.0;
    for (i, s) in out.iter_mut().enumerate() {
        let t = i as f32 / SR;
        let u = t / 0.22;
        let f = 70.0 + 560.0 * (1.0 - u).powf(1.8);
        let body = square(&mut ph, f) * (-t * 11.0).exp() * 0.30;
        let burst = noise.next() * (-t * 18.0).exp() * 0.30;
        *s = body + burst;
    }
}

fn tank_boom(out: &mut [f32], noise: &mut Noise) {
    let mut ph = 0.0;
    for (i, s) in out.iter_mut().enumerate() {
        let t = i as f32 / SR;
        let u = t / 0.85;
        let f = 26.0 + 85.0 * (1.0 - u).powf(2.0);
        let low = osc(&mut ph, f) * (-t * 2.6).exp() * 0.85;
        let blast = noise.next() * (-t * 4.0).exp() * 0.70;
        let debris = noise.next() * (-t * 11.0).exp() * 0.20;
        *s = (low + blast + debris) * 0.80;
    }
}

fn hurt(out: &mut [f32], noise: &mut Noise) {
    let mut ph = 0.0;
    for (i, s) in out.iter_mut().enumerate() {
        let t = i as f32 / SR;
        let u = t / 0.38;
        let f = 55.0 + 150.0 * (1.0 - u).powf(1.2);
        let body = osc(&mut ph, f) * (-t * 7.0).exp() * 0.55;
        let hiss = noise.next() * (-t * 12.0).exp() * 0.28;
        let throb = (t * 34.0).sin();
        *s = (body * (0.75 + 0.25 * throb) + hiss) * 0.75;
    }
}

fn dash(out: &mut [f32], noise: &mut Noise) {
    let mut ph = 0.0;
    for (i, s) in out.iter_mut().enumerate() {
        let t = i as f32 / SR;
        let u = t / 0.26;
        let env = (PI * u).sin().powf(1.3);
        let f = 260.0 + 900.0 * u * u;
        let tone = osc(&mut ph, f) * env * 0.24;
        let whoosh = noise.next() * env * 0.28;
        *s = tone + whoosh;
    }
}

fn reload(out: &mut [f32], noise: &mut Noise) {
    let mut ph = 0.0;
    for (i, s) in out.iter_mut().enumerate() {
        let t = i as f32 / SR;
        let first = if t < 0.012 {
            noise.next() * (1.0 - t / 0.012) * 0.60
        } else {
            0.0
        };
        let second = if (0.070..0.082).contains(&t) {
            noise.next() * (1.0 - (t - 0.070) / 0.012) * 0.45
        } else {
            0.0
        };
        let tone = osc(&mut ph, 1300.0) * (-t * 40.0).exp() * 0.10;
        *s = first + second + tone;
    }
}

fn reloaded(out: &mut [f32], noise: &mut Noise) {
    let mut ph = 0.0;
    for (i, s) in out.iter_mut().enumerate() {
        let t = i as f32 / SR;
        let u = t / 0.16;
        let click = if t < 0.010 {
            noise.next() * (1.0 - t / 0.010) * 0.55
        } else if (0.060..0.072).contains(&t) {
            noise.next() * (1.0 - (t - 0.060) / 0.012) * 0.42
        } else {
            0.0
        };
        let ping = osc(&mut ph, 880.0 + 440.0 * u) * (-t * 22.0).exp() * 0.16;
        *s = click + ping;
    }
}

fn swap(out: &mut [f32], noise: &mut Noise) {
    let mut ph = 0.0;
    for (i, s) in out.iter_mut().enumerate() {
        let t = i as f32 / SR;
        let u = t / 0.10;
        let f = 700.0 + 600.0 * u;
        let click = if t < 0.008 {
            noise.next() * (1.0 - t / 0.008) * 0.35
        } else {
            0.0
        };
        *s = osc(&mut ph, f) * (-t * 35.0).exp() * 0.28 + click;
    }
}

fn pickup_health(out: &mut [f32]) {
    let mut ph = 0.0;
    let notes = [659.25, 880.0, 1318.5];
    for (i, s) in out.iter_mut().enumerate() {
        let t = i as f32 / SR;
        let step = (t / 0.080) as usize;
        if step < notes.len() {
            let local = t - step as f32 * 0.080;
            let env = (-local * 22.0).exp();
            *s = osc(&mut ph, notes[step]) * env * 0.36;
        }
    }
}

fn pickup_ammo(out: &mut [f32], noise: &mut Noise) {
    let mut ph = 0.0;
    for (i, s) in out.iter_mut().enumerate() {
        let t = i as f32 / SR;
        let u = t / 0.28;
        let f = 440.0 + 660.0 * u;
        let tone = osc(&mut ph, f) * (-t * 8.0).exp() * 0.30;
        let click = if t < 0.010 {
            noise.next() * (1.0 - t / 0.010) * 0.32
        } else if (0.070..0.082).contains(&t) {
            noise.next() * (1.0 - (t - 0.070) / 0.012) * 0.26
        } else {
            0.0
        };
        *s = tone + click;
    }
}

fn wave(out: &mut [f32], noise: &mut Noise) {
    let mut ph = 0.0;
    let mut ph2 = 0.0;
    for (i, s) in out.iter_mut().enumerate() {
        let t = i as f32 / SR;
        let u = t / 0.70;
        let f = 110.0 + 330.0 * u.powf(0.7);
        let env = (t / 0.02).min(1.0) * (1.0 - u).powf(1.1);
        let tone = osc(&mut ph, f) * 0.38;
        let shine = osc(&mut ph2, f * 2.0) * 0.16;
        *s = (tone + shine + noise.next() * 0.04) * env;
    }
}

fn wave_clear(out: &mut [f32]) {
    let mut ph = 0.0;
    let notes = [523.25, 659.25, 783.99, 1046.5];
    for (i, s) in out.iter_mut().enumerate() {
        let t = i as f32 / SR;
        let step = (t / 0.100) as usize;
        if step < notes.len() {
            let local = t - step as f32 * 0.100;
            let env = (-local * 14.0).exp();
            *s = osc(&mut ph, notes[step]) * env * 0.34;
        }
    }
}

fn enemy_shot(out: &mut [f32], noise: &mut Noise) {
    let mut ph = 0.0;
    let mut ph2 = 0.0;
    for (i, s) in out.iter_mut().enumerate() {
        let t = i as f32 / SR;
        let u = t / 0.16;
        let f = 300.0 + 850.0 * (1.0 - u);
        let env = (-t * 12.0).exp();
        let zap = square(&mut ph, f) * 0.20;
        let body = osc(&mut ph2, f * 0.5) * 0.18;
        *s = (zap + body + noise.next() * 0.08) * env;
    }
}

fn start(out: &mut [f32]) {
    let mut ph = 0.0;
    let mut ph2 = 0.0;
    for (i, s) in out.iter_mut().enumerate() {
        let t = i as f32 / SR;
        let u = t / 0.45;
        let f = 180.0 + 600.0 * u.powf(1.4);
        let env = (t / 0.02).min(1.0) * (1.0 - u).powf(0.8);
        let sweep = osc(&mut ph, f) * 0.34;
        let ping = if t > 0.18 {
            osc(&mut ph2, 1320.0) * (-(t - 0.18) * 10.0).exp() * 0.18
        } else {
            0.0
        };
        *s = (sweep + ping) * env;
    }
}

fn game_over(out: &mut [f32], noise: &mut Noise) {
    let mut ph = 0.0;
    let mut ph2 = 0.0;
    for (i, s) in out.iter_mut().enumerate() {
        let t = i as f32 / SR;
        let u = t / 1.50;
        let f = 55.0 + 280.0 * (1.0 - u).powf(2.0);
        let env = (t / 0.03).min(1.0) * (1.0 - u).powf(1.4);
        let low = osc(&mut ph, f) * 0.48;
        let minor = osc(&mut ph2, f * 1.2) * 0.20;
        *s = (low + minor + noise.next() * 0.07) * env;
    }
}

fn ui(out: &mut [f32]) {
    let mut ph = 0.0;
    for (i, s) in out.iter_mut().enumerate() {
        let t = i as f32 / SR;
        let u = t / 0.07;
        let f = 880.0 + 220.0 * u;
        *s = osc(&mut ph, f) * (-t * 45.0).exp() * 0.28;
    }
}

// ---------------------------------------------------------------------------
// Windows waveOut backend
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
mod platform {
    use std::ffi::c_void;
    use std::mem::size_of;
    use std::ptr::null_mut;
    use std::sync::mpsc::{Receiver, Sender, TryRecvError};
    use std::thread::{self, JoinHandle};
    use std::time::Duration;

    use super::*;

    const WAVE_MAPPER: u32 = 0xFFFF_FFFF;
    const WAVE_FORMAT_PCM: u16 = 1;
    const WHDR_DONE: u32 = 0x0000_0001;
    const CHANNELS: usize = 2;
    const BUF_FRAMES: usize = 512;
    const BUF_COUNT: usize = 3;
    const MAX_VOICES: usize = 56;
    const MASTER: f32 = 0.90;

    #[repr(C)]
    struct WaveFormatEx {
        format_tag: u16,
        channels: u16,
        samples_per_sec: u32,
        avg_bytes_per_sec: u32,
        block_align: u16,
        bits_per_sample: u16,
        cb_size: u16,
    }

    #[repr(C)]
    struct WaveHdr {
        data: *mut u8,
        buffer_length: u32,
        bytes_recorded: u32,
        user: usize,
        flags: u32,
        loops: u32,
        next: *mut WaveHdr,
        reserved: usize,
    }

    impl WaveHdr {
        fn new(buf: &mut [i16]) -> Self {
            Self {
                data: buf.as_mut_ptr() as *mut u8,
                buffer_length: (buf.len() * size_of::<i16>()) as u32,
                bytes_recorded: 0,
                user: 0,
                flags: 0,
                loops: 0,
                next: null_mut(),
                reserved: 0,
            }
        }
    }

    #[link(name = "winmm")]
    extern "system" {
        fn waveOutOpen(
            handle: *mut *mut c_void,
            device_id: u32,
            format: *const WaveFormatEx,
            callback: usize,
            instance: usize,
            flags: u32,
        ) -> u32;
        fn waveOutPrepareHeader(handle: *mut c_void, header: *mut WaveHdr, size: u32) -> u32;
        fn waveOutWrite(handle: *mut c_void, header: *mut WaveHdr, size: u32) -> u32;
        fn waveOutReset(handle: *mut c_void) -> u32;
        fn waveOutClose(handle: *mut c_void) -> u32;
        fn waveOutUnprepareHeader(handle: *mut c_void, header: *mut WaveHdr, size: u32) -> u32;
        fn waveOutGetNumDevs() -> u32;
    }

    struct Voice {
        samples: Vec<f32>,
        pos: usize,
        left: f32,
        right: f32,
    }

    pub fn start(rx: Receiver<Msg>, ready: Sender<bool>) -> Option<JoinHandle<()>> {
        // Do not spawn a useless thread when Windows reports no output devices.
        if unsafe { waveOutGetNumDevs() } == 0 {
            let _ = ready.send(false);
            return None;
        }
        Some(thread::spawn(move || audio_loop(rx, ready)))
    }

    fn audio_loop(rx: Receiver<Msg>, ready: Sender<bool>) {
        unsafe {
            let format = WaveFormatEx {
                format_tag: WAVE_FORMAT_PCM,
                channels: CHANNELS as u16,
                samples_per_sec: SAMPLE_RATE,
                avg_bytes_per_sec: SAMPLE_RATE * CHANNELS as u32 * 2,
                block_align: (CHANNELS * 2) as u16,
                bits_per_sample: 16,
                cb_size: 0,
            };

            let mut handle: *mut c_void = null_mut();
            if waveOutOpen(&mut handle, WAVE_MAPPER, &format, 0, 0, 0) != 0 || handle.is_null() {
                let _ = ready.send(false);
                return;
            }

            let mut buffers: Vec<Vec<i16>> = (0..BUF_COUNT)
                .map(|_| vec![0i16; BUF_FRAMES * CHANNELS])
                .collect();
            let mut headers: Vec<Box<WaveHdr>> = buffers
                .iter_mut()
                .map(|buf| Box::new(WaveHdr::new(buf)))
                .collect();
            let header_size = size_of::<WaveHdr>() as u32;
            let mut voices: Vec<Voice> = Vec::new();

            // Prime the queue with silence so playback starts immediately.
            for (buf, header) in buffers.iter_mut().zip(headers.iter_mut()) {
                fill_buffer(buf, &mut voices);
                if waveOutPrepareHeader(handle, &mut **header, header_size) != 0
                    || waveOutWrite(handle, &mut **header, header_size) != 0
                {
                    let _ = ready.send(false);
                    shutdown(handle, &mut headers);
                    return;
                }
            }
            let _ = ready.send(true);

            'run: loop {
                loop {
                    match rx.try_recv() {
                        Ok(Msg::Play {
                            samples,
                            pan,
                            gain,
                        }) => {
                            if samples.is_empty() {
                                continue;
                            }
                            if voices.len() >= MAX_VOICES {
                                voices.remove(0);
                            }
                            let (left, right) = pan_gains(pan);
                            voices.push(Voice {
                                samples,
                                pos: 0,
                                left: left * gain,
                                right: right * gain,
                            });
                        }
                        Ok(Msg::Quit) | Err(TryRecvError::Disconnected) => break 'run,
                        Err(TryRecvError::Empty) => break,
                    }
                }

                for i in 0..headers.len() {
                    if headers[i].flags & WHDR_DONE == 0 {
                        continue;
                    }
                    if waveOutUnprepareHeader(handle, &mut *headers[i], header_size) != 0 {
                        continue;
                    }
                    headers[i].flags = 0;
                    fill_buffer(&mut buffers[i], &mut voices);
                    if waveOutPrepareHeader(handle, &mut *headers[i], header_size) != 0 {
                        continue;
                    }
                    let _ = waveOutWrite(handle, &mut *headers[i], header_size);
                }

                thread::sleep(Duration::from_millis(2));
            }

            shutdown(handle, &mut headers);
        }
    }

    unsafe fn shutdown(handle: *mut c_void, headers: &mut [Box<WaveHdr>]) {
        unsafe {
            waveOutReset(handle);
            let header_size = size_of::<WaveHdr>() as u32;
            for header in headers {
                let _ = waveOutUnprepareHeader(handle, &mut **header, header_size);
            }
            waveOutClose(handle);
        }
    }

    fn fill_buffer(buf: &mut [i16], voices: &mut Vec<Voice>) {
        for frame in buf.chunks_exact_mut(2) {
            let mut left = 0.0f32;
            let mut right = 0.0f32;
            for voice in voices.iter_mut() {
                if let Some(sample) = voice.samples.get(voice.pos) {
                    left += *sample * voice.left;
                    right += *sample * voice.right;
                }
                voice.pos += 1;
            }
            frame[0] = to_i16(left);
            frame[1] = to_i16(right);
        }
        voices.retain(|voice| voice.pos < voice.samples.len());
    }

    #[inline]
    fn to_i16(sample: f32) -> i16 {
        ((sample * MASTER).tanh() * 32767.0) as i16
    }
}

// ---------------------------------------------------------------------------
// Android AAudio backend
// ---------------------------------------------------------------------------

#[cfg(target_os = "android")]
mod platform {
    use std::os::raw::c_void;
    use std::sync::mpsc::{Receiver, Sender, TryRecvError};
    use std::thread::{self, JoinHandle};
    use std::time::Duration;

    use ndk::audio::{
        AudioDirection, AudioFormat, AudioPerformanceMode, AudioSharingMode, AudioStreamBuilder,
    };

    use super::*;

    const CHANNELS: usize = 2;
    const BUF_FRAMES: usize = 256;
    const MAX_VOICES: usize = 56;
    const MASTER: f32 = 0.90;

    struct Voice {
        samples: Vec<f32>,
        pos: usize,
        left: f32,
        right: f32,
    }

    pub fn start(rx: Receiver<Msg>, ready: Sender<bool>) -> Option<JoinHandle<()>> {
        Some(thread::spawn(move || audio_loop(rx, ready)))
    }

    fn audio_loop(rx: Receiver<Msg>, ready: Sender<bool>) {
        // AAudio converts 44.1 kHz to the device rate in the mixer. If the
        // stream cannot be opened the game simply runs without sound.
        let opened = AudioStreamBuilder::new().and_then(|builder| {
            builder
                .direction(AudioDirection::Output)
                .format(AudioFormat::PCM_Float)
                .channel_count(CHANNELS as i32)
                .sample_rate(SAMPLE_RATE as i32)
                .performance_mode(AudioPerformanceMode::LowLatency)
                .sharing_mode(AudioSharingMode::Shared)
                .buffer_capacity_in_frames((BUF_FRAMES * 8) as i32)
                .open_stream()
        });
        let stream = match opened {
            Ok(stream) => stream,
            Err(_) => {
                let _ = ready.send(false);
                return;
            }
        };
        if stream.request_start().is_err() {
            let _ = ready.send(false);
            return;
        }
        let _ = stream.set_buffer_size_in_frames((BUF_FRAMES * 2) as i32);
        let _ = ready.send(true);

        let mut voices: Vec<Voice> = Vec::new();
        let mut buf = vec![0.0f32; BUF_FRAMES * CHANNELS];

        'run: loop {
            loop {
                match rx.try_recv() {
                    Ok(Msg::Play {
                        samples,
                        pan,
                        gain,
                    }) => {
                        if samples.is_empty() {
                            continue;
                        }
                        if voices.len() >= MAX_VOICES {
                            voices.remove(0);
                        }
                        let (left, right) = pan_gains(pan);
                        voices.push(Voice {
                            samples,
                            pos: 0,
                            left: left * gain,
                            right: right * gain,
                        });
                    }
                    Ok(Msg::Quit) | Err(TryRecvError::Disconnected) => break 'run,
                    Err(TryRecvError::Empty) => break,
                }
            }

            fill_buffer(&mut buf, &mut voices);
            // 100 ms is long enough to ride out scheduling hiccups without
            // stalling the game if the device stops consuming audio.
            let written = unsafe {
                stream.write(
                    buf.as_ptr() as *const c_void,
                    BUF_FRAMES as i32,
                    100_000_000,
                )
            };
            if written.is_err() {
                thread::sleep(Duration::from_millis(5));
            }
        }

        let _ = stream.request_stop();
    }

    fn fill_buffer(buf: &mut [f32], voices: &mut Vec<Voice>) {
        for frame in buf.chunks_exact_mut(CHANNELS) {
            let mut left = 0.0f32;
            let mut right = 0.0f32;
            for voice in voices.iter_mut() {
                if let Some(sample) = voice.samples.get(voice.pos) {
                    left += *sample * voice.left;
                    right += *sample * voice.right;
                }
                voice.pos += 1;
            }
            frame[0] = (left * MASTER).tanh();
            frame[1] = (right * MASTER).tanh();
        }
        voices.retain(|voice| voice.pos < voice.samples.len());
    }
}

#[cfg(not(any(target_os = "windows", target_os = "android")))]
mod platform {
    use std::sync::mpsc::{Receiver, Sender};
    use std::thread::JoinHandle;

    use super::Msg;

    pub fn start(_rx: Receiver<Msg>, ready: Sender<bool>) -> Option<JoinHandle<()>> {
        let _ = ready.send(false);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Sfx; 26] = [
        Sfx::Pistol,
        Sfx::Smg,
        Sfx::Rifle,
        Sfx::Shotgun,
        Sfx::Sniper,
        Sfx::Rocket,
        Sfx::Explosion,
        Sfx::Throw,
        Sfx::Melee,
        Sfx::Stealth,
        Sfx::Hit,
        Sfx::Kill,
        Sfx::TankBoom,
        Sfx::Hurt,
        Sfx::Dash,
        Sfx::Reload,
        Sfx::Reloaded,
        Sfx::Swap,
        Sfx::PickupHealth,
        Sfx::PickupAmmo,
        Sfx::Wave,
        Sfx::WaveClear,
        Sfx::EnemyShot,
        Sfx::Start,
        Sfx::GameOver,
        Sfx::Ui,
    ];

    #[test]
    fn every_effect_is_audible_and_finite() {
        for (i, sfx) in ALL.iter().enumerate() {
            let buf = render(*sfx, 0x1234_5678 + i as u32);
            assert!(!buf.is_empty(), "{sfx:?} rendered nothing");
            assert!(
                buf.iter().all(|s| s.is_finite()),
                "{sfx:?} produced a non-finite sample"
            );
            let peak = buf.iter().fold(0.0f32, |peak, s| peak.max(s.abs()));
            assert!(peak > 0.01, "{sfx:?} is effectively silent");
        }
    }

    #[test]
    fn pan_law_centres_and_splits() {
        let (left, right) = pan_gains(0.0);
        assert!((left - right).abs() < 1e-5);
        assert!((left - 0.707_106_77).abs() < 1e-4);

        let (left, right) = pan_gains(-1.0);
        assert!(left > 0.99 && right < 0.01);

        let (left, right) = pan_gains(1.0);
        assert!(left < 0.01 && right > 0.99);
    }
}
