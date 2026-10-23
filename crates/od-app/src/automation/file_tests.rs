use std::path::{Path, PathBuf};

use od_api::ErrorCode;
use od_core::{Entity, Point};
use serde_json::{json, Value};

use super::{answer_consent, consent_text, dispatch_test, document_id, Caller};
use crate::state::AppState;

const INSTANCE: &str = "inst1";
pub(super) const ME: Caller = Caller {
    client: "test client",
    grant_id: 7,
    history_any: false,
};
pub(super) const OTHER: Caller = Caller {
    client: "other",
    grant_id: 8,
    history_any: false,
};

pub(super) struct Area(pub(super) PathBuf);

impl Area {
    pub(super) fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("od-files-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("approved")).unwrap();
        std::fs::create_dir_all(dir.join("elsewhere")).unwrap();
        Area(dir)
    }

    pub(super) fn path(&self, relative: &str) -> PathBuf {
        self.0.join(relative)
    }

    pub(super) fn arg(&self, relative: &str) -> String {
        self.path(relative).to_string_lossy().into_owned()
    }
}

impl Drop for Area {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(super) fn line(x: f64) -> Entity {
    Entity::Line {
        start: Point::new(x, 0.0),
        end: Point::new(x + 1.0, 0.0),
    }
}

/// An app that may touch the `approved` folder of `area`.
pub(super) fn app(area: &Area) -> AppState {
    let mut state = AppState::new();
    state.settings.automation_roots = vec![area.path("approved")];
    state
}

pub(super) fn call(
    state: &mut AppState,
    caller: &Caller,
    op: &str,
    mut args: Value,
) -> Result<Value, od_api::ApiError> {
    if args.get("document_id") == Some(&json!("active")) {
        args["document_id"] = json!(document_id(INSTANCE, state.docs[state.active].uid));
    }
    dispatch_test(state, INSTANCE, caller, op, args)
}

pub(super) fn ok(state: &mut AppState, op: &str, args: Value) -> Value {
    call(state, &ME, op, args).unwrap()
}

/// A drawing file with `n` lines, written with the real writer.
pub(super) fn make_file(path: &Path, n: usize) {
    let mut doc = od_core::Document::default();
    for i in 0..n {
        doc.add(line(i as f64 * 3.0));
    }
    od_files::save(path, &doc).unwrap();
}

#[test]
fn opening_a_file_in_an_approved_folder_brings_it_in_as_a_new_drawing() {
    let area = Area::new("open");
    make_file(&area.path("approved/plan.odx"), 3);
    let mut state = app(&area);
    let status = ok(
        &mut state,
        "files.open",
        json!({"path": area.arg("approved/plan.odx")}),
    );
    assert_eq!(status["state"], "succeeded", "{status}");
    assert_eq!(status["kind"], "open");
    assert_eq!(status["file_name"], "plan.odx");
    let id = status["document_id"].as_str().unwrap().to_owned();
    assert_eq!(state.docs.len(), 1, "it replaced the blank one");
    assert_eq!(state.session().document().len(), 3);
    assert_eq!(id, document_id(INSTANCE, state.docs[state.active].uid));
    // The same call again does not open it twice.
    let again = ok(
        &mut state,
        "files.open",
        json!({"path": area.arg("approved/plan.odx")}),
    );
    assert_eq!(again["document_id"], id);
    assert_eq!(state.docs.len(), 1);
    assert!(
        !status
            .to_string()
            .contains(&area.0.to_string_lossy().to_string()),
        "no folder names leak"
    );
}

#[test]
fn a_dxf_opens_as_an_import_and_says_what_it_left_out_if_anything() {
    let area = Area::new("dxf");
    make_file(&area.path("approved/in.dxf"), 2);
    let mut state = app(&area);
    let status = ok(
        &mut state,
        "files.open",
        json!({"path": area.arg("approved/in.dxf")}),
    );
    assert_eq!(status["state"], "succeeded", "{status}");
    assert_eq!(state.session().document().len(), 2);
    assert!(
        state.session().is_modified(),
        "an import has no file of its own yet"
    );
}

#[test]
fn save_writes_the_drawings_own_file_and_never_invents_one() {
    let area = Area::new("save");
    make_file(&area.path("approved/plan.odx"), 1);
    let mut state = app(&area);
    ok(
        &mut state,
        "files.open",
        json!({"path": area.arg("approved/plan.odx")}),
    );
    state.add_entity(line(50.0));
    assert!(state.session().is_modified());
    let revision = state.session().revision().to_string();
    let status = ok(
        &mut state,
        "files.save",
        json!({"document_id": "active", "expected_revision": revision}),
    );
    assert_eq!(status["state"], "succeeded", "{status}");
    assert_eq!(status["revision"], revision);
    assert!(!state.session().is_modified());
    let on_disk = od_files::load(&area.path("approved/plan.odx")).unwrap();
    assert_eq!(on_disk.document.len(), 2);

    // A stale revision writes nothing.
    state.add_entity(line(60.0));
    let stale = call(
        &mut state,
        &ME,
        "files.save",
        json!({"document_id": "active", "expected_revision": revision}),
    );
    assert_eq!(stale.unwrap_err().code, ErrorCode::StaleRevision);
    assert_eq!(
        od_files::load(&area.path("approved/plan.odx"))
            .unwrap()
            .document
            .len(),
        2
    );

    // A drawing with no file is told to use save_as.
    let mut fresh = app(&area);
    let error = call(
        &mut fresh,
        &ME,
        "files.save",
        json!({"document_id": "active"}),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidArguments);
    assert!(error.message.contains("save_as"));
}

#[test]
fn save_as_to_a_new_name_just_happens_and_to_an_existing_file_asks_first() {
    let area = Area::new("saveas");
    let mut state = app(&area);
    state.add_entity(line(0.0));
    let status = ok(
        &mut state,
        "files.save_as",
        json!({"document_id": "active", "path": area.arg("approved/new.odx")}),
    );
    assert_eq!(status["state"], "succeeded", "{status}");
    assert!(area.path("approved/new.odx").exists());
    assert_eq!(state.docs[0].title, "new");
    assert!(!state.session().is_modified());

    // Another file is there already: nothing is written until the user says so.
    make_file(&area.path("approved/old.odx"), 5);
    state.add_entity(line(9.0));
    let waiting = ok(
        &mut state,
        "files.save_as",
        json!({"document_id": "active", "path": area.arg("approved/old.odx")}),
    );
    assert_eq!(waiting["state"], "awaiting_consent");
    assert_eq!(
        od_files::load(&area.path("approved/old.odx"))
            .unwrap()
            .document
            .len(),
        5
    );
    let (id, text) = consent_text(&state).unwrap();
    assert!(
        text.contains("old.odx") && text.contains("test client") && text.contains("save as"),
        "{text}"
    );
    answer_consent(&mut state, id, true);
    let op = waiting["operation_id"].as_str().unwrap().to_owned();
    let done = ok(&mut state, "operations.get", json!({"operation_id": op}));
    assert_eq!(done["state"], "succeeded", "{done}");
    assert_eq!(
        od_files::load(&area.path("approved/old.odx"))
            .unwrap()
            .document
            .len(),
        2
    );
    assert!(consent_text(&state).is_none());
}

#[test]
fn export_checks_the_extension_and_the_layout_and_asks_before_replacing() {
    let area = Area::new("export");
    let mut state = app(&area);
    state.add_entity(line(0.0));
    let export = |state: &mut AppState, path: &str, extra: Value| {
        let mut args = json!({"document_id": "active", "path": area.arg(path), "format": "dxf"});
        for (k, v) in extra.as_object().unwrap() {
            args[k] = v.clone();
        }
        call(state, &ME, "files.export", args)
    };
    let status = export(&mut state, "approved/out.dxf", json!({})).unwrap();
    assert_eq!(status["state"], "succeeded", "{status}");
    assert!(
        std::fs::metadata(area.path("approved/out.dxf"))
            .unwrap()
            .len()
            > 0
    );
    assert!(
        state.docs[0].path.is_none(),
        "an export does not retarget the drawing"
    );

    assert_eq!(
        export(&mut state, "approved/out.odx", json!({}))
            .unwrap_err()
            .code,
        ErrorCode::PathDenied
    );
    assert_eq!(
        export(&mut state, "approved/out.svg", json!({"format": "svg"}))
            .unwrap_err()
            .code,
        ErrorCode::InvalidArguments
    );
    assert_eq!(
        export(&mut state, "approved/out.dxf", json!({"layout_id": "1"}))
            .unwrap_err()
            .code,
        ErrorCode::InvalidArguments
    );
    assert_eq!(
        export(
            &mut state,
            "approved/out.svg",
            json!({"format": "svg", "layout_id": "99"})
        )
        .unwrap_err()
        .code,
        ErrorCode::InvalidReference
    );
    assert_eq!(
        export(&mut state, "elsewhere/out.dxf", json!({}))
            .unwrap_err()
            .code,
        ErrorCode::PathDenied
    );

    // The file is there now, so a second export asks.
    let again = export(&mut state, "approved/out.dxf", json!({})).unwrap();
    assert_eq!(again["state"], "awaiting_consent");
    let (id, _) = consent_text(&state).unwrap();
    answer_consent(&mut state, id, true);
    let done = ok(
        &mut state,
        "operations.get",
        json!({"operation_id": again["operation_id"]}),
    );
    assert_eq!(done["state"], "succeeded");
}

#[test]
fn a_layout_sheet_exports_as_svg() {
    let area = Area::new("sheet");
    let mut state = app(&area);
    state.add_entity(line(0.0));
    state.new_layout();
    let layout = state.session().document().layouts()[0].id.0.to_string();
    let status = ok(
        &mut state,
        "files.export",
        json!({"document_id": "active", "path": area.arg("approved/sheet.svg"), "format": "svg", "layout_id": layout}),
    );
    assert_eq!(status["state"], "succeeded", "{status}");
    let text = std::fs::read_to_string(area.path("approved/sheet.svg")).unwrap();
    assert!(text.starts_with("<svg"));
}
