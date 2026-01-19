/// Enum representing different log levels
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

/// Trait for logging messages at various log levels
/// Implementers can define how logging is handled (e.g., console, file, etc.)
pub trait Logger {
    fn log(&self, level: LogLevel, message: &str);

    fn debug(&self, message: &str) {
        self.log(LogLevel::Debug, message);
    }
    fn info(&self, message: &str) {
        self.log(LogLevel::Info, message);
    }
    fn warn(&self, message: &str) {
        self.log(LogLevel::Warn, message);
    }
    fn error(&self, message: &str) {
        self.log(LogLevel::Error, message);
    }
}

/// Trait for reporting progress of long-running tasks or operations in the workflow
pub trait ProgressReporter {
    fn set_progress(&self, progress: f32);
    fn set_message(&self, message: &str);
}
