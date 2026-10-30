//! A change in a drafter's words, for the card that asks whether to allow it:
//! "Add 2 lines and 1 circle; delete 1 object" instead of the request's JSON.
//! The JSON stays one click away, so what is shown is never a substitute for
//! what will be sent.

use serde_json::Value;

use super::ChangeRequest;
use crate::text::plural;

pub fn change(request: &ChangeRequest) -> String {
    let operations = request.arguments["operations"].as_array();
    let said = match request.operation.as_str() {
        "entities.apply_batch" => operations.map(|o| entities(o)),
        "layers.apply_batch" => operations.map(|o| layers(o)),
        "parts.draw" => part(&request.arguments),
        _ => None,
    };
    said.filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Change the drawing".to_owned())
}

/// "Draw a weld neck flange (outside diameter 100, 4 bolt holes)".
fn part(arguments: &Value) -> Option<String> {
    let part = arguments.get("part")?;
    let kind = part.get("kind")?.as_str()?.replace('_', " ");
    let number = |field: &str| part.get(field).and_then(Value::as_f64);
    let mut details = Vec::new();
    if let Some(d) = number("outside_diameter") {
        details.push(format!("outside diameter {d}"));
    }
    if let Some(n) = number("bolt_count") {
        details.push(plural(n as usize, "bolt hole"));
    }
    Some(if details.is_empty() {
        format!("Draw a {kind}")
    } else {
        format!("Draw a {kind} ({})", details.join(", "))
    })
}

fn tally(operations: &[Value], op: &str) -> usize {
    operations.iter().filter(|o| o["op"] == op).count()
}

fn entities(operations: &[Value]) -> String {
    // What is added, kind by kind, in the order first met.
    let mut kinds: Vec<(String, usize)> = Vec::new();
    for operation in operations.iter().filter(|o| o["op"] == "add") {
        let kind = operation["entity"]["geometry"]["type"]
            .as_str()
            .map_or("object", noun)
            .to_owned();
        match kinds.iter_mut().find(|(k, _)| *k == kind) {
            Some((_, n)) => *n += 1,
            None => kinds.push((kind, 1)),
        }
    }
    let mut parts = Vec::new();
    if !kinds.is_empty() {
        let added: Vec<String> = kinds.iter().map(|(k, n)| plural(*n, k)).collect();
        parts.push(format!("add {}", list(&added)));
    }
    for (op, verb) in [
        ("patch", "change"),
        ("replace", "redraw"),
        ("delete", "delete"),
    ] {
        let n = tally(operations, op);
        if n > 0 {
            parts.push(format!("{verb} {}", plural(n, "object")));
        }
    }
    capital(&list(&parts))
}

fn layers(operations: &[Value]) -> String {
    let mut parts = Vec::new();
    for (op, verb) in [("create", "create"), ("update", "change")] {
        let n = tally(operations, op);
        if n > 0 {
            parts.push(format!("{verb} {}", plural(n, "layer")));
        }
    }
    if tally(operations, "set_current") > 0 {
        parts.push("set the current layer".to_owned());
    }
    capital(&list(&parts))
}

/// The word for a kind of object in the wire's geometry type.
fn noun(kind: &str) -> &str {
    match kind {
        "mtext" => "text",
        "lwpolyline" => "polyline",
        other => other,
    }
}

/// "a", "a and b", "a, b and c".
fn list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [only] => only.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

fn capital(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().collect::<String>() + chars.as_str())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn request(operation: &str, operations: Value) -> ChangeRequest {
        ChangeRequest {
            tool: "t".to_owned(),
            operation: operation.to_owned(),
            arguments: json!({"operations": operations}),
        }
    }

    fn add(kind: &str) -> Value {
        json!({"op": "add", "client_ref": "a", "entity": {"geometry": {"type": kind}}})
    }

    #[test]
    fn adds_are_counted_by_kind() {
        let r = request(
            "entities.apply_batch",
            json!([add("line"), add("circle"), add("line")]),
        );
        assert_eq!(change(&r), "Add 2 lines and 1 circle");
    }

    #[test]
    fn a_mixed_batch_reads_as_a_list() {
        let r = request(
            "entities.apply_batch",
            json!([
                add("line"),
                {"op": "patch", "target": {}, "patch": {}},
                {"op": "patch", "target": {}, "patch": {}},
                {"op": "delete", "target": {}}
            ]),
        );
        assert_eq!(
            change(&r),
            "Add 1 line, change 2 objects and delete 1 object"
        );
    }

    #[test]
    fn layer_changes_are_described() {
        let r = request(
            "layers.apply_batch",
            json!([{"op": "create", "name": "A"}, {"op": "update"}, {"op": "set_current"}]),
        );
        assert_eq!(
            change(&r),
            "Create 1 layer, change 1 layer and set the current layer"
        );
    }

    #[test]
    fn what_cannot_be_read_is_still_said_plainly() {
        assert_eq!(
            change(&request("entities.apply_batch", json!([]))),
            "Change the drawing"
        );
        assert_eq!(
            change(&request("something.else", json!([]))),
            "Change the drawing"
        );
        let odd = ChangeRequest {
            tool: "t".into(),
            operation: "entities.apply_batch".into(),
            arguments: json!("not an object"),
        };
        assert_eq!(change(&odd), "Change the drawing");
    }

    #[test]
    fn a_part_is_named_with_its_main_numbers() {
        let r = ChangeRequest {
            tool: "t".to_owned(),
            operation: "parts.draw".to_owned(),
            arguments: json!({"part": {"kind": "weld_neck_flange", "outside_diameter": 273.0,
                                       "bolt_count": 8}}),
        };
        assert_eq!(
            change(&r),
            "Draw a weld neck flange (outside diameter 273, 8 bolt holes)"
        );
        let bare = ChangeRequest {
            arguments: json!({"part": {"kind": "weld_neck_flange"}}),
            ..r
        };
        assert_eq!(change(&bare), "Draw a weld neck flange");
    }

    #[test]
    fn text_is_called_text() {
        let r = request("entities.apply_batch", json!([add("mtext")]));
        assert_eq!(change(&r), "Add 1 text");
    }
}
