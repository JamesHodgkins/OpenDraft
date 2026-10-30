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
        "entities.construct" => request.arguments["steps"].as_array().map(|s| steps(s)),
        _ => None,
    };
    said.filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Change the drawing".to_owned())
}

/// What each construction step does, in order.
fn steps(steps: &[Value]) -> String {
    const SHOWN: usize = 6;
    let mut phrases: Vec<String> = steps.iter().take(SHOWN).map(step).collect();
    if steps.len() > SHOWN {
        phrases.push(format!("{} more steps", steps.len() - SHOWN));
    }
    capital(&list(&phrases))
}

fn step(step: &Value) -> String {
    let shapes = |key: &str| {
        let n = step[key].as_array().map_or(1, Vec::len);
        plural(n, "shape")
    };
    let copies = if step["copy_as"].is_string() {
        " as copies"
    } else {
        ""
    };
    let number = |key: &str| step[key].as_u64().unwrap_or(0) as usize;
    match step["op"].as_str().unwrap_or("") {
        "polygon" => format!("draw a {}-sided polygon", number("sides")),
        "move" => format!("move {}{copies}", shapes("targets")),
        "rotate" => format!("rotate {}{copies}", shapes("targets")),
        "scale" => format!("scale {}{copies}", shapes("targets")),
        "mirror" => format!("mirror {}{copies}", shapes("targets")),
        "rect_array" => format!(
            "array {} in {} columns by {} rows",
            shapes("targets"),
            number("columns"),
            number("rows")
        ),
        "polar_array" => format!(
            "array {} {} times round a centre",
            shapes("targets"),
            number("count")
        ),
        "offset" => "offset a shape".to_owned(),
        _ => "make a construction step".to_owned(),
    }
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
    fn construction_steps_are_put_in_words() {
        let r = ChangeRequest {
            tool: "t".to_owned(),
            operation: "entities.construct".to_owned(),
            arguments: json!({"steps": [
                {"op": "polygon", "sides": 6},
                {"op": "polar_array", "targets": ["h"], "count": 8},
                {"op": "mirror", "targets": ["a", "b"], "copy_as": "m"},
            ]}),
        };
        assert_eq!(
            change(&r),
            "Draw a 6-sided polygon, array 1 shape 8 times round a centre and mirror 2 shapes as copies"
        );
        let many = ChangeRequest {
            arguments: json!({"steps": vec![json!({"op": "move", "targets": ["a"]}); 9]}),
            ..r
        };
        assert!(
            change(&many).ends_with("and 3 more steps"),
            "{}",
            change(&many)
        );
    }

    #[test]
    fn text_is_called_text() {
        let r = request("entities.apply_batch", json!([add("mtext")]));
        assert_eq!(change(&r), "Add 1 text");
    }
}
