//! Keeping the API key: what the panel's "Remember on this computer" does,
//! against an in-memory store (never the real one).

use std::sync::{Arc, Mutex};

use od_agent::{FakeProvider, Provider, ProviderError, Step};

use super::chat_tests::rig;
use super::{Assistant, KeyStore};

/// A credential store held in memory, which can be told to refuse.
#[derive(Default)]
struct Memory {
    kept: Mutex<Option<String>>,
    refuse: bool,
}

impl KeyStore for Memory {
    fn load(&self) -> Option<String> {
        self.kept.lock().unwrap().clone()
    }

    fn save(&self, key: &str) -> Result<(), String> {
        if self.refuse {
            return Err("the store is locked".to_owned());
        }
        *self.kept.lock().unwrap() = Some(key.to_owned());
        Ok(())
    }

    fn forget(&self) -> Result<(), String> {
        *self.kept.lock().unwrap() = None;
        Ok(())
    }
}

fn with_store(store: Arc<Memory>, key: Option<&str>) -> Assistant {
    let factory: super::chat::ProviderFactory = Arc::new(|_| Err("not used".to_owned()));
    Assistant::new(factory, Arc::new(|| {}), key.map(str::to_owned), store)
}

#[test]
fn a_remembered_key_is_there_the_next_time_the_app_starts() {
    let store = Arc::new(Memory::default());
    let mut first = with_store(store.clone(), None);
    assert!(!first.has_key());
    first.set_key("  sk-kept ", true);
    assert!(first.has_key());
    assert_eq!(
        store.load().as_deref(),
        Some("sk-kept"),
        "trimmed, then kept"
    );
    let second = with_store(store, None);
    assert!(second.has_key(), "no pasting again");
}

#[test]
fn a_key_not_to_be_remembered_is_used_but_not_kept() {
    let store = Arc::new(Memory::default());
    let mut chat = with_store(store.clone(), None);
    chat.set_key("sk-session", false);
    assert!(chat.has_key());
    assert_eq!(store.load(), None);
    assert!(!with_store(store, None).has_key());
}

#[test]
fn a_key_from_the_environment_wins_and_is_not_saved() {
    let store = Arc::new(Memory::default());
    *store.kept.lock().unwrap() = Some("sk-old".to_owned());
    let chat = with_store(store.clone(), Some("sk-env"));
    assert_eq!(chat.key_in_use(), Some("sk-env"));
    assert_eq!(
        store.load().as_deref(),
        Some("sk-old"),
        "the kept one is untouched"
    );
}

#[test]
fn forgetting_removes_the_key_from_the_session_and_the_store() {
    let store = Arc::new(Memory::default());
    let mut chat = with_store(store.clone(), None);
    chat.set_key("sk-kept", true);
    chat.forget_key();
    assert!(!chat.has_key());
    assert_eq!(store.load(), None);
    assert!(!with_store(store, None).has_key());
}

#[test]
fn a_store_that_refuses_still_leaves_the_key_for_the_session_and_says_why() {
    let store = Arc::new(Memory {
        refuse: true,
        ..Memory::default()
    });
    let mut chat = with_store(store, None);
    chat.set_key("sk-x", true);
    assert!(chat.has_key());
    let note = chat.key_note.clone().unwrap();
    assert!(
        note.contains("could not be kept") && note.contains("locked"),
        "{note}"
    );
    chat.set_key("sk-y", false);
    assert_eq!(chat.key_note, None, "the note goes with the next try");
}

#[test]
fn a_refused_key_is_taken_out_of_the_store_too() {
    let store = Arc::new(Memory::default());
    *store.kept.lock().unwrap() = Some("sk-bad".to_owned());
    let factory: super::chat::ProviderFactory = Arc::new(|_| {
        let steps = vec![Step::Fail(ProviderError::Fatal(format!(
            "{} (status 401)",
            od_agent::KEY_REFUSED
        )))];
        let provider: Arc<dyn Provider> = Arc::new(FakeProvider::new(steps));
        Ok(provider)
    });
    let chat = Assistant::new(factory, Arc::new(|| {}), None, store.clone());
    let mut rig = rig(None);
    rig.chat = chat;
    rig.send("hello");
    rig.finish();
    assert!(!rig.chat.has_key());
    assert_eq!(store.load(), None, "a refused key is not kept");
}

#[test]
fn a_new_assistant_may_edit_and_asks_once_for_a_plan() {
    let chat = with_store(Arc::new(Memory::default()), None);
    assert_eq!(chat.kind, super::Kind::Edit);
    assert_eq!(chat.level, super::ApprovalLevel::ApprovePlan);
}
