//! Touch controls for the Android build.
//!
//! The desktop game is played with WASD and a mouse. On a phone the same
//! `Input` struct is driven by two virtual sticks and a handful of on-screen
//! buttons, all laid out in the game's 1280x720 buffer space so hit testing and
//! drawing can never disagree.

use android_activity::input::{MotionAction, MotionEvent};

use crate::canvas::{rgb, Canvas};
use crate::game::{Game, Input, State, H, W, WEAPON_COUNT};
use crate::lang;
use crate::math::Vec2;

/// Radius of a virtual stick, in buffer pixels.
pub const STICK_RADIUS: f32 = 120.0;
/// Below this fraction of the stick radius the stick counts as centred.
const DEAD_ZONE: f32 = 0.20;
/// Contacts that start left of this line drive movement, the rest aim.
const SPLIT_X: f32 = 500.0;

const LEFT_CENTER: Vec2 = Vec2::new(215.0, 490.0);
const RIGHT_CENTER: Vec2 = Vec2::new(930.0, 495.0);

const PAUSE_CENTER: Vec2 = Vec2::new(1218.0, 150.0);
const PAUSE_RADIUS: f32 = 30.0;

const ACTION_RADIUS: f32 = 41.0;
const RELOAD_CENTER: Vec2 = Vec2::new(1178.0, 280.0);
const DASH_CENTER: Vec2 = Vec2::new(1178.0, 380.0);
const SWAP_CENTER: Vec2 = Vec2::new(1178.0, 480.0);

const SLOT_W: i32 = 54;
const SLOT_H: i32 = 46;
const SLOT_GAP: i32 = 4;
const SLOT_Y: i32 = 8;

const AIM_COLOR: u32 = rgb(255, 0, 110);
const MOVE_COLOR: u32 = rgb(0, 255, 220);
const UI_COLOR: u32 = rgb(120, 210, 255);
const WEAPON_COLORS: [u32; 9] = [
    rgb(130, 230, 255),
    rgb(90, 200, 255),
    rgb(255, 170, 80),
    rgb(120, 255, 150),
    rgb(255, 120, 220),
    rgb(255, 220, 90),
    rgb(255, 90, 90),
    rgb(180, 120, 255),
    rgb(200, 255, 255),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Reload,
    Dash,
    Swap,
    Pause,
    Weapon(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    Menu,
    LeftStick,
    RightStick,
    Button(Action),
    /// Extra contacts are tracked only so they are released cleanly.
    Ignored,
}

struct Pointer {
    id: i32,
    role: Role,
}

struct Stick {
    origin: Vec2,
    pos: Vec2,
}

pub struct Touch {
    pointers: Vec<Pointer>,
    left: Option<Stick>,
    right: Option<Stick>,
    menu_pointer: Option<i32>,

    /// Tap position and press edge for the menus.
    mouse: Vec2,
    mouse_down: bool,
    press_click: bool,

    press_pause: bool,
    press_reload: bool,
    press_swap: bool,
    press_dash: bool,
    select: Option<usize>,

    aim_dir: Vec2,

    /// Letterbox transform from screen pixels to buffer pixels.
    scale: f32,
    offset: Vec2,
    screen_w: f32,
    screen_h: f32,
}

impl Touch {
    pub fn new() -> Self {
        Self {
            pointers: Vec::new(),
            left: None,
            right: None,
            menu_pointer: None,
            mouse: Vec2::new(W as f32 * 0.5, 360.0),
            mouse_down: false,
            press_click: false,
            press_pause: false,
            press_reload: false,
            press_swap: false,
            press_dash: false,
            select: None,
            aim_dir: Vec2::new(1.0, 0.0),
            scale: 1.0,
            offset: Vec2::ZERO,
            screen_w: 0.0,
            screen_h: 0.0,
        }
    }

    /// Update the mapping from the surface's pixels to the 1280x720 buffer.
    pub fn set_viewport(&mut self, screen_w: usize, screen_h: usize) {
        let sw = screen_w.max(1) as f32;
        let sh = screen_h.max(1) as f32;
        if (sw - self.screen_w).abs() < 0.5 && (sh - self.screen_h).abs() < 0.5 {
            return;
        }
        self.screen_w = sw;
        self.screen_h = sh;
        self.scale = (sw / W as f32).min(sh / crate::game::H as f32);
        if self.scale <= 0.0 {
            self.scale = 1.0;
        }
        self.offset = Vec2::new(
            (sw - W as f32 * self.scale) * 0.5,
            (sh - crate::game::H as f32 * self.scale) * 0.5,
        );
    }

    fn to_buffer(&self, x: f32, y: f32) -> Option<Vec2> {
        if x.is_finite() && y.is_finite() {
            let p = (Vec2::new(x, y) - self.offset) / self.scale;
            if p.x >= 0.0
                && p.y >= 0.0
                && p.x < W as f32
                && p.y < crate::game::H as f32
            {
                return Some(p);
            }
        }
        None
    }

    /// Feed one Android motion event. The whole pointer set arrives with every
    /// event, which keeps multi-touch handling simple and robust.
    pub fn motion(&mut self, event: &MotionEvent<'_>, state: State) {
        match event.action() {
            // The system took the gesture away (call, notification shade…):
            // drop everything, including pending taps.
            MotionAction::Cancel | MotionAction::Outside => {
                self.cancel();
                return;
            }
            // Lifting a finger must not swallow a tap that the next frame has
            // not consumed yet, so only the held state is dropped here.
            MotionAction::Up => {
                self.lift_all();
                return;
            }
            MotionAction::PointerUp => {
                // The lifted pointer is still present in `event.pointers()`,
                // so release it and skip it while syncing the rest.
                let lifted = event
                    .pointer_at_index(event.pointer_index())
                    .pointer_id();
                self.release(lifted);
                for pointer in event.pointers() {
                    let id = pointer.pointer_id();
                    if id != lifted {
                        self.sync_pointer(id, pointer.x(), pointer.y(), state);
                    }
                }
                return;
            }
            _ => {}
        }

        for pointer in event.pointers() {
            self.sync_pointer(pointer.pointer_id(), pointer.x(), pointer.y(), state);
        }
    }

    fn sync_pointer(&mut self, id: i32, x: f32, y: f32, state: State) {
        let mapped = self.to_buffer(x, y);
        if let Some(existing) = self.pointers.iter().find(|p| p.id == id) {
            let role = existing.role;
            if let Some(p) = mapped {
                self.move_pointer(role, p);
            }
        } else if let Some(p) = mapped {
            self.press(id, p, state);
        } else {
            self.pointers.push(Pointer {
                id,
                role: Role::Ignored,
            });
        }
    }

    /// Back button edge, routed to the game's pause / back handling.
    pub fn press_pause(&mut self) {
        self.press_pause = true;
    }

    /// Release everything, e.g. when the activity goes to the background.
    pub fn cancel(&mut self) {
        self.lift_all();
        self.press_click = false;
        self.press_pause = false;
        self.press_reload = false;
        self.press_swap = false;
        self.press_dash = false;
        self.select = None;
    }

    /// Drop held contacts but keep one-shot taps for the next frame.
    fn lift_all(&mut self) {
        self.pointers.clear();
        self.left = None;
        self.right = None;
        self.menu_pointer = None;
        self.mouse_down = false;
    }

    fn press(&mut self, id: i32, p: Vec2, state: State) {
        if state != State::Playing {
            if self.menu_pointer.is_none() {
                self.menu_pointer = Some(id);
                self.pointers.push(Pointer {
                    id,
                    role: Role::Menu,
                });
                self.mouse = p;
                self.mouse_down = true;
                self.press_click = true;
            } else {
                self.pointers.push(Pointer {
                    id,
                    role: Role::Ignored,
                });
            }
            return;
        }

        if p.dist(PAUSE_CENTER) <= PAUSE_RADIUS + 8.0 {
            self.press_pause = true;
            self.track(id, Role::Button(Action::Pause));
            return;
        }
        if p.dist(RELOAD_CENTER) <= ACTION_RADIUS + 6.0 {
            self.press_reload = true;
            self.track(id, Role::Button(Action::Reload));
            return;
        }
        if p.dist(DASH_CENTER) <= ACTION_RADIUS + 6.0 {
            self.press_dash = true;
            self.track(id, Role::Button(Action::Dash));
            return;
        }
        if p.dist(SWAP_CENTER) <= ACTION_RADIUS + 6.0 {
            self.press_swap = true;
            self.track(id, Role::Button(Action::Swap));
            return;
        }
        for i in 0..WEAPON_COUNT {
            if weapon_rect(i).contains(p) {
                self.select = Some(i);
                self.track(id, Role::Button(Action::Weapon(i)));
                return;
            }
        }

        if p.x < SPLIT_X {
            if self.left.is_none() {
                self.left = Some(Stick { origin: p, pos: p });
                self.track(id, Role::LeftStick);
            } else {
                self.track(id, Role::Ignored);
            }
        } else if self.right.is_none() {
            self.right = Some(Stick { origin: p, pos: p });
            self.track(id, Role::RightStick);
        } else {
            self.track(id, Role::Ignored);
        }
    }

    fn track(&mut self, id: i32, role: Role) {
        self.pointers.push(Pointer { id, role });
    }

    fn move_pointer(&mut self, role: Role, p: Vec2) {
        match role {
            Role::LeftStick => {
                if let Some(stick) = &mut self.left {
                    stick.pos = p;
                }
            }
            Role::RightStick => {
                if let Some(stick) = &mut self.right {
                    stick.pos = p;
                    let offset = stick.pos - stick.origin;
                    let clamped = offset.clamp_len(STICK_RADIUS);
                    if clamped.len() > STICK_RADIUS * DEAD_ZONE {
                        self.aim_dir = clamped.norm();
                    }
                }
            }
            Role::Menu => {
                self.mouse = p;
            }
            _ => {}
        }
    }

    fn release(&mut self, id: i32) {
        let Some(index) = self.pointers.iter().position(|p| p.id == id) else {
            return;
        };
        let role = self.pointers.remove(index).role;
        match role {
            Role::LeftStick => self.left = None,
            Role::RightStick => self.right = None,
            Role::Menu => {
                self.menu_pointer = None;
                self.mouse_down = false;
            }
            _ => {}
        }
    }

    fn action_held(&self, action: Action) -> bool {
        self.pointers
            .iter()
            .any(|pointer| pointer.role == Role::Button(action))
    }

    /// Build this frame's game input and clear one-shot edges.
    pub fn build_input(&mut self, game: &Game) -> Input {
        let mut input = Input::default();
        input.mouse = self.mouse;
        input.mouse_down = self.mouse_down;
        input.press_click = self.press_click;
        input.press_pause = self.press_pause;
        input.reload = self.press_reload;
        input.swap = self.press_swap;
        input.select = self.select.take();
        input.dash = self.press_dash || self.action_held(Action::Dash);

        if game.state() == State::Playing {
            if let Some(stick) = &self.left {
                let offset = (stick.pos - stick.origin).clamp_len(STICK_RADIUS) / STICK_RADIUS;
                let magnitude = offset.len();
                if magnitude > DEAD_ZONE {
                    let dir = offset / magnitude;
                    input.left = dir.x < -0.38;
                    input.right = dir.x > 0.38;
                    input.up = dir.y < -0.38;
                    input.down = dir.y > 0.38;
                }
            }
            if self.right.is_some() {
                input.shoot = true;
            }
            input.mouse = aim_point(game.player_pos(), self.aim_dir);
        } else {
            // Menus are pointer-driven only; drop any stick still held from
            // the previous match so it cannot fire on resume.
            self.left = None;
            self.right = None;
            self.select = None;
            input.dash = false;
        }

        self.press_click = false;
        self.press_pause = false;
        self.press_reload = false;
        self.press_swap = false;
        self.press_dash = false;
        input
    }

    /// Draw the sticks, buttons and weapon strip on top of the arena.
    pub fn draw(&self, c: &mut Canvas, text: &lang::Text, active_weapon: usize) {
        for i in 0..WEAPON_COUNT {
            let rect = weapon_rect(i);
            let active = i == active_weapon;
            let color = WEAPON_COLORS[i % WEAPON_COLORS.len()];
            c.rect_blend(rect.x, rect.y, rect.w, rect.h, rgb(8, 14, 22), 0.62);
            if active {
                c.rect_blend(rect.x, rect.y, rect.w, rect.h, color, 0.18);
            }
            c.rect_outline(
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                if active { 2 } else { 1 },
                color,
                if active { 1.0 } else { 0.45 },
            );
            let label = format!("{}", i + 1);
            let width = c.text_width(&label, 2);
            c.text(
                &label,
                rect.cx() - width / 2,
                rect.y + 14,
                2,
                if active { rgb(255, 255, 255) } else { rgb(120, 140, 165) },
                if active { 1.0 } else { 0.8 },
            );
        }

        self.draw_button(c, PAUSE_CENTER, PAUSE_RADIUS, text.pause, Action::Pause, UI_COLOR);
        self.draw_button(
            c,
            RELOAD_CENTER,
            ACTION_RADIUS,
            text.reload,
            Action::Reload,
            WEAPON_COLORS[1],
        );
        self.draw_button(c, DASH_CENTER, ACTION_RADIUS, text.dash, Action::Dash, MOVE_COLOR);
        self.draw_button(c, SWAP_CENTER, ACTION_RADIUS, text.swap, Action::Swap, AIM_COLOR);

        let left_origin = self.left.as_ref().map_or(LEFT_CENTER, |s| s.origin);
        let left_pos = self.left.as_ref().map_or(LEFT_CENTER, |s| s.pos);
        self.draw_stick(c, left_origin, left_pos, self.left.is_some(), MOVE_COLOR);

        let right_origin = self.right.as_ref().map_or(RIGHT_CENTER, |s| s.origin);
        let right_pos = self.right.as_ref().map_or(RIGHT_CENTER, |s| s.pos);
        self.draw_stick(c, right_origin, right_pos, self.right.is_some(), AIM_COLOR);
        if let Some(stick) = &self.right {
            let knob = stick.pos - stick.origin;
            let knob = knob.clamp_len(STICK_RADIUS);
            let tip = stick.origin + self.aim_dir * (STICK_RADIUS + 26.0);
            c.line(stick.origin + knob, tip, 3.0, AIM_COLOR, 0.8);
        }
    }

    fn draw_button(
        &self,
        c: &mut Canvas,
        center: Vec2,
        radius: f32,
        label: &str,
        action: Action,
        color: u32,
    ) {
        let pressed = self.action_held(action);
        c.circle(center, radius, rgb(8, 14, 22), if pressed { 0.92 } else { 0.55 });
        c.ring(
            center,
            radius,
            if pressed { 4.0 } else { 2.0 },
            color,
            if pressed { 1.0 } else { 0.7 },
        );
        let width = c.text_width(label, 1);
        c.text(
            label,
            center.x as i32 - width / 2,
            center.y as i32 - 3,
            1,
            if pressed { rgb(255, 255, 255) } else { rgb(205, 225, 240) },
            0.95,
        );
    }

    fn draw_stick(&self, c: &mut Canvas, origin: Vec2, pos: Vec2, active: bool, color: u32) {
        c.ring(origin, STICK_RADIUS, 2.0, color, if active { 0.38 } else { 0.13 });
        c.circle(origin, STICK_RADIUS * 0.16, color, if active { 0.12 } else { 0.05 });
        let knob = (pos - origin).clamp_len(STICK_RADIUS);
        let knob_pos = origin + knob;
        c.circle(knob_pos, 34.0, rgb(8, 14, 22), if active { 0.72 } else { 0.35 });
        c.ring(
            knob_pos,
            34.0,
            if active { 3.0 } else { 2.0 },
            color,
            if active { 0.95 } else { 0.30 },
        );
        c.circle(knob_pos, 7.0, color, if active { 0.9 } else { 0.35 });
    }
}

fn weapon_rect(index: usize) -> crate::ui::Rect {
    let total = WEAPON_COUNT as i32 * (SLOT_W + SLOT_GAP) - SLOT_GAP;
    let x0 = (W - total) / 2;
    crate::ui::Rect::new(x0 + index as i32 * (SLOT_W + SLOT_GAP), SLOT_Y, SLOT_W, SLOT_H)
}

/// Where the mouse-driven game should think the player is aiming.
///
/// The aim point is pushed a fixed distance along the stick direction, then
/// pulled back so the crosshair stays inside the arena. Only the distance is
/// shortened, never the direction, so firing stays true even at the walls.
fn aim_point(origin: Vec2, dir: Vec2) -> Vec2 {
    const REACH: f32 = 360.0;
    const MARGIN: f32 = 26.0;
    let mut reach = REACH;
    if dir.x > 1e-4 {
        reach = reach.min((W as f32 - MARGIN - origin.x) / dir.x);
    } else if dir.x < -1e-4 {
        reach = reach.min((MARGIN - origin.x) / dir.x);
    }
    if dir.y > 1e-4 {
        reach = reach.min((H as f32 - MARGIN - origin.y) / dir.y);
    } else if dir.y < -1e-4 {
        reach = reach.min((MARGIN - origin.y) / dir.y);
    }
    origin + dir * reach.max(0.0)
}
