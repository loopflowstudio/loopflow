//! The Home-local Project selection, shared by every checkout of a Wave.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::Deserialize;
use yaml_edit::YamlFile;

use crate::id::WaveId;

#[derive(Debug, thiserror::Error)]
pub enum BindingError {
    #[error("Project binding at {path}: {reason}")]
    Invalid { path: PathBuf, reason: String },
}

fn error(path: &Path, reason: impl ToString) -> BindingError {
    BindingError::Invalid {
        path: path.into(),
        reason: reason.to_string(),
    }
}

fn path(home: &Path, wave: &WaveId) -> PathBuf {
    home.join("waves").join(wave.as_str()).join("config.yaml")
}

#[derive(Deserialize)]
struct Config {
    pm: Option<Pm>,
}

#[derive(Deserialize)]
struct Pm {
    linear_project: Option<String>,
}

fn decode(path: &Path, content: &str) -> Result<Option<String>, BindingError> {
    let config: Config = serde_yaml_ng::from_str(content).map_err(|e| error(path, e))?;
    let project = config.pm.and_then(|pm| pm.linear_project);
    if let Some(id) = &project {
        uuid::Uuid::parse_str(id).map_err(|e| error(path, e))?;
    }
    Ok(project)
}

fn read(path: &Path) -> Result<Option<String>, BindingError> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(Some(content)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(error(path, e)),
    }
}

/// Reading an unconfigured Wave creates neither a file nor a directory.
pub fn read_project_binding(home: &Path, wave: &WaveId) -> Result<Option<String>, BindingError> {
    let path = path(home, wave);
    read(&path)?
        .map(|content| decode(&path, &content))
        .transpose()
        .map(Option::flatten)
}

/// The caller retains the Wave planning guard through provider reconciliation.
pub(crate) fn write_project_binding(
    home: &Path,
    wave: &WaveId,
    expected: Option<&str>,
    project: &str,
    _guard: &crate::store::PlanningLocks,
) -> Result<(), BindingError> {
    let path = path(home, wave);
    uuid::Uuid::parse_str(project).map_err(|e| error(&path, e))?;
    let previous = read(&path)?;
    let selected = previous
        .as_deref()
        .map(|s| decode(&path, s))
        .transpose()?
        .flatten();
    if selected.as_deref() != expected {
        return Err(error(
            &path,
            "Project selection changed; preserve the intervening decision",
        ));
    }
    if selected.as_deref() == Some(project) {
        return Ok(());
    }
    let file = YamlFile::from_str(previous.as_deref().unwrap_or("pm:\n  {}\n"))
        .map_err(|e| error(&path, e))?;
    let document = file
        .document()
        .ok_or_else(|| error(&path, "missing document"))?;
    if document.get_mapping("pm").is_none() {
        document.set("pm", yaml_edit::Mapping::new());
    }
    let pm = document
        .get_mapping("pm")
        .ok_or_else(|| error(&path, "pm must be a mapping"))?;
    pm.set("linear_project", project);
    let rendered = file.to_string();
    if decode(&path, &rendered)?.as_deref() != Some(project) {
        return Err(error(
            &path,
            "edited document did not retain the selected Project",
        ));
    }
    let parent = path.parent().expect("Wave configuration has a parent");
    fs::create_dir_all(parent).map_err(|e| error(&path, e))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|e| error(&path, e))?;
    temporary
        .write_all(rendered.as_bytes())
        .map_err(|e| error(&path, e))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|e| error(&path, e))?;
    // Detect unrelated file edits made while constructing the replacement too.
    if read(&path)? != previous {
        return Err(error(
            &path,
            "configuration changed while preparing the replacement",
        ));
    }
    temporary.persist(&path).map_err(|e| error(&path, e))?;
    File::open(parent)
        .and_then(|dir| dir.sync_all())
        .map_err(|e| error(&path, e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{path, read_project_binding, write_project_binding};
    use crate::id::WaveId;
    use crate::store::PlanningLocks;
    use std::fs;

    const FIRST: &str = "999bdbdd-c045-41a6-8ffc-a97c4a40b0b3";
    const SECOND: &str = "218967b6-a760-4b7c-9a46-11d9d61a42c2";

    #[test]
    fn shared_project_binding_preserves_bytes_and_rejects_stale_replacement() {
        let home = tempfile::tempdir().unwrap();
        let wave = WaveId::new();
        let guard = PlanningLocks::new(tempfile::tempfile().unwrap());
        assert_eq!(read_project_binding(home.path(), &wave).unwrap(), None);
        assert_eq!(fs::read_dir(home.path()).unwrap().count(), 0);
        write_project_binding(home.path(), &wave, None, FIRST, &guard).unwrap();
        let config = path(home.path(), &wave);
        let original = format!("# retained\nname: 'exact bytes' # keep\npm:\n  other: [1, 2]\n  linear_project: {FIRST} # selected\nfooter: |\n  retained text\n");
        fs::write(&config, &original).unwrap();
        write_project_binding(home.path(), &wave, Some(FIRST), SECOND, &guard).unwrap();
        assert_eq!(
            fs::read_to_string(&config).unwrap(),
            original.replace(FIRST, SECOND)
        );
        assert!(write_project_binding(home.path(), &wave, Some(FIRST), FIRST, &guard).is_err());
        assert_eq!(
            read_project_binding(home.path(), &wave).unwrap().as_deref(),
            Some(SECOND)
        );
        write_project_binding(home.path(), &wave, Some(SECOND), SECOND, &guard).unwrap();
        assert_eq!(
            fs::read_to_string(&config).unwrap(),
            original.replace(FIRST, SECOND)
        );
    }

    #[test]
    fn shared_project_binding_adds_missing_or_null_pm_without_losing_policy() {
        let home = tempfile::tempdir().unwrap();
        let wave = WaveId::new();
        let guard = PlanningLocks::new(tempfile::tempfile().unwrap());
        let config = path(home.path(), &wave);
        fs::create_dir_all(config.parent().unwrap()).unwrap();
        for content in [
            "# retained\nowner: 'policy'\n",
            "# retained\nowner: 'policy'\npm: null\n",
        ] {
            fs::write(&config, content).unwrap();
            write_project_binding(home.path(), &wave, None, FIRST, &guard).unwrap();
            assert!(fs::read_to_string(&config)
                .unwrap()
                .starts_with("# retained\nowner: 'policy'\n"));
            assert_eq!(
                read_project_binding(home.path(), &wave).unwrap().as_deref(),
                Some(FIRST)
            );
        }
    }

    #[test]
    fn shared_project_binding_rejects_malformed_configuration_without_writing() {
        let home = tempfile::tempdir().unwrap();
        let wave = WaveId::new();
        let guard = PlanningLocks::new(tempfile::tempfile().unwrap());
        let config = path(home.path(), &wave);
        fs::create_dir_all(config.parent().unwrap()).unwrap();
        for invalid in [
            "pm: [",
            "pm: {linear_project: false}",
            "pm: {linear_project: not-an-id}",
            "[]",
        ] {
            fs::write(&config, invalid).unwrap();
            assert!(read_project_binding(home.path(), &wave).is_err());
            assert!(write_project_binding(home.path(), &wave, None, FIRST, &guard).is_err());
            assert_eq!(fs::read_to_string(&config).unwrap(), invalid);
        }
    }
}
