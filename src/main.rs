//! NEON PURGE - a small top-down arena shooter.
//!
//! Everything is rendered in software into a 1280x720 pixel buffer; the only
//! dependency is the windowing crate `minifb`.
//!
//! Run with `cargo run --release`. Use `--smoke` for a headless self-test.
//!
//! Release builds are linked as GUI programs, so launching the game never
//! opens a console window. The command-line test modes reattach to the
//! terminal they were started from to keep their output visible.

// Debug builds keep the console so `cargo run` and the test harness stay
// chatty; release builds are windowless.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod canvas;
mod font;
mod game;
mod lang;
mod math;
mod platform;
mod settings;
mod ui;

use std::time::{Duration, Instant};

use audio::{Audio, Sfx, SoundEvent};
use canvas::Canvas;
use game::{Game, Input, State, H, W};
use math::Vec2;
use minifb::{Key, KeyRepeat, MouseButton, MouseMode, ScaleMode, Window, WindowOptions};

fn main() {
    if std::env::args().any(|arg| arg == "--smoke") {
        ensure_console();
        smoke_test();
        return;
    }
    if std::env::args().any(|arg| arg == "--audio-test") {
        ensure_console();
        audio_test();
        return;
    }

    if let Err(err) = run() {
        report_fatal(&format!("Could not start the game: {err}"));
        std::process::exit(1);
    }
}

/// Release builds have no console of their own. When a test mode needs to
/// print, reattach to the terminal that started the process (if any).
#[cfg(target_os = "windows")]
fn ensure_console() {
    use std::ffi::c_void;

    const ATTACH_PARENT_PROCESS: u32 = 0xFFFF_FFFF;
    const STD_OUTPUT_HANDLE: u32 = 0xFFFF_FFF5;
    const STD_ERROR_HANDLE: u32 = 0xFFFF_FFF4;
    const GENERIC_READ: u32 = 0x8000_0000;
    const GENERIC_WRITE: u32 = 0x4000_0000;
    const FILE_SHARE_READ: u32 = 0x0000_0001;
    const FILE_SHARE_WRITE: u32 = 0x0000_0002;
    const OPEN_EXISTING: u32 = 3;
    const INVALID_HANDLE_VALUE: isize = -1;

    #[link(name = "kernel32")]
    extern "system" {
        fn GetStdHandle(which: u32) -> *mut c_void;
        fn SetStdHandle(which: u32, handle: *mut c_void) -> i32;
        fn AttachConsole(process_id: u32) -> i32;
        fn CreateFileW(
            name: *const u16,
            access: u32,
            share: u32,
            security: *mut c_void,
            disposition: u32,
            flags: u32,
            template: *mut c_void,
        ) -> *mut c_void;
    }

    unsafe {
        // Started with redirected or inherited handles (e.g. `cargo run`):
        // output already works, leave the handles alone.
        let existing = GetStdHandle(STD_OUTPUT_HANDLE);
        if !existing.is_null() && existing as isize != INVALID_HANDLE_VALUE {
            return;
        }

        if AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
            return;
        }

        let name: Vec<u16> = "CONOUT$".encode_utf16().chain(std::iter::once(0)).collect();
        let console = CreateFileW(
            name.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null_mut(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        );
        if console.is_null() || console as isize == INVALID_HANDLE_VALUE {
            return;
        }
        SetStdHandle(STD_OUTPUT_HANDLE, console);
        SetStdHandle(STD_ERROR_HANDLE, console);
    }
}

#[cfg(not(target_os = "windows"))]
fn ensure_console() {}

/// Fatal errors are invisible without a console, so surface them in a dialog.
#[cfg(target_os = "windows")]
fn report_fatal(message: &str) {
    use std::ffi::c_void;

    const MB_OK: u32 = 0x0000_0000;
    const MB_ICONERROR: u32 = 0x0000_0010;

    #[link(name = "user32")]
    extern "system" {
        fn MessageBoxW(
            hwnd: *mut c_void,
            text: *const u16,
            caption: *const u16,
            flags: u32,
        ) -> i32;
    }

    eprintln!("{message}");

    let wide = |s: &str| -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() };
    let text = wide(message);
    let caption = wide("NEON PURGE");
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            caption.as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}

#[cfg(not(target_os = "windows"))]
fn report_fatal(message: &str) {
    eprintln!("{message}");
}

fn run() -> Result<(), minifb::Error> {
    let mut canvas = Canvas::new(W, H);
    let mut game = Game::new();
    let mut audio = Audio::new();

    let mut window = create_window()?;
    platform::apply(&window, game.settings().display);
    let mut cursor_visible = true;

    let mut mouse = Vec2::new(W as f32 * 0.5, H as f32 * 0.5);
    let mut prev_left = false;
    let mut last = Instant::now();

    while window.is_open() && !game.quit {
        let now = Instant::now();
        let dt = (now - last).as_secs_f32().min(1.0 / 20.0);
        last = now;

        // The window can differ from the 1280x720 buffer (fullscreen), so map
        // the raw client coordinates back into buffer space. `AspectRatioStretch`
        // letterboxes the buffer, so the same scale and offsets apply to input.
        let (win_w, win_h) = window.get_size();
        if let Some((x, y)) = window.get_unscaled_mouse_pos(MouseMode::Discard) {
            if x.is_finite() && y.is_finite() && win_w > 0 && win_h > 0 {
                let scale = (win_w as f32 / W as f32).min(win_h as f32 / H as f32);
                if scale > 0.0 {
                    let off_x = (win_w as f32 - W as f32 * scale) * 0.5;
                    let off_y = (win_h as f32 - H as f32 * scale) * 0.5;
                    mouse = Vec2::new((x - off_x) / scale, (y - off_y) / scale);
                }
            }
        }

        let left = window.get_mouse_down(MouseButton::Left);
        let input = Input {
            mouse,
            up: window.is_key_down(Key::W) || window.is_key_down(Key::Up),
            down: window.is_key_down(Key::S) || window.is_key_down(Key::Down),
            left: window.is_key_down(Key::A) || window.is_key_down(Key::Left),
            right: window.is_key_down(Key::D) || window.is_key_down(Key::Right),
            shoot: left,
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
            press_click: left && !prev_left,
            mouse_down: left,
        };
        prev_left = left;

        game.update(dt, &input);
        audio.set_muted(game.sound_muted());
        for event in game.drain_sounds() {
            audio.play(event);
        }
        game.draw(&mut canvas);

        // Display-mode changes rebuild the native window; minifb has no
        // fullscreen switch of its own.
        if let Some(mode) = game.take_display_request() {
            window = create_window()?;
            platform::apply(&window, mode);
            cursor_visible = true;
            // Swallow the click that triggered the switch; the fresh window
            // reports a new press edge otherwise.
            prev_left = true;
        }

        let want_cursor = game.cursor_visible();
        if want_cursor != cursor_visible {
            window.set_cursor_visibility(want_cursor);
            cursor_visible = want_cursor;
        }

        window.update_with_buffer(&canvas.px, W as usize, H as usize)?;
    }

    Ok(())
}

fn create_window() -> Result<Window, minifb::Error> {
    let mut window = Window::new(
        "NEON PURGE",
        W as usize,
        H as usize,
        WindowOptions {
            resize: false,
            scale_mode: ScaleMode::AspectRatioStretch,
            ..WindowOptions::default()
        },
    )?;
    window.set_target_fps(60);
    window.set_background_color(0, 0, 0);
    Ok(window)
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
