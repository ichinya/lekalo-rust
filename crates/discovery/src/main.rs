use serde_json::{json, Value};
use std::io::{self, Read};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    io::stdin()
        .take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err("request-limit".into());
    }
    let request: Value = serde_json::from_slice(&bytes)?;
    if request["protocol"] != "lekalo/runtime/v1"
        || !request["request_id"]
            .as_str()
            .is_some_and(|id| id.len() == 71 && id.starts_with("sha256:"))
    {
        return Err("request-envelope".into());
    }
    let result = dispatch(
        request["operation"].as_str().ok_or("operation")?,
        &request["input"],
    )?;
    print!(
        "{}",
        json!({"protocol":"lekalo/runtime/v1","request_id":request["request_id"],"result":result})
    );
    Ok(())
}

use lekalo_rust_discovery as d;
const ID: &str = "lekalo-rust-discovery";
const FILES: &[&str] = &["Cargo.toml", "Cargo.lock"];
const READS: &[&str] = &["Cargo.toml"];

fn path_dir(path: &str) -> &str {
    path.rsplit_once('/').map(|(d, _)| d).unwrap_or(".")
}
fn dispatch(op: &str, input: &Value) -> Result<Value, Box<dyn std::error::Error>> {
    if op == "describe" {
        return Ok(
            json!({"id":ID,"version":"0.1.0","rules_api":[],"capabilities":["discovery/v1"],"operations":["discovery.describe","discovery.inspect"]}),
        );
    }
    if op == "discovery.describe" {
        return Ok(json!({"schema":"lekalo/discovery/v1","files":FILES,"reads":READS}));
    }
    if op != "discovery.inspect" || input["schema"] != "lekalo/discovery/v1" {
        return Err("operation-input".into());
    }
    let files = input["files"].as_array().ok_or("files")?;
    if files.len() > 10000 {
        return Err("files-limit".into());
    }
    let mut out = json!({"schema":"lekalo/discovery/v1","manifests":[],"managers":[],"hints":[],"workspaces":[],"tools":[],"gates":[],"modules":[],"names":[],"refused":[]});
    for f in files {
        let path = f["path"].as_str().ok_or("path")?;
        let name = path.rsplit('/').next().ok_or("path")?;
        let text = f["content"].as_str();
        let value = &f["value"];
        for (file, ecosystem) in d::MANIFESTS {
            if name == *file {
                out["manifests"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"path":path,"detail":ecosystem}));
            }
        }
        inspect(name, path, text, value, &mut out);
    }
    finish(&mut out);
    Ok(out)
}
fn push(out: &mut Value, key: &str, value: Value) {
    out[key].as_array_mut().unwrap().push(value);
}
fn module(
    out: &mut Value,
    path: &str,
    name: Option<&str>,
    standalone: bool,
    priority: u8,
    source: &str,
) {
    push(
        out,
        "modules",
        json!({"path":path_dir(path),"manifest":path,"name":name,"standalone":standalone,"priority":40}),
    );
    if let Some(name) = name {
        push(
            out,
            "names",
            json!({"path":path,"name":name,"source":source,"priority":priority}),
        );
    }
}
fn gates(out: &mut Value, path: &str, rows: &[(&str, &str, &str)]) {
    for (_, gate, command) in rows
        .iter()
        .filter(|(n, _, _)| Some(*n) == path.rsplit('/').next())
    {
        push(
            out,
            "gates",
            json!({"gate":gate,"command":command,"source":"derived","evidence":path,"confidence":"medium"}),
        );
    }
}

fn inspect(name: &str, path: &str, text: Option<&str>, _: &Value, out: &mut Value) {
    if name == "Cargo.lock" {
        push(out, "managers", json!({"path":path,"detail":"cargo"}));
    }
    if name == "Cargo.toml" {
        push(
            out,
            "hints",
            json!({"label":"rust","kind":"language","path":path,"field":null,"confidence":"high"}),
        );
        gates(out, path, d::GATES);
        let n = text.and_then(|s| d::cargo_package_name_bytes(s.as_bytes()));
        module(
            out,
            path,
            n.as_deref(),
            false,
            30,
            "Cargo.toml#package.name",
        );
        if let Some(text) = text.filter(|s| d::has_cargo_workspace(s.as_bytes())) {
            push(
                out,
                "workspaces",
                json!({"path":path_dir(path),"ecosystem":"rust","members":d::cargo_workspace_members(text.as_bytes(),64),"confidence":"high"}),
            );
        }
    }
}
fn finish(_: &mut Value) {}
