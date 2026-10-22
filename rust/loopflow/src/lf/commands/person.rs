//! People who share this repository's `origin`, as planning sync last fetched them.

use anyhow::anyhow;

use crate::lf::PersonCommand;

pub fn run(cmd: &PersonCommand) -> anyhow::Result<()> {
    let PersonCommand::List { json } = cmd;
    let runtime = tokio::runtime::Runtime::new()?;
    let store = runtime
        .block_on(crate::store::open_existing_store())
        .ok_or_else(|| anyhow!("person commands need an initialized local store"))?;
    let repo = crate::repo::working_directory()?;
    let people = crate::ops::people::list(&store.sqlite, &repo)?;
    if *json {
        println!("{}", serde_json::to_string(&people)?);
        return Ok(());
    }
    if people.is_empty() {
        println!("No people fetched from origin yet");
    }
    for person in &people {
        let accounts: Vec<String> = person
            .accounts
            .iter()
            .map(|account| format!("{:?}", account.provider).to_lowercase())
            .collect();
        println!(
            "{}  {}{}{}",
            person.key,
            person.name,
            if accounts.is_empty() {
                String::new()
            } else {
                format!("  {}", accounts.join(", "))
            },
            if person.is_you { "  (you)" } else { "" }
        );
    }
    Ok(())
}
