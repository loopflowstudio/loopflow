use std::path::PathBuf;
use uuid::Uuid;

pub fn machine_id() -> String {
    let machine_id_path = machine_id_path();

    if let Ok(id) = std::fs::read_to_string(&machine_id_path) {
        let id = id.trim();
        if !id.is_empty() {
            return id.to_string();
        }
    }

    let id = Uuid::new_v4().to_string();
    if let Some(parent) = machine_id_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&machine_id_path, &id);
    id
}

pub fn machine_name() -> String {
    gethostname::gethostname().to_string_lossy().into_owned()
}

fn machine_id_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".lf")
        .join("machine_id")
}
