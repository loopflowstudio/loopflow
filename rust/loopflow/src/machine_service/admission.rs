//! Explicit local enrollment grants a phone access to this Machine. Neither
//! tailnet reachability nor proxy headers confer the Unix peer's authority.
use std::fs::OpenOptions;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use anyhow::{Context, Result};
use secrecy::{ExposeSecret, SecretString};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::store::sqlite::SqliteStore;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct DeviceId(pub(crate) String);

#[derive(Debug, Clone, Serialize)]
pub struct Device {
    pub id: DeviceId,
    pub name: String,
    pub created_at: i64,
    pub revoked_at: Option<i64>,
}

pub(super) fn hash(value: &str) -> [u8; 32] {
    Sha256::digest(value.as_bytes()).into()
}

pub(super) fn secret() -> SecretString {
    SecretString::new(hex::encode(rand::random::<[u8; 32]>()))
}

pub(super) async fn store() -> Result<SqliteStore> {
    Ok(crate::store::open_existing_store()
        .await?
        .context("Initialize this Machine before pairing a phone")?
        .sqlite)
}

pub async fn pair(endpoint: &str, name: &str, output: &Path) -> Result<()> {
    let endpoint = reqwest::Url::parse(endpoint)?;
    anyhow::ensure!(
        endpoint.scheme() == "https"
            && endpoint
                .host_str()
                .is_some_and(|host| host.ends_with(".ts.net"))
            && endpoint.username().is_empty()
            && endpoint.password().is_none()
            && endpoint.query().is_none()
            && endpoint.fragment().is_none()
            && endpoint.path() == "/",
        "Use the host Tailscale Serve HTTPS origin, such as https://machine.tailnet.ts.net:8443"
    );
    anyhow::ensure!(
        !name.trim().is_empty() && name.len() <= 128,
        "Device name must contain 1–128 bytes"
    );
    let store = store().await?;
    // Runtime credentials are generated here, not deployment secrets. Only
    // hashes enter SQLite; the one-use ticket goes directly to a private file.
    let ticket = secret();
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(output)
        .context("Choose a new private file for the phone's setup ticket")?;
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    store.create_machine_pairing(&hash(ticket.expose_secret()), name.trim(), now, now + 600)?;
    serde_json::to_writer(
        file,
        &serde_json::json!({
            "version": 1, "endpoint": endpoint.as_str(), "ticket": ticket.expose_secret()
        }),
    )?;
    println!(
        "Phone setup saved to {}. It expires in 10 minutes and can be used once.",
        output.display()
    );
    Ok(())
}

pub async fn list() -> Result<()> {
    println!(
        "{}",
        serde_json::to_string(&store().await?.machine_devices()?)?
    );
    Ok(())
}

pub async fn revoke(id: &str) -> Result<()> {
    let changed = store().await?.revoke_machine_device(
        &DeviceId(id.into()),
        time::OffsetDateTime::now_utc().unix_timestamp(),
    )?;
    anyhow::ensure!(changed, "No active device has that ID");
    println!("Device revoked. Open connections close on their next authorization check.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{hash, DeviceId};
    use crate::store::sqlite::SqliteStore;

    #[test]
    fn pairing_is_one_use_expires_and_revocation_removes_authority() {
        let temp = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&temp.path().join("test.db")).unwrap();
        let ticket = hash("test enrollment");
        let token = hash("test credential");
        let id = DeviceId("phone-test".into());
        store
            .create_machine_pairing(&ticket, "Phone", 100, 200)
            .unwrap();
        assert!(store.admitted_machine_device(&ticket).unwrap().is_none());
        let device = store
            .redeem_machine_pairing(&ticket, &token, &id, 150)
            .unwrap()
            .unwrap();
        assert_eq!(device.name, "Phone");
        assert!(store
            .redeem_machine_pairing(&ticket, &hash("second"), &DeviceId("other".into()), 151)
            .unwrap()
            .is_none());
        assert_eq!(
            store.admitted_machine_device(&token).unwrap(),
            Some(id.clone())
        );
        assert!(store.revoke_machine_device(&id, 160).unwrap());
        assert!(!store.machine_device_active(&id).unwrap());
        assert!(store.admitted_machine_device(&token).unwrap().is_none());
        assert_eq!(store.machine_devices().unwrap()[0].revoked_at, Some(160));

        store
            .create_machine_pairing(&ticket, "Expired", 200, 201)
            .unwrap();
        assert!(store
            .redeem_machine_pairing(&ticket, &token, &id, 201)
            .unwrap()
            .is_none());
    }
}
