use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum AsciiAnimError {
    #[error("unknown preset: {name}")]
    UnknownPreset { name: String },

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

    #[error("terminal error: {0}")]
    Terminal(String),
}

pub type Result<T, E = AsciiAnimError> = std::result::Result<T, E>;
