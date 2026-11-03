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
    let output = inspect(repo, wave, skill, graph, step)?;
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
                        for (index, candidate) in candidates.iter().enumerate().rev() {
                            let status = if index + 1 == candidates.len() {
                                "selected"
                            } else {
                                "earlier"
                            };
                            println!(
                                "  {status}: {} · {} · {} = {}",
                                candidate["source"].as_str().unwrap_or("builtin"),
                                candidate["scope"].as_str().unwrap_or("global"),
                                candidate["target"].as_str().unwrap_or("general"),
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

/// The CLI and in-process Desktop read share this inspection.
pub fn inspect(
    repo: &std::path::Path,
    wave: Option<&str>,
    skill: Option<&str>,
    graph: Option<&str>,
    step: Option<&str>,
) -> anyhow::Result<Vec<serde_json::Value>> {
    anyhow::ensure!(
        skill.is_none() || graph.is_none(),
        "choose --skill or --graph"
    );
    anyhow::ensure!(step.is_none() || graph.is_some(), "--step requires --graph");
    let config = crate::config::load_wave_config(Some(repo), wave)?.unwrap_or_default();
    let mut selections = Vec::new();
    if let Some(graph) = graph {
        fn collect(
            steps: &[crate::graph::ConcreteStep],
            id: Option<&str>,
            out: &mut Vec<crate::graph::Skill>,
        ) {
            for step in steps {
                match step {
                    crate::graph::ConcreteStep::Skill(skill)
                        if id.is_none() || skill.id.as_deref() == id =>
                    {
                        out.push(skill.skill.clone())
                    }
                    crate::graph::ConcreteStep::Xor(branch) => {
                        if id.is_none() {
                            out.push(branch.router.clone());
                        }
                        for path in branch.paths.values() {
                            collect(&path.steps, id, out);
                        }
                    }
                    _ => {}
                }
            }
        }
        let definition = crate::graph::load_authored_graph(graph, repo)?;
        collect(
            &crate::graph::compile_graph(&definition, repo)?,
            step,
            &mut selections,
        );
        anyhow::ensure!(!selections.is_empty(), "no matching agent steps in {graph}");
    } else if let Some(skill) = skill {
        selections.push(crate::graph::load_skill(skill, repo)?);
    }
    let mut output = Vec::new();
    for selection in if selections.is_empty() {
        vec![None]
    } else {
        selections.iter().map(Some).collect()
    } {
        let resolved = crate::config::settings::resolve(&config, selection)?;
        output.push(serde_json::json!({"skill": selection.map(|s| &s.name), "occurrence": selection.and_then(|s| s.occurrence.as_ref()), "settings": resolved.settings}));
    }
    Ok(output)
}
