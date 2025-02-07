use std::fmt::Display;
use wasm_bindgen::JsError;

/// Shortcut for Result<T, JsError>.
pub type JsResult<T> = Result<T, JsError>;

/// NadaValue error.
// We create this error separately because it needs to contain a message, whereas other error types don't.
pub(crate) struct ValueError {
    message: String,
}

impl ValueError {
    pub(crate) fn new_err(message: &str) -> JsError {
        let message = &format!("ValueError: {message}");
        JsError::new(message)
    }
}

impl Display for ValueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
