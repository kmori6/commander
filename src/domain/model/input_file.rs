use crate::domain::util::data_uri::encode_data_uri;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputFile {
    pub filename: String,
    pub file_data: String,
}

impl InputFile {
    pub fn from_data(filename: String, mime_type: &str, data: &[u8]) -> Self {
        Self {
            filename,
            file_data: encode_data_uri(mime_type, data),
        }
    }
}
