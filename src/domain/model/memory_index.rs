#[derive(Debug, Clone, PartialEq)]
pub struct MemoryIndexChunk {
    pub path: String,
    pub chunk_index: i32,
    pub content: String,
    pub embedding: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemoryIndexSearchResult {
    pub path: String,
    pub chunk_index: i32,
    pub content: String,
    pub distance: f64,
}
