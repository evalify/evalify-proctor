use std::env;
use std::os::windows::process::CommandExt;
use std::process::Command;

use anyhow::{Context, Result};
use windows::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{
    OpenProcess, WaitForSingleObject, CREATE_NO_WINDOW, PROCESS_SYNCHRONIZE,
};

use super::{hooks, registry_restore};

const GUARD_ARG: &str = "--evalify-windows-lockdown-guard";
const INFINITE: u32 = u32::MAX;

pub(super) fn maybe_run_from_args() -> Result<bool> {
    let mut args = env::args().skip(1);
    let Some(first) = args.next() else {
        return Ok(false);
    };

    if first != GUARD_ARG {
        return Ok(false);
    }

    let parent_pid = args
        .next()
        .context("Missing parent process id for Windows lockdown guard")?
        .parse::<u32>()
        .context("Invalid parent process id for Windows lockdown guard")?;

    let _ = wait_for_process_exit(parent_pid);
    registry_restore::restore_all().context("Failed to restore Windows lockdown journal")?;
    hooks::apply_touchpad_setting_changes()
        .context("Failed to apply restored Windows touchpad settings")?;
    Ok(true)
}

pub(super) fn spawn_for_current_process() -> Result<()> {
    let exe = env::current_exe().context("Failed to locate current executable")?;
    let parent_pid = std::process::id().to_string();

    Command::new(exe)
        .arg(GUARD_ARG)
        .arg(parent_pid)
        .creation_flags(CREATE_NO_WINDOW.0)
        .spawn()
        .context("Failed to start Windows lockdown guard")?;

    Ok(())
}

fn wait_for_process_exit(pid: u32) -> Result<()> {
    let process = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, pid) }
        .with_context(|| format!("Failed to open parent process {pid}"))?;

    let wait_result = unsafe { WaitForSingleObject(process, INFINITE) };
    unsafe {
        let _ = CloseHandle(process);
    }

    if wait_result != WAIT_OBJECT_0 {
        anyhow::bail!("Unexpected wait result while watching parent process: {wait_result:?}");
    }

    Ok(())
}
