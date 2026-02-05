use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize)]
struct Credentials {
    jwt: Option<String>,
    token: Option<String>,
}

pub fn load_jwt() -> Option<String> {
    let credentials_path = credentials_path();
    if !credentials_path.exists() {
        return None;
    }

    let content = std::fs::read_to_string(&credentials_path).ok()?;
    let creds: Credentials = serde_json::from_str(&content).ok()?;

    creds
        .jwt
        .or(creds.token)
        .filter(|t| !t.trim().is_empty())
        .map(|t| t.trim().to_string())
}

fn credentials_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".lf")
        .join("credentials.json")
}
