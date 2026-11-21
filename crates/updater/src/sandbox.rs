//! Fail closed on platforms without the implemented OS confinement.
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::Launcher;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::{Launcher, enter_isolation};

#[cfg(not(any(windows, target_os = "linux")))]
pub struct Launcher;
#[cfg(not(any(windows, target_os = "linux")))]
impl Launcher {
    pub fn new() -> Result<Self, String> {
        Err("Source update checks require the Windows AppContainer downloader. This build remains offline.".into())
    }
    pub fn run(
        &self,
        _: &crate::protocol::Request,
    ) -> Result<(crate::protocol::Reply, Vec<u8>), String> {
        Err("The isolated downloader is unavailable.".into())
    }
}

pub fn require_isolation() -> Result<(), String> {
    #[cfg(windows)]
    {
        windows::require_isolation()
    }
    #[cfg(target_os = "linux")]
    {
        linux::require_isolation()
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        Err("An OS-isolated downloader is required before networking.".into())
    }
}
