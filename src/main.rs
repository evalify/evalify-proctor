mod browsers;
mod platform;
mod proxy;

use anyhow::{Context, Result};
use platform::ProctorPlatform;

#[cfg(target_os = "windows")]
use platform::windows::WindowsPlatform;

#[cfg(target_os = "linux")]
use platform::linux::LinuxPlatform;

#[cfg(target_os = "macos")]
use platform::mac::MacPlatform;

mod config;

#[tokio::main]
async fn main() -> Result<()> {
    #[cfg(target_os = "windows")]
    if platform::windows::maybe_run_lockdown_guard()? {
        return Ok(());
    }

    use std::net::TcpListener;

    let target_url = &crate::config::CONFIG.target_url;
    let proxy_port = crate::config::CONFIG.proxy_port;

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], proxy_port));
    let listener = TcpListener::bind(addr).context("Failed to bind proxy port")?;
    listener
        .set_nonblocking(true)
        .context("Failed to set listener nonblocking")?;

    let logout_signal = std::sync::Arc::new(tokio::sync::Notify::new());
    let proxy_shutdown = std::sync::Arc::new(tokio::sync::Notify::new());
    let logout_clone = logout_signal.clone();
    let proxy_shutdown_clone = proxy_shutdown.clone();

    println!("Starting Proxy on port {}...", proxy_port);
    tokio::spawn(async move {
        proxy::server::run(listener, logout_clone, proxy_shutdown_clone).await;
    });

    #[cfg(target_os = "windows")]
    let platform = WindowsPlatform::new();

    #[cfg(target_os = "linux")]
    let platform = LinuxPlatform::new();

    #[cfg(target_os = "macos")]
    let platform = MacPlatform::new();

    println!("Platform Initialized");
    platform.setup().await?;

    println!("Launching Kiosk Browser: {}", target_url);
    let _browser_child = platform
        .launch_kioski(target_url)
        .await
        .context("Failed to launch browser")?;

    println!("Kiosk mode active. Press Ctrl+C to exit.");

    // Wait for either Ctrl+C or a logout signal from the proxy
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            println!("\nReceived Ctrl+C, shutting down...");
        }
        _ = logout_signal.notified() => {
            println!("\nLogout detected, shutting down...");
        }
    }

    // Teardown platform (unhook keys, re-enable devtools, etc.)
    platform.teardown().await?;

    // Now shut down the proxy server
    proxy_shutdown.notify_one();

    println!("Shutdown Complete.");
    Ok(())
}
