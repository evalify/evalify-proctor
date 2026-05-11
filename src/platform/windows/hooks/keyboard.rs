use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
use windows::Win32::UI::WindowsAndMessaging::{CallNextHookEx, KBDLLHOOKSTRUCT, LLKHF_ALTDOWN};

const VK_BACK: u32 = 0x08;
const VK_TAB: u32 = 0x09;
const VK_RETURN: u32 = 0x0D;
const VK_SHIFT: u32 = 0x10;
const VK_CONTROL: u32 = 0x11;
const VK_MENU: u32 = 0x12;
const VK_PAUSE: u32 = 0x13;
const VK_ESCAPE: u32 = 0x1B;
const VK_SPACE: u32 = 0x20;
const VK_PRIOR: u32 = 0x21;
const VK_NEXT: u32 = 0x22;
const VK_HOME: u32 = 0x24;
const VK_LEFT: u32 = 0x25;
const VK_RIGHT: u32 = 0x27;
const VK_SNAPSHOT: u32 = 0x2C;
const VK_DELETE: u32 = 0x2E;
const VK_0: u32 = 0x30;
const VK_1: u32 = 0x31;
const VK_9: u32 = 0x39;
const VK_A: u32 = 0x41;
const VK_C: u32 = 0x43;
const VK_D: u32 = 0x44;
const VK_E: u32 = 0x45;
const VK_F: u32 = 0x46;
const VK_G: u32 = 0x47;
const VK_H: u32 = 0x48;
const VK_I: u32 = 0x49;
const VK_J: u32 = 0x4A;
const VK_K: u32 = 0x4B;
const VK_L: u32 = 0x4C;
const VK_N: u32 = 0x4E;
const VK_O: u32 = 0x4F;
const VK_P: u32 = 0x50;
const VK_Q: u32 = 0x51;
const VK_R: u32 = 0x52;
const VK_S: u32 = 0x53;
const VK_T: u32 = 0x54;
const VK_U: u32 = 0x55;
const VK_V: u32 = 0x56;
const VK_W: u32 = 0x57;
const VK_X: u32 = 0x58;
const VK_F1: u32 = 0x70;
const VK_F4: u32 = 0x73;
const VK_F5: u32 = 0x74;
const VK_F6: u32 = 0x75;
const VK_F10: u32 = 0x79;
const VK_F11: u32 = 0x7A;
const VK_F12: u32 = 0x7B;
const VK_LWIN: u32 = 0x5B;
const VK_RWIN: u32 = 0x5C;
const VK_APPS: u32 = 0x5D;

pub(super) unsafe extern "system" fn proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let info = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        if is_blocked_key(info) {
            return LRESULT(1);
        }
    }

    CallNextHookEx(None, code, wparam, lparam)
}

fn is_blocked_key(info: &KBDLLHOOKSTRUCT) -> bool {
    let vk = info.vkCode;
    let modifiers = Modifiers::capture(info);

    if is_windows_key(vk) || modifiers.win {
        return true;
    }

    if vk == VK_SNAPSHOT || vk == VK_PAUSE || vk == VK_APPS {
        return true;
    }

    if matches!(vk, VK_F1 | VK_F6 | VK_F10 | VK_F11 | VK_F12) {
        return true;
    }

    if modifiers.alt && blocks_shell_alt_shortcut(vk) {
        return true;
    }

    if modifiers.ctrl && blocks_shell_ctrl_shortcut(vk, modifiers) {
        return true;
    }

    if modifiers.shift && matches!(vk, VK_F10 | VK_ESCAPE) {
        return true;
    }

    false
}

fn blocks_shell_alt_shortcut(vk: u32) -> bool {
    matches!(
        vk,
        VK_TAB | VK_ESCAPE | VK_F4 | VK_SPACE | VK_RETURN | VK_LEFT | VK_RIGHT | VK_HOME | VK_BACK
    )
}

fn blocks_shell_ctrl_shortcut(vk: u32, modifiers: Modifiers) -> bool {
    if modifiers.shift && matches!(vk, VK_ESCAPE | VK_I | VK_J | VK_C | VK_N | VK_T | VK_DELETE) {
        return true;
    }

    if (VK_1..=VK_9).contains(&vk) {
        return true;
    }

    matches!(
        vk,
        VK_TAB
            | VK_PRIOR
            | VK_NEXT
            | VK_ESCAPE
            | VK_F4
            | VK_F5
            | VK_L
            | VK_N
            | VK_T
            | VK_W
            | VK_P
            | VK_S
            | VK_O
            | VK_U
            | VK_H
            | VK_D
            | VK_E
            | VK_K
            | VK_J
            | VK_R
            | VK_F
            | VK_G
            | VK_Q
            | VK_A
            | VK_X
            | VK_V
            | VK_0
    )
}

#[derive(Clone, Copy)]
struct Modifiers {
    alt: bool,
    ctrl: bool,
    shift: bool,
    win: bool,
}

impl Modifiers {
    fn capture(info: &KBDLLHOOKSTRUCT) -> Self {
        let flags = info.flags.0;
        Self {
            alt: (flags & LLKHF_ALTDOWN.0) != 0 || key_down(VK_MENU),
            ctrl: key_down(VK_CONTROL),
            shift: key_down(VK_SHIFT),
            win: key_down(VK_LWIN) || key_down(VK_RWIN),
        }
    }
}

fn is_windows_key(vk: u32) -> bool {
    vk == VK_LWIN || vk == VK_RWIN
}

fn key_down(vk: u32) -> bool {
    unsafe { GetAsyncKeyState(vk as i32) < 0 }
}
