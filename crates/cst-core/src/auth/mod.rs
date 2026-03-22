//! Authentication — one module per auth type.
//!
//! Each module is responsible for:
//! 1. Storing credentials securely
//! 2. Injecting the correct environment variables on profile activate
//! 3. Validating credentials (health check)

pub mod apikey;
pub mod bedrock;
pub mod oauth;
pub mod secrets;
pub mod vertex;

pub use secrets::SecretSource;

use anyhow::Result;
use std::collections::HashMap;

/// Environment variables to export when a profile is activated.
pub type EnvMap = HashMap<String, String>;

/// Common interface for all auth providers.
pub trait AuthProvider: Send + Sync {
    /// Produce the environment variables needed to authenticate.
    fn env_vars(&self) -> Result<EnvMap>;

    /// Validate that credentials are present and usable.
    fn validate(&self) -> Result<()>;
}
