//! NEON PURGE for Android.
//!
//! This crate wraps the desktop game in a `NativeActivity`: it reuses every
//! shared module (gameplay, rendering, menus, language, sound synthesis) and
//! only replaces the window, the input source and the save-file location.
//!
//! The game draws into the same 1280x720 software buffer as the desktop build.
//! Each frame is letterboxed into the `ANativeWindow` buffer, so phones with
//! unusual aspect ratios get black bars instead of stretched pixels. Touch
//! coordinates are mapped through the exact same transform.

// The shared modules are also compiled into the desktop host, which uses a
// few helpers (stats, display switching, forced death) that the Android build
// does not need. Silencing them crate-wide is more honest than scattering
// cfg_attr attributes through shared code.
#![allow(dead_code)]

#[path = "../../src/audio.rs"]
mod audio;
#[path = "../../src/canvas.rs"]
mod canvas;
#[path = "../../src/font.rs"]
mod font;
#[path = "../../src/game/mod.rs"]
mod game;
#[path = "../../src/lang.rs"]
mod lang;
#[path = "../../src/math.rs"]
mod math;
#[path = "../../src/settings.rs"]
mod settings;
#[path = "../../src/ui.rs"]
mod ui;

mod storage;
mod touch;

use std::time::{Duration, Instant};

use android_activity::input::{InputEvent, KeyAction, Keycode};
use android_activity::{AndroidApp, InputStatus, MainEvent, PollEvent};
use ndk::hardware_buffer_format::HardwareBufferFormat;
use ndk::native_window::NativeWindow;

use audio::Audio;
use canvas::Canvas;
use game::{Game, State, H, W};
use touch::Touch;

/// 60 updates per second, matching the desktop build's target frame rate.
const FRAME: Duration = Duration::from_nanos(1_000_000_000 / 60);
/// How long the event loop may sleep while the app is in the background.
const IDLE_WAIT: Duration = Duration::from_millis(100);

#[no_mangle]
fn android_main(app: AndroidApp) {
    // Settings and the high score live in the app's private data directory;
    // the shared code asks `crate::storage` for those paths.
    if let Some(dir) = app.internal_data_path() {
        storage::init(dir);
    }

    let mut game = Game::new();
    let mut canvas = Canvas::new(W, H);
    let mut audio = Audio::new();
    let mut touch = Touch::new();
    let mut presenter = Presenter::new();

    let mut window: Option<NativeWindow> = None;
    let mut surface_size = (0, 0);
    let mut paused = false;
    let mut redraw = true;
    let mut running = true;
    let mut last = Instant::now();

    while running {
        let timeout = if paused || window.is_none() {
            IDLE_WAIT
        } else {
            FRAME
        };

        app.poll_events(Some(timeout), |event| match event {
            PollEvent::Main(MainEvent::InputAvailable) => {}
            PollEvent::Main(main_event) => match main_event {
                MainEvent::InitWindow { .. } => {
                    window = app.native_window();
                    surface_size = (0, 0);
                    redraw = true;
                }
                MainEvent::TerminateWindow { .. } => {
                    window = None;
                    surface_size = (0, 0);
                }
                MainEvent::WindowResized { .. }
                | MainEvent::RedrawNeeded { .. }
                | MainEvent::ContentRectChanged { .. }
                | MainEvent::InsetsChanged { .. } => redraw = true,
                MainEvent::Pause | MainEvent::Stop | MainEvent::LostFocus => {
                    paused = true;
                    touch.cancel();
                }
                MainEvent::Resume { .. } | MainEvent::Start | MainEvent::GainedFocus => {
                    paused = false;
                    redraw = true;
                }
                MainEvent::Destroy => running = false,
                _ => {}
            },
            _ => {}
        });

        // Input events are drained even while paused so that the queue never
        // backs up; the touch layer drops contacts when the activity sleeps.
        if let Some(win) = window.as_ref() {
            sync_surface(win, &mut touch, &mut surface_size);
        }
        if let Ok(mut events) = app.input_events_iter() {
            loop {
                let read = events.next(|event| match event {
                    InputEvent::MotionEvent(motion) => {
                        touch.motion(motion, game.state());
                        InputStatus::Handled
                    }
                    InputEvent::KeyEvent(key) => {
                        if key.action() == KeyAction::Down
                            && matches!(key.key_code(), Keycode::Back | Keycode::Escape)
                        {
                            // Android has no Esc key, so Back doubles as pause,
                            // back and quit depending on where the player is.
                            if game.state() == State::Title {
                                game.quit = true;
                            } else {
                                touch.press_pause();
                            }
                            InputStatus::Handled
                        } else {
                            InputStatus::Unhandled
                        }
                    }
                    _ => InputStatus::Unhandled,
                });
                if !read {
                    break;
                }
            }
        }

        if game.quit {
            running = false;
        }
        if !running {
            break;
        }

        let Some(win) = window.as_ref() else {
            last = Instant::now();
            continue;
        };
        if paused {
            last = Instant::now();
            continue;
        }

        let now = Instant::now();
        let elapsed = now.saturating_duration_since(last);
        if elapsed < FRAME && !redraw {
            continue;
        }

        if elapsed >= FRAME {
            last = now;
            let input = touch.build_input(&game);
            game.update(elapsed.as_secs_f32().min(1.0 / 20.0), &input);
            audio.set_muted(game.sound_muted());
            for sound in game.drain_sounds() {
                audio.play(sound);
            }
            if game.quit {
                break;
            }
        }

        game.draw(&mut canvas);
        if game.state() == State::Playing {
            touch.draw(
                &mut canvas,
                lang::text(game.settings().lang),
                game.weapon_index(),
            );
        }
        presenter.present(win, &canvas);
        redraw = false;
    }
}

/// Keep the window buffer and the touch transform in sync with the surface.
fn sync_surface(window: &NativeWindow, touch: &mut Touch, size: &mut (i32, i32)) {
    let width = window.width();
    let height = window.height();
    if width <= 0 || height <= 0 || (width, height) == *size {
        return;
    }
    // R8G8B8A8 maps to the 0xRRGGBB colours the software canvas already uses.
    let _ = window.set_buffers_geometry(
        width,
        height,
        Some(HardwareBufferFormat::R8G8B8A8_UNORM),
    );
    touch.set_viewport(width as usize, height as usize);
    *size = (width, height);
}

/// Blits the 1280x720 canvas into the native window, preserving aspect ratio.
struct Presenter {
    xmap: Vec<usize>,
    ymap: Vec<usize>,
}

impl Presenter {
    fn new() -> Self {
        Self {
            xmap: Vec::new(),
            ymap: Vec::new(),
        }
    }

    fn present(&mut self, window: &NativeWindow, canvas: &Canvas) {
        let mut buffer = match window.lock(None) {
            Ok(buffer) => buffer,
            Err(_) => return,
        };
        let width = buffer.width();
        let height = buffer.height();
        let stride = buffer.stride();
        if width == 0 || height == 0 || stride < width {
            return;
        }

        let bits = buffer.bits();
        if bits.is_null() {
            return;
        }
        let pixels = unsafe { std::slice::from_raw_parts_mut(bits as *mut u32, stride * height) };

        let scale = (width as f32 / W as f32).min(height as f32 / H as f32);
        if !scale.is_finite() || scale <= 0.0 {
            return;
        }
        let dst_w = ((W as f32 * scale) as usize).clamp(1, width);
        let dst_h = ((H as f32 * scale) as usize).clamp(1, height);
        let off_x = (width - dst_w) / 2;
        let off_y = (height - dst_h) / 2;

        self.xmap.resize(dst_w, 0);
        for (x, slot) in self.xmap.iter_mut().enumerate() {
            *slot = (x * W as usize / dst_w).min(W as usize - 1);
        }
        self.ymap.resize(dst_h, 0);
        for (y, slot) in self.ymap.iter_mut().enumerate() {
            *slot = (y * H as usize / dst_h).min(H as usize - 1);
        }

        const BLACK: u32 = 0xFF00_0000;
        for row in 0..off_y {
            pixels[row * stride..(row + 1) * stride].fill(BLACK);
        }
        for row in off_y + dst_h..height {
            pixels[row * stride..(row + 1) * stride].fill(BLACK);
        }

        for (dy, &sy) in self.ymap.iter().enumerate() {
            let row = (off_y + dy) * stride;
            pixels[row..row + off_x].fill(BLACK);
            let right = row + off_x + dst_w;
            pixels[right..row + stride].fill(BLACK);

            let src = sy * W as usize;
            let dst = row + off_x;
            for (dx, &sx) in self.xmap.iter().enumerate() {
                let colour = canvas.px[src + sx];
                // The native buffer is RGBA8888 in memory: R, G, B, A bytes.
                pixels[dst + dx] = 0xFF00_0000
                    | ((colour & 0xFF) << 16)
                    | (colour & 0x00FF_00)
                    | ((colour >> 16) & 0xFF);
            }
        }
    }
}
