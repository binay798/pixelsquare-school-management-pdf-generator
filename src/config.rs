//! Configuration of the PDF generator service.
//!
//! Every value can be overridden through an environment variable so the same
//! binary can be deployed in different environments without a rebuild.

use std::env;
use std::path::PathBuf;

/// Environment variable used to override the host the server binds to.
pub const ENV_HOST: &str = "PDF_GEN_HOST";
/// Environment variable used to override the port the server binds to.
pub const ENV_PORT: &str = "PDF_GEN_PORT";
/// Environment variable used to override the templates directory.
pub const ENV_TEMPLATES_DIR: &str = "PDF_GEN_TEMPLATES_DIR";
/// Environment variable used to override the output (pdfs) directory.
pub const ENV_OUTPUT_DIR: &str = "PDF_GEN_OUTPUT_DIR";
/// Environment variable used to override the maximum accepted body size (bytes).
pub const ENV_MAX_PAYLOAD_BYTES: &str = "PDF_GEN_MAX_PAYLOAD_BYTES";
/// Environment variable used to override the amount of parallel renders.
pub const ENV_MAX_CONCURRENT_RENDERS: &str = "PDF_GEN_MAX_CONCURRENT_RENDERS";
/// Environment variable used to disable scanning of the system fonts.
pub const ENV_DISABLE_SYSTEM_FONTS: &str = "PDF_GEN_DISABLE_SYSTEM_FONTS";
/// Environment variable with extra font directories (separated by `:`).
pub const ENV_EXTRA_FONT_DIRS: &str = "PDF_GEN_FONT_DIRS";
/// Environment variable controlling the log filter (same syntax as `RUST_LOG`).
pub const ENV_LOG: &str = "RUST_LOG";

const DEFAULT_HOST: &str = "0.0.0.0";
const DEFAULT_PORT: u16 = 8080;
const DEFAULT_TEMPLATES_DIR: &str = "templates";
const DEFAULT_OUTPUT_DIR: &str = "pdfs";
const DEFAULT_MAX_PAYLOAD_BYTES: usize = 2 * 1024 * 1024; // 2 MiB

/// Runtime configuration of the service.
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Address the HTTP server binds to.
    pub host: String,
    /// Port the HTTP server binds to.
    pub port: u16,
    /// Directory that contains the `.typ` templates.
    pub templates_dir: PathBuf,
    /// Directory the generated PDFs are written to.
    pub output_dir: PathBuf,
    /// Maximum accepted request body size in bytes.
    pub max_payload_bytes: usize,
    /// Maximum amount of Typst compilations that may run in parallel.
    pub max_concurrent_renders: usize,
    /// Whether fonts installed on the host should be made available to Typst.
    pub load_system_fonts: bool,
    /// Additional directories that are scanned for fonts.
    pub extra_font_dirs: Vec<PathBuf>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            host: DEFAULT_HOST.to_string(),
            port: DEFAULT_PORT,
            templates_dir: PathBuf::from(DEFAULT_TEMPLATES_DIR),
            output_dir: PathBuf::from(DEFAULT_OUTPUT_DIR),
            max_payload_bytes: DEFAULT_MAX_PAYLOAD_BYTES,
            max_concurrent_renders: default_concurrency(),
            load_system_fonts: true,
            extra_font_dirs: Vec::new(),
        }
    }
}

impl AppConfig {
    /// Builds the configuration from environment variables, falling back to
    /// sensible defaults for every value that is not set.
    pub fn from_env() -> Self {
        let defaults = Self::default();

        Self {
            host: env_string(ENV_HOST).unwrap_or(defaults.host),
            port: env_parsed(ENV_PORT).unwrap_or(defaults.port),
            templates_dir: env_string(ENV_TEMPLATES_DIR)
                .map(PathBuf::from)
                .unwrap_or(defaults.templates_dir),
            output_dir: env_string(ENV_OUTPUT_DIR)
                .map(PathBuf::from)
                .unwrap_or(defaults.output_dir),
            max_payload_bytes: env_parsed(ENV_MAX_PAYLOAD_BYTES)
                .unwrap_or(defaults.max_payload_bytes),
            max_concurrent_renders: env_parsed(ENV_MAX_CONCURRENT_RENDERS)
                .unwrap_or(defaults.max_concurrent_renders)
                .max(1),
            load_system_fonts: !env_flag(ENV_DISABLE_SYSTEM_FONTS),
            extra_font_dirs: env_string(ENV_EXTRA_FONT_DIRS)
                .map(|raw| {
                    env::split_paths(&raw)
                        .filter(|path| !path.as_os_str().is_empty())
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    /// Address the HTTP server binds to, e.g. `0.0.0.0:8080`.
    pub fn binding_address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

fn env_string(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn env_parsed<T: std::str::FromStr>(key: &str) -> Option<T> {
    env_string(key).and_then(|value| value.parse().ok())
}

fn env_flag(key: &str) -> bool {
    match env_string(key) {
        Some(value) => matches!(
            value.to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        None => false,
    }
}

fn default_concurrency() -> usize {
    std::thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_usable() {
        let config = AppConfig::default();
        assert_eq!(config.port, DEFAULT_PORT);
        assert_eq!(config.templates_dir, PathBuf::from(DEFAULT_TEMPLATES_DIR));
        assert_eq!(config.output_dir, PathBuf::from(DEFAULT_OUTPUT_DIR));
        assert!(config.max_concurrent_renders >= 1);
        assert_eq!(config.binding_address(), "0.0.0.0:8080");
    }
}
