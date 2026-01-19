/// Represents the data types that can be used in task inputs and outputs.
/// This enum is used for type checking and validation within the workflow system.
/// It includes common primitive types as well as complex types like JSON and File.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DataType {
    String,
    Integer,
    Float,
    Boolean,
    Json,
    File,
}

impl std::fmt::Display for DataType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let type_str = match self {
            DataType::String => "string",
            DataType::Integer => "integer",
            DataType::Float => "float",
            DataType::Boolean => "boolean",
            DataType::Json => "json",
            DataType::File => "file",
        };
        write!(f, "{}", type_str)
    }
}

impl From<DataType> for String {
    fn from(data_type: DataType) -> Self {
        data_type.to_string()
    }
}
