use std::path::PathBuf;

use crate::task::DataType;

/// Represents a value used in task inputs and outputs
#[derive(Debug, Clone, PartialEq)]
pub enum Value<'a> {
    String(&'a str),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Json(serde_json::Value),
    FilePath(PathBuf),
    Null,
}

impl<'a> Value<'a> {
    /// Gets the DataType of the Value
    pub fn get_type(&self) -> DataType {
        match self {
            Value::String(_) => DataType::String,
            Value::Integer(_) => DataType::Integer,
            Value::Float(_) => DataType::Float,
            Value::Boolean(_) => DataType::Boolean,
            Value::Json(_) => DataType::Json,
            Value::FilePath(_) => DataType::File,
            Value::Null => DataType::Null,
        }
    }
}
