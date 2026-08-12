use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Datenbankfehler: {0}")]
    Db(#[from] sqlx::Error),
    #[error("Dateisystemfehler: {0}")]
    Io(#[from] std::io::Error),
    #[error("Druckfehler: {0}")]
    Print(String),
    #[error("{0}")]
    Other(String),
}

// Tauri commands must return Result<T, E: Serialize>; flatten to a plain string
// so no error taxonomy leaks into TypeScript.
impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
