//! The two rendezvous mechanisms published by Tailscale's Darwin safesocket.
use std::ffi::CStr;
use std::fs;
use std::mem::{size_of, size_of_val, MaybeUninit};
use std::path::{Path, PathBuf};

use super::Endpoint;

pub(super) fn endpoints() -> Vec<Endpoint> {
    let mut endpoints = extension_endpoints();
    endpoints.extend(standalone(Path::new("/Library/Tailscale")));
    endpoints.push(Endpoint::Unix("/var/run/tailscaled.socket".into()));
    endpoints
}

fn standalone(directory: &Path) -> Option<Endpoint> {
    let port = fs::read_link(directory.join("ipnport")).ok()?;
    let port = port.to_str()?.parse::<u16>().ok()?;
    let token = fs::read_to_string(directory.join(format!("sameuserproof-{port}"))).ok()?;
    let token = token.trim();
    (!token.is_empty()).then(|| Endpoint::Loopback {
        port,
        token: token.to_owned(),
    })
}

pub(super) fn app_installed() -> bool {
    std::iter::once(PathBuf::from("/Applications"))
        .chain(
            loopflow_execution::installation::account_home()
                .ok()
                .map(|p| p.join("Applications")),
        )
        .filter_map(|dir| fs::read_dir(dir).ok())
        .flatten()
        .flatten()
        .any(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with("Tailscale") && name.ends_with(".app"))
        })
}

fn extension_endpoints() -> Vec<Endpoint> {
    const PROC_UID_ONLY: u32 = 4;
    // SAFETY: getuid has no preconditions; a null buffer asks for the byte count.
    let uid = unsafe { libc::getuid() };
    // SAFETY: proc_listpids accepts null for the size query.
    let bytes = unsafe { libc::proc_listpids(PROC_UID_ONLY, uid, std::ptr::null_mut(), 0) };
    if bytes <= 0 {
        return Vec::new();
    }
    let mut pids = vec![0i32; bytes as usize / size_of::<i32>()];
    // SAFETY: the supplied byte length is the allocated slice size.
    let bytes = unsafe {
        libc::proc_listpids(
            PROC_UID_ONLY,
            uid,
            pids.as_mut_ptr().cast(),
            size_of_val(&pids[..]) as i32,
        )
    };
    pids.truncate(bytes.max(0) as usize / size_of::<i32>());
    let mut endpoints = Vec::new();
    for pid in pids {
        let mut name = [0u8; 64];
        // SAFETY: proc_name fills at most the supplied buffer.
        let count = unsafe { libc::proc_name(pid, name.as_mut_ptr().cast(), name.len() as u32) };
        if count > 0
            && CStr::from_bytes_until_nul(&name).is_ok_and(|n| n.to_bytes() == b"IPNExtension")
        {
            extension_proofs(pid, &mut endpoints);
        }
    }
    endpoints
}

// sys/proc_info.h's vnode_fdinfowithpath. libc exposes its vnode_info_path
// member but not the preceding proc_fileinfo.
#[repr(C)]
struct VnodeFdInfoWithPath {
    open_flags: u32,
    status: u32,
    offset: libc::off_t,
    file_type: i32,
    guard_flags: u32,
    node: libc::vnode_info_path,
}

fn extension_proofs(pid: i32, endpoints: &mut Vec<Endpoint>) {
    const PROC_PIDFDVNODEPATHINFO: i32 = 2;
    // SAFETY: a null buffer requests the number of bytes for the fd list.
    let bytes =
        unsafe { libc::proc_pidinfo(pid, libc::PROC_PIDLISTFDS, 0, std::ptr::null_mut(), 0) };
    if bytes <= 0 {
        return;
    }
    let mut fds = vec![
        libc::proc_fdinfo {
            proc_fd: 0,
            proc_fdtype: 0
        };
        bytes as usize / size_of::<libc::proc_fdinfo>()
    ];
    // SAFETY: fds is initialized and its slice size bounds the write.
    let bytes = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDLISTFDS,
            0,
            fds.as_mut_ptr().cast(),
            size_of_val(&fds[..]) as i32,
        )
    };
    fds.truncate(bytes.max(0) as usize / size_of::<libc::proc_fdinfo>());
    for fd in fds
        .iter()
        .filter(|fd| fd.proc_fdtype == libc::PROX_FDTYPE_VNODE as u32)
    {
        let mut info = MaybeUninit::<VnodeFdInfoWithPath>::zeroed();
        let size = size_of::<VnodeFdInfoWithPath>() as i32;
        // SAFETY: info matches the Darwin vnode_fdinfowithpath layout and size.
        let bytes = unsafe {
            libc::proc_pidfdinfo(
                pid,
                fd.proc_fd,
                PROC_PIDFDVNODEPATHINFO,
                info.as_mut_ptr().cast(),
                size,
            )
        };
        if bytes != size {
            continue;
        }
        // SAFETY: proc_pidfdinfo filled this integer-only structure in full.
        let info = unsafe { info.assume_init() };
        let path = info.node.vip_path.as_flattened();
        // SAFETY: c_char and u8 have the same size and alignment.
        let path = unsafe { std::slice::from_raw_parts(path.as_ptr().cast::<u8>(), path.len()) };
        let Some(path) = CStr::from_bytes_until_nul(path)
            .ok()
            .and_then(|s| s.to_str().ok())
        else {
            continue;
        };
        if let Some((port, token)) = path
            .rsplit_once(".tailscale.ipn.macos/sameuserproof-")
            .and_then(|(_, proof)| proof.split_once('-'))
            .and_then(|(port, token)| Some((port.parse::<u16>().ok()?, token)))
            .filter(|(_, token)| !token.is_empty() && token.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            endpoints.push(Endpoint::Loopback {
                port,
                token: token.to_owned(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{extension_proofs, standalone, Endpoint};
    use crate::server::mobile::Tailscale;
    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::get;
    use serde_json::json;
    use std::fs;
    use std::os::unix::fs::symlink;

    #[test]
    fn app_store_rendezvous_follows_an_open_proof() {
        let dir = tempfile::tempdir().unwrap();
        let container = dir.path().join("fixture.tailscale.ipn.macos");
        fs::create_dir(&container).unwrap();
        let file = fs::File::create(container.join("sameuserproof-23456-abcdef")).unwrap();
        let present = || {
            let mut endpoints = Vec::new();
            extension_proofs(std::process::id() as i32, &mut endpoints);
            endpoints.iter().any(|endpoint| matches!(endpoint, Endpoint::Loopback { port: 23456, token } if token == "abcdef"))
        };
        assert!(present());
        drop(file);
        assert!(!present());
    }

    #[tokio::test]
    async fn standalone_rediscovers_the_authenticated_port_after_rotation() {
        let directory = tempfile::tempdir().unwrap();
        let ipnport = directory.path().join("ipnport");
        let mut old = None;
        for (proof, expected) in [("one", "Basic Om9uZQ=="), ("two", "Basic OnR3bw==")] {
            let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
                .await
                .unwrap();
            let port = listener.local_addr().unwrap().port();
            let status = axum::Router::new().route(
                "/localapi/v0/status",
                get(move |headers: HeaderMap| async move {
                    if headers["host"] == "local-tailscaled.sock"
                        && headers.get("authorization").is_some_and(|v| v == expected)
                    {
                        Ok(axum::Json(json!({"BackendState":"Running"})))
                    } else {
                        Err(StatusCode::UNAUTHORIZED)
                    }
                }),
            );
            let server = tokio::spawn(async move { axum::serve(listener, status).await.unwrap() });
            if let Some((old_port, old_server)) = old.take() {
                let old_server: tokio::task::JoinHandle<()> = old_server;
                assert_ne!(port, old_port);
                old_server.abort();
                old_server.await.unwrap_err();
                fs::remove_file(&ipnport).unwrap();
            }
            symlink(port.to_string(), &ipnport).unwrap();
            let file = directory.path().join(format!("sameuserproof-{port}"));
            fs::write(&file, "obsolete").unwrap();
            let api = || Tailscale::new(standalone(directory.path()).unwrap()).unwrap();
            assert!(api().json("status").await.is_err());
            fs::write(file, proof).unwrap();
            assert_eq!(
                api().json("status").await.unwrap()["BackendState"],
                "Running"
            );
            old = Some((port, server));
        }
        let (_, server) = old.unwrap();
        server.abort();
        server.await.unwrap_err();
    }
}
