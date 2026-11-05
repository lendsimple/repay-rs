//! Responses the schema does not describe.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// A JSON value whose shape the schema leaves open.
///
/// About half of the API's operations declare their response only as "an object".
/// They return a `Json`, which you can read as it is or convert to a type of your own.
///
/// ```
/// use repay::types::Json;
///
/// #[derive(serde::Deserialize)]
/// struct Token {
///     card_token: String,
/// }
///
/// let json = Json::from(serde_json::json!({ "card_token": "ABC123", "last4": "1111" }));
/// assert_eq!(json.get("last4").and_then(|v| v.as_str()), Some("1111"));
/// let token: Token = json.deserialize().unwrap();
/// assert_eq!(token.card_token, "ABC123");
/// ```
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Json(serde_json::Value);

impl Json {
    /// The value of a field of an object, if the value is an object that has it.
    pub fn get(&self, key: &str) -> Option<&serde_json::Value> {
        self.0.get(key)
    }

    /// The value.
    pub fn as_value(&self) -> &serde_json::Value {
        &self.0
    }

    /// The value, by value.
    pub fn into_inner(self) -> serde_json::Value {
        self.0
    }

    /// Converts the value to `T`.
    pub fn deserialize<T: DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        T::deserialize(&self.0)
    }
}

impl From<serde_json::Value> for Json {
    fn from(value: serde_json::Value) -> Self {
        Self(value)
    }
}

impl From<Json> for serde_json::Value {
    fn from(value: Json) -> Self {
        value.0
    }
}

crate::validate::no_constraints!(Json);
