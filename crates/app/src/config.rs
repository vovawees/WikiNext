use std::env;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::net::{IpAddr, SocketAddr};
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

use serde::Deserialize;
use thiserror::Error;
use url::{Host, Url};

const MAX_CONFIG_BYTES: u64 = 1024 * 1024;
const MIGRATION_URL_ENV: &str = "WIKINEXT_DATABASE_MIGRATION_URL";
const AMBIENT_POSTGRES_ENV: &[&str] = &[
    "PGPORT",
    "PGHOSTADDR",
    "PGHOST",
    "PGUSER",
    "PGDATABASE",
    "PGPASSWORD",
    "PGSSLROOTCERT",
    "PGSSLCERT",
    "PGSSLKEY",
    "PGSSLMODE",
    "PGAPPNAME",
    "PGOPTIONS",
    "PGPASSFILE",
];

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub app: AppConfig,
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub search: SearchConfig,
    pub storage: StorageConfig,
    #[serde(default)]
    pub observability: ObservabilityConfig,
}

#[derive(Clone, Debug)]
pub struct MigrationConfig {
    pub database: DatabaseConfig,
    pub observability: ObservabilityConfig,
    migration_url: SecretString,
    application_role: String,
}

#[derive(Deserialize)]
struct MigrationConfigFile {
    database: DatabaseConfig,
    #[serde(default)]
    observability: ObservabilityConfig,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppConfig {
    pub name: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerConfig {
    pub bind: SocketAddr,
    #[serde(default = "default_request_timeout_ms")]
    pub request_timeout_ms: u64,
    #[serde(default = "default_shutdown_timeout_seconds")]
    pub shutdown_timeout_seconds: u64,
}

impl ServerConfig {
    pub fn request_timeout(&self) -> Duration {
        Duration::from_millis(self.request_timeout_ms)
    }

    pub fn shutdown_timeout(&self) -> Duration {
        Duration::from_secs(self.shutdown_timeout_seconds)
    }
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatabaseConfig {
    pub url: String,
    #[serde(default = "default_database_connections")]
    pub max_connections: u32,
    #[serde(default = "default_database_acquire_timeout_ms")]
    pub acquire_timeout_ms: u64,
}

impl DatabaseConfig {
    pub fn acquire_timeout(&self) -> Duration {
        Duration::from_millis(self.acquire_timeout_ms)
    }
}

impl fmt::Debug for DatabaseConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DatabaseConfig")
            .field("url", &"[REDACTED]")
            .field("max_connections", &self.max_connections)
            .field("acquire_timeout_ms", &self.acquire_timeout_ms)
            .finish()
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchConfig {
    pub url: String,
    pub api_key: SecretString,
    #[serde(default = "default_search_timeout_ms")]
    pub request_timeout_ms: u64,
}

impl SearchConfig {
    pub fn request_timeout(&self) -> Duration {
        Duration::from_millis(self.request_timeout_ms)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageConfig {
    pub data_dir: PathBuf,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservabilityConfig {
    #[serde(default = "default_log_filter")]
    pub filter: String,
    #[serde(default)]
    pub format: LogFormat,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            filter: default_log_filter(),
            format: LogFormat::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogFormat {
    #[default]
    Pretty,
    Json,
}

impl FromStr for LogFormat {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "pretty" => Ok(Self::Pretty),
            "json" => Ok(Self::Json),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Deserialize)]
#[serde(transparent)]
pub struct SecretString(String);

impl SecretString {
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("не удалось прочитать конфигурацию {path}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("конфигурация {path} превышает предел {limit} байт")]
    TooLarge { path: PathBuf, limit: u64 },
    #[error("не удалось разобрать конфигурацию")]
    Parse(#[from] toml::de::Error),
    #[error("некорректная переменная окружения {key}")]
    Environment { key: &'static str },
    #[error("переменная окружения {key} разрешена только для команды migrate")]
    ForbiddenEnvironment { key: &'static str },
    #[error(
        "переменная окружения {key} может неявно изменить PostgreSQL DSN; \
         используйте только WIKINEXT_DATABASE_URL"
    )]
    AmbientPostgresEnvironment { key: &'static str },
    #[error("некорректное поле конфигурации {field}: {reason}")]
    Validation {
        field: &'static str,
        reason: &'static str,
    },
}

impl Config {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        reject_runtime_migration_secret(read_environment(MIGRATION_URL_ENV)?)?;
        reject_ambient_postgres_environment()?;
        let path = path.as_ref();
        let text = read_bounded(path)?;
        let mut config: Self = toml::from_str(&text)?;
        config.apply_environment()?;
        config.validate()?;
        Ok(config)
    }

    pub fn from_toml(text: &str) -> Result<Self, ConfigError> {
        let config: Self = toml::from_str(text)?;
        config.validate()?;
        Ok(config)
    }

    fn apply_environment(&mut self) -> Result<(), ConfigError> {
        override_string("WIKINEXT_APP_NAME", &mut self.app.name)?;
        override_parsed("WIKINEXT_BIND", &mut self.server.bind)?;
        override_parsed(
            "WIKINEXT_REQUEST_TIMEOUT_MS",
            &mut self.server.request_timeout_ms,
        )?;
        override_parsed(
            "WIKINEXT_SHUTDOWN_TIMEOUT_SECONDS",
            &mut self.server.shutdown_timeout_seconds,
        )?;
        override_string("WIKINEXT_DATABASE_URL", &mut self.database.url)?;
        override_parsed(
            "WIKINEXT_DATABASE_MAX_CONNECTIONS",
            &mut self.database.max_connections,
        )?;
        override_parsed(
            "WIKINEXT_DATABASE_ACQUIRE_TIMEOUT_MS",
            &mut self.database.acquire_timeout_ms,
        )?;
        override_string("WIKINEXT_MEILISEARCH_URL", &mut self.search.url)?;
        if let Some(value) = read_environment("WIKINEXT_MEILISEARCH_API_KEY")? {
            self.search.api_key = SecretString(value);
        }
        override_parsed(
            "WIKINEXT_MEILISEARCH_TIMEOUT_MS",
            &mut self.search.request_timeout_ms,
        )?;
        if let Some(value) = read_environment("WIKINEXT_DATA_DIR")? {
            self.storage.data_dir = PathBuf::from(value);
        }
        override_string("WIKINEXT_LOG_FILTER", &mut self.observability.filter)?;
        override_parsed("WIKINEXT_LOG_FORMAT", &mut self.observability.format)?;
        Ok(())
    }

    fn validate(&self) -> Result<(), ConfigError> {
        let app_name = self.app.name.trim();
        if app_name.is_empty() || app_name.chars().count() > 80 {
            return Err(invalid(
                "app.name",
                "нужна строка длиной от 1 до 80 символов",
            ));
        }

        if self.server.request_timeout_ms == 0 || self.server.request_timeout_ms > 120_000 {
            return Err(invalid(
                "server.request_timeout_ms",
                "допустим диапазон 1..=120000",
            ));
        }

        if self.server.shutdown_timeout_seconds == 0 || self.server.shutdown_timeout_seconds > 300 {
            return Err(invalid(
                "server.shutdown_timeout_seconds",
                "допустим диапазон 1..=300",
            ));
        }

        validate_database_config(&self.database)?;

        validate_search_url(&self.search.url)?;

        if self.search.api_key.expose().len() < 16 {
            return Err(invalid(
                "search.api_key",
                "ключ Meilisearch должен содержать не менее 16 байт",
            ));
        }

        if self.search.request_timeout_ms == 0 || self.search.request_timeout_ms > 60_000 {
            return Err(invalid(
                "search.request_timeout_ms",
                "допустим диапазон 1..=60000",
            ));
        }

        let storage_components: Vec<_> = self.storage.data_dir.components().collect();
        if self.storage.data_dir.as_os_str().is_empty()
            || storage_components
                .iter()
                .any(|component| matches!(component, Component::ParentDir))
            || !storage_components
                .iter()
                .any(|component| matches!(component, Component::Normal(_)))
        {
            return Err(invalid(
                "storage.data_dir",
                "нужен отдельный каталог без переходов через ..",
            ));
        }

        validate_observability(&self.observability)
    }
}

impl MigrationConfig {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        reject_ambient_postgres_environment()?;
        let path = path.as_ref();
        let text = read_bounded(path)?;
        let mut file: MigrationConfigFile = toml::from_str(&text)?;
        file.apply_environment()?;
        let migration_url =
            read_environment(MIGRATION_URL_ENV)?.unwrap_or_else(|| file.database.url.clone());
        Self::build(file, migration_url)
    }

    pub fn from_toml(text: &str) -> Result<Self, ConfigError> {
        let file: MigrationConfigFile = toml::from_str(text)?;
        let migration_url = file.database.url.clone();
        Self::build(file, migration_url)
    }

    fn build(file: MigrationConfigFile, migration_url: String) -> Result<Self, ConfigError> {
        validate_database_config(&file.database)?;
        validate_database_url("migration.database_url", &migration_url)?;
        database_role("migration.database_url", &migration_url)?;
        validate_observability(&file.observability)?;
        let application_role = database_role("database.url", &file.database.url)?;

        Ok(Self {
            database: file.database,
            observability: file.observability,
            migration_url: SecretString(migration_url),
            application_role,
        })
    }

    pub fn migration_url(&self) -> &str {
        self.migration_url.expose()
    }

    pub fn application_role(&self) -> &str {
        &self.application_role
    }
}

impl MigrationConfigFile {
    fn apply_environment(&mut self) -> Result<(), ConfigError> {
        override_string("WIKINEXT_DATABASE_URL", &mut self.database.url)?;
        override_parsed(
            "WIKINEXT_DATABASE_MAX_CONNECTIONS",
            &mut self.database.max_connections,
        )?;
        override_parsed(
            "WIKINEXT_DATABASE_ACQUIRE_TIMEOUT_MS",
            &mut self.database.acquire_timeout_ms,
        )?;
        override_string("WIKINEXT_LOG_FILTER", &mut self.observability.filter)?;
        override_parsed("WIKINEXT_LOG_FORMAT", &mut self.observability.format)?;
        Ok(())
    }
}

fn read_bounded(path: &Path) -> Result<String, ConfigError> {
    let file = File::open(path).map_err(|source| ConfigError::Read {
        path: path.to_owned(),
        source,
    })?;
    let mut text = String::new();
    file.take(MAX_CONFIG_BYTES + 1)
        .read_to_string(&mut text)
        .map_err(|source| ConfigError::Read {
            path: path.to_owned(),
            source,
        })?;

    if text.len() as u64 > MAX_CONFIG_BYTES {
        return Err(ConfigError::TooLarge {
            path: path.to_owned(),
            limit: MAX_CONFIG_BYTES,
        });
    }

    Ok(text)
}

fn override_string(key: &'static str, target: &mut String) -> Result<(), ConfigError> {
    if let Some(value) = read_environment(key)? {
        *target = value;
    }
    Ok(())
}

fn override_parsed<T>(key: &'static str, target: &mut T) -> Result<(), ConfigError>
where
    T: FromStr,
{
    if let Some(value) = read_environment(key)? {
        *target = value
            .parse()
            .map_err(|_| ConfigError::Environment { key })?;
    }
    Ok(())
}

fn read_environment(key: &'static str) -> Result<Option<String>, ConfigError> {
    match env::var(key) {
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => Err(ConfigError::Environment { key }),
    }
}

fn reject_runtime_migration_secret(value: Option<String>) -> Result<(), ConfigError> {
    if value.is_some() {
        return Err(ConfigError::ForbiddenEnvironment {
            key: MIGRATION_URL_ENV,
        });
    }
    Ok(())
}

fn reject_ambient_postgres_environment() -> Result<(), ConfigError> {
    for &key in AMBIENT_POSTGRES_ENV {
        reject_ambient_postgres_value(key, read_environment(key)?)?;
    }
    Ok(())
}

fn reject_ambient_postgres_value(
    key: &'static str,
    value: Option<String>,
) -> Result<(), ConfigError> {
    if value.is_some() {
        return Err(ConfigError::AmbientPostgresEnvironment { key });
    }
    Ok(())
}

fn invalid(field: &'static str, reason: &'static str) -> ConfigError {
    ConfigError::Validation { field, reason }
}

fn validate_database_config(database: &DatabaseConfig) -> Result<(), ConfigError> {
    if !(1..=128).contains(&database.max_connections) {
        return Err(invalid(
            "database.max_connections",
            "допустим диапазон 1..=128",
        ));
    }
    if database.acquire_timeout_ms == 0 || database.acquire_timeout_ms > 60_000 {
        return Err(invalid(
            "database.acquire_timeout_ms",
            "допустим диапазон 1..=60000",
        ));
    }
    validate_database_url("database.url", &database.url)?;
    database_role("database.url", &database.url)?;
    Ok(())
}

fn validate_database_url(field: &'static str, value: &str) -> Result<(), ConfigError> {
    let url = Url::parse(value).map_err(|_| invalid(field, "некорректный PostgreSQL URL"))?;
    if !matches!(url.scheme(), "postgres" | "postgresql") {
        return Err(invalid(field, "ожидается postgres:// или postgresql://"));
    }
    if url.host().is_none() {
        return Err(invalid(field, "PostgreSQL URL должен явно задавать host"));
    }
    if url.password().is_none_or(str::is_empty) {
        return Err(invalid(
            field,
            "PostgreSQL URL должен явно задавать непустой password",
        ));
    }
    if url.path().trim_matches('/').is_empty() {
        return Err(invalid(
            field,
            "PostgreSQL URL должен явно задавать имя базы",
        ));
    }
    if url.fragment().is_some() {
        return Err(invalid(field, "fragment в PostgreSQL URL запрещён"));
    }

    let mut ssl_mode = None;
    for (key, value) in url.query_pairs() {
        let key = key.as_ref();
        if matches!(
            key,
            "host" | "hostaddr" | "user" | "password" | "dbname" | "port" | "options"
        ) || key.starts_with("options[")
        {
            return Err(invalid(
                field,
                "query-переопределение параметров подключения запрещено",
            ));
        }
        match key {
            "ssl-mode" => {
                return Err(invalid(field, "используйте только параметр sslmode"));
            }
            "sslmode" if ssl_mode.replace(value.into_owned()).is_some() => {
                return Err(invalid(field, "параметр sslmode нельзя дублировать"));
            }
            "sslmode" => {}
            _ => {
                return Err(invalid(
                    field,
                    "из query-параметров разрешён только sslmode",
                ));
            }
        }
    }

    let loopback = url_host_is_loopback(&url);
    let verifies_server = ssl_mode.as_deref() == Some("verify-full");

    if !loopback && !verifies_server {
        return Err(invalid(
            field,
            "удалённый PostgreSQL требует sslmode=verify-full",
        ));
    }
    Ok(())
}

fn validate_search_url(value: &str) -> Result<(), ConfigError> {
    let url = Url::parse(value).map_err(|_| invalid("search.url", "некорректный URL"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(invalid("search.url", "ожидается http:// или https://"));
    }
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid(
            "search.url",
            "URL не должен содержать credentials, query или fragment",
        ));
    }
    if url.scheme() == "http" && !url_host_is_loopback(&url) {
        return Err(invalid("search.url", "удалённый Meilisearch требует HTTPS"));
    }
    Ok(())
}

fn database_role(field: &'static str, value: &str) -> Result<String, ConfigError> {
    let url = Url::parse(value).map_err(|_| invalid(field, "некорректный PostgreSQL URL"))?;
    let role = url.username();
    let valid = !role.is_empty()
        && role.len() <= 63
        && role
            .chars()
            .next()
            .is_some_and(|character| character == '_' || character.is_ascii_alphabetic())
        && role
            .chars()
            .all(|character| character == '_' || character.is_ascii_alphanumeric());

    if !valid {
        return Err(invalid(
            field,
            "имя роли PostgreSQL должно быть простым ASCII identifier",
        ));
    }
    Ok(role.to_owned())
}

fn url_host_is_loopback(url: &Url) -> bool {
    match url.host() {
        Some(Host::Domain(host)) => {
            host.eq_ignore_ascii_case("localhost")
                || host
                    .parse::<IpAddr>()
                    .is_ok_and(|address| address.is_loopback())
        }
        Some(Host::Ipv4(address)) => address.is_loopback(),
        Some(Host::Ipv6(address)) => address.is_loopback(),
        None => false,
    }
}

fn validate_observability(config: &ObservabilityConfig) -> Result<(), ConfigError> {
    if config.filter.trim().is_empty() {
        return Err(invalid(
            "observability.filter",
            "фильтр логов не может быть пустым",
        ));
    }
    Ok(())
}

const fn default_request_timeout_ms() -> u64 {
    10_000
}

const fn default_shutdown_timeout_seconds() -> u64 {
    20
}

const fn default_database_connections() -> u32 {
    16
}

const fn default_database_acquire_timeout_ms() -> u64 {
    5_000
}

const fn default_search_timeout_ms() -> u64 {
    2_000
}

fn default_log_filter() -> String {
    "wikinext=info,tower_http=info".to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_CONFIG: &str = r#"
        [app]
        name = "WikiNEXT"

        [server]
        bind = "127.0.0.1:3000"

        [database]
        url = "postgres://wikinext:database-secret@127.0.0.1/wikinext"

        [search]
        url = "http://127.0.0.1:7700"
        api_key = "search-development-key"

        [storage]
        data_dir = "./data"
    "#;

    #[test]
    fn parses_defaults_and_redacts_secrets() {
        let config = Config::from_toml(VALID_CONFIG).expect("valid config");

        assert_eq!(config.server.request_timeout_ms, 10_000);
        assert_eq!(config.database.max_connections, 16);

        let rendered = format!("{config:?}");
        assert!(!rendered.contains("database-secret"));
        assert!(!rendered.contains("search-development-key"));
    }

    #[test]
    fn rejects_unknown_fields() {
        let text =
            VALID_CONFIG.replace("name = \"WikiNEXT\"", "name = \"WikiNEXT\"\nunknown = true");

        assert!(matches!(
            Config::from_toml(&text),
            Err(ConfigError::Parse(_))
        ));
    }

    #[test]
    fn rejects_root_storage_directory() {
        let text = VALID_CONFIG.replace("data_dir = \"./data\"", "data_dir = \"/\"");

        assert!(matches!(
            Config::from_toml(&text),
            Err(ConfigError::Validation {
                field: "storage.data_dir",
                ..
            })
        ));
    }

    #[test]
    fn rejects_storage_parent_traversal() {
        let text = VALID_CONFIG.replace("data_dir = \"./data\"", "data_dir = \"/tmp/..\"");

        assert!(matches!(
            Config::from_toml(&text),
            Err(ConfigError::Validation {
                field: "storage.data_dir",
                ..
            })
        ));
    }

    #[test]
    fn remote_database_requires_verified_tls() {
        let insecure = VALID_CONFIG.replace("127.0.0.1/wikinext", "db.example.test/wikinext");
        assert!(matches!(
            Config::from_toml(&insecure),
            Err(ConfigError::Validation {
                field: "database.url",
                ..
            })
        ));

        let secure = insecure.replace(
            "db.example.test/wikinext",
            "db.example.test/wikinext?sslmode=verify-full",
        );
        Config::from_toml(&secure).expect("verified remote database URL is accepted");
    }

    #[test]
    fn rejects_database_url_override_bypasses() {
        for suffix in [
            "?sslmode=verify-full&sslmode=disable",
            "?sslmode=verify-full&ssl-mode=disable",
            "?host=127.0.0.1&sslmode=verify-full",
            "?hostaddr=127.0.0.1&sslmode=verify-full",
            "?user=other_role&sslmode=verify-full",
            "?options=-csearch_path%3Dattacker&sslmode=verify-full",
            "?options%5Bsearch_path%5D=attacker&sslmode=verify-full",
            "?passwrod=secret&sslmode=verify-full",
        ] {
            let text = VALID_CONFIG.replace(
                "127.0.0.1/wikinext",
                &format!("db.example.test/wikinext{suffix}"),
            );
            assert!(matches!(
                Config::from_toml(&text),
                Err(ConfigError::Validation {
                    field: "database.url",
                    ..
                })
            ));
        }
    }

    #[test]
    fn runtime_rejects_process_scoped_migration_secret() {
        assert!(matches!(
            reject_runtime_migration_secret(Some("ddl-secret".to_owned())),
            Err(ConfigError::ForbiddenEnvironment {
                key: MIGRATION_URL_ENV
            })
        ));
        reject_runtime_migration_secret(None).expect("absent migration secret is valid");
    }

    #[test]
    fn rejects_ambient_postgres_configuration() {
        assert!(matches!(
            reject_ambient_postgres_value("PGOPTIONS", Some("-c search_path=attacker".to_owned())),
            Err(ConfigError::AmbientPostgresEnvironment { key: "PGOPTIONS" })
        ));
        reject_ambient_postgres_value("PGOPTIONS", None)
            .expect("absent ambient PostgreSQL configuration is valid");
    }

    #[test]
    fn remote_search_requires_https() {
        let insecure = VALID_CONFIG.replace("http://127.0.0.1:7700", "http://search.example.test");
        assert!(matches!(
            Config::from_toml(&insecure),
            Err(ConfigError::Validation {
                field: "search.url",
                ..
            })
        ));

        let secure = insecure.replace("http://search.example.test", "https://search.example.test");
        Config::from_toml(&secure).expect("remote HTTPS search URL is accepted");
    }

    #[test]
    fn migration_config_needs_only_database_section() {
        let minimal = r#"
            [database]
            url = "postgres://migrator:secret@127.0.0.1/wikinext"
        "#;
        let config = MigrationConfig::from_toml(minimal).expect("minimal migration config");
        assert_eq!(config.database.max_connections, 16);
        assert_eq!(config.application_role(), "migrator");
        assert!(!format!("{config:?}").contains("secret"));

        MigrationConfig::from_toml(VALID_CONFIG)
            .expect("migration loader ignores unrelated application sections");
    }
}
