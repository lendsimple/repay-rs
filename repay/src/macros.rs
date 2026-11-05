//! Declarative macros for the schema's string enums and ID newtypes. The generated
//! schema code invokes them.

/// An enum of the string values the schema lists, plus `Other` for values it does
/// not, so that a new value from the API never fails a response.
///
/// ```ignore
/// string_enum! {
///     /// The result of a name check.
///     pub enum NameMatch {
///         /// The names match.
///         Match = "0",
///         PartialMatch = "1",
///         NoMatch = "2",
///     }
/// }
/// ```
// Only the tests use it until the generated schema does.
#[allow(unused_macros)]
macro_rules! string_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident {
            $(
                $(#[$variant_meta:meta])*
                $variant:ident = $value:literal
            ),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        ///
        /// A value the schema does not list is kept in `Other`. Match unknown values
        /// with a guard on [`as_str`](Self::as_str) rather than on `Other`: a later
        /// release may give the value its own variant.
        #[non_exhaustive]
        #[derive(Clone, Debug, PartialEq, Eq, Hash)]
        pub enum $name {
            $(
                $(#[$variant_meta])*
                #[doc = concat!("`\"", $value, "\"`")]
                $variant,
            )+
            /// A value the schema does not list.
            Other(::std::string::String),
        }

        impl $name {
            /// The value as sent over the wire.
            pub fn as_str(&self) -> &str {
                match self {
                    $(Self::$variant => $value,)+
                    Self::Other(value) => value,
                }
            }
        }

        impl ::core::fmt::Display for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl ::core::convert::From<&str> for $name {
            fn from(value: &str) -> Self {
                match value {
                    $($value => Self::$variant,)+
                    other => Self::Other(other.into()),
                }
            }
        }

        impl ::core::convert::From<::std::string::String> for $name {
            fn from(value: ::std::string::String) -> Self {
                match Self::from(value.as_str()) {
                    Self::Other(_) => Self::Other(value),
                    known => known,
                }
            }
        }

        impl ::serde::Serialize for $name {
            fn serialize<S: ::serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D: ::serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                <::std::string::String as ::serde::Deserialize>::deserialize(deserializer).map(Self::from)
            }
        }

        $crate::validate::no_constraints!($name);
    };
}

/// A newtype for a numeric ID. It is sent as a number and read from a number or a
/// string of digits, since the API is not consistent about which it sends.
macro_rules! int_id {
    ($($(#[$meta:meta])* $name:ident;)+) => {$(
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(i32);

        impl $name {
            /// Wraps an ID.
            pub const fn new(id: i32) -> Self {
                Self(id)
            }

            /// The ID's value.
            pub const fn get(self) -> i32 {
                self.0
            }
        }

        impl ::core::convert::From<i32> for $name {
            fn from(id: i32) -> Self {
                Self(id)
            }
        }

        impl ::core::convert::From<$name> for i32 {
            fn from(id: $name) -> Self {
                id.0
            }
        }

        impl ::core::fmt::Display for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                ::core::fmt::Display::fmt(&self.0, f)
            }
        }

        impl ::core::str::FromStr for $name {
            type Err = ::core::num::ParseIntError;

            fn from_str(text: &str) -> Result<Self, Self::Err> {
                text.parse().map(Self)
            }
        }

        impl ::serde::Serialize for $name {
            fn serialize<S: ::serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_i32(self.0)
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D: ::serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                $crate::types::ids::deserialize_i32(deserializer).map(Self)
            }
        }

        $crate::validate::no_constraints!($name);
    )+};
}

/// A newtype for an ID that is text.
macro_rules! string_id {
    ($($(#[$meta:meta])* $name:ident;)+) => {$(
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, ::serde::Serialize, ::serde::Deserialize)]
        #[serde(transparent)]
        pub struct $name(::std::string::String);

        impl $name {
            /// Wraps an ID.
            pub fn new(id: impl ::core::convert::Into<::std::string::String>) -> Self {
                Self(id.into())
            }

            /// The ID's value.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl ::core::convert::From<&str> for $name {
            fn from(id: &str) -> Self {
                Self(id.into())
            }
        }

        impl ::core::convert::From<::std::string::String> for $name {
            fn from(id: ::std::string::String) -> Self {
                Self(id)
            }
        }

        impl ::core::convert::AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl ::core::fmt::Display for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                f.write_str(&self.0)
            }
        }

        $crate::validate::no_constraints!($name);
    )+};
}

#[allow(unused_imports)]
pub(crate) use string_enum;
pub(crate) use {int_id, string_id};

#[cfg(test)]
mod tests {
    string_enum! {
        /// The result of a name check.
        pub enum NameMatch {
            Match = "0",
            PartialMatch = "1",
            NoMatch = "2",
        }
    }

    #[test]
    fn known_values_read_as_their_variant() {
        let value: NameMatch = serde_json::from_str("\"1\"").unwrap();
        assert_eq!(value, NameMatch::PartialMatch);
        assert_eq!(serde_json::to_string(&value).unwrap(), "\"1\"");
    }

    #[test]
    fn unknown_values_read_as_other_and_round_trip() {
        let value: NameMatch = serde_json::from_str("\"9\"").unwrap();
        assert_eq!(value, NameMatch::Other("9".into()));
        assert_eq!(value.as_str(), "9");
        assert_eq!(serde_json::to_string(&value).unwrap(), "\"9\"");
    }

    #[test]
    fn from_string_prefers_a_listed_variant() {
        assert_eq!(NameMatch::from(String::from("2")), NameMatch::NoMatch);
        assert_eq!(NameMatch::NoMatch.to_string(), "2");
    }
}
