use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, WM_MBUTTONDBLCLK, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_RBUTTONDBLCLK,
    WM_RBUTTONDOWN, WM_RBUTTONUP, WM_XBUTTONDBLCLK, WM_XBUTTONDOWN, WM_XBUTTONUP,
};

pub(super) unsafe extern "system" fn proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && is_blocked_mouse_message(wparam.0 as u32) {
        return LRESULT(1);
    }

    CallNextHookEx(None, code, wparam, lparam)
}

fn is_blocked_mouse_message(message: u32) -> bool {
    matches!(
        message,
        WM_RBUTTONDOWN
            | WM_RBUTTONUP
            | WM_RBUTTONDBLCLK
            | WM_MBUTTONDOWN
            | WM_MBUTTONUP
            | WM_MBUTTONDBLCLK
            | WM_XBUTTONDOWN
            | WM_XBUTTONUP
            | WM_XBUTTONDBLCLK
    )
}
