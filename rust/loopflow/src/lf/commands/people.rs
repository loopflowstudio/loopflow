//! `lf repo people`: the people who own this repository's Tasks.
use anyhow::{anyhow, Result};

use crate::ops::people::repo_people;

pub fn run(json: bool) -> Result<()> {
    let repo = crate::repository::CanonicalRepo::current()?
        .ok_or_else(|| anyhow!("run `lf repo people` in a repository"))?;
    let runtime = tokio::runtime::Runtime::new()?;
    let store = runtime.block_on(crate::store::open_store(
        &crate::store::storage_config_from_env()?,
    ))?;
    let (people, warnings) = repo_people(&store.sqlite, repo.as_path())?;
    if json {
        println!("{}", serde_json::to_string(&people)?);
        return Ok(());
    }
    for warning in warnings {
        eprintln!("warning: GitHub unavailable: {warning}");
    }
    for person in people {
        let name = person.name.as_deref().or(person
            .github
            .as_ref()
            .and_then(|github| github.name.as_deref()));
        let login = person
            .github
            .as_ref()
            .map_or(String::new(), |github| format!("  @{}", github.login));
        match name {
            Some(name) => println!("{name} <{}>{login}", person.email),
            None => println!("{}{login}", person.email),
        }
    }
    Ok(())
}
