//! API key authentication — multi-key pool with pluggable secret providers.
//!
//! Each slot can store its secret in the OS Keychain (default), 1Password,
//! Doppler, or a plain environment variable.  See `auth::secrets::SecretSource`.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::{secrets::SecretSource, EnvMap};

const SERVICE_NAME: &str = "claude-sentinel";

/// A single API key entry in the pool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyEntry {
    /// Slot number (1-based). Determines priority order.
    pub slot: u8,
    /// Where to retrieve this key from.
    ///
    /// Defaults to `SecretSource::Keychain` when absent (legacy format).
    #[serde(
        default = "default_keychain_source",
        skip_serializing_if = "is_default_keychain"
    )]
    pub source: SecretSource,
    /// Keychain account name (kept for backwards-compatibility; used when
    /// `source` is `Keychain`).
    #[serde(default)]
    pub keychain_account: String,
    /// Human note about this key.
    #[serde(default)]
    pub note: String,
}

fn default_keychain_source() -> SecretSource {
    SecretSource::Keychain {
        account: String::new(),
    }
}

fn is_default_keychain(s: &SecretSource) -> bool {
    matches!(s, SecretSource::Keychain { .. })
}

/// The API key pool stored in `auth/api_keys.toml`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiKeyPool {
    pub keys: Vec<ApiKeyEntry>,
}

impl ApiKeyPool {
    /// Store a new API key in the OS Keychain and add it to the pool.
    pub fn add_key(
        &mut self,
        profile_name: &str,
        slot: u8,
        api_key: &str,
        note: &str,
    ) -> Result<()> {
        let account = format!("{profile_name}-slot{slot}");
        store_in_keychain(&account, api_key)?;
        // Remove existing entry for this slot if any
        self.keys.retain(|k| k.slot != slot);
        self.keys.push(ApiKeyEntry {
            slot,
            source: SecretSource::Keychain {
                account: account.clone(),
            },
            keychain_account: account,
            note: note.to_string(),
        });
        self.keys.sort_by_key(|k| k.slot);
        Ok(())
    }

    /// Add a key that lives in an external provider (1Password, Doppler, env var).
    ///
    /// Unlike `add_key`, this does not write anything to the OS Keychain —
    /// the secret stays in the external provider.
    pub fn add_external_key(&mut self, slot: u8, source: SecretSource, note: &str) -> Result<()> {
        source.check_tool_available()?;
        self.keys.retain(|k| k.slot != slot);
        self.keys.push(ApiKeyEntry {
            slot,
            keychain_account: String::new(),
            source,
            note: note.to_string(),
        });
        self.keys.sort_by_key(|k| k.slot);
        Ok(())
    }

    /// Remove a key slot.
    ///
    /// Only deletes from the OS Keychain when the slot's source is
    /// `SecretSource::Keychain`. External providers (1Password, Doppler, env
    /// var) manage their own secrets; calling keychain deletion with an empty
    /// account name would touch a spurious Keychain entry.
    pub fn remove_key(&mut self, slot: u8) -> Result<()> {
        let entry = self
            .keys
            .iter()
            .find(|k| k.slot == slot)
            .ok_or_else(|| anyhow::anyhow!("slot {slot} not found"))?;
        if let SecretSource::Keychain { account } = &entry.source {
            let acct = if account.is_empty() {
                &entry.keychain_account
            } else {
                account
            };
            delete_from_keychain(acct)?;
        }
        self.keys.retain(|k| k.slot != slot);
        Ok(())
    }

    /// Retrieve the API key for a given slot from its configured provider.
    pub fn retrieve_key(&self, slot: u8) -> Result<String> {
        let entry = self
            .keys
            .iter()
            .find(|k| k.slot == slot)
            .ok_or_else(|| anyhow::anyhow!("slot {slot} not found in key pool"))?;
        // Use the source if it carries a real account; otherwise fall back
        // to legacy keychain_account field.
        match &entry.source {
            SecretSource::Keychain { account } => {
                let acct = if account.is_empty() {
                    &entry.keychain_account
                } else {
                    account
                };
                retrieve_from_keychain(acct)
            }
            other => other.retrieve(),
        }
    }

    /// Get the highest-priority valid key's env vars.
    pub fn env_vars_for_slot(&self, slot: u8) -> Result<EnvMap> {
        let key = self.retrieve_key(slot)?;
        let mut map = EnvMap::new();
        map.insert("ANTHROPIC_API_KEY".to_string(), key);
        Ok(map)
    }

    /// Describe the source for each slot (for `cst validate` output).
    pub fn describe_sources(&self) -> Vec<(u8, String)> {
        self.keys
            .iter()
            .map(|k| (k.slot, k.source.describe()))
            .collect()
    }

    /// Return slots sorted by priority.
    pub fn sorted_slots(&self) -> Vec<u8> {
        let mut slots: Vec<u8> = self.keys.iter().map(|k| k.slot).collect();
        slots.sort();
        slots
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}

fn store_in_keychain(account: &str, secret: &str) -> Result<()> {
    let entry = keyring::Entry::new(SERVICE_NAME, account).context("creating keychain entry")?;
    entry
        .set_password(secret)
        .context("storing key in keychain")?;
    Ok(())
}

fn retrieve_from_keychain(account: &str) -> Result<String> {
    let entry = keyring::Entry::new(SERVICE_NAME, account).context("creating keychain entry")?;
    entry
        .get_password()
        .context("retrieving key from keychain — run `cst add-key` to add credentials")
}

fn delete_from_keychain(account: &str) -> Result<()> {
    let entry = keyring::Entry::new(SERVICE_NAME, account).context("creating keychain entry")?;
    // Ignore error if entry doesn't exist
    let _ = entry.delete_credential();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(slot: u8, account: &str) -> ApiKeyEntry {
        ApiKeyEntry {
            slot,
            source: super::SecretSource::Keychain {
                account: account.to_string(),
            },
            keychain_account: account.to_string(),
            note: String::new(),
        }
    }

    #[test]
    fn test_api_key_pool_add_and_sorted_slots() {
        let mut pool = ApiKeyPool::default();
        pool.keys.push(make_entry(2, "p-slot2"));
        pool.keys.push(make_entry(1, "p-slot1"));
        let slots = pool.sorted_slots();
        assert_eq!(slots, vec![1, 2]);
    }

    #[test]
    fn test_api_key_pool_is_empty() {
        let pool = ApiKeyPool::default();
        assert!(pool.is_empty());
    }

    #[test]
    fn test_api_key_pool_remove_slot() {
        let mut pool = ApiKeyPool {
            keys: vec![make_entry(1, "p-slot1"), make_entry(2, "p-slot2")],
        };
        pool.keys.retain(|k| k.slot != 1);
        assert_eq!(pool.sorted_slots(), vec![2]);
    }

    #[test]
    fn test_describe_sources() {
        let pool = ApiKeyPool {
            keys: vec![make_entry(1, "work-slot1")],
        };
        let desc = pool.describe_sources();
        assert_eq!(desc.len(), 1);
        assert_eq!(desc[0].0, 1);
        assert!(desc[0].1.contains("keychain:work-slot1"));
    }

    #[test]
    fn test_remove_external_source_does_not_touch_keychain() {
        // Removing a 1Password slot must not attempt keychain deletion
        // (which would call keyring::Entry::new with an empty account name).
        let mut pool = ApiKeyPool {
            keys: vec![ApiKeyEntry {
                slot: 1,
                source: super::SecretSource::OnePassword {
                    reference: "op://Personal/Claude/cred".to_string(),
                },
                keychain_account: String::new(), // empty — external slots have no account
                note: String::new(),
            }],
        };
        // Should succeed without touching the OS Keychain
        let result = pool.remove_key(1);
        assert!(result.is_ok(), "remove_key for external slot must not error: {result:?}");
        assert!(pool.keys.is_empty());
    }

    #[test]
    fn test_remove_keychain_source_uses_source_account() {
        // When source is Keychain, use the account from the source field.
        // The keychain_account legacy field is a fallback for older entries.
        // Here we verify the slot is removed from the pool regardless of
        // whether the keychain call succeeds (it will fail in tests without a
        // real keychain, but that's fine — we're testing pool removal logic).
        let mut pool = ApiKeyPool {
            keys: vec![make_entry(1, "work-slot1")],
        };
        // delete_from_keychain ignores errors via `let _ = entry.delete_credential()`
        // so this should always return Ok even in a test environment.
        let result = pool.remove_key(1);
        assert!(result.is_ok());
        assert!(pool.keys.is_empty());
    }
}
