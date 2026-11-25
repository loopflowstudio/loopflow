//! Build and open interactive human-comprehension review pages.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use loopflow_execution::machine::client::Connection;

use crate::lf::commands::open::{viewed_placement, OpenHost};
use crate::lf::{Cli, DesktopCommand, DesktopTargetArgs, PageCommand, PageHost};
use crate::server::wire::{Operation, PageUrl, ServerOperation};

pub fn run(cli: &Cli, cmd: &PageCommand) -> Result<()> {
    match cmd {
        PageCommand::Build { source, base } => {
            let output = super::page_build::build(cli, source, base.as_deref())?;
            println!("{}", output.display());
            Ok(())
        }
        PageCommand::Open { page, host, print } => open(cli, page, *host, *print),
    }
}

/// Open a compiled HTML review page in Desktop Files or the browser, or print
/// its loopback capability URL when headless or `--print` is passed.
fn open(cli: &Cli, page: &Path, host: PageHost, print_only: bool) -> Result<()> {
    if is_page_source(page) {
        bail!(
            "{} is a page source; build it first:\n  lf page build {}",
            page.display(),
            page.display()
        );
    }

    let resolved = resolve_existing_file(page)?;
    if print_only {
        let url = mint_loopback_url(&resolved)?;
        println!("{url}");
        return Ok(());
    }

    match decide_open_route(host) {
        OpenRoute::Desktop => open_in_desktop(cli, page),
        OpenRoute::Browser => {
            let url = mint_loopback_url(&resolved)?;
            loopflow_execution::platform::open_url_checked(&url)
                .with_context(|| format!("Could not open browser for {url}"))?;
            println!("{url}");
            Ok(())
        }
        OpenRoute::PrintQuiet => {
            let url = mint_loopback_url(&resolved)?;
            println!("{url}");
            Ok(())
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpenRoute {
    Desktop,
    Browser,
    PrintQuiet,
}

fn decide_open_route(host: PageHost) -> OpenRoute {
    let has_agent_caller = loopflow_execution::journal::agent_caller().is_some();
    let in_desktop_env = std::env::var_os("LF_DESKTOP").is_some_and(|value| !value.is_empty());
    let placement = has_agent_caller
        .then(viewed_placement)
        .flatten()
        .map(|(host, _)| host);
    route_for(host, has_agent_caller, in_desktop_env, placement)
}

fn route_for(
    host: PageHost,
    has_agent_caller: bool,
    in_desktop_env: bool,
    viewed_host: Option<OpenHost>,
) -> OpenRoute {
    match host {
        PageHost::Desktop => OpenRoute::Desktop,
        PageHost::Browser => OpenRoute::Browser,
        PageHost::Auto => {
            if !has_agent_caller {
                if in_desktop_env {
                    OpenRoute::Desktop
                } else {
                    OpenRoute::Browser
                }
            } else {
                match viewed_host {
                    Some(OpenHost::Desktop) => OpenRoute::Desktop,
                    Some(_) => OpenRoute::Browser,
                    None => OpenRoute::PrintQuiet,
                }
            }
        }
    }
}

fn open_in_desktop(cli: &Cli, page: &Path) -> Result<()> {
    let command = DesktopCommand::Files {
        path: Some(page.to_string_lossy().into_owned()),
        mode: None,
        target: DesktopTargetArgs::default(),
    };
    crate::lf::commands::desktop::run(cli, &command)
}

fn is_page_source(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.to_ascii_lowercase().ends_with(".page.html"))
}

fn resolve_existing_file(path: &Path) -> Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let canonical = std::fs::canonicalize(&absolute)
        .with_context(|| format!("Page not found: {}", path.display()))?;
    if !canonical.is_file() {
        bail!("{} is not a file", path.display());
    }
    Ok(canonical)
}

fn mint_loopback_url(path: &Path) -> Result<String> {
    let mut connection = match Connection::open() {
        Ok(connection) => connection,
        Err(error) if service_is_stopped(&error) => {
            loopflow_execution::installation::server::ensure()?;
            Connection::open()?
        }
        Err(error) => return Err(error),
    };
    let answer = connection.request(Operation::Server(ServerOperation::PageUrl {
        path: path.to_path_buf(),
    }))?;
    let reply: PageUrl = serde_json::from_value(answer.result)
        .context("Update the Loopflow server to mint review page URLs")?;
    Ok(reply.url)
}

fn service_is_stopped(error: &anyhow::Error) -> bool {
    error
        .root_cause()
        .downcast_ref::<std::io::Error>()
        .is_some_and(|io| {
            matches!(
                io.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
            )
        })
}

#[cfg(test)]
mod tests {
    use super::{is_page_source, route_for, OpenRoute};
    use crate::lf::commands::open::OpenHost;
    use crate::lf::PageHost;
    use std::path::Path;

    #[test]
    fn rejects_page_source_extension() {
        assert!(is_page_source(Path::new("scratch/ship-review.page.html")));
        assert!(is_page_source(Path::new("scratch/EXPLORE.PAGE.HTML")));
        assert!(!is_page_source(Path::new("scratch/ship-review.html")));
    }

    #[test]
    fn open_routing_respects_direct_interactive_and_headless_callers() {
        // Direct CLI outside Desktop opens the browser.
        assert_eq!(
            route_for(PageHost::Auto, false, false, None),
            OpenRoute::Browser
        );
        // Direct CLI inside Desktop opens Desktop Files.
        assert_eq!(
            route_for(PageHost::Auto, false, true, None),
            OpenRoute::Desktop
        );
        // Interactive agent viewed in Desktop opens Desktop Files.
        assert_eq!(
            route_for(PageHost::Auto, true, false, Some(OpenHost::Desktop)),
            OpenRoute::Desktop
        );
        // Interactive agent viewed in Rex opens the browser.
        assert_eq!(
            route_for(PageHost::Auto, true, false, Some(OpenHost::Rex)),
            OpenRoute::Browser
        );
        // Headless agent (no viewer) prints the URL without popping a browser window.
        assert_eq!(
            route_for(PageHost::Auto, true, false, None),
            OpenRoute::PrintQuiet
        );
        // Explicit overrides win without falling back.
        assert_eq!(
            route_for(PageHost::Desktop, true, false, None),
            OpenRoute::Desktop
        );
        assert_eq!(
            route_for(PageHost::Browser, false, true, None),
            OpenRoute::Browser
        );
    }
}
