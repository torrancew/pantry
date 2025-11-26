mod assets;
mod fswatch;
mod markdown;
mod recipe;
mod routes;
mod search;
mod templates;

use std::{net::SocketAddr, path::PathBuf};

use anyhow::Context;
use clap::Parser;
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

#[derive(Parser)]
struct Args {
    #[arg(long, short, env = "PANTRY_ADDRESS", default_value = "127.0.0.1:3000")]
    listen_on: SocketAddr,
    #[arg(long, short = 'd', env = "PANTRY_RECIPE_DIR")]
    recipe_dir: Option<PathBuf>,
}

fn resolve_recipe_dir(args: &Args) -> Option<PathBuf> {
    [
        args.recipe_dir.clone(),
        dirs::data_dir().map(|d| d.join("pantry")),
        dirs::home_dir().map(|d| d.join(".pantry")),
    ]
    .into_iter()
    .find(|d| d.is_some())
    .flatten()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let logger = FmtSubscriber::builder()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(tracing::Level::INFO.into())
                .from_env_lossy(),
        )
        .finish();

    tracing::subscriber::set_global_default(logger)?;

    let args = Args::parse();
    let recipe_dir = resolve_recipe_dir(&args).context("Unable to find recipe directory")?;

    let app_state = routes::AppState::new(&recipe_dir);

    let _reloader = {
        let app_state = app_state.clone();
        let mut watcher = fswatch::AsyncWatcher::new(&recipe_dir)?;
        tokio::spawn(async move {
            while let Some(ev) = watcher.next().await {
                match ev {
                    Ok(fswatch::Event::Update(paths)) => app_state.reload(Some(paths)).await,
                    Ok(fswatch::Event::Remove(paths)) => app_state.remove(paths).await,
                    Err(_) => {}
                }
            }
        })
    };

    let listener = TcpListener::bind(args.listen_on).await?;
    info!("Listening on {}", args.listen_on);

    // Perform an initial load of the dataset
    app_state.reload(None).await;
    Ok(axum::serve(listener, routes::router(app_state).into_make_service()).await?)
}

// https://notgull.net/new-smol-rs-subcrates/
