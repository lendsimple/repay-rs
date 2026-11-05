//! The model the Rust code is generated from: the merged spec with
//! `schema/overrides.toml` applied.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use heck::ToUpperCamelCase;
use serde::Deserialize;

use crate::swagger::{Location, Operation, Parameter, Schema, Spec};

/// `schema/overrides.toml`. Every key must match something in the spec; a stale
/// key is an error.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Overrides {
    /// Definitions that are not part of the API (.NET types the generator leaked).
    #[serde(default)]
    pub skip_definitions: Vec<String>,
    /// Definition name -> Rust name.
    #[serde(default)]
    pub rename: BTreeMap<String, String>,
    #[serde(default)]
    pub retype: Retype,
    #[serde(default)]
    pub required: RequiredFixes,
    /// `Def.field` whose `minimum`/`maximum` are not checked.
    #[serde(default)]
    pub ignore_bounds: Vec<String>,
    /// Field names whose values `Debug` output redacts.
    #[serde(default)]
    pub sensitive: Vec<String>,
    /// Pattern -> the `repay::validate::rules` function that checks it.
    #[serde(default)]
    pub patterns: BTreeMap<String, String>,
    /// Enum name -> the `Def.field` (or `OperationId.param`) properties that use it.
    #[serde(default)]
    pub enums: BTreeMap<String, Vec<String>>,
    /// Enum name -> value -> variant name, for values that are not identifiers.
    #[serde(default)]
    pub enum_variants: BTreeMap<String, BTreeMap<String, String>>,
    /// Path parameter name -> Rust type.
    #[serde(default)]
    pub path_params: BTreeMap<String, String>,
    /// Operation ID -> where it goes in the client.
    #[serde(default)]
    pub operations: BTreeMap<String, OperationOverride>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Retype {
    /// Field name -> Rust type, for every definition where the type fits.
    #[serde(default)]
    pub by_name: BTreeMap<String, String>,
    /// Field name -> Rust type, in request definitions only.
    #[serde(default)]
    pub requests: BTreeMap<String, String>,
    /// `Def.field` -> Rust type.
    #[serde(default)]
    pub fields: BTreeMap<String, String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequiredFixes {
    /// `Def.field` the spec leaves optional but the API needs.
    #[serde(default)]
    pub add: Vec<String>,
    /// `Def.field` the spec marks required but that are one of several alternatives.
    #[serde(default)]
    pub remove: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationOverride {
    pub resource: String,
    pub method: String,
    /// The body is a multipart CSV upload, which the spec does not declare.
    #[serde(default)]
    pub multipart: bool,
    /// Query parameters to leave out (wire names).
    #[serde(default)]
    pub drop_query: Vec<String>,
}

/// A Rust type for a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ty {
    String,
    Bool,
    I32,
    I64,
    F64,
    DateTime,
    Json,
    /// `BTreeMap<String, String>`.
    Map,
    Vec(Box<Ty>),
    /// A generated type, or a runtime type such as `Amount`.
    Named(String),
}

impl Ty {
    /// The type with its element type replaced, for retyping inside a `Vec`.
    fn map_scalar(&self, f: impl FnOnce(&Ty) -> Ty) -> Ty {
        match self {
            Ty::Vec(inner) => Ty::Vec(Box::new(inner.map_scalar(f))),
            other => f(other),
        }
    }

    fn scalar(&self) -> &Ty {
        match self {
            Ty::Vec(inner) => inner.scalar(),
            other => other,
        }
    }

    pub fn is_vec(&self) -> bool {
        matches!(self, Ty::Vec(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Request,
    Response,
    Both,
}

impl Role {
    pub fn serializes(self) -> bool {
        matches!(self, Role::Request | Role::Both)
    }

    pub fn deserializes(self) -> bool {
        matches!(self, Role::Response | Role::Both)
    }
}

#[derive(Debug)]
pub struct Model {
    pub structs: Vec<StructDef>,
    pub enums: Vec<EnumDef>,
    pub operations: Vec<OperationDef>,
    pub sensitive: Vec<String>,
}

#[derive(Debug)]
pub struct StructDef {
    pub name: String,
    /// The definition name in the spec.
    pub origin: String,
    pub role: Role,
    pub doc: Option<String>,
    pub fields: Vec<Field>,
}

#[derive(Debug)]
pub struct Field {
    /// The property name, which is also the Rust field name.
    pub name: String,
    pub ty: Ty,
    /// The field is `T` (or a `Vec` that is always sent) rather than `Option<T>`.
    pub required: bool,
    pub doc: Option<String>,
    pub facets: Facets,
    pub sensitive: bool,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Facets {
    pub min_length: Option<u64>,
    pub max_length: Option<u64>,
    pub min: Option<String>,
    pub max: Option<String>,
    pub rule: Option<String>,
}

#[derive(Debug)]
pub struct EnumDef {
    pub name: String,
    pub variants: Vec<Variant>,
}

#[derive(Debug)]
pub struct Variant {
    pub name: String,
    pub value: String,
}

#[derive(Debug)]
pub struct OperationDef {
    pub id: String,
    pub resource: String,
    pub method: String,
    pub http_method: String,
    /// The path after `/v{version}`, with `{name}` placeholders using the wire
    /// names.
    pub path: String,
    pub path_params: Vec<Param>,
    pub query: Vec<QueryParam>,
    pub body: Body,
    pub merchant: Merchant,
    /// `None` for an empty response.
    pub response: Option<Ty>,
    /// The operation takes a `format` parameter that switches to a CSV, Excel or
    /// PDF download.
    pub export: bool,
    pub deprecated: bool,
    pub doc: Option<String>,
}

#[derive(Debug)]
pub struct Param {
    pub field: String,
    pub wire: String,
    pub ty: Ty,
    pub doc: Option<String>,
}

#[derive(Debug)]
pub struct QueryParam {
    pub field: String,
    pub wire: String,
    pub ty: Ty,
    /// An array sent as repeated keys (`collectionFormat: multi`).
    pub multi: bool,
    pub doc: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Body {
    None,
    Json(Ty),
    Multipart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Merchant {
    None,
    Optional,
    Required,
}

/// The parameter that switches a report to a download.
const FORMAT_PARAMS: &[&str] = &["format", "request.format"];
/// Headers every operation declares, which the client sends from its credentials.
const AUTH_HEADERS: &[&str] = &["rg-api-user", "rg-api-secure-token"];
const MERCHANT_HEADER: &str = "rg-merchant-id";

/// Builds the model.
pub fn build(spec: &Spec, overrides: &Overrides) -> Result<Model, String> {
    Builder::new(spec, overrides).build()
}

struct Builder<'a> {
    spec: &'a Spec,
    ov: &'a Overrides,
    /// Override keys that matched something, as `section.key`.
    used: HashSet<String>,
    /// `Def.field` / `OperationId.param` -> enum name.
    enum_names: BTreeMap<String, String>,
    /// Enum name -> values, in spec order.
    enum_values: BTreeMap<String, Vec<String>>,
}

impl<'a> Builder<'a> {
    fn new(spec: &'a Spec, ov: &'a Overrides) -> Self {
        let enum_names = ov
            .enums
            .iter()
            .flat_map(|(name, props)| props.iter().map(move |p| (p.clone(), name.clone())))
            .collect();
        Self {
            spec,
            ov,
            used: HashSet::new(),
            enum_names,
            enum_values: BTreeMap::new(),
        }
    }

    fn build(mut self) -> Result<Model, String> {
        let roles = self.roles()?;
        let mut structs = Vec::new();
        for (name, def) in &self.spec.definitions {
            if self.ov.skip_definitions.contains(name) {
                self.used.insert(format!("skip_definitions.{name}"));
                continue;
            }
            let role = roles[name.as_str()];
            structs.push(self.struct_def(name, def, role)?);
        }
        structs.sort_by(|a, b| a.name.cmp(&b.name));
        if let Some(pair) = structs.windows(2).find(|w| w[0].name == w[1].name) {
            return Err(format!("two definitions are named {}", pair[0].name));
        }

        let mut operations = Vec::new();
        for (method, path, op) in self.spec.operations() {
            operations.push(self.operation(method, path, op)?);
        }
        operations.sort_by(|a, b| (&a.resource, &a.method).cmp(&(&b.resource, &b.method)));
        if let Some(pair) = operations
            .windows(2)
            .find(|w| (&w[0].resource, &w[0].method) == (&w[1].resource, &w[1].method))
        {
            return Err(format!(
                "{} and {} are both {}.{}",
                pair[0].id, pair[1].id, pair[0].resource, pair[0].method
            ));
        }

        let enums = self.enum_defs()?;
        self.check_overrides_used()?;
        let mut sensitive = self.ov.sensitive.clone();
        sensitive.sort();
        Ok(Model {
            structs,
            enums,
            operations,
            sensitive,
        })
    }

    /// Whether each definition is reachable from request bodies, responses or
    /// both. A definition reachable from neither must be skipped, and a skipped
    /// one must not be reachable.
    fn roles(&self) -> Result<BTreeMap<&'a str, Role>, String> {
        let mut requests = BTreeSet::new();
        let mut responses = BTreeSet::new();
        for (_, _, op) in self.spec.operations() {
            for param in &op.parameters {
                if let Some(schema) = &param.schema {
                    self.reach(schema, &mut requests)?;
                }
            }
            for response in op.responses.values() {
                if let Some(schema) = &response.schema {
                    self.reach(schema, &mut responses)?;
                }
            }
        }
        let mut roles = BTreeMap::new();
        for name in self.spec.definitions.keys() {
            let name = name.as_str();
            let role = match (requests.contains(name), responses.contains(name)) {
                (true, true) => Some(Role::Both),
                (true, false) => Some(Role::Request),
                (false, true) => Some(Role::Response),
                (false, false) => None,
            };
            let skipped = self.ov.skip_definitions.iter().any(|s| s == name);
            match (role, skipped) {
                (Some(role), false) => {
                    roles.insert(name, role);
                }
                (None, true) => {}
                (Some(_), true) => {
                    return Err(format!("skipped definition {name} is used by an operation"));
                }
                (None, false) => {
                    return Err(format!(
                        "definition {name} is not used by any operation; add it to skip_definitions"
                    ));
                }
            }
        }
        Ok(roles)
    }

    fn reach(&self, schema: &'a Schema, seen: &mut BTreeSet<&'a str>) -> Result<(), String> {
        if let Some(name) = schema.ref_name() {
            let (name, def) = self
                .spec
                .definitions
                .get_key_value(name)
                .ok_or_else(|| format!("$ref to unknown definition {name}"))?;
            if seen.insert(name.as_str()) {
                self.reach(def, seen)?;
            }
        }
        for prop in schema.properties.values() {
            self.reach(prop, seen)?;
        }
        for nested in [&schema.items, &schema.additional_properties]
            .into_iter()
            .flatten()
        {
            self.reach(nested, seen)?;
        }
        Ok(())
    }

    fn type_name(&mut self, definition: &str) -> String {
        match self.ov.rename.get(definition) {
            Some(name) => {
                self.used.insert(format!("rename.{definition}"));
                name.clone()
            }
            None => definition.to_owned(),
        }
    }

    fn struct_def(&mut self, name: &str, def: &Schema, role: Role) -> Result<StructDef, String> {
        if def.ty.as_deref().is_some_and(|t| t != "object") {
            return Err(format!("definition {name} is not an object"));
        }
        let mut required: BTreeSet<&str> = def.required.iter().map(String::as_str).collect();
        for key in &self.ov.required.add {
            if let Some(field) = key.strip_prefix(&format!("{name}.")) {
                self.used.insert(format!("required.add.{key}"));
                if !def.properties.contains_key(field) || !required.insert(field) {
                    return Err(format!("required.add {key}: no such optional field"));
                }
            }
        }
        for key in &self.ov.required.remove {
            if let Some(field) = key.strip_prefix(&format!("{name}.")) {
                self.used.insert(format!("required.remove.{key}"));
                if !required.remove(field) {
                    return Err(format!("required.remove {key}: no such required field"));
                }
            }
        }
        if let Some(missing) = required.iter().find(|f| !def.properties.contains_key(**f)) {
            return Err(format!("{name} requires unknown field {missing}"));
        }

        let mut fields = Vec::new();
        for (field, prop) in &def.properties {
            let context = format!("{name}.{field}");
            let ty = self.schema_type(prop, &context)?;
            let ty = self.retype(ty, prop, field, &context, role)?;
            let facets = if role.serializes() {
                self.facets(prop, &ty, &context)?
            } else {
                Facets::default()
            };
            let sensitive = self.ov.sensitive.iter().any(|s| s == field);
            if sensitive {
                self.used.insert(format!("sensitive.{field}"));
            }
            fields.push(Field {
                name: field.clone(),
                ty,
                required: role.serializes() && required.contains(field.as_str()),
                doc: clean_doc(prop.description.as_deref()),
                facets,
                sensitive,
            });
        }
        Ok(StructDef {
            name: self.type_name(name),
            origin: name.to_owned(),
            role,
            doc: clean_doc(def.description.as_deref()),
            fields,
        })
    }

    /// The Rust type for a schema, before overrides.
    fn schema_type(&mut self, schema: &Schema, context: &str) -> Result<Ty, String> {
        if let Some(name) = schema.ref_name() {
            return Ok(Ty::Named(self.type_name(name)));
        }
        if let Some(values) = &schema.values {
            return self.enum_type(values, context);
        }
        let ty = match (schema.ty.as_deref(), schema.format.as_deref()) {
            (Some("string"), Some("date-time")) => Ty::DateTime,
            (Some("string"), None | Some("uuid")) => Ty::String,
            (Some("boolean"), None) => Ty::Bool,
            (Some("integer"), Some("int32")) => Ty::I32,
            (Some("integer"), Some("int64")) => Ty::I64,
            (Some("number"), Some("double")) => Ty::F64,
            (Some("array"), None) => {
                let items = schema
                    .items
                    .as_deref()
                    .ok_or_else(|| format!("{context}: array without items"))?;
                if *items == Schema::default() {
                    // `items: {}`: values of any type, sent as strings.
                    Ty::Vec(Box::new(Ty::String))
                } else {
                    Ty::Vec(Box::new(self.schema_type(items, context)?))
                }
            }
            (Some("object"), None) => match schema.additional_properties.as_deref() {
                Some(Schema { ty: Some(t), .. }) if t == "string" => Ty::Map,
                _ => Ty::Json,
            },
            (ty, format) => {
                return Err(format!(
                    "{context}: unsupported type {ty:?} with format {format:?}"
                ));
            }
        };
        Ok(ty)
    }

    fn enum_type(&mut self, values: &[String], context: &str) -> Result<Ty, String> {
        let name = self
            .enum_names
            .get(context)
            .cloned()
            .ok_or_else(|| format!("{context} is an enum; name it in [enums]"))?;
        self.used.insert(format!("enums.{context}"));
        match self.enum_values.get(&name) {
            Some(existing) if existing != values => Err(format!(
                "enum {name}: {context} has values {values:?}, but another property has {existing:?}"
            )),
            Some(_) => Ok(Ty::Named(name)),
            None => {
                self.enum_values.insert(name.clone(), values.to_vec());
                Ok(Ty::Named(name))
            }
        }
    }

    fn retype(
        &mut self,
        ty: Ty,
        prop: &Schema,
        field: &str,
        context: &str,
        role: Role,
    ) -> Result<Ty, String> {
        if let Some(target) = self.ov.retype.fields.get(context) {
            self.used.insert(format!("retype.fields.{context}"));
            return Ok(ty.map_scalar(|_| named(target)));
        }
        let by_role = if role.serializes() {
            self.ov.retype.requests.get(field).map(|t| ("requests", t))
        } else {
            None
        };
        let by_name = self.ov.retype.by_name.get(field).map(|t| ("by_name", t));
        for (section, target) in by_role.into_iter().chain(by_name) {
            let swagger_type = schema_scalar(prop).ty.as_deref().unwrap_or("");
            if fits(target, swagger_type)? {
                self.used.insert(format!("retype.{section}.{field}"));
                return Ok(ty.map_scalar(|_| named(target)));
            }
        }
        Ok(ty)
    }

    fn facets(&mut self, prop: &Schema, ty: &Ty, context: &str) -> Result<Facets, String> {
        let prop = schema_scalar(prop);
        let mut facets = Facets {
            min_length: prop.min_length.filter(|n| *n > 0),
            max_length: prop.max_length,
            ..Facets::default()
        };
        if let Some(pattern) = &prop.pattern {
            let rule = self.ov.patterns.get(pattern).ok_or_else(|| {
                format!("{context}: no rule for pattern {pattern:?} in [patterns]")
            })?;
            self.used.insert(format!("patterns.{pattern}"));
            facets.rule = Some(rule.clone());
        }
        if prop.minimum.is_some() || prop.maximum.is_some() {
            if self.ov.ignore_bounds.iter().any(|k| k == context) {
                self.used.insert(format!("ignore_bounds.{context}"));
            } else if matches!(ty.scalar(), Ty::I32 | Ty::I64) {
                facets.min = prop.minimum.as_ref().map(ToString::to_string);
                facets.max = prop.maximum.as_ref().map(ToString::to_string);
            } else {
                return Err(format!(
                    "{context}: minimum/maximum on a non-integer; add it to ignore_bounds"
                ));
            }
        }
        Ok(facets)
    }

    fn operation(
        &mut self,
        method: &str,
        path: &str,
        op: &Operation,
    ) -> Result<OperationDef, String> {
        let id = &op.operation_id;
        let ov = self
            .ov
            .operations
            .get(id)
            .ok_or_else(|| format!("operation {id} has no entry in [operations]"))?;
        self.used.insert(format!("operations.{id}"));
        let path = path
            .strip_prefix("/v{version}")
            .ok_or_else(|| format!("{id}: path {path} does not start with /v{{version}}"))?;

        let mut def = OperationDef {
            id: id.clone(),
            resource: ov.resource.clone(),
            method: ov.method.clone(),
            http_method: method.to_uppercase(),
            path: path.to_owned(),
            path_params: Vec::new(),
            query: Vec::new(),
            body: if ov.multipart {
                Body::Multipart
            } else {
                Body::None
            },
            merchant: Merchant::None,
            response: self.response(op)?,
            export: false,
            deprecated: op.deprecated,
            doc: operation_doc(op),
        };
        let mut dropped = BTreeSet::new();
        for param in &op.parameters {
            let context = format!("{id}.{}", param.name);
            match param.location {
                Location::Path if param.name == "version" => {}
                Location::Path => {
                    let ty = self.param_type(param, &context)?;
                    let ty = match self.ov.path_params.get(&param.name) {
                        Some(target) if fits(target, param.ty.as_deref().unwrap_or(""))? => {
                            self.used.insert(format!("path_params.{}", param.name));
                            Ty::Named(target.clone())
                        }
                        _ => ty,
                    };
                    def.path_params.push(Param {
                        field: field_name(&param.name),
                        wire: param.name.clone(),
                        ty,
                        doc: clean_doc(param.description.as_deref()),
                    });
                }
                Location::Query if ov.drop_query.contains(&param.name) => {
                    dropped.insert(param.name.as_str());
                }
                Location::Query if FORMAT_PARAMS.contains(&param.name.as_str()) => {
                    def.export = true;
                }
                Location::Query => {
                    let multi = param.collection_format.as_deref() == Some("multi");
                    if let Some(format) = param.collection_format.as_deref().filter(|_| !multi) {
                        return Err(format!("{context}: unsupported collectionFormat {format}"));
                    }
                    if param.pattern.is_some() {
                        return Err(format!(
                            "{context}: patterns on query parameters are not checked"
                        ));
                    }
                    def.query.push(QueryParam {
                        field: field_name(&param.name),
                        wire: param.name.clone(),
                        ty: self.param_type(param, &context)?,
                        multi,
                        doc: clean_doc(param.description.as_deref()),
                    });
                }
                Location::Header if AUTH_HEADERS.contains(&param.name.as_str()) => {}
                Location::Header if param.name == MERCHANT_HEADER => {
                    def.merchant = if param.required {
                        Merchant::Required
                    } else {
                        Merchant::Optional
                    };
                }
                Location::Header => return Err(format!("{context}: unknown header")),
                Location::Body => {
                    if def.body != Body::None {
                        return Err(format!("{id}: a body and a multipart upload"));
                    }
                    let schema = param
                        .schema
                        .as_ref()
                        .ok_or_else(|| format!("{context}: body without a schema"))?;
                    def.body = Body::Json(self.schema_type(schema, &context)?);
                }
            }
        }
        if let Some(missing) = ov.drop_query.iter().find(|q| !dropped.contains(q.as_str())) {
            return Err(format!(
                "{id}: drop_query {missing} is not a query parameter"
            ));
        }
        for param in &def.path_params {
            if !def.path.contains(&format!("{{{}}}", param.wire)) {
                return Err(format!(
                    "{id}: path parameter {} is not in the path",
                    param.wire
                ));
            }
        }
        if def.path.matches('{').count() != def.path_params.len() {
            return Err(format!("{id}: the path has undeclared parameters"));
        }
        Ok(def)
    }

    fn param_type(&mut self, param: &Parameter, context: &str) -> Result<Ty, String> {
        let schema = Schema {
            ty: param.ty.clone(),
            format: param.format.clone(),
            items: param.items.clone(),
            values: param.values.clone(),
            ..Schema::default()
        };
        self.schema_type(&schema, context)
    }

    /// The type of the first successful response that declares one: 200 is often
    /// an untyped object while 201 or 202 carries the real type.
    fn response(&mut self, op: &Operation) -> Result<Option<Ty>, String> {
        let successes: Vec<_> = op
            .responses
            .iter()
            .filter(|(code, _)| code.starts_with('2'))
            .collect();
        if successes.is_empty() {
            return Err(format!("{}: no successful response", op.operation_id));
        }
        let typed = successes.iter().find_map(|(_, r)| {
            r.schema
                .as_ref()
                .filter(|s| s.ref_name().is_some() || s.ty.as_deref() == Some("array"))
        });
        if let Some(schema) = typed {
            return self.schema_type(schema, &op.operation_id).map(Some);
        }
        Ok(
            match successes[0].1.schema.as_ref().and_then(|s| s.ty.as_deref()) {
                None => None,
                Some("string") => Some(Ty::String),
                Some(_) => Some(Ty::Json),
            },
        )
    }

    fn enum_defs(&mut self) -> Result<Vec<EnumDef>, String> {
        let mut enums = Vec::new();
        for (name, values) in &self.enum_values {
            let names = self.ov.enum_variants.get(name);
            if names.is_some() {
                self.used.insert(format!("enum_variants.{name}"));
            }
            let mut variants: Vec<Variant> = Vec::new();
            for value in values {
                let variant = match names.and_then(|n| n.get(value)) {
                    Some(variant) => variant.clone(),
                    None => value.to_upper_camel_case(),
                };
                if !variant.starts_with(|c: char| c.is_ascii_uppercase())
                    || syn::parse_str::<syn::Ident>(&variant).is_err()
                {
                    return Err(format!(
                        "enum {name}: value {value:?} needs a variant name in [enum_variants.{name}]"
                    ));
                }
                if variants.iter().any(|v| v.name == variant) || variant == "Other" {
                    return Err(format!("enum {name}: two values are named {variant}"));
                }
                variants.push(Variant {
                    name: variant,
                    value: value.clone(),
                });
            }
            if let Some(names) = names
                && let Some(stale) = names.keys().find(|v| !values.contains(v))
            {
                return Err(format!("enum_variants.{name}: no value {stale:?}"));
            }
            enums.push(EnumDef {
                name: name.clone(),
                variants,
            });
        }
        Ok(enums)
    }

    fn check_overrides_used(&self) -> Result<(), String> {
        let ov = self.ov;
        let keys = |section: &'static str, keys: Vec<String>| {
            keys.into_iter().map(move |k| format!("{section}.{k}"))
        };
        let all = keys("skip_definitions", ov.skip_definitions.clone())
            .chain(keys("rename", ov.rename.keys().cloned().collect()))
            .chain(keys(
                "retype.by_name",
                ov.retype.by_name.keys().cloned().collect(),
            ))
            .chain(keys(
                "retype.requests",
                ov.retype.requests.keys().cloned().collect(),
            ))
            .chain(keys(
                "retype.fields",
                ov.retype.fields.keys().cloned().collect(),
            ))
            .chain(keys("required.add", ov.required.add.clone()))
            .chain(keys("required.remove", ov.required.remove.clone()))
            .chain(keys("ignore_bounds", ov.ignore_bounds.clone()))
            .chain(keys("sensitive", ov.sensitive.clone()))
            .chain(keys("patterns", ov.patterns.keys().cloned().collect()))
            .chain(keys(
                "enums",
                ov.enums.values().flatten().cloned().collect(),
            ))
            .chain(keys(
                "enum_variants",
                ov.enum_variants.keys().cloned().collect(),
            ))
            .chain(keys(
                "path_params",
                ov.path_params.keys().cloned().collect(),
            ))
            .chain(keys("operations", ov.operations.keys().cloned().collect()));
        let stale: Vec<String> = all.filter(|k| !self.used.contains(k)).collect();
        if stale.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "schema/overrides.toml has entries that match nothing in the spec: {}",
                stale.join(", ")
            ))
        }
    }
}

/// The type a retype target names.
fn named(target: &str) -> Ty {
    match target {
        "String" => Ty::String,
        other => Ty::Named(other.to_owned()),
    }
}

/// The schema of an array's elements, or the schema itself.
fn schema_scalar(schema: &Schema) -> &Schema {
    match (&schema.ty, &schema.items) {
        (Some(t), Some(items)) if t == "array" => schema_scalar(items),
        _ => schema,
    }
}

/// Whether a retype target can stand for a value of the given Swagger type, so
/// that a name-based retype skips fields of another type (`transaction_id` is a
/// string in one place).
fn fits(target: &str, swagger_type: &str) -> Result<bool, String> {
    let accepts: &[&str] = match target {
        "Amount" => &["number"],
        "AmountString" | "ExpDate" | "TemplateId" | "OrgId" | "AuditKey" => &["string"],
        "TransactionId" | "CustomerKey" | "CardTokenKey" | "AchTokenKey" | "ContractKey"
        | "AuthFormId" | "RuleId" | "CustomFieldId" | "MerchantId" => &["integer"],
        // The spec types card tokens in requests as `object`.
        "String" => &["object", "string"],
        other => return Err(format!("unknown retype target {other}")),
    };
    Ok(accepts.contains(&swagger_type))
}

/// The Rust field name for a parameter: `query.start_date` -> `start_date`.
fn field_name(wire: &str) -> String {
    let bare = wire
        .strip_prefix("query.")
        .or_else(|| wire.strip_prefix("request."))
        .unwrap_or(wire);
    bare.replace(['.', '-'], "_")
}

fn operation_doc(op: &Operation) -> Option<String> {
    let summary = clean_doc(op.summary.as_deref());
    let description = clean_doc(op.description.as_deref());
    match (summary, description) {
        (Some(s), Some(d)) if s != d => Some(format!("{s}\n\n{d}")),
        (Some(s), _) => Some(s),
        (None, d) => d,
    }
}

/// Tidies a spec description: line endings, HTML entities, the XML doc tags and
/// `{Namespace.Type}` references .NET left in, surrounding space, and the "Gets or
/// sets" of .NET property docs.
pub fn clean_doc(text: Option<&str>) -> Option<String> {
    let mut text = text?.replace("\r\n", "\n");
    for (tag, with) in [
        ("<list type=\"bullet\">", "\n"),
        ("<list type=\"number\">", "\n"),
        ("</list>", "\n\n"),
        ("<item><description>", "\n- "),
        ("<item>", "\n- "),
        ("<description>", ""),
        ("</description>", ""),
        ("</item>", ""),
        ("<para>", "\n\n"),
        ("</para>", "\n\n"),
        ("<b>", ""),
        ("</b>", ""),
    ] {
        text = text.replace(tag, with);
    }
    let text = unqualify_references(&text)
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&");
    // Trim each line, and collapse runs of blank lines.
    let mut lines: Vec<&str> = Vec::new();
    for line in text.lines().map(str::trim) {
        if line.is_empty() && lines.last().is_none_or(|l| l.is_empty()) {
            continue;
        }
        lines.push(line);
    }
    let text = lines.join("\n");
    let text = text.trim();
    let text = match text.strip_prefix("Gets or sets ") {
        Some(rest) => {
            let mut chars = rest.chars();
            chars
                .next()
                .map(|c| c.to_uppercase().chain(chars).collect())
                .unwrap_or_default()
        }
        None => text.to_owned(),
    };
    (!text.is_empty()).then_some(text)
}

/// `{RepayGatewayApi.Models.Models.CardSaleRequest}` -> `` `CardSaleRequest` ``.
fn unqualify_references(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let end = after.find('}');
        match end.map(|end| &after[..end]) {
            Some(path)
                if path.contains('.')
                    && path
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_') =>
            {
                let name = path.rsplit('.').next().unwrap_or(path);
                out.push('`');
                out.push_str(name);
                out.push('`');
                rest = &after[path.len() + 1..];
            }
            _ => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC: &str = r##"{
        "swagger": "2.0", "info": {}, "basePath": "/rgapi",
        "paths": {
            "/v{version}/things/{thing_id}": {
                "post": {
                    "operationId": "Things_Create",
                    "parameters": [
                        {"name": "version", "in": "path", "required": true, "type": "string"},
                        {"name": "thing_id", "in": "path", "required": true, "type": "integer", "format": "int32"},
                        {"name": "query.start_date", "in": "query", "required": false, "type": "string", "format": "date-time"},
                        {"name": "format", "in": "query", "required": false, "type": "string"},
                        {"name": "rg-api-user", "in": "header", "required": true, "type": "string"},
                        {"name": "rg-merchant-id", "in": "header", "required": false, "type": "string"},
                        {"name": "request", "in": "body", "required": true, "schema": {"$ref": "#/definitions/ThingRequest"}}
                    ],
                    "responses": {"200": {"schema": {"$ref": "#/definitions/Thing"}}}
                }
            }
        },
        "definitions": {
            "ThingRequest": {
                "type": "object",
                "required": ["name", "token"],
                "properties": {
                    "name": {"type": "string", "maxLength": 10, "minLength": 0, "pattern": "^(?!.*<).*$"},
                    "token": {"type": "string"},
                    "amount": {"type": "number", "format": "double"},
                    "kind": {"type": "string", "enum": ["0", "1"]}
                }
            },
            "Thing": {
                "type": "object",
                "properties": {
                    "kind": {"type": "string", "enum": ["0", "1"]},
                    "notes": {"type": "string", "description": "Gets or sets the notes &amp; such."}
                }
            },
            "Leaked": {"type": "object"}
        }
    }"##;

    const OVERRIDES: &str = r#"
        skip_definitions = ["Leaked"]
        sensitive = ["token"]
        [retype.by_name]
        amount = "Amount"
        [required]
        remove = ["ThingRequest.token"]
        [patterns]
        "^(?!.*<).*$" = "no_angle_bracket"
        [enums]
        Kind = ["ThingRequest.kind", "Thing.kind"]
        [enum_variants.Kind]
        "0" = "Zero"
        "1" = "One"
        [path_params]
        thing_id = "TransactionId"
        [operations]
        Things_Create = { resource = "things", method = "create" }
    "#;

    fn build_with(spec: &str, overrides: &str) -> Result<Model, String> {
        let spec = Spec::parse(spec).unwrap();
        let overrides: Overrides = toml::from_str(overrides).unwrap();
        build(&spec, &overrides)
    }

    fn model() -> Model {
        build_with(SPEC, OVERRIDES).unwrap()
    }

    #[test]
    fn builds_requests_and_responses() {
        let model = model();
        let request = &model.structs[1];
        assert_eq!(request.name, "ThingRequest");
        assert_eq!(request.role, Role::Request);
        let name = &request.fields[0];
        assert_eq!(name.name, "name");
        assert!(name.required);
        assert_eq!(
            name.facets,
            Facets {
                max_length: Some(10),
                rule: Some("no_angle_bracket".into()),
                ..Facets::default()
            }
        );
        let token = &request.fields[1];
        assert!(!token.required && token.sensitive);
        assert_eq!(request.fields[2].ty, Ty::Named("Amount".into()));
        assert_eq!(request.fields[3].ty, Ty::Named("Kind".into()));

        let response = &model.structs[0];
        assert_eq!(response.role, Role::Response);
        assert_eq!(response.fields[1].doc.as_deref(), Some("The notes & such."));
        assert_eq!(model.enums[0].variants[1].name, "One");
    }

    #[test]
    fn builds_operations() {
        let model = model();
        let op = &model.operations[0];
        assert_eq!(op.path, "/things/{thing_id}");
        assert_eq!(op.path_params[0].ty, Ty::Named("TransactionId".into()));
        assert_eq!(op.query[0].field, "start_date");
        assert_eq!(op.query[0].wire, "query.start_date");
        assert_eq!(op.query.len(), 1);
        assert!(op.export);
        assert_eq!(op.merchant, Merchant::Optional);
        assert_eq!(op.body, Body::Json(Ty::Named("ThingRequest".into())));
        assert_eq!(op.response, Some(Ty::Named("Thing".into())));
    }

    fn error(spec: &str, overrides: &str) -> String {
        build_with(spec, overrides).unwrap_err()
    }

    #[test]
    fn stale_override_fails() {
        let err = error(SPEC, &format!("{OVERRIDES}\n[rename]\nNope = \"Nah\""));
        assert!(err.contains("rename.Nope"), "{err}");
    }

    #[test]
    fn unmapped_operation_fails() {
        let overrides = OVERRIDES.replace("Things_Create = ", "Other = ");
        let err = error(SPEC, &overrides);
        assert_eq!(err, "operation Things_Create has no entry in [operations]");
    }

    #[test]
    fn unnamed_enum_fails() {
        let overrides = OVERRIDES.replace(r#", "Thing.kind""#, "");
        assert_eq!(
            error(SPEC, &overrides),
            "Thing.kind is an enum; name it in [enums]"
        );
    }

    #[test]
    fn conflicting_enum_values_fail() {
        let spec = SPEC.replacen(r#"["0", "1"]"#, r#"["0", "2"]"#, 1);
        let err = error(&spec, OVERRIDES);
        assert!(err.starts_with("enum Kind: "), "{err}");
    }

    #[test]
    fn variant_names_must_be_identifiers() {
        let overrides = OVERRIDES.replace(r#""1" = "One""#, "");
        let err = error(SPEC, &overrides);
        assert!(err.contains(r#"value "1" needs a variant name"#), "{err}");
    }

    #[test]
    fn duplicate_client_method_fails() {
        let spec = SPEC.replace(
            r#""responses": {"200""#,
            r#""responses": {"200": {}}}, "put": {"operationId": "Things_Update", "parameters": [{"name": "version", "in": "path", "required": true, "type": "string"}, {"name": "thing_id", "in": "path", "required": true, "type": "integer", "format": "int32"}], "responses": {"200""#,
        );
        let overrides = format!(
            "{OVERRIDES}\nThings_Update = {{ resource = \"things\", method = \"create\" }}"
        );
        let err = error(&spec, &overrides);
        assert!(err.ends_with("are both things.create"), "{err}");
    }

    #[test]
    fn skipped_definition_must_be_unused() {
        let overrides = OVERRIDES.replace(r#"["Leaked"]"#, r#"["Leaked", "Thing"]"#);
        assert_eq!(
            error(SPEC, &overrides),
            "skipped definition Thing is used by an operation"
        );
        let overrides = OVERRIDES.replace(r#"["Leaked"]"#, "[]");
        assert!(error(SPEC, &overrides).starts_with("definition Leaked is not used"));
    }

    #[test]
    fn unknown_pattern_fails() {
        let overrides = OVERRIDES.replace(r#""^(?!.*<).*$" = "no_angle_bracket""#, "");
        assert!(error(SPEC, &overrides).contains("no rule for pattern"));
    }

    #[test]
    fn cleans_docs() {
        assert_eq!(
            clean_doc(Some("  Gets or sets the a &lt;b&gt;\r\n  next  ")).as_deref(),
            Some("The a <b>\nnext")
        );
        assert_eq!(clean_doc(Some("  ")), None);
        assert_eq!(
            clean_doc(Some(
                "Maps the {RepayGatewayApi.Models.Models.Foo} to {id}."
            ))
            .as_deref(),
            Some("Maps the `Foo` to {id}.")
        );
        assert_eq!(
            clean_doc(Some("Returns:<list type=\"bullet\"><item><description>one</description></item><item><description><b>two</b></description></item></list>Done.")).as_deref(),
            Some("Returns:\n\n- one\n- two\n\nDone.")
        );
    }

    #[test]
    fn field_names_drop_model_binding_prefixes() {
        assert_eq!(field_name("query.start_date"), "start_date");
        assert_eq!(
            field_name("request.wallet_merchant_id"),
            "wallet_merchant_id"
        );
        assert_eq!(field_name("skip"), "skip");
    }
}
