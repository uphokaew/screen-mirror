use anyhow::{anyhow, Context, Result};
use std::env;
use std::path::PathBuf;
use tracing::debug;

pub struct Assets;

impl Assets {
    /// Finds the path to the scrcpy-server binary (or jar).
    /// Searches in the same directory as the executable first, then current working directory.
    pub fn get_server_path() -> Result<PathBuf> {
        Self::find_asset("scrcpy-server")
            .or_else(|_| Self::find_asset("scrcpy-server.jar"))
            .context("Could not find scrcpy-server or scrcpy-server.jar in the executable directory or current working directory.")
    }

    /// Finds the path to the adb binary.
    /// Searches in the same directory as the executable first, then current working directory.
    pub fn get_adb_path() -> Result<PathBuf> {
        #[cfg(target_os = "windows")]
        let binary_name = "adb.exe";
        #[cfg(not(target_os = "windows"))]
        let binary_name = "adb";

        Self::find_asset(binary_name).context(format!(
            "Could not find {} in the executable directory or current working directory.",
            binary_name
        ))
    }

    fn find_asset(name: &str) -> Result<PathBuf> {
        // 1. Try next to the executable
        if let Ok(exe_path) = env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                let candidate = exe_dir.join(name);
                if candidate.exists() {
                    debug!("Found asset {} at {:?}", name, candidate);
                    return Ok(candidate);
                }
            }
        }

        // 2. Try current working directory
        let candidate = env::current_dir()?.join(name);
        if candidate.exists() {
            debug!(
                "Found asset {} at current working directory: {:?}",
                name, candidate
            );
            return Ok(candidate);
        }

        Err(anyhow!("Asset {} not found", name))
    }
}
