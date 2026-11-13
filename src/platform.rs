//! Platform window management that `minifb` does not expose.
//!
//! `minifb` can create a borderless window, but it has no fullscreen switch and
//! keeps the caption in its "borderless" mode. On Windows we talk to `user32`
//! directly: strip the frame and either centre a 1280x720 popup or stretch it
//! over the current monitor. The fullscreen window is also made topmost so the
//! taskbar and other windows cannot draw over it. Other platforms keep the
//! normal window.

use minifb::Window;

use crate::settings::DisplayMode;

#[cfg(target_os = "windows")]
pub fn apply(window: &Window, mode: DisplayMode) {
    use std::ffi::c_void;

    #[repr(C)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[repr(C)]
    struct MonitorInfo {
        cb_size: u32,
        rc_monitor: Rect,
        rc_work: Rect,
        flags: u32,
    }

    const GWL_STYLE: i32 = -16;
    const WS_POPUP: u32 = 0x8000_0000;
    const WS_VISIBLE: u32 = 0x1000_0000;
    const SWP_NOSIZE: u32 = 0x0001;
    const SWP_NOMOVE: u32 = 0x0002;
    const SWP_NOZORDER: u32 = 0x0004;
    const SWP_NOACTIVATE: u32 = 0x0010;
    const SWP_FRAMECHANGED: u32 = 0x0020;
    const SWP_SHOWWINDOW: u32 = 0x0040;
    const MONITOR_DEFAULTTONEAREST: u32 = 2;
    const SM_CXSCREEN: i32 = 0;
    const SM_CYSCREEN: i32 = 1;
    const HWND_TOPMOST: *mut c_void = -1isize as *mut c_void;
    const HWND_NOTOPMOST: *mut c_void = -2isize as *mut c_void;

    #[link(name = "user32")]
    extern "system" {
        fn SetWindowLongW(hwnd: *mut c_void, index: i32, value: i32) -> i32;
        fn SetWindowPos(
            hwnd: *mut c_void,
            after: *mut c_void,
            x: i32,
            y: i32,
            cx: i32,
            cy: i32,
            flags: u32,
        ) -> i32;
        fn GetWindowRect(hwnd: *mut c_void, rect: *mut Rect) -> i32;
        fn GetSystemMetrics(index: i32) -> i32;
        fn MonitorFromWindow(hwnd: *mut c_void, flags: u32) -> *mut c_void;
        fn GetMonitorInfoW(monitor: *mut c_void, info: *mut MonitorInfo) -> i32;
        fn SetForegroundWindow(hwnd: *mut c_void) -> i32;
    }

    let hwnd = window.get_window_handle();
    if hwnd.is_null() {
        return;
    }

    unsafe {
        match mode {
            DisplayMode::Windowed => {
                // The fresh window already has the normal decorated style;
                // just put it in the middle of the screen.
                let mut rect = Rect {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                };
                if GetWindowRect(hwnd, &mut rect) != 0 {
                    let w = rect.right - rect.left;
                    let h = rect.bottom - rect.top;
                    let x = (GetSystemMetrics(SM_CXSCREEN) - w) / 2;
                    let y = (GetSystemMetrics(SM_CYSCREEN) - h) / 2;
                    SetWindowPos(
                        hwnd,
                        std::ptr::null_mut(),
                        x,
                        y,
                        0,
                        0,
                        SWP_NOSIZE | SWP_NOZORDER | SWP_SHOWWINDOW,
                    );
                }
            }
            DisplayMode::Borderless | DisplayMode::Fullscreen => {
                SetWindowLongW(hwnd, GWL_STYLE, (WS_POPUP | WS_VISIBLE) as i32);

                let screen_w = GetSystemMetrics(SM_CXSCREEN);
                let screen_h = GetSystemMetrics(SM_CYSCREEN);
                let (x, y, w, h) = if mode == DisplayMode::Fullscreen {
                    let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
                    let mut info = MonitorInfo {
                        cb_size: std::mem::size_of::<MonitorInfo>() as u32,
                        rc_monitor: Rect {
                            left: 0,
                            top: 0,
                            right: 0,
                            bottom: 0,
                        },
                        rc_work: Rect {
                            left: 0,
                            top: 0,
                            right: 0,
                            bottom: 0,
                        },
                        flags: 0,
                    };
                    if !monitor.is_null() && GetMonitorInfoW(monitor, &mut info) != 0 {
                        (
                            info.rc_monitor.left,
                            info.rc_monitor.top,
                            info.rc_monitor.right - info.rc_monitor.left,
                            info.rc_monitor.bottom - info.rc_monitor.top,
                        )
                    } else {
                        (0, 0, screen_w, screen_h)
                    }
                } else {
                    let w = crate::game::W;
                    let h = crate::game::H;
                    ((screen_w - w) / 2, (screen_h - h) / 2, w, h)
                };

                SetWindowPos(
                    hwnd,
                    std::ptr::null_mut(),
                    x,
                    y,
                    w,
                    h,
                    SWP_FRAMECHANGED | SWP_SHOWWINDOW | SWP_NOZORDER,
                );
            }
        }

        // The taskbar is a topmost window, so a plain borderless popup still
        // has the taskbar drawn over its bottom edge. Fullscreen keeps itself
        // above the taskbar; the other modes return to the normal z-order.
        let insert_after = if mode == DisplayMode::Fullscreen {
            HWND_TOPMOST
        } else {
            HWND_NOTOPMOST
        };
        SetWindowPos(
            hwnd,
            insert_after,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
        SetForegroundWindow(hwnd);
    }
}

#[cfg(not(target_os = "windows"))]
pub fn apply(_window: &Window, _mode: DisplayMode) {}
