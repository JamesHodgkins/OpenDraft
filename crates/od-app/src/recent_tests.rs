use std::collections::HashMap;

use eframe::Storage;

use super::*;

#[derive(Default)]
struct MemoryStorage(HashMap<String, String>);

impl eframe::Storage for MemoryStorage {
    fn get_string(&self, key: &str) -> Option<String> {
        self.0.get(key).cloned()
    }

    fn set_string(&mut self, key: &str, value: String) {
        self.0.insert(key.to_owned(), value);
    }

    fn flush(&mut self) {}
}

fn p(name: &str) -> PathBuf {
    PathBuf::from(name)
}

fn filled(names: &[&str]) -> RecentFiles {
    let mut recent = RecentFiles::default();
    for name in names {
        recent.add(&p(name));
    }
    recent
}

fn id_of(recent: &RecentFiles, name: &str) -> RecentId {
    recent
        .items()
        .iter()
        .find(|i| i.path == p(name))
        .unwrap()
        .id
}

#[test]
fn newest_first_without_duplicates() {
    let mut recent = filled(&["a.odx", "b.odx"]);
    let a = id_of(&recent, "a.odx");
    recent.add(&p("a.odx"));
    assert_eq!(recent.paths(), [p("a.odx"), p("b.odx")]);
    assert_eq!(id_of(&recent, "a.odx"), a, "the id stays");
}

#[test]
fn the_oldest_fall_off_past_the_limit() {
    let mut recent = RecentFiles::default();
    for i in 0..MAX_RECENT + 3 {
        recent.add(&p(&format!("{i}.odx")));
    }
    assert_eq!(recent.items().len(), MAX_RECENT);
    assert_eq!(recent.paths()[0], p(&format!("{}.odx", MAX_RECENT + 2)));
    assert!(!recent.paths().contains(&p("0.odx")));
}

#[test]
fn pinned_entries_come_first_and_survive_eviction() {
    let mut recent = filled(&["old.odx"]);
    let old = id_of(&recent, "old.odx");
    recent.pin(old).unwrap();
    for i in 0..MAX_RECENT + 3 {
        recent.add(&p(&format!("{i}.odx")));
    }
    assert_eq!(recent.items().len(), MAX_RECENT);
    assert_eq!(recent.paths()[0], p("old.odx"));
    assert!(recent.items()[0].pinned);
}

#[test]
fn only_five_may_be_pinned() {
    let mut recent = filled(&["1", "2", "3", "4", "5", "6"]);
    for name in ["1", "2", "3", "4", "5"] {
        recent.pin(id_of(&recent, name)).unwrap();
    }
    assert_eq!(recent.pin(id_of(&recent, "6")), Err(PinLimit));
    assert_eq!(recent.pinned_count(), MAX_PINNED);
    recent.pin(id_of(&recent, "5")).unwrap();
    recent.unpin(id_of(&recent, "1"));
    recent.pin(id_of(&recent, "6")).unwrap();
}

#[test]
fn a_full_list_of_pins_still_admits_a_new_drawing() {
    let names: Vec<String> = (0..MAX_RECENT).map(|i| i.to_string()).collect();
    let mut recent = RecentFiles::default();
    for n in &names {
        recent.add(&p(n));
    }
    for n in names.iter().take(MAX_PINNED) {
        recent.pin(id_of(&recent, n)).unwrap();
    }
    recent.add(&p("new"));
    assert_eq!(recent.items().len(), MAX_RECENT);
    assert!(recent.paths().contains(&p("new")));
    assert_eq!(recent.pinned_count(), MAX_PINNED);
}

#[test]
fn clearing_keeps_pins_and_counts_what_went() {
    let mut recent = filled(&["a", "b", "c"]);
    recent.pin(id_of(&recent, "b")).unwrap();
    assert_eq!((recent.items().len() - recent.pinned_count()), 2);
    assert_eq!(recent.clear_unpinned(), 2);
    assert_eq!(recent.paths(), [p("b")]);
}

#[test]
fn removing_forgets_one_entry() {
    let mut recent = filled(&["a", "b"]);
    recent.remove(id_of(&recent, "a"));
    recent.remove(RecentId(999));
    assert_eq!(recent.paths(), [p("b")]);
}

#[test]
fn locating_swaps_the_path_and_keeps_the_pin() {
    let mut recent = filled(&["gone.odx", "other.odx"]);
    let gone = id_of(&recent, "gone.odx");
    recent.pin(gone).unwrap();
    recent.add_replacing(&p("found.odx"), Some(gone));
    assert_eq!(recent.paths(), [p("found.odx"), p("other.odx")]);
    assert!(recent.items()[0].pinned);
}

#[test]
fn locating_an_entry_already_listed_pins_the_one_entry() {
    let mut recent = filled(&["gone.odx", "found.odx"]);
    let gone = id_of(&recent, "gone.odx");
    recent.pin(gone).unwrap();
    recent.add_replacing(&p("found.odx"), Some(gone));
    assert_eq!(recent.paths(), [p("found.odx")]);
    assert!(recent.items()[0].pinned);
}

#[test]
fn entries_found_to_be_one_file_merge_into_the_newest() {
    let mut recent = filled(&["a/x.odx", "b/x.odx", "c.odx"]);
    let (a, b) = (id_of(&recent, "a/x.odx"), id_of(&recent, "b/x.odx"));
    recent.pin(a).unwrap();
    let found = vec![
        (a, p("a/x.odx"), p("real/x.odx")),
        (b, p("b/x.odx"), p("real/x.odx")),
        (id_of(&recent, "c.odx"), p("c.odx"), p("real/c.odx")),
    ];
    assert!(recent.merge_resolved(&found));
    // b was opened after a, so b's id and path stay; the pin is kept.
    assert_eq!(recent.paths(), [p("b/x.odx"), p("c.odx")]);
    assert_eq!(recent.items()[0].id, b);
    assert!(recent.items()[0].pinned);
    assert!(!recent.merge_resolved(&found), "nothing left to merge");
}

#[test]
fn a_lookup_for_an_entry_that_has_since_changed_is_ignored() {
    let mut recent = filled(&["a.odx", "b.odx"]);
    let (a, b) = (id_of(&recent, "a.odx"), id_of(&recent, "b.odx"));
    let found = vec![
        (a, p("old-a.odx"), p("same.odx")),
        (b, p("b.odx"), p("same.odx")),
    ];
    assert!(!recent.merge_resolved(&found));
    assert_eq!(recent.items().len(), 2);
}

#[test]
fn saved_and_loaded_lists_match() {
    let mut recent = filled(&["dir with spaces/a.odx", "b.odx"]);
    recent.pin(id_of(&recent, "b.odx")).unwrap();
    let mut storage = MemoryStorage::default();
    recent.save(&mut storage);
    assert_eq!(RecentFiles::load(Some(&storage)), recent);
    assert_eq!(RecentFiles::load(None), RecentFiles::default());
}

#[test]
fn the_old_newline_list_migrates_in_order_with_fresh_ids() {
    let mut storage = MemoryStorage::default();
    storage.set_string("recent_files", "new.odx\nmiddle.odx\n\nold.odx".to_owned());
    let recent = RecentFiles::load(Some(&storage));
    assert_eq!(
        recent.paths(),
        [p("new.odx"), p("middle.odx"), p("old.odx")]
    );
    let ids: Vec<u64> = recent.items().iter().map(|i| i.id.0).collect();
    assert_eq!(ids, [1, 2, 3]);
    let mut recent = recent;
    assert_eq!(recent.add(&p("fresh.odx")), RecentId(4));
    assert_eq!(recent.paths()[0], p("fresh.odx"));
    // Saving writes the new format, which then wins.
    recent.save(&mut storage);
    assert_eq!(RecentFiles::load(Some(&storage)), recent);
}

#[test]
fn one_malformed_record_and_unknown_fields_do_not_spoil_the_rest() {
    let mut storage = MemoryStorage::default();
    storage.set_string(
        "recent_drawings",
        r#"{"version":9,"future":true,"next_id":5,"items":[
            {"id":1,"path":"a.odx","pinned":true,"last_opened":50,"extra":1},
            {"id":2,"pinned":false},
            "junk",
            {"id":3,"path":"b.odx"}]}"#
            .to_owned(),
    );
    let recent = RecentFiles::load(Some(&storage));
    assert_eq!(recent.paths(), [p("a.odx"), p("b.odx")]);
    assert!(recent.items()[0].pinned);
}

#[test]
fn bad_ids_are_repaired_in_stored_order() {
    let mut storage = MemoryStorage::default();
    storage.set_string(
        "recent_drawings",
        r#"{"next_id":0,"items":[
            {"id":0,"path":"a.odx","last_opened":9},
            {"id":2,"path":"b.odx","last_opened":8},
            {"id":2,"path":"c.odx","last_opened":7}]}"#
            .to_owned(),
    );
    let recent = RecentFiles::load(Some(&storage));
    assert_eq!(recent.paths(), [p("a.odx"), p("b.odx"), p("c.odx")]);
    let ids: Vec<u64> = recent.items().iter().map(|i| i.id.0).collect();
    assert_eq!(ids, [3, 2, 4]);
    let mut recent = recent;
    assert_eq!(recent.add(&p("d.odx")), RecentId(5));
}

#[test]
fn an_exhausted_counter_renumbers_everything() {
    let mut storage = MemoryStorage::default();
    let max = u64::MAX;
    storage.set_string(
        "recent_drawings",
        format!(
            r#"{{"next_id":{max},"items":[
                {{"id":{max},"path":"a.odx","last_opened":9}},
                {{"id":4,"path":"b.odx","last_opened":8}}]}}"#
        ),
    );
    let recent = RecentFiles::load(Some(&storage));
    let ids: Vec<u64> = recent.items().iter().map(|i| i.id.0).collect();
    assert_eq!(ids, [1, 2]);
}

#[test]
fn more_than_five_stored_pins_keep_only_the_first_five() {
    let items: Vec<String> = (1..=7)
        .map(|i| {
            format!(
                r#"{{"id":{i},"path":"{i}.odx","pinned":true,"last_opened":{}}}"#,
                100 - i
            )
        })
        .collect();
    let mut storage = MemoryStorage::default();
    storage.set_string(
        "recent_drawings",
        format!(r#"{{"next_id":8,"items":[{}]}}"#, items.join(",")),
    );
    let recent = RecentFiles::load(Some(&storage));
    assert_eq!(recent.pinned_count(), MAX_PINNED);
    assert_eq!(recent.items().len(), 7);
}

#[test]
fn the_chosen_view_is_remembered_and_grid_is_the_default() {
    let mut storage = MemoryStorage::default();
    assert_eq!(RecentFiles::load(Some(&storage)).view, RecentView::Grid);
    storage.set_string("recent_files", "a.odx".to_owned());
    assert_eq!(RecentFiles::load(Some(&storage)).view, RecentView::Grid);
    let mut recent = filled(&["a.odx"]);
    recent.view = RecentView::List;
    recent.save(&mut storage);
    assert_eq!(RecentFiles::load(Some(&storage)).view, RecentView::List);
}

#[test]
fn a_dismissed_note_is_remembered_by_its_exact_version_and_only_sixteen_are_kept() {
    let mut recent = RecentFiles::default();
    assert!(!recent.whats_new_seen("0.2.0"));
    recent.dismiss_whats_new("0.2.0");
    assert!(recent.whats_new_seen("0.2.0"));
    assert!(
        !recent.whats_new_seen("0.2.0-beta.1"),
        "a prerelease is its own version"
    );
    let mut storage = MemoryStorage::default();
    recent.save(&mut storage);
    assert!(RecentFiles::load(Some(&storage)).whats_new_seen("0.2.0"));
    // Dismissing again moves it to the newest, not twice.
    for i in 0..20 {
        recent.dismiss_whats_new(&format!("1.0.{i}"));
    }
    recent.dismiss_whats_new("1.0.5");
    assert_eq!(recent.seen_notes.len(), 16);
    assert_eq!(recent.seen_notes.last().map(String::as_str), Some("1.0.5"));
    assert_eq!(
        recent.seen_notes.iter().filter(|v| *v == "1.0.5").count(),
        1
    );
    assert!(!recent.whats_new_seen("0.2.0"), "the oldest went");
    assert!(recent.whats_new_seen("1.0.19"));
    // An old file without the list loads with none seen.
    storage.set_string("recent_drawings", r#"{"items":[]}"#.to_owned());
    assert!(!RecentFiles::load(Some(&storage)).whats_new_seen("1.0.19"));
}
