#[derive(Debug, thiserror::Error)]
pub enum TexturePackError {
    #[error("network request to Modrinth failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("image decode error: {0}")]
    Image(#[from] image::ImageError),
    #[error("could not determine a cache directory for texture packs")]
    NoCacheDir,
    #[error("pack has no downloadable .zip file")]
    NoZipFile,
    #[error("downloaded file's sha1 ({actual}) does not match Modrinth's ({expected})")]
    HashMismatch { expected: String, actual: String },
    #[error("pack.mcmeta is missing or invalid: {0}")]
    InvalidPackMeta(String),
}

pub type Result<T> = std::result::Result<T, TexturePackError>;
