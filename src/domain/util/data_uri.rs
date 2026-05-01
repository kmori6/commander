use base64::{Engine as _, engine::general_purpose::STANDARD};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DataUriError {
    #[error("data URI must start with `data:`")]
    MissingPrefix,

    #[error("data URI must contain a comma separator")]
    MissingComma,

    #[error("data URI must include a MIME type")]
    MissingMimeType,

    #[error("only base64 data URIs are supported")]
    UnsupportedEncoding,

    #[error("failed to decode base64 data: {0}")]
    InvalidBase64(#[from] base64::DecodeError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedDataUri {
    pub mime_type: String,
    pub data: Vec<u8>,
}

pub fn encode_data_uri(mime_type: &str, data: &[u8]) -> String {
    let encoded = STANDARD.encode(data);
    format!("data:{mime_type};base64,{encoded}")
}

pub fn decode_data_uri(value: &str) -> Result<DecodedDataUri, DataUriError> {
    // data:{mime_type};base64,{base64}
    let value = value
        .strip_prefix("data:")
        .ok_or(DataUriError::MissingPrefix)?;

    let (metadata, encoded_data) = value.split_once(',').ok_or(DataUriError::MissingComma)?;

    let mut metadata_parts = metadata.split(';');
    let mime_type = metadata_parts
        .next()
        .filter(|mime_type| !mime_type.is_empty())
        .ok_or(DataUriError::MissingMimeType)?;

    let is_base64 = metadata_parts.any(|part| part == "base64");
    if !is_base64 {
        return Err(DataUriError::UnsupportedEncoding);
    }

    let data = STANDARD.decode(encoded_data)?;

    Ok(DecodedDataUri {
        mime_type: mime_type.to_string(),
        data,
    })
}

pub fn is_data_uri(value: &str) -> bool {
    value.starts_with("data:")
}
