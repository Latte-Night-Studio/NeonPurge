//! Platform window management that `minifb` does not expose.
//!
//! `minifb` can create a borderless window, but it has no fullscreen switch and
//! keeps the caption in its "borderless" mode. On Windows we talk to `user32`
//! directly: strip the frame, centre the chosen resolution or stretch the
//! window over the current monitor.
//!
//! The window is always *created* at its final size. `minifb` only learns about
//! a new size from `WM_SIZE`, and it ignores that message until the first
//! buffer update installs its window pointer. Resizing a fresh window after
//! creation therefore left `minifb` convinced it was still 1280x720, which made
//! fullscreen paint the frame 1:1 in the top-left corner. Creating it at the
//! right size avoids that race entirely; later mode changes rebuild the window
//! the same way.

use minifb::Window;

use crate::settings::{DisplayMode, Resolution};

/// Client-area size the window must be created with.
pub fn initial_size(mode: DisplayMode, resolution: Resolution) -> (usize, usize) {
    #[cfg(target_os = "windows")]
    {
        win32::initial_size(mode, resolution)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = mode;
        (
            resolution.width.max(320) as usize,
            resolution.height.max(240) as usize,
        )
    }
}

#[cfg(target_os = "windows")]
pub fn apply(window: &Window, mode: DisplayMode, resolution: Resolution) {
    win32::apply(window, mode, resolution);
}

#[cfg(not(target_os = "windows"))]
pub fn apply(_window: &Window, _mode: DisplayMode, _resolution: Resolution) {}

#[cfg(target_os = "windows")]
mod win32 {
    use std::ffi::c_void;

    use minifb::Window;

    use crate::settings::{DisplayMode, Resolution};

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
    const SPI_GETWORKAREA: u32 = 0x0030;
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
        fn SystemParametersInfoW(action: u32, param: u32, data: *mut c_void, update: u32) -> i32;
        fn SetForegroundWindow(hwnd: *mut c_void) -> i32;
    }

    fn screen_size() -> (i32, i32) {
        unsafe {
            (
                GetSystemMetrics(SM_CXSCREEN).max(1),
                GetSystemMetrics(SM_CYSCREEN).max(1),
            )
        }
    }

    fn work_area() -> Rect {
        let mut rect = Rect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        let ok = unsafe {
            SystemParametersInfoW(
                SPI_GETWORKAREA,
                0,
                &mut rect as *mut Rect as *mut c_void,
                0,
            )
        };
        if ok == 0 || rect.right <= rect.left || rect.bottom <= rect.top {
            let (w, h) = screen_size();
            rect = Rect {
                left: 0,
                top: 0,
                right: w,
                bottom: h,
            };
        }
        rect
    }

    /// Scale `resolution` down so it fits into `max_w` x `max_h`, keeping its
    /// aspect ratio. Never scales up: a small window stays small.
    fn fit(resolution: Resolution, max_w: i32, max_h: i32) -> (i32, i32) {
        let width = resolution.width.max(320) as f64;
        let height = resolution.height.max(240) as f64;
        let scale = (max_w as f64 / width)
            .min(max_h as f64 / height)
            .min(1.0)
            .max(0.05);
        (
            ((width * scale) as i32).max(320),
            ((height * scale) as i32).max(240),
        )
    }

    pub(super) fn initial_size(mode: DisplayMode, resolution: Resolution) -> (usize, usize) {
        match mode {
            // Fullscreen always covers the whole monitor; the saved resolution
            // is used when the player returns to a window.
            DisplayMode::Fullscreen => {
                let (w, h) = screen_size();
                (w as usize, h as usize)
            }
            // A borderless window should stay inside the desktop, above the
            // taskbar rather than under it.
            DisplayMode::Borderless => {
                let work = work_area();
                let (w, h) = fit(
                    resolution,
                    work.right - work.left,
                    work.bottom - work.top,
                );
                (w as usize, h as usize)
            }
            // The decorated frame needs a little room around the client area.
            DisplayMode::Windowed => {
                let work = work_area();
                let (w, h) = fit(
                    resolution,
                    work.right - work.left - 16,
                    work.bottom - work.top - 48,
                );
                (w as usize, h as usize)
            }
        }
    }

    pub(super) fn apply(window: &Window, mode: DisplayMode, resolution: Resolution) {
        let hwnd = window.get_window_handle();
        if hwnd.is_null() {
            return;
        }

        unsafe {
            match mode {
                DisplayMode::Windowed => {
                    // The fresh window already has the normal decorated style
                    // and was created with the right client size; only move it
                    // to the middle of the work area.
                    let mut rect = Rect {
                        left: 0,
                        top: 0,
                        right: 0,
                        bottom: 0,
                    };
                    if GetWindowRect(hwnd, &mut rect) != 0 {
                        let w = rect.right - rect.left;
                        let h = rect.bottom - rect.top;
                        let work = work_area();
                        let x = work.left + (work.right - work.left - w) / 2;
                        let y = work.top + (work.bottom - work.top - h) / 2;
                        SetWindowPos(
                            hwnd,
                            std::ptr::null_mut(),
                            x.max(work.left),
                            y.max(work.top),
                            0,
                            0,
                            SWP_NOSIZE | SWP_NOZORDER | SWP_SHOWWINDOW,
                        );
                    }
                }
                DisplayMode::Borderless | DisplayMode::Fullscreen => {
                    SetWindowLongW(hwnd, GWL_STYLE, (WS_POPUP | WS_VISIBLE) as i32);

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
                            let (sw, sh) = screen_size();
                            (0, 0, sw, sh)
                        }
                    } else {
                        let work = work_area();
                        let (w, h) = initial_size(mode, resolution);
                        let (w, h) = (w as i32, h as i32);
                        (
                            work.left + (work.right - work.left - w) / 2,
                            work.top + (work.bottom - work.top - h) / 2,
                            w,
                            h,
                        )
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
            // has the taskbar drawn over its bottom edge. Fullscreen keeps
            // itself above the taskbar; the other modes return to the normal
            // z-order.
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
}
