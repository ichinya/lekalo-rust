//! Portable syntax evidence from Rust source supplied by the Lekalo host.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::{self, Read, Write};
use std::process::ExitCode;
use tree_sitter::{Language, Parser, Query, QueryCursor, StreamingIterator, Tree};

const PROTOCOL: &str = "lekalo/runtime/v1";
const RULES_API: &str = "lekalo/contract-rules/v1";
const DECLARATIONS: &str = "source.declarations/v1";
const MATCHES: &str = "source.matches/v1";
const QUERIES: &str = "lekalo-rust.queries/v1";
const INPUT_LIMIT: u64 = 8 * 1024 * 1024;
const FACT_LIMIT: usize = 10_000;
// Leave room in the host's 8 MiB envelope for package and rule IDs.
const FACT_BYTES: usize = 7 * 1024 * 1024;

#[derive(Deserialize)]
struct Request {
    protocol: String,
    request_id: String,
    operation: String,
    input: Value,
}

#[derive(Deserialize)]
struct CheckInput {
    files: Vec<Source>,
    rules: Vec<Package>,
}

#[derive(Deserialize)]
struct Source {
    path: String,
    content: String,
}

#[derive(Deserialize)]
struct Package {
    id: String,
    rules: Rules,
}

#[derive(Deserialize)]
struct Rules {
    requires: Vec<String>,
    checks: Vec<Rule>,
    #[serde(default)]
    bindings: Value,
}

#[derive(Deserialize)]
struct Rule {
    id: String,
}

#[derive(Deserialize)]
struct GrammarConfig {
    grammars: [Grammar; 1],
}

#[derive(Deserialize)]
struct Grammar {
    declarations: String,
}

#[derive(Serialize)]
struct Capture<'a> {
    path: &'a str,
    kind: &'a str,
    name: &'a str,
}

fn captures<'a>(query: &'a Query, trees: &'a [(&Source, Tree)]) -> Result<Vec<Capture<'a>>, ()> {
    let mut rows = Vec::new();
    let mut cursor = QueryCursor::new();
    cursor.set_match_limit(FACT_LIMIT as u32);
    for (source, tree) in trees {
        let bytes = source.content.as_bytes();
        let mut matches = cursor.captures(query, tree.root_node(), bytes);
        while let Some((matched, index)) = matches.next() {
            if rows.len() == FACT_LIMIT {
                return Err(());
            }
            let capture = matched.captures[*index];
            rows.push(Capture {
                path: &source.path,
                kind: query.capture_names()[capture.index as usize],
                name: capture.node.utf8_text(bytes).map_err(|_| ())?,
            });
        }
        if cursor.did_exceed_match_limit() {
            return Err(());
        }
    }
    Ok(rows)
}

struct FactBudget(usize);

impl Write for FactBudget {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.0 = self.0.checked_sub(data.len()).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::FileTooLarge,
                "syntax evidence exceeds the wire budget",
            )
        })?;
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl FactBudget {
    fn encode(&mut self, key: &str, rows: &[Capture<'_>]) -> Result<Value, ()> {
        let fact = BTreeMap::from([(key, rows)]);
        // Count the encoded size while captures still borrow the source.
        serde_json::to_writer(self, &fact).map_err(|_| ())?;
        serde_json::to_value(fact).map_err(|_| ())
    }
}

fn fact(
    language: &Language,
    trees: &[(&Source, Tree)],
    declarations: &[Capture<'_>],
    rules: &Rules,
    rule: &Rule,
    budget: &mut FactBudget,
) -> Result<Value, ()> {
    let matches = rules
        .requires
        .iter()
        .any(|capability| capability == MATCHES);
    if let Some(binding) = rules.bindings[QUERIES].get(&rule.id).filter(|_| matches) {
        let query = Query::new(language, binding.as_str().ok_or(())?).map_err(|_| ())?;
        return budget.encode("matches", &captures(&query, trees)?);
    }
    if rules.requires == [DECLARATIONS] {
        return budget.encode("declarations", declarations);
    }
    Err(())
}

fn check(input: CheckInput) -> Result<Value, ()> {
    let language = tree_sitter_rust::LANGUAGE.into();
    let grammar: GrammarConfig =
        serde_json::from_str(include_str!("../../../runtime/grammar.json")).map_err(|_| ())?;
    let declarations = Query::new(&language, &grammar.grammars[0].declarations).map_err(|_| ())?;
    let mut parser = Parser::new();
    parser.set_language(&language).map_err(|_| ())?;
    let mut trees = Vec::new();
    for file in input.files.iter().filter(|file| file.path.ends_with(".rs")) {
        let tree = parser.parse(&file.content, None).ok_or(())?;
        if tree.root_node().has_error() {
            return Err(());
        }
        trees.push((file, tree));
    }
    let declarations = captures(&declarations, &trees)?;
    let mut budget = FactBudget(FACT_BYTES);
    let mut facts = BTreeMap::new();
    for package in &input.rules {
        let mut checks = BTreeMap::new();
        for rule in &package.rules.checks {
            let result = fact(
                &language,
                &trees,
                &declarations,
                &package.rules,
                rule,
                &mut budget,
            )?;
            if checks.insert(&rule.id, result).is_some() {
                return Err(());
            }
        }
        if facts.insert(&package.id, checks).is_some() {
            return Err(());
        }
    }
    Ok(json!({ "state": "complete", "facts": facts }))
}

fn respond(input: &[u8]) -> Result<Value, ()> {
    let request: Request = serde_json::from_slice(input).map_err(|_| ())?;
    if request.protocol != PROTOCOL {
        return Err(());
    }
    let result = match request.operation.as_str() {
        "describe" => json!({
            "id": "lekalo-rust",
            "version": env!("CARGO_PKG_VERSION"),
            "rules_api": [RULES_API],
            "capabilities": [DECLARATIONS, MATCHES],
            "operations": ["check"]
        }),
        "check" => serde_json::from_value(request.input)
            .map_err(|_| ())
            .and_then(check)
            .unwrap_or_else(|_| json!({ "state": "partial", "facts": {} })),
        _ => return Err(()),
    };
    Ok(json!({ "protocol": PROTOCOL, "request_id": request.request_id, "result": result }))
}

fn main() -> ExitCode {
    let mut input = Vec::new();
    if io::stdin()
        .take(INPUT_LIMIT + 1)
        .read_to_end(&mut input)
        .is_err()
        || input.len() as u64 > INPUT_LIMIT
    {
        return ExitCode::from(2);
    }
    let Ok(response) = respond(&input) else {
        return ExitCode::from(2);
    };
    let mut output = io::stdout().lock();
    if serde_json::to_writer(&mut output, &response).is_err() || output.flush().is_err() {
        return ExitCode::from(2);
    }
    ExitCode::SUCCESS
}
