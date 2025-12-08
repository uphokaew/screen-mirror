#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use self::windows::*;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use self::linux::*;

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
compile_error!("Unsupported platform! Only Windows and Linux are supported.");
