#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
mod unsupported {
    fn unsupported() -> Result<(), Box<dyn std::error::Error>> {
        Err("service management is only supported on macOS and Linux".into())
    }

    pub fn install() -> Result<(), Box<dyn std::error::Error>> {
        unsupported()
    }

    pub fn uninstall() -> Result<(), Box<dyn std::error::Error>> {
        unsupported()
    }

    pub fn start() -> Result<(), Box<dyn std::error::Error>> {
        unsupported()
    }

    pub fn stop() -> Result<(), Box<dyn std::error::Error>> {
        unsupported()
    }

    pub fn status() -> Result<(), Box<dyn std::error::Error>> {
        unsupported()
    }
}

#[cfg(target_os = "linux")]
use linux as platform;
#[cfg(target_os = "macos")]
use macos as platform;
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
use unsupported as platform;

pub fn install() -> Result<(), Box<dyn std::error::Error>> {
    platform::install()
}

pub fn uninstall() -> Result<(), Box<dyn std::error::Error>> {
    platform::uninstall()
}

pub fn start() -> Result<(), Box<dyn std::error::Error>> {
    platform::start()
}

pub fn stop() -> Result<(), Box<dyn std::error::Error>> {
    platform::stop()
}

pub fn status() -> Result<(), Box<dyn std::error::Error>> {
    platform::status()
}
