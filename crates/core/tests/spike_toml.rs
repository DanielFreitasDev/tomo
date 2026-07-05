//! Spike: prove toml_edit gives us surgical, comment-preserving edits and
//! forced `'''` (literal multiline) string representation.
//!
//! These are the go/no-go regression anchors for the format layer (M2+).

use toml_edit::{DocumentMut, Item, Value};

const SRC: &str = r#"# Tomo request — hand-written comments must survive app saves.

[meta]
name = "Get users" # display name

[http]
method = "GET"
url = "https://old.example/api"
headers = [
  { name = "Accept", value = "application/json" }, # keep me
  { name = "X-Debug", value = "1", enabled = false },
]
"#;

#[test]
fn surgical_edit_preserves_comments_and_touches_one_line() {
    let mut doc: DocumentMut = SRC.parse().expect("valid TOML");

    doc["http"]["url"] = toml_edit::value("https://new.example/api");

    let out = doc.to_string();

    // every comment survives
    assert!(out.contains("# Tomo request — hand-written comments must survive app saves."));
    assert!(out.contains("# display name"));
    assert!(out.contains("# keep me"));

    // the diff is exactly one changed line
    let src_lines: Vec<&str> = SRC.lines().collect();
    let out_lines: Vec<&str> = out.lines().collect();
    assert_eq!(
        src_lines.len(),
        out_lines.len(),
        "line count must not change"
    );
    let changed: Vec<usize> = src_lines
        .iter()
        .zip(out_lines.iter())
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        changed.len(),
        1,
        "exactly one line must differ, got {changed:?}"
    );
    assert!(out_lines[changed[0]].contains("https://new.example/api"));
}

#[test]
fn array_of_inline_tables_roundtrips_untouched() {
    let doc: DocumentMut = SRC.parse().expect("valid TOML");
    assert_eq!(
        doc.to_string(),
        SRC,
        "no-op parse+serialize must be byte-identical"
    );
}

/// Build a toml_edit Value that renders as a literal multiline string (`'''`).
/// toml_edit has no first-class setter for the repr, so we parse a snippet and
/// steal the value — the "parse-a-snippet trick" from the plan.
fn multiline_literal(text: &str) -> Value {
    assert!(
        !text.contains("'''"),
        "caller must use fallback for embedded '''"
    );
    let mut body = text.to_string();
    if !body.ends_with('\n') {
        body.push('\n');
    }
    let snippet = format!("v = '''\n{body}'''");
    let doc: DocumentMut = snippet.parse().expect("snippet must parse");
    doc["v"].as_value().expect("value").clone()
}

#[test]
fn forced_literal_multiline_repr() {
    let mut doc: DocumentMut = "[body]\ntype = \"json\"\n".parse().unwrap();
    let json =
        "{\n  \"name\": \"Ada\",\n  \"quote\": \"backslash \\\\ and \\\"quotes\\\" stay raw\"\n}";
    doc["body"]["content"] = Item::Value(multiline_literal(json));

    let out = doc.to_string();
    assert!(
        out.contains("content = '''\n"),
        "must use literal multiline: {out}"
    );
    // inside a literal string nothing is escaped — the JSON appears verbatim
    assert!(out.contains(r#""quote": "backslash \\ and \"quotes\" stay raw""#));

    // and it round-trips to the same logical string
    let reparsed: DocumentMut = out.parse().unwrap();
    let got = reparsed["body"]["content"].as_str().unwrap();
    assert_eq!(got, format!("{json}\n"));
}

#[test]
fn fallback_when_text_contains_triple_quotes() {
    // content with ''' cannot use the literal form; plain quoted string still round-trips
    let tricky = "a '''weird''' script";
    let mut doc: DocumentMut = "[scripts]\n".parse().unwrap();
    doc["scripts"]["pre_request"] = toml_edit::value(tricky);
    let out = doc.to_string();
    let reparsed: DocumentMut = out.parse().unwrap();
    assert_eq!(reparsed["scripts"]["pre_request"].as_str().unwrap(), tricky);
}
