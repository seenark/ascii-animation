use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum AsciiAnimError {
    #[error("unknown preset: {name}")]
    UnknownPreset { name: String },

    #[error("unknown scene: {name}")]
    UnknownScene { name: String },

    #[error("unknown option `{option}` for preset `{preset}`")]
    UnknownOption { preset: String, option: String },

    #[error("invalid value for `{option}`: expected {expected}, got {actual}")]
    InvalidOptionType {
        option: String,
        expected: &'static str,
        actual: String,
    },

    #[error("option `{option}` is out of range: expected {min}..={max}, got {actual}")]
    OptionOutOfRange {
        option: String,
        min: String,
        max: String,
        actual: String,
    },

    #[error("invalid choice for `{option}`: expected one of {choices:?}, got `{actual}`")]
    InvalidChoice {
        option: String,
        choices: Vec<String>,
        actual: String,
    },

    #[error("option `{option}` is too long: expected at most {max} characters, got {actual}")]
    TextTooLong {
        option: String,
        max: usize,
        actual: usize,
    },

    #[error("cannot combine {input_source} with direct preset inputs: {conflicts}")]
    ConflictingRunInputs {
        input_source: &'static str,
        conflicts: String,
    },

    #[error("cannot combine {left} with {right}")]
    ConflictingSceneInputs {
        left: &'static str,
        right: &'static str,
    },
    #[error("scene must contain at least one animation instance")]
    EmptyScene,

    #[error("failed to parse scene config at {path}: {source}")]
    SceneConfigParse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("failed to write scene config at {path}: {source}")]
    SceneConfigWrite {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("clipboard error: {0}")]
    Clipboard(String),
    #[error("terminal error: {0}")]
    Terminal(String),
}

pub type Result<T> = std::result::Result<T, AsciiAnimError>;
