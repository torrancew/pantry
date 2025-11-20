mod assets;
mod fswatch;
mod markdown;
mod recipe;
mod routes;
mod search;
mod templates;

use std::{io, net::SocketAddr, path::PathBuf, sync::Arc};

use clap::Parser;
use macro_rules_attribute::apply;
use smol::{net::TcpListener, stream::StreamExt};
use smol_macros::{main, Executor};
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

#[apply(main)]
async fn main(ex: &Arc<Executor<'_>>) -> io::Result<()> {
    let logger = FmtSubscriber::builder()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(tracing::Level::INFO.into())
                .from_env_lossy(),
        )
        .finish();
    tracing::subscriber::set_global_default(logger)?;

    let args = Args::parse();
    let recipe_dir = resolve_recipe_dir(&args)
        .expect("Unable to find data directory, please specify --recipe-dir!");

    let app_state = routes::AppState::new(&recipe_dir);

    let _reloader = {
        let app_state = app_state.clone();
        let mut watcher = fswatch::AsyncWatcher::new(&recipe_dir)?;
        ex.spawn(async move {
            use fswatch::Event;
            while let Some(ev) = watcher.next().await {
                match ev {
                    Event::Update(paths) => app_state.reload(Some(paths)).await,
                    Event::Remove(paths) => app_state.remove(paths).await,
                }
            }
        })
    };

    let listener = TcpListener::bind(args.listen_on).await?;
    info!("Listening on {}", args.listen_on);

    // Perform an initial load of the dataset
    app_state.reload(None).await;
    smol_axum::serve(ex.clone(), listener, routes::router(app_state)).await
}

// https://notgull.net/new-smol-rs-subcrates/
