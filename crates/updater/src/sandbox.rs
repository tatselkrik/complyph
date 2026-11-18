//! Fail closed on platforms without the implemented OS confinement.
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::Launcher;

#[cfg(not(windows))]
pub struct Launcher;
#[cfg(not(windows))]
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
    #[cfg(not(windows))]
    {
        Err("An OS-isolated downloader is required before networking.".into())
    }
}
