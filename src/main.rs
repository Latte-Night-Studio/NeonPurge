//! NEON PURGE - a small top-down arena shooter.
//!
//! Everything is rendered in software into a 1280x720 pixel buffer; the only
//! dependency is the windowing crate `minifb`.
//!
//! Run with `cargo run --release`. Use `--smoke` for a headless self-test.

mod audio;
mod canvas;
mod font;
mod game;
mod math;

use std::time::{Duration, Instant};

use audio::{Audio, Sfx, SoundEvent};
use canvas::Canvas;
use game::{Game, Input, State, H, W};
use math::Vec2;
use minifb::{Key, KeyRepeat, MouseButton, MouseMode, Window, WindowOptions};

fn main() {
    if std::env::args().any(|arg| arg == "--smoke") {
        smoke_test();
        return;
    }
    if std::env::args().any(|arg| arg == "--audio-test") {
        audio_test();
        return;
    }

    if let Err(err) = run() {
        eprintln!("Could not start the game: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), minifb::Error> {
    let mut window = Window::new(
        "NEON PURGE",
        W as usize,
        H as usize,
        WindowOptions {
            resize: false,
            ..WindowOptions::default()
        },
    )?;
    window.set_target_fps(60);
    window.set_cursor_visibility(false);

    let mut canvas = Canvas::new(W, H);
    let mut game = Game::new();
    let mut audio = Audio::new();
    let mut mouse = Vec2::new(W as f32 * 0.5, H as f32 * 0.5);
    let mut last = Instant::now();

    while window.is_open() && !game.quit {
        let now = Instant::now();
        let dt = (now - last).as_secs_f32().min(1.0 / 20.0);
        last = now;

        if let Some((x, y)) = window.get_mouse_pos(MouseMode::Discard) {
            if x.is_finite() && y.is_finite() {
                mouse = Vec2::new(x, y);
            }
        }

        let input = Input {
            mouse,
            up: window.is_key_down(Key::W) || window.is_key_down(Key::Up),
            down: window.is_key_down(Key::S) || window.is_key_down(Key::Down),
            left: window.is_key_down(Key::A) || window.is_key_down(Key::Left),
            right: window.is_key_down(Key::D) || window.is_key_down(Key::Right),
            shoot: window.get_mouse_down(MouseButton::Left),
            dash: window.is_key_down(Key::LeftShift)
                || window.is_key_down(Key::RightShift)
                || window.is_key_down(Key::Space),
            reload: window.is_key_pressed(Key::R, KeyRepeat::No),
            swap: window.is_key_pressed(Key::Q, KeyRepeat::No),
            press_start: window.is_key_pressed(Key::Enter, KeyRepeat::No)
                || window.is_key_pressed(Key::Space, KeyRepeat::No),
            press_pause: window.is_key_pressed(Key::P, KeyRepeat::No)
                || window.is_key_pressed(Key::Escape, KeyRepeat::No),
            press_restart: window.is_key_pressed(Key::R, KeyRepeat::No),
            press_mute: window.is_key_pressed(Key::M, KeyRepeat::No),
        };

        game.update(dt, &input);
        audio.set_muted(game.sound_muted());
        for event in game.drain_sounds() {
            audio.play(event);
        }
        game.draw(&mut canvas);

        window.update_with_buffer(&canvas.px, W as usize, H as usize)?;
    }

    Ok(())
}

/// Plays every effect once, panning from left to right. Useful for checking
/// that the machine actually has a working output device.
fn audio_test() {
    let mut audio = Audio::new();
    if !audio.available() {
        println!("audio test: no output device available");
        return;
    }

    let effects = [
        Sfx::Rifle,
        Sfx::Shotgun,
        Sfx::EnemyShot,
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
        Sfx::Ui,
        Sfx::Start,
        Sfx::GameOver,
    ];
    for (i, sfx) in effects.iter().enumerate() {
        let pan = if effects.len() <= 1 {
            0.0
        } else {
            i as f32 / (effects.len() - 1) as f32 * 2.0 - 1.0
        };
        audio.play(SoundEvent::new(*sfx, pan, 0.8));
        println!("playing {sfx:?}");
        std::thread::sleep(Duration::from_millis(420));
    }
    println!("audio test ok: {} effects", effects.len());
}

/// Headless test: drives the game with synthetic input and renders every
/// frame, so logic and drawing code are exercised without a window.
fn smoke_test() {
    let dt = 1.0 / 60.0;
    let mut game = Game::new();
    let mut canvas = Canvas::new(W, H);

    let mut max_score = 0;
    let mut saw_dead = false;
    let mut was_dead = false;
    let mut restarted_after_death = false;
    let mut sound_events = 0usize;
    let mut frames = 0usize;

    for frame in 0..2700 {
        let mut input = Input {
            mouse: Vec2::new(W as f32 * 0.5, H as f32 * 0.5),
            ..Input::default()
        };

        if frame == 0 {
            input.press_start = true;
        }
        if game.state() == State::Title && frame > 0 {
            input.press_start = true;
        }
        if let Some(target) = game.nearest_enemy() {
            input.mouse = target;
        }

        // Simple wandering bot.
        let phase = (frame / 45) % 4;
        input.left = phase == 0 || phase == 3;
        input.right = phase == 1;
        input.up = phase == 2;
        input.down = phase == 2 && frame % 120 < 45;
        input.shoot = frame % 240 < 200;
        input.dash = frame % 150 == 0;
        input.reload = frame % 300 == 0;
        input.swap = frame % 420 == 0;

        if frame == 900 && game.state() == State::Playing {
            input.press_pause = true;
        }
        if frame == 960 && game.state() == State::Paused {
            input.press_pause = true;
        }
        if game.state() == State::Dead && frame % 90 == 0 {
            input.press_restart = true;
        }
        if frame == 1800 {
            game.force_death();
        }

        game.update(dt, &input);
        game.draw(&mut canvas);
        sound_events += game.drain_sounds().count();

        let (score, _wave, _kills, _hp) = game.stats();
        max_score = max_score.max(score);
        match game.state() {
            State::Dead => {
                saw_dead = true;
                was_dead = true;
            }
            State::Playing => {
                if was_dead {
                    restarted_after_death = true;
                    was_dead = false;
                }
            }
            _ => {}
        }
        frames += 1;
    }

    assert!(
        canvas.px.iter().any(|&p| p != 0),
        "renderer produced an empty frame"
    );
    assert!(saw_dead, "death screen was never reached");
    assert!(restarted_after_death, "could not restart after dying");
    assert!(max_score > 0, "bot never scored a kill");
    assert!(sound_events > 0, "game never emitted a sound event");

    let (score, wave, kills, hp) = game.stats();
    println!(
        "smoke test ok: {frames} frames, bot best score {max_score}, final run score {score}, wave {wave}, kills {kills}, hp {hp:.1}, sounds {sound_events}"
    );
}
