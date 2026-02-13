#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;

pub fn install() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    return macos::install();

    #[cfg(target_os = "linux")]
    return linux::install();

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    Err("service installation is only supported on macOS and Linux".into())
}

pub fn uninstall() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    return macos::uninstall();

    #[cfg(target_os = "linux")]
    return linux::uninstall();

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    Err("service management is only supported on macOS and Linux".into())
}

pub fn start() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    return macos::start();

    #[cfg(target_os = "linux")]
    return linux::start();

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    Err("service management is only supported on macOS and Linux".into())
}

pub fn stop() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    return macos::stop();

    #[cfg(target_os = "linux")]
    return linux::stop();

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    Err("service management is only supported on macOS and Linux".into())
}

pub fn status() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    return macos::status();

    #[cfg(target_os = "linux")]
    return linux::status();

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    Err("service management is only supported on macOS and Linux".into())
}
