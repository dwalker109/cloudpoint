use crate::{
    app::AppState,
    services::updater::{UpdaterMode, client_app::Updater},
};
use clap::Parser;
use dialoguer::Confirm;
use sqlx::postgres::PgPoolOptions;
use std::{path::PathBuf, time::Duration};

mod app;
mod handlers;
mod hex_u128;
mod postcard;
mod services;

mod util {
    pub mod gc;
    pub mod import_v0;
    pub mod verify;
}

#[derive(Debug, clap::Parser)]
struct Cli {
    /// Database connection details
    #[arg(long, env = "DATABASE_URL")]
    database_url: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, clap::Subcommand)]
enum Command {
    /// Run Cloudpoint and wait for connections
    Serve {
        /// Disable checking GitHub for new app releases
        #[arg(long, env = "DISABLE_APP_UPDATER")]
        disable_app_updater: bool,
    },
    /// Import V0 (DUFS) data from the filesystem and exit
    ImportV0 { root: PathBuf },
    /// GC unreachable chunks
    Gc,
    /// Verify all versions can reach their chunks
    Verify,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    let db_pool = PgPoolOptions::new()
        .max_connections(20)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&cli.database_url)
        .await?;

    sqlx::migrate!().run(&db_pool).await?;

    match cli.command {
        Command::ImportV0 { root } => {
            util::import_v0::run(&db_pool, &root).await?;
        }
        Command::Gc => {
            if let Ok(true) = Confirm::new()
                .with_prompt("Ensure the server is NOT running when you run gc! Continue?")
                .interact()
            {
                util::gc::run(&db_pool).await?;
            }
        }
        Command::Verify => {
            util::verify::run(&db_pool).await?;
        }
        Command::Serve {
            disable_app_updater,
        } => {
            let app_updater = if disable_app_updater {
                Updater::new(UpdaterMode::Disabled)
            } else {
                Updater::new(UpdaterMode::Enabled)
            };

            axum::serve(
                tokio::net::TcpListener::bind("0.0.0.0:6776").await?,
                app::make(AppState {
                    db_pool,
                    app_updater,
                }),
            )
            .await?;
        }
    }

    Ok(())
}
