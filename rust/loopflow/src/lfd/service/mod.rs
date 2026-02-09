#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;

use anyhow::Result;

macro_rules! dispatch {
    ($name:ident) => {
        pub fn $name() -> Result<()> {
            #[cfg(target_os = "macos")]
            return macos::$name();

            #[cfg(target_os = "linux")]
            return linux::$name();

            #[cfg(not(any(target_os = "macos", target_os = "linux")))]
            anyhow::bail!("service management is not supported on this platform");
        }
    };
}

dispatch!(install);
dispatch!(uninstall);
dispatch!(start);
dispatch!(stop);
dispatch!(status);
