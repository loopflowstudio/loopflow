pub fn print_user(json: bool) -> anyhow::Result<()> {
    let name = crate::config::load_user_name()?;
    if json {
        println!("{}", serde_json::to_string(&name)?);
    } else if let Some(name) = name {
        println!("{name}");
    }
    Ok(())
}
