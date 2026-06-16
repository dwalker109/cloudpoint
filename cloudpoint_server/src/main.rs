use crate::app::AppState;
use clap::Parser;
use sqlx::postgres::PgPoolOptions;
use std::{path::PathBuf, time::Duration};

mod app;
mod handlers;
mod hex_u128;
mod import_v0;
mod services;

#[derive(Debug, clap::Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, clap::Subcommand)]
enum Command {
    /// Run Cloudpoint and wait for connections
    Serve,
    /// Import V0 (DUFS) data from the filesystem and exit
    ImportV0 { root: PathBuf },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let db_pool = PgPoolOptions::new()
        .max_connections(20)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&std::env::var("DATABASE_URL")?)
        .await?;

    sqlx::migrate!().run(&db_pool).await?;

    match Cli::parse().command {
        Command::ImportV0 { root } => {
            import_v0::run(&db_pool, &root).await?;
        }
        Command::Serve => {
            let listener = tokio::net::TcpListener::bind("0.0.0.0:6776").await?;
            axum::serve(listener, app::make(AppState { db_pool })).await?;
        }
    }

    Ok(())
}
