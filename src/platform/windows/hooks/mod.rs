mod keyboard;
mod mouse;
mod touchpad;
mod touchpad_cache;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};

use anyhow::{Context, Result};
use windows::Win32::Foundation::{HINSTANCE, LPARAM, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    GetMessageW, PeekMessageW, PostThreadMessageW, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK,
    HOOKPROC, MSG, PM_NOREMOVE, WH_KEYBOARD_LL, WH_MOUSE_LL, WINDOWS_HOOK_ID, WM_QUIT,
};

use touchpad::TouchpadPolicy;

pub struct HookManager {
    running: Arc<AtomicBool>,
    state: Mutex<HookThreadState>,
    touchpad_policy: TouchpadPolicy,
}

pub(super) fn apply_touchpad_setting_changes() -> Result<()> {
    touchpad_cache::apply_touchpad_setting_changes()
}

#[derive(Default)]
struct HookThreadState {
    thread_id: Option<u32>,
    handle: Option<JoinHandle<()>>,
}

impl HookManager {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            state: Mutex::new(HookThreadState::default()),
            touchpad_policy: TouchpadPolicy::new(),
        }
    }

    pub fn install(&self) -> Result<()> {
        {
            let state = self.state.lock().unwrap();
            if state.handle.is_some() || self.running.load(Ordering::SeqCst) {
                anyhow::bail!("Windows input lockdown is already installed");
            }
        }

        if let Err(err) = self.touchpad_policy.install() {
            let _ = self.touchpad_policy.uninstall();
            return Err(err).context("Failed to apply Windows touchpad and shell policies");
        }

        let running = Arc::clone(&self.running);
        let (tx, rx) = mpsc::channel::<Result<u32>>();
        let handle = match thread::Builder::new()
            .name("windows-input-hooks".to_string())
            .spawn(move || run_hook_loop(running, tx))
        {
            Ok(handle) => handle,
            Err(err) => {
                let _ = self.touchpad_policy.uninstall();
                return Err(err).context("Failed to spawn Windows hook thread");
            }
        };

        match rx.recv() {
            Ok(thread_id) => match thread_id {
                Ok(thread_id) => {
                    let mut state = self.state.lock().unwrap();
                    state.thread_id = Some(thread_id);
                    state.handle = Some(handle);
                    Ok(())
                }
                Err(err) => {
                    let _ = handle.join();
                    let _ = self.touchpad_policy.uninstall();
                    Err(err)
                }
            },
            Err(err) => {
                let _ = handle.join();
                let _ = self.touchpad_policy.uninstall();
                Err(err).context("Windows hook thread exited during startup")
            }
        }
    }

    pub fn uninstall(&self) -> Result<()> {
        let (thread_id, handle) = {
            let mut state = self.state.lock().unwrap();
            (state.thread_id.take(), state.handle.take())
        };

        if let Some(thread_id) = thread_id {
            unsafe {
                let _ = PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
            }
        }

        if let Some(handle) = handle {
            let _ = handle.join();
        }

        self.running.store(false, Ordering::SeqCst);
        self.touchpad_policy
            .uninstall()
            .context("Failed to restore Windows touchpad and shell policies")?;
        Ok(())
    }

    pub fn restore_legacy_lockdown_defaults(&self) -> Result<()> {
        self.touchpad_policy
            .restore_legacy_lockdown_defaults()
            .context("Failed to restore legacy Windows input lockdown values")
    }
}

impl Drop for HookManager {
    fn drop(&mut self) {
        let _ = self.uninstall();
    }
}

struct InstalledHook {
    handle: HHOOK,
}

impl InstalledHook {
    fn new(handle: HHOOK) -> Self {
        Self { handle }
    }
}

impl Drop for InstalledHook {
    fn drop(&mut self) {
        unsafe {
            let _ = UnhookWindowsHookEx(self.handle);
        }
    }
}

fn run_hook_loop(running: Arc<AtomicBool>, startup: mpsc::Sender<Result<u32>>) {
    let keyboard_hook = match install_hook("keyboard", WH_KEYBOARD_LL, Some(keyboard::proc)) {
        Ok(hook) => hook,
        Err(err) => {
            let _ = startup.send(Err(err));
            return;
        }
    };

    let mouse_hook = match install_hook("mouse", WH_MOUSE_LL, Some(mouse::proc)) {
        Ok(hook) => hook,
        Err(err) => {
            let _ = startup.send(Err(err));
            return;
        }
    };

    create_message_queue();

    running.store(true, Ordering::SeqCst);
    let thread_id = unsafe { GetCurrentThreadId() };
    if startup.send(Ok(thread_id)).is_err() {
        running.store(false, Ordering::SeqCst);
        return;
    }

    let mut msg = MSG::default();
    loop {
        let result = unsafe { GetMessageW(&mut msg, None, 0, 0) }.0;
        if result <= 0 {
            break;
        }
    }

    running.store(false, Ordering::SeqCst);
    drop(mouse_hook);
    drop(keyboard_hook);
}

fn install_hook(name: &str, hook_id: WINDOWS_HOOK_ID, proc: HOOKPROC) -> Result<InstalledHook> {
    let handle = unsafe { SetWindowsHookExW(hook_id, proc, HINSTANCE::default(), 0) }
        .with_context(|| format!("Failed to install {name} hook"))?;
    Ok(InstalledHook::new(handle))
}

fn create_message_queue() {
    let mut msg = MSG::default();
    unsafe {
        let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE);
    }
}
