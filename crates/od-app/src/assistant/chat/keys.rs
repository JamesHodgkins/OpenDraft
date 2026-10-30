//! The key: used for the session, and kept in the computer's credential store
//! only when the user says so.

use super::Assistant;

impl Assistant {
    /// Use `key` for this session; also keep it in the credential store when
    /// `remember`. If it cannot be kept the session still has it, and the
    /// reason is in `key_note`.
    pub fn set_key(&mut self, key: &str, remember: bool) {
        let key = key.trim().to_owned();
        self.key_note = None;
        if key.is_empty() {
            self.key = None;
            return;
        }
        if remember {
            if let Err(why) = self.store.save(&key) {
                self.key_note = Some(format!(
                    "The key is in use for this session but could not be kept: {why}"
                ));
            }
        }
        self.key = Some(key);
    }

    /// Forget the key everywhere: this session and the credential store.
    pub fn forget_key(&mut self) {
        self.key = None;
        self.key_note = self
            .store
            .forget()
            .err()
            .map(|why| format!("The kept key could not be removed: {why}"));
    }

    /// The key in use, for tests to see which one won.
    #[cfg(test)]
    pub(in crate::assistant) fn key_in_use(&self) -> Option<&str> {
        self.key.as_deref()
    }

    /// A key the service refused is no use to keep anywhere.
    pub(super) fn drop_refused_key(&mut self) {
        self.forget_key();
    }
}
