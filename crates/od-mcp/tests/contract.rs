//! The real `opendraft-mcp` binary, spoken to as an MCP client would over
//! stdio, with a fake app behind it.

#![allow(clippy::unwrap_used)] // helpers outside #[test] functions

mod support;

use serde_json::json;
use support::{scratch, FakeApp, Sidecar, TOKEN};

#[test]
fn it_initialises_and_lists_read_only_tools() {
    let dir = scratch("list");
    let mut sidecar = Sidecar::start(&dir, &[]);
    let init = sidecar.initialise();
    let info = &init["result"]["serverInfo"];
    assert_eq!(info["name"], "uk.openengineering.opendraft");
    assert!(
        init["result"]["capabilities"]["tools"].is_object(),
        "{init}"
    );
    assert!(
        init["result"]["capabilities"]["resources"].is_null(),
        "nothing is advertised that is not implemented: {init}"
    );
    let tools = sidecar.request("tools/list", json!({}))["result"]["tools"].clone();
    let names: Vec<&str> = tools
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "opendraft_instances_list",
            "opendraft_documents_list",
            "opendraft_document_get",
            "opendraft_entities_query",
            "opendraft_entities_get",
            "opendraft_layers_list",
            "opendraft_selection_get",
            "opendraft_geometry_measure",
            "opendraft_geometry_intersections",
            "opendraft_drawing_summary",
            "opendraft_drawing_audit",
            "opendraft_view_render",
            "opendraft_commands_list",
            "opendraft_commands_invoke",
            "opendraft_commands_start_interactive",
            "opendraft_events_poll",
            "opendraft_files_open",
            "opendraft_files_save",
            "opendraft_files_save_as",
            "opendraft_files_export",
            "opendraft_operations_get",
            "opendraft_operations_cancel",
            "opendraft_entities_apply_batch",
            "opendraft_entities_construct",
            "opendraft_layers_apply_batch",
            "opendraft_selection_set",
            "opendraft_selection_request",
            "opendraft_history_undo",
            "opendraft_history_redo"
        ]
    );
    for tool in tools.as_array().unwrap() {
        assert_eq!(tool["inputSchema"]["type"], "object");
        let writes = tool["name"].as_str().unwrap().ends_with("_batch")
            || tool["name"] == "opendraft_entities_construct"
            || tool["name"] == "opendraft_commands_invoke"
            || tool["name"] == "opendraft_commands_start_interactive"
            || tool["name"].as_str().unwrap().contains("_files_")
            || tool["name"] == "opendraft_operations_cancel"
            || tool["name"].as_str().unwrap().contains("_history_")
            || tool["name"] == "opendraft_selection_set"
            || tool["name"] == "opendraft_selection_request";
        assert_eq!(
            tool["annotations"]["readOnlyHint"], !writes,
            "{}",
            tool["name"]
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn it_also_serves_the_stateless_2026_request_style() {
    let dir = scratch("stateless");
    let mut sidecar = Sidecar::start(&dir, &[]);
    let meta = json!({"io.modelcontextprotocol/protocolVersion": "2026-07-28",
                      "io.modelcontextprotocol/clientInfo": {"name": "t", "version": "0"},
                      "io.modelcontextprotocol/clientCapabilities": {}});
    let reply = sidecar.request("server/discover", json!({"_meta": meta}));
    assert!(reply["result"].is_object(), "{reply}");
    let reply = sidecar.request("tools/list", json!({"_meta": meta}));
    assert_eq!(
        reply["result"]["tools"].as_array().unwrap().len(),
        29,
        "{reply}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_tool_call_reaches_the_app_and_comes_back_structured() {
    let dir = scratch("call");
    let app = FakeApp::start(&dir, "aaaa", TOKEN);
    let mut sidecar = Sidecar::start(&dir, &[]);
    sidecar.initialise();
    let result = sidecar.call("opendraft_documents_list", json!({}));
    assert_ne!(result["isError"], true, "{result}");
    assert_eq!(
        result["structuredContent"]["documents"][0]["document_id"],
        "doc_fake_1"
    );

    let result = sidecar.call(
        "opendraft_entities_query",
        json!({"document_id": "doc_fake_1", "limit": 5, "kinds": ["line"]}),
    );
    assert_eq!(result["structuredContent"]["operation"], "entities.query");
    assert_eq!(result["structuredContent"]["arguments"]["limit"], 5);
    assert_eq!(app.operations(), ["documents.list", "entities.query"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_domain_failure_is_a_tool_error_with_a_stable_code() {
    let dir = scratch("errors");
    let _app = FakeApp::start(&dir, "bbbb", TOKEN);
    let mut sidecar = Sidecar::start(&dir, &[]);
    sidecar.initialise();
    let result = sidecar.call(
        "opendraft_document_get",
        json!({"document_id": "doc_blocked"}),
    );
    assert_eq!(result["isError"], true, "{result}");
    assert_eq!(result["structuredContent"]["code"], "block_editor_open");
    assert_eq!(result["structuredContent"]["retryable"], true);
    let result = sidecar.call(
        "opendraft_document_get",
        json!({"document_id": "doc_missing"}),
    );
    assert_eq!(result["structuredContent"]["code"], "document_not_found");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unknown_tool_is_a_protocol_error() {
    let dir = scratch("unknown");
    let mut sidecar = Sidecar::start(&dir, &[]);
    sidecar.initialise();
    let reply = sidecar.request(
        "tools/call",
        json!({"name": "opendraft_delete_everything", "arguments": {}}),
    );
    assert!(reply["error"].is_object(), "{reply}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_app_running_says_so_in_a_way_a_model_can_act_on() {
    let dir = scratch("none");
    let mut sidecar = Sidecar::start(&dir, &[]);
    sidecar.initialise();
    let result = sidecar.call("opendraft_documents_list", json!({}));
    assert_eq!(result["isError"], true);
    assert_eq!(result["structuredContent"]["code"], "app_not_running");
    let listed = sidecar.call("opendraft_instances_list", json!({}));
    assert_eq!(listed["structuredContent"]["instances"], json!([]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn two_instances_need_a_choice_and_the_token_is_never_shown() {
    let dir = scratch("two");
    let _a = FakeApp::start(&dir, "cccc", TOKEN);
    let b = FakeApp::start(&dir, "dddd", TOKEN);
    let mut sidecar = Sidecar::start(&dir, &[]);
    sidecar.initialise();
    let result = sidecar.call("opendraft_documents_list", json!({}));
    assert_eq!(result["structuredContent"]["code"], "instance_required");
    let listed = sidecar.call("opendraft_instances_list", json!({}));
    let text = listed.to_string();
    assert!(
        !text.contains(TOKEN),
        "the token must not reach the model: {text}"
    );
    assert_eq!(
        listed["structuredContent"]["instances"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    drop(sidecar);

    let mut chosen = Sidecar::start(&dir, &["--instance", "dddd"]);
    chosen.initialise();
    let result = chosen.call("opendraft_documents_list", json!({}));
    assert_ne!(result["isError"], true, "{result}");
    assert_eq!(b.operations(), ["documents.list"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_wrong_token_is_reported_not_retried() {
    let dir = scratch("token");
    let _app = FakeApp::start(&dir, "eeee", "a-different-token");
    // The descriptor says one thing, the app expects another.
    let mut sidecar = Sidecar::start(&dir, &[]);
    sidecar.initialise();
    let result = sidecar.call("opendraft_documents_list", json!({}));
    assert_eq!(result["isError"], true);
    assert_eq!(result["structuredContent"]["code"], "scope_denied");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_app_going_away_gives_a_clear_error_and_the_server_keeps_running() {
    let dir = scratch("gone");
    let app = FakeApp::start(&dir, "ffff", TOKEN);
    let mut sidecar = Sidecar::start(&dir, &[]);
    sidecar.initialise();
    assert_ne!(
        sidecar.call("opendraft_documents_list", json!({}))["isError"],
        true
    );

    // The app stops answering but still looks alive: reconnecting does not help.
    app.stop();
    let result = sidecar.call(
        "opendraft_selection_get",
        json!({"document_id": "doc_fake_1"}),
    );
    assert_eq!(
        result["structuredContent"]["code"], "instance_gone",
        "{result}"
    );

    // It then exits cleanly and removes its descriptor.
    od_api::instance::remove(&app.dir, app.id());
    let result = sidecar.call(
        "opendraft_selection_get",
        json!({"document_id": "doc_fake_1"}),
    );
    assert_eq!(
        result["structuredContent"]["code"], "app_not_running",
        "{result}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_tools_ask_the_app_with_the_write_scope() {
    let dir = scratch("scope");
    let app = FakeApp::start(&dir, "gggg", TOKEN);
    let mut sidecar = Sidecar::start(&dir, &[]);
    sidecar.initialise();
    let batch = json!({"document_id": "doc_fake_1", "expected_revision": "7", "operations": []});
    let result = sidecar.call("opendraft_entities_apply_batch", batch);
    assert_ne!(result["isError"], true, "{result}");
    sidecar.call(
        "opendraft_selection_set",
        json!({"document_id": "doc_fake_1", "mode": "clear"}),
    );
    sidecar.call("opendraft_documents_list", json!({}));
    let scopes: Vec<_> = app
        .seen
        .lock()
        .unwrap()
        .iter()
        .map(|r| (r.operation.clone(), serde_json::to_value(r.scope).unwrap()))
        .collect();
    assert_eq!(
        scopes,
        [
            ("entities.apply_batch".to_owned(), json!("drawing.write")),
            ("selection.set".to_owned(), json!("ui.selection")),
            ("documents.list".to_owned(), json!("drawing.read")),
        ]
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_key_passes_through_to_the_app_and_a_lost_write_is_never_repeated() {
    let dir = scratch("keys");
    let app = FakeApp::start(&dir, "llll", TOKEN);
    let mut sidecar = Sidecar::start(&dir, &[]);
    sidecar.initialise();
    let args = json!({"document_id": "doc_fake_1", "expected_revision": "7", "operations": [],
                      "idempotency_key": "try-1"});
    let result = sidecar.call("opendraft_entities_apply_batch", args);
    assert_ne!(result["isError"], true, "{result}");
    assert_eq!(
        result["structuredContent"]["arguments"]["idempotency_key"],
        "try-1"
    );

    // The app stops answering. A read is asked again on a new connection
    // (and fails plainly); a change is not, because its reply may be what was
    // lost, and the caller is told so.
    app.stop();
    let read = sidecar.call(
        "opendraft_selection_get",
        json!({"document_id": "doc_fake_1"}),
    );
    assert_eq!(read["structuredContent"]["code"], "instance_gone");
    assert!(read["structuredContent"]["details"].is_null(), "{read}");
    let seen = app.operations().len();
    let write = sidecar.call(
        "opendraft_entities_apply_batch",
        json!({"document_id": "doc_fake_1", "expected_revision": "7", "operations": []}),
    );
    assert_eq!(
        write["structuredContent"]["code"], "instance_gone",
        "{write}"
    );
    assert_eq!(
        write["structuredContent"]["details"]["outcome_unknown"],
        "true"
    );
    assert!(
        write["structuredContent"]["message"]
            .as_str()
            .unwrap()
            .contains("idempotency_key"),
        "it says how to retry safely"
    );
    assert_eq!(app.operations().len(), seen, "nothing more reached the app");
    let _ = std::fs::remove_dir_all(&dir);
}
