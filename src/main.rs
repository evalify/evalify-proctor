mod platform;
mod browsers;
mod proxy;

use anyhow::{Result, Context};
use platform::{ProctorPlatform, windows::WindowsPlatform};

mod config;

#[tokio::main]
async fn main() -> Result<()> {
    use std::net::TcpListener;

    let target_url = &crate::config::CONFIG.target_url;
    let proxy_port = crate::config::CONFIG.proxy_port;

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], proxy_port));
    let listener = TcpListener::bind(addr).context("Failed to bind proxy port")?;
    listener.set_nonblocking(true).context("Failed to set listener nonblocking")?;
    
    println!("Starting Proxy on port {}...", proxy_port);
    tokio::spawn(async move {
        proxy::server::run(listener).await;
    });

    #[cfg(target_os = "windows")]
    let platform = WindowsPlatform::new();

    #[cfg(target_os = "linux")]
    let platform = LinuxPlatform::new();

    println!("Platform Initialized: Windows");
    platform.setup().await?;

    println!("Launching Kiosk Browser: {}", target_url);
    let mut browser_child = platform.launch_kioski(target_url).await
        .context("Failed to launch browser")?;

    match browser_child.wait().await {
        Ok(status) => println!("Browser exited with status: {}", status),
        Err(e) => eprintln!("Error waiting for browser process: {}", e),
    }

    platform.teardown().await?;
    
    println!("Shutdown Complete.");
    Ok(())
}
