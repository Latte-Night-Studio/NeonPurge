//! Shared geometry for the mouse-driven menus.
//!
//! Both the update code (hit testing) and the renderer (hover / pressed state)
//! use these rectangles, so a button can never react in one place and be drawn
//! in another.

use super::W;
use crate::lang::Lang;
use crate::math::Vec2;
use crate::settings::DisplayMode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }

    pub fn contains(self, p: Vec2) -> bool {
        p.is_finite()
            && p.x >= self.x as f32
            && p.y >= self.y as f32
            && p.x < (self.x + self.w) as f32
            && p.y < (self.y + self.h) as f32
    }

    pub fn cx(self) -> i32 {
        self.x + self.w / 2
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Start,
    OpenSettings,
    Quit,
    Resume,
    Restart,
    MainMenu,
    Back,
    Display(DisplayMode),
    Language(Lang),
    Sound(bool),
}

/// Buttons on the title screen (right-hand column).
pub fn title_buttons() -> [(Rect, Action); 3] {
    let x = 740;
    let w = 430;
    let h = 56;
    [
        (Rect::new(x, 300, w, h), Action::Start),
        (Rect::new(x, 370, w, h), Action::OpenSettings),
        (Rect::new(x, 440, w, h), Action::Quit),
    ]
}

/// Buttons on the pause screen.
pub fn pause_buttons() -> [(Rect, Action); 4] {
    let x = 460;
    let w = 360;
    let h = 54;
    [
        (Rect::new(x, 300, w, h), Action::Resume),
        (Rect::new(x, 366, w, h), Action::OpenSettings),
        (Rect::new(x, 432, w, h), Action::Restart),
        (Rect::new(x, 498, w, h), Action::MainMenu),
    ]
}

/// Buttons on the death screen.
pub fn dead_buttons() -> [(Rect, Action); 3] {
    let x = 460;
    let w = 360;
    let h = 52;
    [
        (Rect::new(x, 390, w, h), Action::Restart),
        (Rect::new(x, 452, w, h), Action::OpenSettings),
        (Rect::new(x, 514, w, h), Action::MainMenu),
    ]
}

/// Display-mode selector: windowed / borderless / fullscreen.
pub fn settings_display_buttons() -> [(Rect, Action); 3] {
    let y = 205;
    let w = 280;
    let h = 56;
    [
        (
            Rect::new(200, y, w, h),
            Action::Display(DisplayMode::Windowed),
        ),
        (
            Rect::new(500, y, w, h),
            Action::Display(DisplayMode::Borderless),
        ),
        (
            Rect::new(800, y, w, h),
            Action::Display(DisplayMode::Fullscreen),
        ),
    ]
}

pub fn settings_language_buttons() -> [(Rect, Action); 2] {
    let y = 345;
    let w = 280;
    let h = 56;
    [
        (Rect::new(350, y, w, h), Action::Language(Lang::Uk)),
        (Rect::new(650, y, w, h), Action::Language(Lang::En)),
    ]
}

pub fn settings_sound_buttons() -> [(Rect, Action); 2] {
    let y = 485;
    let w = 200;
    let h = 52;
    [
        (Rect::new(420, y, w, h), Action::Sound(true)),
        (Rect::new(660, y, w, h), Action::Sound(false)),
    ]
}

pub fn settings_back_button() -> (Rect, Action) {
    (Rect::new((W - 300) / 2, 580, 300, 50), Action::Back)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::H;

    fn all_menus() -> Vec<Vec<(Rect, Action)>> {
        vec![
            title_buttons().to_vec(),
            pause_buttons().to_vec(),
            dead_buttons().to_vec(),
            settings_display_buttons().to_vec(),
            settings_language_buttons().to_vec(),
            settings_sound_buttons().to_vec(),
            vec![settings_back_button()],
        ]
    }

    #[test]
    fn buttons_stay_on_screen() {
        for menu in all_menus() {
            for (r, _) in menu {
                assert!(r.x >= 0 && r.y >= 0, "button starts off-screen: {r:?}");
                assert!(r.x + r.w <= W, "button overflows right: {r:?}");
                assert!(r.y + r.h <= H, "button overflows bottom: {r:?}");
            }
        }
    }

    #[test]
    fn buttons_inside_one_menu_do_not_overlap() {
        for menu in all_menus() {
            for i in 0..menu.len() {
                for j in (i + 1)..menu.len() {
                    let a = menu[i].0;
                    let b = menu[j].0;
                    let overlap = a.x < b.x + b.w
                        && b.x < a.x + a.w
                        && a.y < b.y + b.h
                        && b.y < a.y + a.h;
                    assert!(!overlap, "buttons overlap: {a:?} and {b:?}");
                }
            }
        }
    }

    #[test]
    fn hit_testing_uses_the_whole_rectangle() {
        let r = Rect::new(10, 20, 100, 50);
        assert!(r.contains(Vec2::new(10.0, 20.0)));
        assert!(r.contains(Vec2::new(109.9, 69.9)));
        assert!(!r.contains(Vec2::new(110.0, 40.0)));
        assert!(!r.contains(Vec2::new(40.0, 70.0)));
        assert!(!r.contains(Vec2::new(f32::NAN, 40.0)));
    }
}
