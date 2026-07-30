use std::fmt;
use std::time::Duration;

use sqlx::migrate::Migrator;
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};
use thiserror::Error;
use tracing::instrument;

pub const DATABASE_SCHEMA_VERSION: i32 = 1;
const EXPECTED_POSTGRES_VERSION: &str = "18.4";
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
const MIGRATION_TIMEOUT: Duration = Duration::from_secs(300);
static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

#[derive(Clone)]
pub struct DatabaseOptions {
    pub url: String,
    pub max_connections: u32,
    pub acquire_timeout: Duration,
}

impl fmt::Debug for DatabaseOptions {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DatabaseOptions")
            .field("url", &"[REDACTED]")
            .field("max_connections", &self.max_connections)
            .field("acquire_timeout", &self.acquire_timeout)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseStatus {
    pub server_version: String,
    pub schema_version: i32,
}

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("не удалось подключиться к PostgreSQL")]
    Connect(#[source] sqlx::Error),
    #[error("проверка PostgreSQL завершилась ошибкой")]
    Probe(#[source] sqlx::Error),
    #[error("ожидался PostgreSQL {expected}, получена версия {actual}")]
    IncompatibleServer {
        expected: &'static str,
        actual: String,
    },
    #[error("ожидалась схема БД версии {expected}, получена версия {actual}")]
    IncompatibleSchema { expected: i32, actual: i32 },
    #[error("не удалось применить миграции PostgreSQL")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("не удалось настроить deadline миграции PostgreSQL")]
    MigrationSetup(#[source] sqlx::Error),
    #[error("операция PostgreSQL {operation} превысила deadline {timeout:?}")]
    Timeout {
        operation: &'static str,
        timeout: Duration,
    },
}

#[derive(Clone)]
pub struct PostgresStore {
    pool: PgPool,
}

impl fmt::Debug for PostgresStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresStore")
            .finish_non_exhaustive()
    }
}

impl PostgresStore {
    #[instrument(name = "postgres.connect", skip(options))]
    pub async fn connect(options: &DatabaseOptions) -> Result<Self, StoreError> {
        let pool = PgPoolOptions::new()
            .max_connections(options.max_connections)
            .acquire_timeout(options.acquire_timeout)
            .test_before_acquire(true)
            .connect(&options.url)
            .await
            .map_err(StoreError::Connect)?;

        Ok(Self { pool })
    }

    #[instrument(name = "postgres.migrate", skip(self))]
    pub async fn migrate(&self, application_role: &str) -> Result<(), StoreError> {
        self.ensure_compatible_server().await?;
        let migration = async {
            let mut connection = self
                .pool
                .acquire()
                .await
                .map_err(StoreError::MigrationSetup)?;
            sqlx::query("SET lock_timeout = '10s'")
                .execute(&mut *connection)
                .await
                .map_err(StoreError::MigrationSetup)?;
            sqlx::query("SET statement_timeout = '5min'")
                .execute(&mut *connection)
                .await
                .map_err(StoreError::MigrationSetup)?;
            // Leaving pg_catalog implicit keeps it ahead of public for name
            // resolution, while unqualified migration metadata is created in public.
            sqlx::query("SET search_path = public")
                .execute(&mut *connection)
                .await
                .map_err(StoreError::MigrationSetup)?;
            sqlx::query("SELECT set_config('wikinext.application_role', $1, false)")
                .bind(application_role)
                .execute(&mut *connection)
                .await
                .map_err(StoreError::MigrationSetup)?;
            MIGRATOR.run(&mut *connection).await?;
            Ok::<(), StoreError>(())
        };
        tokio::time::timeout(MIGRATION_TIMEOUT, migration)
            .await
            .map_err(|_| StoreError::Timeout {
                operation: "migration",
                timeout: MIGRATION_TIMEOUT,
            })??;
        self.diagnose().await?;
        Ok(())
    }

    #[instrument(name = "postgres.probe", skip(self))]
    pub async fn diagnose(&self) -> Result<DatabaseStatus, StoreError> {
        let server_version = self.ensure_compatible_server().await?;
        let row = tokio::time::timeout(
            PROBE_TIMEOUT,
            sqlx::query(
                "SELECT version AS schema_version \
                 FROM public.wikinext_schema_state \
                 WHERE component = 'database'",
            )
            .fetch_one(&self.pool),
        )
        .await
        .map_err(|_| StoreError::Timeout {
            operation: "schema probe",
            timeout: PROBE_TIMEOUT,
        })?
        .map_err(StoreError::Probe)?;
        let schema_version: i32 = row.try_get("schema_version").map_err(StoreError::Probe)?;

        if schema_version != DATABASE_SCHEMA_VERSION {
            return Err(StoreError::IncompatibleSchema {
                expected: DATABASE_SCHEMA_VERSION,
                actual: schema_version,
            });
        }

        Ok(DatabaseStatus {
            server_version,
            schema_version,
        })
    }

    async fn ensure_compatible_server(&self) -> Result<String, StoreError> {
        let row = tokio::time::timeout(
            PROBE_TIMEOUT,
            sqlx::query("SELECT current_setting('server_version') AS server_version")
                .fetch_one(&self.pool),
        )
        .await
        .map_err(|_| StoreError::Timeout {
            operation: "server version probe",
            timeout: PROBE_TIMEOUT,
        })?
        .map_err(StoreError::Probe)?;
        let server_version: String = row.try_get("server_version").map_err(StoreError::Probe)?;

        if !is_expected_postgres_version(&server_version) {
            return Err(StoreError::IncompatibleServer {
                expected: EXPECTED_POSTGRES_VERSION,
                actual: server_version,
            });
        }

        Ok(server_version)
    }
}

fn is_expected_postgres_version(actual: &str) -> bool {
    actual == EXPECTED_POSTGRES_VERSION
        || actual
            .strip_prefix(EXPECTED_POSTGRES_VERSION)
            .is_some_and(|suffix| suffix.starts_with(' ') || suffix.starts_with('.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_options_redact_credentials() {
        let options = DatabaseOptions {
            url: "postgres://wiki:secret@localhost/wiki".to_owned(),
            max_connections: 16,
            acquire_timeout: Duration::from_secs(5),
        };

        let rendered = format!("{options:?}");
        assert!(!rendered.contains("secret"));
        assert!(rendered.contains("[REDACTED]"));
    }

    #[test]
    fn accepts_expected_postgres_patch_builds() {
        assert!(is_expected_postgres_version("18.4"));
        assert!(is_expected_postgres_version("18.4 (Debian 18.4-1)"));
        assert!(is_expected_postgres_version("18.4.1"));
    }

    #[test]
    fn rejects_other_postgres_versions() {
        assert!(!is_expected_postgres_version("18.3"));
        assert!(!is_expected_postgres_version("19.0"));
        assert!(!is_expected_postgres_version("18.40"));
    }
}
