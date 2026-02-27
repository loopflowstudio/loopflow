use serde::Deserialize;
use std::net::{IpAddr, SocketAddr, UdpSocket};
use tokio::process::Command;
use tokio::time::{timeout, Duration};

const TAILSCALE_STATUS_TIMEOUT: Duration = Duration::from_millis(800);

#[derive(Debug, Deserialize)]
struct TailscaleStatus {
    #[serde(rename = "TailscaleIPs", default)]
    tailscale_ips: Vec<String>,
}

pub async fn detect_lfd_url(bind_addr: SocketAddr) -> String {
    let tailscale_ip = detect_tailscale_ip().await;
    let interface_ip = detect_primary_interface_ip();
    let host_ip = select_preferred_ip(tailscale_ip, interface_ip, bind_addr.ip());
    format_url(host_ip, bind_addr.port())
}

fn select_preferred_ip(
    tailscale_ip: Option<IpAddr>,
    interface_ip: Option<IpAddr>,
    bind_ip: IpAddr,
) -> IpAddr {
    tailscale_ip.or(interface_ip).unwrap_or(bind_ip)
}

async fn detect_tailscale_ip() -> Option<IpAddr> {
    let output = timeout(
        TAILSCALE_STATUS_TIMEOUT,
        Command::new("tailscale")
            .args(["status", "--json"])
            .output(),
    )
    .await
    .ok()?
    .ok()?;

    if !output.status.success() {
        return None;
    }

    let status_json = String::from_utf8(output.stdout).ok()?;
    parse_tailscale_ip(&status_json)
}

fn parse_tailscale_ip(status_json: &str) -> Option<IpAddr> {
    let status: TailscaleStatus = serde_json::from_str(status_json).ok()?;
    status
        .tailscale_ips
        .into_iter()
        .find_map(|ip| ip.parse::<IpAddr>().ok())
}

fn detect_primary_interface_ip() -> Option<IpAddr> {
    for target in ["1.1.1.1:80", "8.8.8.8:80", "[2606:4700:4700::1111]:80"] {
        let Some(ip) = detect_routable_ip(target) else {
            continue;
        };
        if !ip.is_loopback() && !ip.is_unspecified() {
            return Some(ip);
        }
    }
    None
}

fn detect_routable_ip(target: &str) -> Option<IpAddr> {
    let bind_addr = if target.starts_with('[') {
        "[::]:0"
    } else {
        "0.0.0.0:0"
    };

    let socket = UdpSocket::bind(bind_addr).ok()?;
    socket.connect(target).ok()?;
    Some(socket.local_addr().ok()?.ip())
}

fn format_url(ip: IpAddr, port: u16) -> String {
    match ip {
        IpAddr::V4(ipv4) => format!("http://{ipv4}:{port}"),
        IpAddr::V6(ipv6) => format!("http://[{ipv6}]:{port}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn parse_tailscale_ip_returns_first_ip() {
        let status = r#"{"TailscaleIPs":["100.64.1.5","fd7a:115c:a1e0::1"]}"#;
        let ip = parse_tailscale_ip(status);
        assert_eq!(ip, Some(IpAddr::V4(Ipv4Addr::new(100, 64, 1, 5))));
    }

    #[test]
    fn parse_tailscale_ip_returns_none_when_missing() {
        let status = r#"{"Self":{"HostName":"devbox"}}"#;
        assert_eq!(parse_tailscale_ip(status), None);
    }

    #[test]
    fn select_preferred_ip_uses_fallback_chain() {
        let bind = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let iface_ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 10));
        let tailscale_ip = IpAddr::V4(Ipv4Addr::new(100, 101, 102, 103));
        let iface = Some(iface_ip);
        let tailscale = Some(tailscale_ip);

        assert_eq!(select_preferred_ip(tailscale, iface, bind), tailscale_ip);
        assert_eq!(select_preferred_ip(None, iface, bind), iface_ip);
        assert_eq!(select_preferred_ip(None, None, bind), bind);
    }

    #[test]
    fn format_url_wraps_ipv6() {
        let ip = IpAddr::V6(Ipv6Addr::LOCALHOST);
        assert_eq!(format_url(ip, 2486), "http://[::1]:2486");
    }
}
