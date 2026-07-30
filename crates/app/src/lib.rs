pub mod config;
pub mod password;
mod runtime;

pub use runtime::{
    AppServices, CheckStatus, DiagnosticCheck, DiagnosticReport, StartupError, migrate, run_doctor,
};

pub fn service_name() -> &'static str {
    "wikinext"
}
