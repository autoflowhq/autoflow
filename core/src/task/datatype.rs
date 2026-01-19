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
