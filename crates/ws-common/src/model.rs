use std::cmp::Ordering;
use serde::{Deserialize, Serialize};

/// File entry type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryType {
    File,
    Dir,
    SymLink,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMeta {
    rel_path: String,
    entry_type: EntryType,
}

impl Eq for FileMeta {}

impl PartialEq<Self> for FileMeta {
    fn eq(&self, other: &Self) -> bool {
        self.rel_path == other.rel_path
    }
}

impl PartialOrd<Self> for FileMeta {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.rel_path.partial_cmp(&other.rel_path)
    }
}

impl Ord for FileMeta {
    fn cmp(&self, other: &Self) -> Ordering {
        self.rel_path.cmp(&other.rel_path)
    }
}

impl FileMeta {
    pub fn new(rel_path: String, entry_type: EntryType) -> Self {
        Self { rel_path, entry_type }
    }

    pub fn rel_path(&self) -> &str {
        &self.rel_path
    }

    pub fn entry_type(&self) -> &EntryType {
        &self.entry_type
    }
}

/// File chunk
///
/// Currently, we don't support sending large file in chunks, so `offset` is reserved for future use.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileChunk {
    offset: u64,
    payload: Vec<u8>,
}

impl FileChunk {
    pub fn new(offset: u64, payload: Vec<u8>) -> Self {
        Self { offset, payload }
    }

    pub fn offset(&self) -> u64 {
        self.offset
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    file_meta: FileMeta,
    file_chunk: Option<FileChunk>,
}

impl FileEntry {
    pub fn new(file_meta: FileMeta, file_chunk: Option<FileChunk>) -> Self {
        Self { file_meta, file_chunk }
    }

    pub fn file_meta(&self) -> &FileMeta {
        &self.file_meta
    }

    pub fn file_chunk(&self) -> Option<&FileChunk> {
        self.file_chunk.as_ref()
    }
}
