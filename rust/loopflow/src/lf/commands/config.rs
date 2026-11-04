pub fn retained(replace: bool, keep: bool) -> anyhow::Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(&crate::config::remote::inspect(replace, keep)?)?
    );
    Ok(())
}

pub fn print_user(json: bool) -> anyhow::Result<()> {
    let name = crate::config::participant_name()?;
    if json {
        println!("{}", serde_json::to_string(&name)?);
    } else if let Some(name) = name {
        println!("{name}");
    }
    Ok(())
}

pub fn show(
    repo: &std::path::Path,
    wave: Option<&str>,
    skill: Option<&str>,
    graph: Option<&str>,
    step: Option<&str>,
    json: bool,
) -> anyhow::Result<()> {
    let output = crate::config::api::inspect(repo, wave, skill, graph, step)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        for item in output {
            if let Some(skill) = item["skill"].as_str() {
                println!("{skill}");
            }
            if let Some(settings) = item["settings"].as_object() {
                for (key, setting) in settings {
                    println!("{key}: {}", setting["value"]);
                    if let Some(candidates) = setting["candidates"].as_array() {
                        for candidate in candidates.iter().rev() {
                            let status = candidate["reason"].as_str().expect("resolver reason");
                            println!(
                                "  {status}: {} · {} · {} · {} · {} = {}",
                                candidate["key"].as_str().expect("candidate key"),
                                candidate["source"].as_str().unwrap_or("builtin"),
                                candidate["scope"].as_str().unwrap_or("global"),
                                candidate["target"],
                                candidate["locator"].as_str().expect("candidate locator"),
                                candidate["value"]
                            );
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// The catalogue is available before a store or provider is installed.
pub fn catalog(json: bool) -> anyhow::Result<()> {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(crate::config::catalog::DEFINITIONS)?
        );
    } else {
        for definition in crate::config::catalog::DEFINITIONS {
            println!(
                "{}: {}{}",
                definition.key,
                definition.description,
                if definition.lifecycle == crate::config::catalog::Lifecycle::Inactive {
                    " (inactive)"
                } else {
                    ""
                }
            );
            println!(
                "  {:?} · {:?} · {:?}",
                definition.applicability, definition.combination, definition.portability
            );
        }
    }
    Ok(())
}
