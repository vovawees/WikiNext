use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use thiserror::Error;
use tracing::{error, info};
use wikinext_app::config::{Config, ConfigError, MigrationConfig};
use wikinext_app::{AppServices, StartupError};
use wikinext_server::{ServerError, TelemetryError};

#[derive(Debug, Parser)]
#[command(name = "wikinext", version, about = "WikiNEXT wiki engine")]
struct Cli {
    #[arg(
        long,
        global = true,
        env = "WIKINEXT_CONFIG",
        default_value = "config.toml"
    )]
    config: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Запустить HTTP-сервер и встроенные фоновые службы.
    Serve,
    /// Применить forward-only миграции PostgreSQL.
    Migrate,
    /// Проверить PostgreSQL, Meilisearch и локальное хранилище.
    Doctor {
        /// Вывести отчёт в JSON.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Error)]
enum CliError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Telemetry(#[from] TelemetryError),
    #[error(transparent)]
    Startup(#[from] StartupError),
    #[error(transparent)]
    Server(#[from] ServerError),
    #[error("не удалось сериализовать диагностический отчёт")]
    Serialize(#[from] serde_json::Error),
    #[error("doctor обнаружил ошибки")]
    DoctorFailed,
}

#[tokio::main]
async fn main() -> ExitCode {
    match run(Cli::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            error!(%error, "команда завершилась с ошибкой");
            eprintln!("Ошибка: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Command::Serve => {
            let config = Config::load(&cli.config)?;
            wikinext_server::init_tracing(&config.observability)?;
            let services = AppServices::connect(&config).await?;
            wikinext_server::serve(
                config.server.bind,
                services,
                config.server.request_timeout(),
                config.server.shutdown_timeout(),
            )
            .await?;
        }
        Command::Migrate => {
            let config = MigrationConfig::load(&cli.config)?;
            wikinext_server::init_tracing(&config.observability)?;
            wikinext_app::migrate(&config).await?;
            info!("миграции успешно применены");
        }
        Command::Doctor { json } => {
            let config = Config::load(&cli.config)?;
            wikinext_server::init_tracing(&config.observability)?;
            let report = wikinext_app::run_doctor(&config).await;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                for check in &report.checks {
                    let marker = if check.is_ok() { "OK" } else { "ОШИБКА" };
                    println!("[{marker}] {}: {}", check.component, check.detail);
                }
            }

            if !report.healthy {
                return Err(CliError::DoctorFailed);
            }
        }
    }

    Ok(())
}
