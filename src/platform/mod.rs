use async_trait::async_trait;
use anyhow::Result;
use tokio::process::Child;

pub mod windows;


#[async_trait]
pub trait ProctorPlatform {
    async fn setup(&self) -> Result<()>;
    async fn launch_kioski(&self, url: &str) -> Result<Child>;
    async fn teardown(&self) -> Result<()>;
}