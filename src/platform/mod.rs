use async_trait::async_trait;
use anyhow::Result;
use tokio::process::Child;

#[cfg(target_os = "windows")]
pub mod windows;
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod mac;


#[async_trait]
pub trait ProctorPlatform {
    async fn setup(&self) -> Result<()>;
    async fn launch_kioski(&self, url: &str) -> Result<Child>;
    async fn teardown(&self) -> Result<()>;
}
