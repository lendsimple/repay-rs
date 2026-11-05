//! `cargo xtask fetch` and `cargo xtask drift`: compare the vendored specs with
//! the ones the sandbox serves.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde_json::Value;

/// Where the sandbox serves each vendored spec.
pub const SOURCES: [(&str, &str); 2] = [
    (
        "repay-v1.json",
        "https://api.sandbox.repayonline.com/rgapi/v1/swagger",
    ),
    (
        "repay-v1-internal.json",
        "https://api.sandbox.repayonline.com/rgapi/v1-internal/swagger",
    ),
];

pub fn download(url: &str) -> Result<String, String> {
    ureq::get(url)
        .call()
        .and_then(|mut response| response.body_mut().read_to_string())
        .map_err(|e| format!("GET {url}: {e}"))
}

/// The operations (by ID) and definitions (by name) in a set of specs.
#[derive(Default)]
struct Items {
    operations: BTreeMap<String, Value>,
    definitions: BTreeMap<String, Value>,
}

fn items(specs: &[&str]) -> Result<Items, String> {
    let mut items = Items::default();
    for spec in specs {
        let spec: Value = serde_json::from_str(spec).map_err(|e| e.to_string())?;
        for methods in spec["paths"]
            .as_object()
            .into_iter()
            .flat_map(|p| p.values())
        {
            for (method, op) in methods.as_object().into_iter().flatten() {
                let id = op["operationId"].as_str().unwrap_or(method).to_owned();
                let mut op = op.clone();
                // Response examples carry the time the spec was served.
                for response in op["responses"]
                    .as_object_mut()
                    .into_iter()
                    .flat_map(|r| r.values_mut())
                {
                    if let Some(response) = response.as_object_mut() {
                        response.remove("examples");
                    }
                }
                items.operations.insert(id, op);
            }
        }
        for (name, def) in spec["definitions"].as_object().into_iter().flatten() {
            items.definitions.insert(name.clone(), def.clone());
        }
    }
    Ok(items)
}

/// What changed between the vendored specs and the live ones, or `None` if
/// nothing did. Response examples are ignored: the server fills in their dates
/// when it serves the spec.
pub fn report(vendored: &[&str], live: &[&str]) -> Result<Option<String>, String> {
    let (old, new) = (items(vendored)?, items(live)?);
    let mut out = String::new();
    section(&mut out, "operations", &old.operations, &new.operations);
    section(&mut out, "definitions", &old.definitions, &new.definitions);
    Ok((!out.is_empty()).then_some(out))
}

fn section(
    out: &mut String,
    what: &str,
    old: &BTreeMap<String, Value>,
    new: &BTreeMap<String, Value>,
) {
    let added: Vec<&str> = new
        .keys()
        .filter(|k| !old.contains_key(*k))
        .map(String::as_str)
        .collect();
    let removed: Vec<&str> = old
        .keys()
        .filter(|k| !new.contains_key(*k))
        .map(String::as_str)
        .collect();
    let changed: Vec<&str> = old
        .iter()
        .filter(|(k, v)| new.get(*k).is_some_and(|n| n != *v))
        .map(|(k, _)| k.as_str())
        .collect();
    for (label, names) in [("added", added), ("removed", removed), ("changed", changed)] {
        if !names.is_empty() {
            writeln!(out, "{what} {label}:").unwrap();
            for name in names {
                writeln!(out, "  {name}").unwrap();
            }
        }
    }
}

/// Today's date in UTC, as `YYYY-MM-DD`.
pub fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("the clock is after 1970")
        .as_secs();
    civil_date(i64::try_from(secs / 86_400).expect("in range"))
}

/// The date `days` after 1970-01-01 (Howard Hinnant's `civil_from_days`).
fn civil_date(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Rewrites the date and checksum in the `schema/README.md` row for `file`.
pub fn update_readme(readme: &str, file: &str, date: &str, sha256: &str) -> Result<String, String> {
    let prefix = format!("| `{file}` |");
    let mut found = false;
    let lines: Vec<String> = readme
        .lines()
        .map(|line| {
            if !line.starts_with(&prefix) {
                return line.to_owned();
            }
            found = true;
            let cells: Vec<&str> = line.split(" | ").collect();
            // | `file` | <url> | date | `sha` |
            format!("{} | {} | {date} | `{sha256}` |", cells[0], cells[1])
        })
        .collect();
    if !found {
        return Err(format!("schema/README.md has no row for {file}"));
    }
    Ok(lines.join("\n") + "\n")
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .fold(String::new(), |mut hex, b| {
            write!(hex, "{b:02x}").unwrap();
            hex
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD: &str = r#"{"paths": {"/a": {"get": {"operationId": "A_Get"}},
                                    "/b": {"get": {"operationId": "B_Get"}}},
                          "definitions": {"X": {}, "Y": {"type": "object"}}}"#;
    const NEW: &str = r#"{"paths": {"/a": {"get": {"operationId": "A_Get", "deprecated": true}},
                                    "/c": {"post": {"operationId": "C_Post"}}},
                          "definitions": {"X": {}, "Y": {"type": "object"}}}"#;

    #[test]
    fn reports_added_removed_and_changed_items() {
        assert_eq!(
            report(&[OLD], &[NEW]).unwrap().unwrap(),
            "operations added:\n  C_Post\noperations removed:\n  B_Get\noperations changed:\n  A_Get\n"
        );
        assert_eq!(report(&[OLD], &[OLD]).unwrap(), None);
    }

    #[test]
    fn ignores_response_examples() {
        let spec = |date: &str| {
            format!(
                r#"{{"paths": {{"/a": {{"get": {{"operationId": "A_Get", "responses":
                    {{"200": {{"examples": {{"application/json": {{"date": "{date}"}}}}}}}}}}}}}},
                    "definitions": {{}}}}"#
            )
        };
        let (old, new) = (spec("2026-11-04"), spec("2026-11-05"));
        assert_eq!(report(&[&old], &[&new]).unwrap(), None);
    }

    #[test]
    fn dates() {
        assert_eq!(civil_date(0), "1970-01-01");
        assert_eq!(civil_date(20_761), "2026-11-04");
        assert_eq!(civil_date(11_016), "2000-02-29");
    }

    #[test]
    fn updates_the_readme_row() {
        let readme = "| File | Source | Fetched | SHA-256 |\n\
                      | `a.json` | <https://x/a> | 2026-01-01 | `00` |\n\
                      | `b.json` | <https://x/b> | 2026-01-01 | `11` |\n";
        assert_eq!(
            update_readme(readme, "b.json", "2026-11-04", "ff").unwrap(),
            "| File | Source | Fetched | SHA-256 |\n\
             | `a.json` | <https://x/a> | 2026-01-01 | `00` |\n\
             | `b.json` | <https://x/b> | 2026-11-04 | `ff` |\n"
        );
        assert!(update_readme(readme, "c.json", "", "").is_err());
    }

    #[test]
    fn hashes() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
