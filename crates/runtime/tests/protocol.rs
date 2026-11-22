use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Stdio};

fn request(operation: &str, input: Value) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_lekalo-rust-check"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let wire = json!({
        "protocol": "lekalo/runtime/v1", "request_id": "test-request",
        "operation": operation, "input": input
    });
    stdin
        .write_all(&serde_json::to_vec(&wire).unwrap())
        .unwrap();
    drop(stdin);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let response: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["protocol"], wire["protocol"]);
    assert_eq!(response["request_id"], wire["request_id"]);
    response["result"].clone()
}

#[test]
fn advertises_the_same_syntax_capabilities_as_the_portable_runtime() {
    let info = request("describe", json!({}));
    assert_eq!(info["id"], "lekalo-rust");
    assert_eq!(
        info["capabilities"],
        json!(["source.declarations/v1", "source.matches/v1"])
    );
    assert_eq!(info["operations"], json!(["check"]));
    assert_eq!(info["rules_api"], json!(["lekalo/contract-rules/v1"]));
}

#[test]
fn uses_the_shared_declaration_fixture() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../test/portable-contract.json")).unwrap();
    let run = |content: &Value| {
        request(
            "check",
            json!({
                "files": [{"path": fixture["path"], "content": content}],
                "rules": [{"id": "fixture", "rules": fixture["rules"]}]
            }),
        )
    };
    assert_eq!(
        run(&fixture["valid"]),
        json!({"state":"complete","facts":{"fixture":{"public-declaration":{"declarations":[
            {"path":"lib.rs","kind":"function","name":"health"}
        ]}}}})
    );
    assert_eq!(
        run(&fixture["invalid"])["facts"]["fixture"]["public-declaration"]["declarations"],
        json!([])
    );
}

#[test]
fn captures_from_selected_rust_files_and_reports_incomplete_evidence() {
    let query = r#"((struct_item name: (type_identifier) @boundary) (#eq? @boundary "Response"))"#;
    let run = |content: &str, query: &str| {
        request(
            "check",
            json!({
                "files": [
                    {"path": "notes.md", "content": "ignored"},
                    {"path": "mod.rs", "content": content}
                ],
                "rules": [{"id":"wire","rules":{
                    "requires": ["source.matches/v1"],
                    "bindings": {"lekalo-rust.queries/v1": {"response": query}},
                    "checks": [{"id":"response"}]
                }}]
            }),
        )
    };
    assert_eq!(
        run("struct Response {} struct Other {}", query),
        json!({"state":"complete","facts":{"wire":{"response":{"matches":[
            {"path":"mod.rs","kind":"boundary","name":"Response"}
        ]}}}})
    );
    let partial = json!({"state":"partial","facts":{}});
    assert_eq!(run("struct Response {", query), partial);
    assert_eq!(run("struct Response {}", "(not_a_rust_node)"), partial);
}
