use crate::domain::util::data_uri::encode_data_uri;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputImage {
    pub image_url: String,
}

impl InputImage {
    pub fn from_data(mime_type: &str, data: &[u8]) -> Self {
        Self {
            image_url: encode_data_uri(mime_type, data),
        }
    }
}
