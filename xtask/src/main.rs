//! Development tasks: `cargo xtask <task>`.

mod emit;
mod model;
mod swagger;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "usage: cargo xtask <task>

tasks:
    codegen [--check]   generate the schema code from schema/*.json
                        (--check: fail if the committed files are out of date)";

/// The vendored specs, public first.
const SPECS: [&str; 2] = ["repay-v1.json", "repay-v1-internal.json"];

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["codegen"] => codegen(false),
        ["codegen", "--check"] => codegen(true),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is inside the workspace")
        .to_path_buf()
}

fn read(path: &Path) -> Result<String> {
    std::fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()).into())
}

/// The public and internal specs, merged.
fn load_spec() -> Result<swagger::Spec> {
    let dir = workspace_root().join("schema");
    let [public, internal] = SPECS.map(|name| {
        let path = dir.join(name);
        read(&path).and_then(|json| {
            swagger::Spec::parse(&json).map_err(|e| format!("{}: {e}", path.display()).into())
        })
    });
    Ok(swagger::merge(public?, internal?)?)
}

fn load_overrides() -> Result<model::Overrides> {
    let path = workspace_root().join("schema/overrides.toml");
    Ok(toml::from_str(&read(&path)?).map_err(|e| format!("schema/overrides.toml: {e}"))?)
}

/// Builds the model from the vendored specs and overrides.
pub fn load_model() -> Result<model::Model> {
    Ok(model::build(&load_spec()?, &load_overrides()?)?)
}

fn codegen(check: bool) -> Result<()> {
    let model = load_model()?;
    let out_dir = workspace_root().join("repay/src/schema");
    let rendered = emit::render(&model);
    let stale = stale_files(&rendered, &out_dir);
    if check {
        if !stale.is_empty() {
            return Err(format!(
                "generated code is out of date; run `cargo xtask codegen`:\n  {}",
                stale.join("\n  ")
            )
            .into());
        }
        return Ok(());
    }
    std::fs::create_dir_all(&out_dir)?;
    for (name, contents) in &rendered {
        if stale.contains(name) {
            let path = out_dir.join(name);
            std::fs::write(&path, contents)?;
            println!("wrote {}", path.display());
        }
    }
    Ok(())
}

/// The names of the rendered files whose committed copy in `dir` differs.
fn stale_files<'a>(rendered: &[(&'a str, String)], dir: &Path) -> Vec<&'a str> {
    rendered
        .iter()
        .filter(|(name, contents)| {
            std::fs::read_to_string(dir.join(name)).ok().as_ref() != Some(contents)
        })
        .map(|(name, _)| *name)
        .collect()
}

#[cfg(test)]
mod tests {
    //! Snapshots of how the trickiest definitions and operations map to Rust.
    //! Review changes with `cargo insta review` (or rerun with
    //! `INSTA_UPDATE=always`).

    use super::*;
    use crate::model::Model;

    fn structure(model: &Model, name: &str) -> String {
        let def = model.structs.iter().find(|s| s.name == name);
        format!(
            "{:#?}",
            def.unwrap_or_else(|| panic!("no generated type {name}"))
        )
    }

    fn enum_def(model: &Model, name: &str) -> String {
        let def = model.enums.iter().find(|e| e.name == name);
        format!(
            "{:#?}",
            def.unwrap_or_else(|| panic!("no generated enum {name}"))
        )
    }

    fn operation(model: &Model, id: &str) -> String {
        let op = model.operations.iter().find(|o| o.id == id);
        format!("{:#?}", op.unwrap_or_else(|| panic!("no operation {id}")))
    }

    #[test]
    fn stale_files_finds_missing_and_edited_files() {
        let dir = std::env::temp_dir().join(format!("repay-xtask-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let rendered = [("a.rs", "a\n".to_owned()), ("b.rs", "b\n".to_owned())];
        assert_eq!(stale_files(&rendered, &dir), ["a.rs", "b.rs"]);
        std::fs::write(dir.join("a.rs"), "a\n").unwrap();
        std::fs::write(dir.join("b.rs"), "b\n").unwrap();
        assert!(stale_files(&rendered, &dir).is_empty());
        std::fs::write(dir.join("b.rs"), "b, edited by hand\n").unwrap();
        assert_eq!(stale_files(&rendered, &dir), ["b.rs"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn vendored_specs_merge() {
        let spec = load_spec().unwrap();
        assert_eq!(spec.operations().count(), 138);
        assert_eq!(spec.definitions.len(), 133);
    }

    #[test]
    fn every_operation_and_definition_is_accounted_for() {
        let model = load_model().unwrap();
        assert_eq!(model.operations.len(), 138);
        // 133 definitions, less the 5 skipped.
        assert_eq!(model.structs.len(), 128);
    }

    #[test]
    fn card_sale_request() {
        insta::assert_snapshot!(structure(&load_model().unwrap(), "CardSaleRequest"));
    }

    #[test]
    fn ach_sale_request_alternatives_are_optional() {
        insta::assert_snapshot!(structure(&load_model().unwrap(), "AchSaleRequest"));
    }

    #[test]
    fn generic_report_is_concrete() {
        insta::assert_snapshot!(structure(
            &load_model().unwrap(),
            "ReconciliationCardTransactionsReport"
        ));
    }

    #[test]
    fn address_and_its_enums() {
        let model = load_model().unwrap();
        insta::assert_snapshot!(format!(
            "{}\n{}\n{}",
            structure(&model, "Address"),
            enum_def(&model, "State"),
            enum_def(&model, "CountryCode")
        ));
    }

    #[test]
    fn error_response() {
        let model = load_model().unwrap();
        insta::assert_snapshot!(format!(
            "{}\n{}",
            structure(&model, "ErrorResponse"),
            enum_def(&model, "StatusCode")
        ));
    }

    #[test]
    fn operations() {
        let model = load_model().unwrap();
        insta::assert_snapshot!(
            [
                "CardTransactions_ProcessSaleAsync",
                "CardTransactions_ProcessCapture",
                "DigitalWallet_GetMerchant",
                "CardTransactions_QueryExportTransactions",
                "CardSettlementBatchClosing_CloseCardSettlementBatch",
                "Adjustments_ProcessBulkDisbursementFundingAdjustment",
            ]
            .map(|id| operation(&model, id))
            .join("\n")
        );
    }
}
