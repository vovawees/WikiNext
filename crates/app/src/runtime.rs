use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use thiserror::Error;
use tokio::join;
use tokio::sync::Mutex;
use tokio::time::Instant;
use tracing::warn;
use wikinext_search::{SearchClient, SearchError};
use wikinext_store::{DatabaseOptions, LocalStorage, PostgresStore, StorageError, StoreError};

use crate::config::{Config, DatabaseConfig, MigrationConfig};

const DIAGNOSTIC_CACHE_TTL: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Ok,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiagnosticCheck {
    pub component: &'static str,
    pub status: CheckStatus,
    pub detail: String,
}

impl DiagnosticCheck {
    fn success(component: &'static str, detail: impl Into<String>) -> Self {
        Self {
            component,
            status: CheckStatus::Ok,
            detail: detail.into(),
        }
    }

    fn failure(component: &'static str, error: impl std::fmt::Display) -> Self {
        Self {
            component,
            status: CheckStatus::Error,
            detail: error.to_string(),
        }
    }

    pub fn is_ok(&self) -> bool {
        self.status == CheckStatus::Ok
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiagnosticReport {
    pub healthy: bool,
    pub checks: Vec<DiagnosticCheck>,
}

impl DiagnosticReport {
    fn new(checks: Vec<DiagnosticCheck>) -> Self {
        Self {
            healthy: checks.iter().all(DiagnosticCheck::is_ok),
            checks,
        }
    }
}

#[derive(Debug, Error)]
pub enum StartupError {
    #[error(transparent)]
    Database(#[from] StoreError),
    #[error(transparent)]
    Search(#[from] SearchError),
    #[error(transparent)]
    Storage(#[from] StorageError),
}

#[derive(Clone, Debug)]
pub struct AppServices {
    database: PostgresStore,
    search: SearchClient,
    storage: LocalStorage,
    readiness_cache: Arc<Mutex<Option<CachedReport>>>,
    search_cache: Arc<Mutex<Option<CachedReport>>>,
}

#[derive(Clone, Debug)]
struct CachedReport {
    checked_at: Instant,
    report: DiagnosticReport,
}

impl CachedReport {
    fn fresh(&self) -> bool {
        self.checked_at.elapsed() < DIAGNOSTIC_CACHE_TTL
    }
}

impl AppServices {
    pub async fn connect(config: &Config) -> Result<Self, StartupError> {
        let search = build_search_client(config)?;
        let database_options = database_options(&config.database);
        let database_future = PostgresStore::connect(&database_options);
        let storage_future = LocalStorage::prepare(config.storage.data_dir.clone());
        let (database, storage) = join!(database_future, storage_future);
        let database = database?;
        database.diagnose().await?;

        Ok(Self {
            database,
            search,
            storage: storage?,
            readiness_cache: Arc::new(Mutex::new(None)),
            search_cache: Arc::new(Mutex::new(None)),
        })
    }

    pub async fn readiness(&self) -> DiagnosticReport {
        let mut cache = self.readiness_cache.lock().await;
        if let Some(cached) = cache.as_ref().filter(|cached| cached.fresh()) {
            return cached.report.clone();
        }

        let (database, storage) = join!(self.database.diagnose(), self.storage.diagnose());

        let report = DiagnosticReport::new(vec![
            match database {
                Ok(_) => DiagnosticCheck::success("postgresql", "PostgreSQL доступен"),
                Err(error) => {
                    warn!(%error, "readiness: PostgreSQL недоступен");
                    DiagnosticCheck::failure("postgresql", "PostgreSQL недоступен или несовместим")
                }
            },
            match storage {
                Ok(_) => DiagnosticCheck::success("storage", "локальное хранилище доступно"),
                Err(error) => {
                    warn!(%error, "readiness: локальное хранилище недоступно");
                    DiagnosticCheck::failure("storage", "локальное хранилище недоступно")
                }
            },
        ]);
        *cache = Some(CachedReport {
            checked_at: Instant::now(),
            report: report.clone(),
        });
        report
    }

    pub async fn search_status(&self) -> DiagnosticReport {
        let mut cache = self.search_cache.lock().await;
        if let Some(cached) = cache.as_ref().filter(|cached| cached.fresh()) {
            return cached.report.clone();
        }

        let check = match self.search.diagnose().await {
            Ok(_) => DiagnosticCheck::success("meilisearch", "Meilisearch доступен"),
            Err(error) => {
                warn!(%error, "status: Meilisearch недоступен");
                DiagnosticCheck::failure("meilisearch", "Meilisearch недоступен или несовместим")
            }
        };
        let report = DiagnosticReport::new(vec![check]);
        *cache = Some(CachedReport {
            checked_at: Instant::now(),
            report: report.clone(),
        });
        report
    }
}

pub async fn migrate(config: &MigrationConfig) -> Result<(), StartupError> {
    let options = DatabaseOptions {
        url: config.migration_url().to_owned(),
        max_connections: config.database.max_connections,
        acquire_timeout: config.database.acquire_timeout(),
    };
    let database = PostgresStore::connect(&options).await?;
    database.migrate(config.application_role()).await?;
    Ok(())
}

pub async fn run_doctor(config: &Config) -> DiagnosticReport {
    let database_options = database_options(&config.database);
    let database_future = async {
        let database = PostgresStore::connect(&database_options).await?;
        database.diagnose().await
    };
    let search_future = async {
        let search = build_search_client(config)?;
        search.diagnose().await
    };
    let storage_future = async {
        let storage = LocalStorage::prepare(config.storage.data_dir.clone()).await?;
        storage.status().await
    };

    let (database, search, storage) = join!(database_future, search_future, storage_future);

    DiagnosticReport::new(vec![
        match database {
            Ok(status) => DiagnosticCheck::success(
                "postgresql",
                format!(
                    "PostgreSQL {}, схема {}",
                    status.server_version, status.schema_version
                ),
            ),
            Err(error) => DiagnosticCheck::failure("postgresql", error),
        },
        match search {
            Ok(status) => DiagnosticCheck::success(
                "meilisearch",
                format!("Meilisearch {} доступен", status.version),
            ),
            Err(error) => DiagnosticCheck::failure("meilisearch", error),
        },
        match storage {
            Ok(status) => DiagnosticCheck::success(
                "storage",
                format!("каталог {} доступен", status.root.display()),
            ),
            Err(error) => DiagnosticCheck::failure("storage", error),
        },
    ])
}

fn database_options(config: &DatabaseConfig) -> DatabaseOptions {
    DatabaseOptions {
        url: config.url.clone(),
        max_connections: config.max_connections,
        acquire_timeout: config.acquire_timeout(),
    }
}

fn build_search_client(config: &Config) -> Result<SearchClient, SearchError> {
    SearchClient::new(
        &config.search.url,
        config.search.api_key.expose(),
        config.search.request_timeout(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_is_healthy_only_when_every_check_passes() {
        let healthy = DiagnosticReport::new(vec![DiagnosticCheck::success("test", "ok")]);
        assert!(healthy.healthy);

        let unhealthy = DiagnosticReport::new(vec![
            DiagnosticCheck::success("first", "ok"),
            DiagnosticCheck::failure("second", "failed"),
        ]);
        assert!(!unhealthy.healthy);
    }
}
