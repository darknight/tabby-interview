use std::cmp::Ordering;
use std::fmt::Debug;
use serde::{Deserialize, Serialize};
use tokio_tungstenite::tungstenite;
use crate::Result;

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

#[derive(Clone, Serialize, Deserialize)]
pub struct FileEntry {
    file_meta: FileMeta,
    file_chunk: Option<FileChunk>,
}

impl Debug for FileEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileEntry")
            .field("file_meta", &self.file_meta)
            .field("file_chunk_size", &self.file_chunk
                .as_ref().map_or(0, |chunk| chunk.payload.len()))
            .finish()
    }
}

impl FileEntry {
    pub fn new(file_meta: FileMeta, file_chunk: Option<FileChunk>) -> Self {
        Self { file_meta, file_chunk }
    }

    pub fn file_offset(&self) -> Option<u64> {
        self.file_chunk.as_ref().map(|c| c.offset)
    }

    pub fn file_content(&self) -> Option<&[u8]> {
        self.file_chunk.as_ref().map(|c| c.payload.as_slice())
    }

    pub fn rel_path(&self) -> &str {
        &self.file_meta.rel_path
    }

    pub fn is_file(&self) -> bool {
        self.file_meta.entry_type == EntryType::File
    }

    pub fn is_dir(&self) -> bool {
        self.file_meta.entry_type == EntryType::Dir
    }

    pub fn file_meta(&self) -> FileMeta {
        self.file_meta.clone()
    }
}

/// Communication protocol between sender and receiver
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WsRequest {
    /// Write file request, by saying file here it refers to either file or directory
    WriteFile(FileEntry),
    /// File meta list of sender's local dir
    ///
    /// Receiver can use this list as filter to decide which files and directories need
    /// to be removed.
    ClearDir(Vec<FileMeta>),
}

impl WsRequest {
    /// Create WriteFile message
    pub fn new_write_file_message(file_entry: FileEntry) -> Result<tungstenite::Message> {
        let ws_req = WsRequest::WriteFile(file_entry);
        let message = tungstenite::Message::Text(serde_json::to_string(&ws_req)?);
        Ok(message)
    }

    /// Create ClearDir message
    pub fn new_clear_dir_message(file_metas: Vec<FileMeta>) -> Result<tungstenite::Message> {
        let ws_req = WsRequest::ClearDir(file_metas);
        let message = tungstenite::Message::Text(serde_json::to_string(&ws_req)?);
        Ok(message)
    }
}

/// Communication protocol between sender and receiver
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WsResponse {
    /// Write file success response
    WriteSuccess(FileMeta),
    /// Write file failure response
    WriteFailed(FileMeta),
    /// This is the response to notify sender that receiver is ready to accept files
    /// The file meta list filed contains files which are kept on receiver side
    /// It may happen due to many reasons, such as
    /// 1. permission denied
    /// 2. file is being used by other process
    /// 3. file is the same as sender's
    ///
    /// This is a extension design, currently we don't use it
    /// It can be used as a filter to avoid sending unnecessary files, which can improve performance
    ClearDirDone(Vec<FileMeta>)
}

impl WsResponse {
    /// Create WriteSuccess message
    pub fn new_write_success_message(file_meta: FileMeta) -> Result<tungstenite::Message> {
        let ws_resp = WsResponse::WriteSuccess(file_meta);
        let message = tungstenite::Message::Text(serde_json::to_string(&ws_resp)?);
        Ok(message)
    }

    /// Create WriteFailed message
    pub fn new_write_failed_message(file_meta: FileMeta) -> Result<tungstenite::Message> {
        let ws_resp = WsResponse::WriteFailed(file_meta);
        let message = tungstenite::Message::Text(serde_json::to_string(&ws_resp)?);
        Ok(message)
    }

    /// Create ClearDirDone message
    pub fn new_clear_dir_done_message(file_metas: Vec<FileMeta>) -> Result<tungstenite::Message> {
        let ws_resp = WsResponse::ClearDirDone(file_metas);
        let message = tungstenite::Message::Text(serde_json::to_string(&ws_resp)?);
        Ok(message)
    }
}
