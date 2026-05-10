//! Pipeline notification dispatch.
//!
//! Native desktop notifications would be a nice-to-have, but adding a
//! `notify-rust` dependency in this crate is out of scope for the initial
//! implementation. We log at INFO via `tracing` and never propagate
//! notification failures — a failed notification must not kill the daemon.

use anyhow::Result;

/// Send a pipeline notification. Errors are swallowed and logged.
pub fn send(title: &str, body: &str) -> Result<()> {
    if let Err(e) = try_send(title, body) {
        tracing::warn!("pipeline notification failed: {e}");
    }
    Ok(())
}

fn try_send(title: &str, body: &str) -> Result<()> {
    tracing::info!("[pipeline] {title}: {body}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_send_never_errors() {
        send("title", "body").unwrap();
    }
}
