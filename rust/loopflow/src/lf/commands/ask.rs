use std::sync::Arc;

use anyhow::Context;

use crate::lf::AskArgs;
use crate::store::{open_store, storage_config_from_env, Store};

pub fn run(args: &AskArgs) -> anyhow::Result<()> {
    tokio::runtime::Runtime::new()?.block_on(run_async(args))
}

async fn run_async(args: &AskArgs) -> anyhow::Result<()> {
    let question = args.question.join(" ").trim().to_string();
    let store = open_shared_store().await?;
    let summary = crate::ops::human_session::ask(&store, &question).await?;
    println!("Human session complete: {summary}");
    Ok(())
}

async fn open_shared_store() -> anyhow::Result<Arc<Store>> {
    let config = storage_config_from_env().context("resolve the shared Loopflow store")?;
    Ok(Arc::new(
        open_store(&config)
            .await
            .context("open the shared Loopflow store")?,
    ))
}
