use std::path::PathBuf;

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use loopflow::world::{recipes, World, WorldEnv};

#[derive(Parser, Debug)]
#[command(name = "lf-world")]
#[command(about = "Build fake Loopflow worlds for demos and tests")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Build a recipe into a new directory and print its environment
    Build { recipe: String, dir: PathBuf },
    /// List recipes with their rung and what each demos
    List,
}

fn main() -> Result<()> {
    loopflow::build_info::initialize();
    match Args::parse().command {
        Command::Build { recipe, dir } => {
            let recipe = recipes::find(&recipe)
                .ok_or_else(|| anyhow!("unknown recipe {recipe}; see `lf-world list`"))?;
            let world = World::create(&dir)?;
            (recipe.build)(&world)?;
            describe(&world.finish()?)?;
        }
        Command::List => {
            for recipe in recipes::RECIPES {
                println!(
                    "{:<15} {:<10} {}",
                    recipe.name,
                    recipe.rung.as_str(),
                    recipe.demos
                );
            }
        }
    }
    Ok(())
}

/// Print the environment and, on stderr, what its directory's length costs.
fn describe(env: &WorldEnv) -> Result<()> {
    if let Some(notice) = env.desktop_socket_notice() {
        eprintln!("{notice}");
    }
    println!("{}", serde_json::to_string_pretty(env)?);
    Ok(())
}

#[cfg(test)]
#[ctor::ctor(unsafe)]
fn initialize_test_build() {
    loopflow_execution::test_support::isolate_from_the_developers_machine();
    loopflow::build_info::initialize();
}
