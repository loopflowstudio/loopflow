use crate::commands::util::find_repo_root;
use crate::output::Colors;
use crate::Cli;
use anyhow::Result;

pub fn show_all(cli: &Cli) -> Result<()> {
    let repo_root = find_repo_root()?;
    let colors = Colors::default();

    let steps = crate::discovery::list_steps(&repo_root);
    let flows = crate::discovery::list_flows(&repo_root);
    let directions = crate::discovery::list_directions(&repo_root);

    println!(
        "{bold}{cyan}STEPS{reset}",
        bold = colors.bold,
        cyan = colors.cyan,
        reset = colors.reset
    );
    if steps.is_empty() {
        println!(
            "  {dim}(none){reset}",
            dim = colors.dim,
            reset = colors.reset
        );
    } else {
        for name in steps {
            println!(
                "  {bold}{name}{reset}",
                bold = colors.bold,
                name = name,
                reset = colors.reset
            );
        }
    }

    println!();
    println!(
        "{bold}{cyan}FLOWS{reset}",
        bold = colors.bold,
        cyan = colors.cyan,
        reset = colors.reset
    );
    if flows.is_empty() {
        println!(
            "  {dim}(none){reset}",
            dim = colors.dim,
            reset = colors.reset
        );
    } else {
        for name in flows {
            println!(
                "  {bold}{name}{reset}",
                bold = colors.bold,
                name = name,
                reset = colors.reset
            );
        }
    }

    println!();
    println!(
        "{bold}{cyan}DIRECTIONS{reset}",
        bold = colors.bold,
        cyan = colors.cyan,
        reset = colors.reset
    );
    if directions.is_empty() {
        println!(
            "  {dim}(none){reset}",
            dim = colors.dim,
            reset = colors.reset
        );
    } else {
        for name in directions {
            println!(
                "  {bold}{name}{reset}",
                bold = colors.bold,
                name = name,
                reset = colors.reset
            );
        }
    }

    if cli.list {
        println!();
        println!(
            "{dim}Built-ins work anywhere. Run lf <step> or lf <step>: args{reset}",
            dim = colors.dim,
            reset = colors.reset
        );
    }

    Ok(())
}

pub fn steps(_cli: &Cli) -> Result<()> {
    let repo_root = find_repo_root()?;
    for name in crate::discovery::list_steps(&repo_root) {
        println!("{}", name);
    }
    Ok(())
}

pub fn flows(_cli: &Cli) -> Result<()> {
    let repo_root = find_repo_root()?;
    for name in crate::discovery::list_flows(&repo_root) {
        println!("{}", name);
    }
    Ok(())
}

pub fn directions(_cli: &Cli) -> Result<()> {
    let repo_root = find_repo_root()?;
    for name in crate::discovery::list_directions(&repo_root) {
        println!("{}", name);
    }
    Ok(())
}
