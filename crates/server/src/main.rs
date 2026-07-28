use tracing::info;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config_path = std::env::var("WIKINEXT_CONFIG").unwrap_or_else(|_| "config.toml".to_owned());

    match wikinext_app::config::Config::load(&config_path) {
        Ok(config) => info!(
            service = wikinext_app::service_name(),
            app = %config.app.name,
            "started"
        ),
        Err(error) => info!(
            service = wikinext_app::service_name(),
            error = %error,
            "config not loaded"
        ),
    }
}
