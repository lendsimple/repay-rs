//! The subset of Swagger 2.0 the Repay specs use.
//!
//! Unknown keys are errors, so that a spec feature this code does not interpret
//! fails loudly instead of being dropped.

use std::collections::BTreeMap;

use indexmap::IndexMap;
use serde::Deserialize;

/// A Swagger 2.0 document.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    pub swagger: String,
    #[allow(dead_code)]
    pub info: serde_json::Value,
    #[allow(dead_code)]
    pub host: Option<String>,
    #[serde(rename = "basePath")]
    pub base_path: Option<String>,
    #[allow(dead_code)]
    pub schemes: Option<Vec<String>>,
    /// Path template -> HTTP method -> operation.
    pub paths: BTreeMap<String, BTreeMap<String, Operation>>,
    pub definitions: BTreeMap<String, Schema>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Operation {
    #[allow(dead_code)]
    #[serde(default)]
    pub tags: Vec<String>,
    pub summary: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "operationId")]
    pub operation_id: String,
    #[allow(dead_code)]
    #[serde(default)]
    pub consumes: Vec<String>,
    #[allow(dead_code)]
    #[serde(default)]
    pub produces: Vec<String>,
    #[serde(default)]
    pub parameters: Vec<Parameter>,
    /// Status code -> response.
    pub responses: BTreeMap<String, Response>,
    #[serde(default)]
    pub deprecated: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameter {
    pub name: String,
    #[serde(rename = "in")]
    pub location: Location,
    #[serde(default)]
    pub required: bool,
    #[serde(rename = "type")]
    pub ty: Option<String>,
    pub format: Option<String>,
    pub description: Option<String>,
    pub items: Option<Box<Schema>>,
    #[serde(rename = "collectionFormat")]
    pub collection_format: Option<String>,
    pub pattern: Option<String>,
    #[serde(rename = "enum")]
    pub values: Option<Vec<String>>,
    /// The schema of a body parameter.
    pub schema: Option<Schema>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Location {
    Path,
    Query,
    Header,
    Body,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    #[allow(dead_code)]
    pub description: Option<String>,
    pub schema: Option<Schema>,
    #[allow(dead_code)]
    pub examples: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Schema {
    #[serde(rename = "$ref")]
    pub reference: Option<String>,
    #[serde(rename = "type")]
    pub ty: Option<String>,
    pub format: Option<String>,
    pub description: Option<String>,
    pub items: Option<Box<Schema>>,
    /// In spec order, which the generated fields follow.
    #[serde(default)]
    pub properties: IndexMap<String, Schema>,
    #[serde(default)]
    pub required: Vec<String>,
    #[serde(rename = "enum")]
    pub values: Option<Vec<String>>,
    #[serde(rename = "additionalProperties")]
    pub additional_properties: Option<Box<Schema>>,
    #[serde(rename = "readOnly", default)]
    pub read_only: bool,
    #[serde(rename = "maxLength")]
    pub max_length: Option<u64>,
    #[serde(rename = "minLength")]
    pub min_length: Option<u64>,
    /// A number, or (once, from .NET enum leakage) a string.
    pub minimum: Option<serde_json::Value>,
    pub maximum: Option<serde_json::Value>,
    pub pattern: Option<String>,
    #[allow(dead_code)]
    pub example: Option<serde_json::Value>,
}

impl Schema {
    /// The definition name a `$ref` points at.
    pub fn ref_name(&self) -> Option<&str> {
        self.reference
            .as_deref()
            .map(|r| r.strip_prefix("#/definitions/").unwrap_or(r))
    }
}

impl Spec {
    pub fn parse(json: &str) -> Result<Self, String> {
        let spec: Spec = serde_json::from_str(json).map_err(|e| e.to_string())?;
        if spec.swagger != "2.0" {
            return Err(format!("expected Swagger 2.0, found {}", spec.swagger));
        }
        Ok(spec)
    }

    /// Every operation, as (method, path, operation).
    pub fn operations(&self) -> impl Iterator<Item = (&str, &str, &Operation)> {
        self.paths.iter().flat_map(|(path, methods)| {
            methods
                .iter()
                .map(move |(method, op)| (method.as_str(), path.as_str(), op))
        })
    }
}

/// Combines the public and internal specs. They share a base path; an operation or
/// path that appears in both, or a definition that differs between them, is an
/// error.
pub fn merge(public: Spec, internal: Spec) -> Result<Spec, String> {
    if public.base_path != internal.base_path {
        return Err(format!(
            "the specs have different base paths: {:?} and {:?}",
            public.base_path, internal.base_path
        ));
    }
    let mut merged = public;
    let ids: std::collections::HashSet<String> = merged
        .operations()
        .map(|(_, _, op)| op.operation_id.clone())
        .collect();
    for (path, methods) in internal.paths {
        let entry = merged.paths.entry(path.clone()).or_default();
        for (method, op) in methods {
            if ids.contains(&op.operation_id) {
                return Err(format!("operation {} is in both specs", op.operation_id));
            }
            if entry.insert(method.clone(), op).is_some() {
                return Err(format!("{} {path} is in both specs", method.to_uppercase()));
            }
        }
    }
    for (name, def) in internal.definitions {
        match merged.definitions.get(&name) {
            Some(existing) if *existing != def => {
                return Err(format!("definition {name} differs between the specs"));
            }
            Some(_) => {}
            None => {
                merged.definitions.insert(name, def);
            }
        }
    }
    Ok(merged)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(paths: &str, definitions: &str) -> Spec {
        Spec::parse(&format!(
            r#"{{"swagger": "2.0", "info": {{}}, "basePath": "/rgapi",
                "paths": {paths}, "definitions": {definitions}}}"#
        ))
        .unwrap()
    }

    const OP: &str = r#"{"operationId": "A_Get", "responses": {}}"#;

    #[test]
    fn rejects_unknown_keys() {
        let err = Spec::parse(
            r#"{"swagger": "2.0", "info": {}, "paths": {},
                "definitions": {"X": {"type": "object", "oneOf": []}}}"#,
        )
        .unwrap_err();
        assert!(err.contains("oneOf"), "{err}");
    }

    #[test]
    fn merge_adds_internal_operations_and_definitions() {
        let public = spec(&format!(r#"{{"/a": {{"get": {OP}}}}}"#), r#"{"X": {}}"#);
        let internal = spec(
            r#"{"/b": {"post": {"operationId": "B_Post", "responses": {}}}}"#,
            r#"{"X": {}, "Y": {}}"#,
        );
        let merged = merge(public, internal).unwrap();
        assert_eq!(merged.operations().count(), 2);
        assert_eq!(merged.definitions.len(), 2);
    }

    #[test]
    fn merge_rejects_conflicts() {
        let public = spec(&format!(r#"{{"/a": {{"get": {OP}}}}}"#), r#"{"X": {}}"#);
        let internal = spec(&format!(r#"{{"/c": {{"get": {OP}}}}}"#), "{}");
        assert_eq!(
            merge(public, internal).unwrap_err(),
            "operation A_Get is in both specs"
        );

        let public = spec("{}", r#"{"X": {"type": "object"}}"#);
        let internal = spec("{}", r#"{"X": {"type": "string"}}"#);
        assert_eq!(
            merge(public, internal).unwrap_err(),
            "definition X differs between the specs"
        );
    }
}
