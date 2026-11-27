//! Typed property schemas, values, and patch primitives.

use serde::{Deserialize, Serialize};

/// Supported property data types in database views.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PropertyType {
    Text,
    Number,
    Currency,
    Date,
    Select,
    MultiSelect,
    Checkbox,
    Relation,
}

/// Strongly typed property value extracted from YAML frontmatter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum PropertyValue {
    Text(String),
    Number(f64),
    Date(String),
    Bool(bool),
    List(Vec<String>),
    Null,
}

impl PropertyValue {
    /// Returns a human-readable string representation when applicable.
    pub fn as_text(&self) -> Option<String> {
        match self {
            Self::Text(s) | Self::Date(s) => Some(s.clone()),
            Self::Number(n) => Some(format_number(*n)),
            Self::Bool(b) => Some(b.to_string()),
            Self::List(items) => Some(items.join(", ")),
            Self::Null => None,
        }
    }

    /// Returns a numeric representation if the value is a number.
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Self::Number(n) => Some(*n),
            Self::Text(s) => s.trim().parse::<f64>().ok(),
            _ => None,
        }
    }

    /// Returns a boolean representation if the value is boolean.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

fn format_number(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        n.to_string()
    }
}

/// Single property mutation applied to a document's frontmatter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PropertyPatch {
    pub key: String,
    /// `Some(value)` sets or updates the key; `None` removes the key.
    pub value: Option<PropertyValue>,
}

/// Schema definition for a property column/field inside a view schema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropertySchema {
    pub label: String,
    #[serde(rename = "type")]
    pub prop_type: PropertyType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
}
