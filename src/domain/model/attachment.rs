#[derive(Debug, Clone)]
pub struct Attachment {
    pub filename: String,
    pub mime_type: String,
    pub data: Vec<u8>,
}

impl Attachment {
    pub fn new(filename: impl Into<String>, mime_type: impl Into<String>, data: Vec<u8>) -> Self {
        Self {
            filename: filename.into(),
            mime_type: mime_type.into(),
            data,
        }
    }

    /// Returns true if this attachment is an image (image/*)
    pub fn is_image(&self) -> bool {
        self.mime_type.starts_with("image/")
    }

    /// Returns true if this attachment is a document (PDF, Word, etc.)
    pub fn is_document(&self) -> bool {
        !self.is_image()
    }
}
