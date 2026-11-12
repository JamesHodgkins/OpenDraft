//! Where the assistant's API key can be kept between runs: the computer's own
//! credential store (Windows Credential Manager, or the Secret Service on
//! Linux), never a file of ours. Whether to keep it is the user's choice.

use std::sync::Arc;

/// A place for one secret. Errors are the reason, ready to show.
pub trait KeyStore: Send + Sync {
    fn load(&self) -> Option<String>;
    fn save(&self, key: &str) -> Result<(), String>;
    fn forget(&self) -> Result<(), String>;
}

const SERVICE: &str = "OpenDraft";
const ACCOUNT: &str = "Mistral API key";

/// The operating system's credential store, under one account name.
struct Os(&'static str);

impl Os {
    fn entry(&self) -> Result<keyring::Entry, String> {
        keyring::Entry::new(SERVICE, self.0).map_err(|e| e.to_string())
    }
}

impl KeyStore for Os {
    fn load(&self) -> Option<String> {
        self.entry()
            .ok()?
            .get_password()
            .ok()
            .filter(|k| !k.trim().is_empty())
    }

    fn save(&self, key: &str) -> Result<(), String> {
        self.entry()?.set_password(key).map_err(|e| e.to_string())
    }

    fn forget(&self) -> Result<(), String> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

pub fn os_key_store() -> Arc<dyn KeyStore> {
    Arc::new(Os(ACCOUNT))
}

/// A store that keeps nothing, for a machine without one and for tests that
/// must not touch the real one.
pub struct NoStore;

impl KeyStore for NoStore {
    fn load(&self) -> Option<String> {
        None
    }

    fn save(&self, _: &str) -> Result<(), String> {
        Err("There is no credential store to keep it in.".to_owned())
    }

    fn forget(&self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs a real credential store, and uses an account of its own so the
    /// user's key is never touched.
    #[test]
    #[ignore = "uses the real credential store of the machine"]
    fn the_operating_systems_store_keeps_and_forgets_a_secret() {
        let store = Os("Mistral API key (test)");
        store.forget().unwrap();
        assert_eq!(store.load(), None);
        store.save("sk-test-value").unwrap();
        assert_eq!(store.load().as_deref(), Some("sk-test-value"));
        store.forget().unwrap();
        assert_eq!(store.load(), None);
        store.forget().unwrap();
    }
}
