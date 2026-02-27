use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use anyhow::Result;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, PostThreadMessageW, SetWindowsHookExW,
    UnhookWindowsHookEx, KBDLLHOOKSTRUCT, LLKHF_ALTDOWN,
    WH_KEYBOARD_LL, WH_MOUSE_LL, WM_QUIT, WM_RBUTTONDOWN, WM_RBUTTONUP,
    MSG,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_LWIN, VK_RWIN,
};

// Virtual Key Constants
const VK_TAB: u32 = 0x09;
const VK_V: u32 = 0x56;
const VK_SNAPSHOT: u32 = 0x2C;
const VK_ESCAPE: u32 = 0x1B;

fn is_blocked_key(info: &KBDLLHOOKSTRUCT) -> bool {
    let vk = info.vkCode;
    let flags = info.flags.0;
    let alt = (flags & LLKHF_ALTDOWN.0) != 0;
    let win_held = || unsafe {
        GetAsyncKeyState(VK_LWIN.0 as i32) < 0
            || GetAsyncKeyState(VK_RWIN.0 as i32) < 0
    };
    // Alt + Tab — window switcher
    if vk == VK_TAB && alt { return true; }
    // Alt + Escape — old-style window cycle
    if vk == VK_ESCAPE && alt { return true; }
    // Win + V — clipboard history
    if vk == VK_V && win_held() { return true; }
    // Print Screen
    if vk == VK_SNAPSHOT { return true; }
    false
}

unsafe extern "system" fn keyboard_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code >= 0 {
        let info = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        if is_blocked_key(info) {
            return LRESULT(1);
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

unsafe extern "system" fn mouse_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code >= 0 {
        let msg = wparam.0 as u32;
        // Block right-click down & up
        if msg == WM_RBUTTONDOWN || msg == WM_RBUTTONUP {
            return LRESULT(1);
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

pub struct HookManager {
    running: Arc<AtomicBool>,
    thread_id: std::sync::Mutex<Option<u32>>,
    handle: std::sync::Mutex<Option<JoinHandle<()>>>,
}

impl HookManager {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            thread_id: std::sync::Mutex::new(None),
            handle: std::sync::Mutex::new(None),
        }
    }

    pub fn install(&self) -> Result<()> {
        if self.running.load(Ordering::SeqCst) {
            anyhow::bail!("Hooks are already installed");
        }

        let running = self.running.clone();
        let (tx, rx) = mpsc::channel::<Result<u32>>();
        let handle = thread::Builder::new()
            .name("input-hooks".to_string())
            .spawn(move || {
                run_hook_loop(&running, tx);
            })
            .map_err(|e| anyhow::anyhow!("Failed to spawn hook thread: {}", e))?;

        let tid = rx
            .recv()
            .map_err(|e| anyhow::anyhow!("Failed to receive thread id: {}", e))??;

        *self.thread_id.lock().unwrap() = Some(tid);
        *self.handle.lock().unwrap() = Some(handle);
        Ok(())
    }

    pub fn uninstall(&self) -> Result<()> {
        if !self.running.load(Ordering::SeqCst) {
            return Ok(());
        }

        if let Some(tid) = self.thread_id.lock().unwrap().take() {
            unsafe {
                let _ = PostThreadMessageW(tid, WM_QUIT, WPARAM(0), LPARAM(0));
            }
        }

        if let Some(h) = self.handle.lock().unwrap().take() {
            let _ = h.join();
        }

        self.running.store(false, Ordering::SeqCst);
        Ok(())
    }
}

impl Drop for HookManager {
    fn drop(&mut self) {
        let _ = self.uninstall();
    }
}

fn run_hook_loop(running: &Arc<AtomicBool>, tx: mpsc::Sender<Result<u32>>) {
    macro_rules! fail {
        ($e:expr) => {{
            let _ = tx.send(Err($e));
            return;
        }};
    }

    let kb_hook = match unsafe {
        SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), None, 0)
    } {
        Ok(h) => h,
        Err(e) => fail!(anyhow::anyhow!("Failed to install keyboard hook: {e}")),
    };

    let mouse_hook = match unsafe {
        SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), None, 0)
    } {
        Ok(h) => h,
        Err(e) => {
            unsafe { let _ = UnhookWindowsHookEx(kb_hook); }
            fail!(anyhow::anyhow!("Failed to install mouse hook: {e}"));
        }
    };

    running.store(true, Ordering::SeqCst);
    let tid = unsafe { windows::Win32::System::Threading::GetCurrentThreadId() };
    let _ = tx.send(Ok(tid));

    let mut msg = MSG::default();
    while unsafe { GetMessageW(&mut msg, None, 0, 0) }.0 > 0 {}

    running.store(false, Ordering::SeqCst);
    unsafe {
        let _ = UnhookWindowsHookEx(kb_hook);
        let _ = UnhookWindowsHookEx(mouse_hook);
    }
}