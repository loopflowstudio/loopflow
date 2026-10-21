use std::path::PathBuf;

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use loopflow::world::{recipes, World};

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
    match Args::parse().command {
        Command::Build { recipe, dir } => {
            let recipe = recipes::find(&recipe)
                .ok_or_else(|| anyhow!("unknown recipe {recipe}; see `lf-world list`"))?;
            let world = World::create(&dir)?;
            (recipe.build)(&world)?;
            let env = world.finish()?;
            println!("{}", serde_json::to_string_pretty(&env)?);
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
