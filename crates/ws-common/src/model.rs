use std::cmp::Ordering;
use std::fmt::Debug;
use serde::{Deserialize, Serialize};
use tokio_tungstenite::tungstenite;
use tokio_tungstenite::tungstenite::Message;
use crate::Result;

/// File entry type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryType {
    File,
    Dir,
    SymLink,
}

/// File meta info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMeta {
    rel_path: String,
    entry_type: EntryType,
    file_size: u64,
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
    /// Create file meta
    pub fn new(rel_path: String, entry_type: EntryType, file_size: u64) -> Self {
        Self { rel_path, entry_type, file_size }
    }

    /// Get relative path info
    pub fn rel_path(&self) -> &str {
        &self.rel_path
    }

    /// Return true if current file meta is a file
    pub fn is_file(&self) -> bool {
        self.entry_type == EntryType::File
    }

    /// Return true if current file meta is a directory
    pub fn is_dir(&self) -> bool {
        self.entry_type == EntryType::Dir
    }

    /// Return true if current file meta is a symbolic link
    pub fn is_sym_link(&self) -> bool {
        self.entry_type == EntryType::SymLink
    }

    /// Get file size info
    pub fn file_size(&self) -> u64 {
        self.file_size
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
    /// Create file chunk
    pub fn new(offset: u64, payload: Vec<u8>) -> Self {
        Self { offset, payload }
    }

    /// Get file offset info
    pub fn offset(&self) -> u64 {
        self.offset
    }

    /// Get file content
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}

/// File entry is the basic unit of communication between sender and receiver
#[derive(Clone, Serialize, Deserialize)]
pub struct FileEntry {
    file_meta: FileMeta,
    file_chunk: Option<FileChunk>,
}

impl Debug for FileEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileEntry")
            .field("file_meta", &self.file_meta)
            .field("file_chunk_offset", &self.file_chunk
                .as_ref().map_or(0, |chunk| chunk.offset))
            .field("file_chunk_size", &self.file_chunk
                .as_ref().map_or(0, |chunk| chunk.payload.len()))
            .finish()
    }
}

impl FileEntry {
    /// Create file entry
    pub fn new(file_meta: FileMeta, file_chunk: Option<FileChunk>) -> Self {
        Self { file_meta, file_chunk }
    }

    /// Get file offset info
    pub fn file_offset(&self) -> Option<u64> {
        self.file_chunk.as_ref().map(|c| c.offset)
    }

    /// Get file content
    pub fn file_content(&self) -> Option<&[u8]> {
        self.file_chunk.as_ref().map(|c| c.payload.as_slice())
    }

    /// Get file relative path info
    pub fn rel_path(&self) -> &str {
        &self.file_meta.rel_path
    }

    /// Return true if current file entry is a file
    pub fn is_file(&self) -> bool {
        self.file_meta.is_file()
    }

    /// Return true if current file entry is a directory
    pub fn is_dir(&self) -> bool {
        self.file_meta.is_dir()
    }

    /// Return true if current file entry is a symbolic link
    pub fn is_sym_link(&self) -> bool {
        self.file_meta.is_sym_link()
    }

    /// Get file meta info
    pub fn file_meta(&self) -> FileMeta {
        self.file_meta.clone()
    }
}

/// Communication protocol between sender and receiver
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WsRequest {
    /// Create file request
    CreateFile(FileMeta),
    /// Write file request, by saying file here it refers to either file or directory
    WriteFile(FileEntry),
    /// File meta list of sender's local dir
    ///
    /// Receiver can use this list as filter to decide which files and directories need
    /// to be removed.
    ClearDir(Vec<FileMeta>),
}

impl WsRequest {
    /// Create CreateFile message
    pub fn new_create_file_message(file_meta: FileMeta) -> Result<tungstenite::Message> {
        let ws_req = WsRequest::CreateFile(file_meta);
        let message = Message::Text(serde_json::to_string(&ws_req)?);
        Ok(message)
    }

    /// Create WriteFile message
    pub fn new_write_file_message(file_entry: FileEntry) -> Result<tungstenite::Message> {
        let ws_req = WsRequest::WriteFile(file_entry);
        let message = Message::Text(serde_json::to_string(&ws_req)?);
        Ok(message)
    }

    /// Create ClearDir message
    pub fn new_clear_dir_message(file_metas: Vec<FileMeta>) -> Result<tungstenite::Message> {
        let ws_req = WsRequest::ClearDir(file_metas);
        let message = Message::Text(serde_json::to_string(&ws_req)?);
        Ok(message)
    }
}

/// Communication protocol between sender and receiver
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WsResponse {
    /// Create file success response
    CreateSuccess(FileMeta),
    /// Create file failure response
    CreateFailed(FileMeta),
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
    ClearDirDone(Vec<FileMeta>),
}

impl WsResponse {
    /// Create CreateSuccess message
    pub fn new_create_success_message(file_meta: FileMeta) -> Result<tungstenite::Message> {
        let ws_resp = WsResponse::CreateSuccess(file_meta);
        let message = Message::Text(serde_json::to_string(&ws_resp)?);
        Ok(message)
    }

    /// Create CreateFailed message
    pub fn new_create_failed_message(file_meta: FileMeta) -> Result<tungstenite::Message> {
        let ws_resp = WsResponse::CreateFailed(file_meta);
        let message = Message::Text(serde_json::to_string(&ws_resp)?);
        Ok(message)
    }

    /// Create WriteSuccess message
    pub fn new_write_success_message(file_meta: FileMeta) -> Result<tungstenite::Message> {
        let ws_resp = WsResponse::WriteSuccess(file_meta);
        let message = Message::Text(serde_json::to_string(&ws_resp)?);
        Ok(message)
    }

    /// Create WriteFailed message
    pub fn new_write_failed_message(file_meta: FileMeta) -> Result<tungstenite::Message> {
        let ws_resp = WsResponse::WriteFailed(file_meta);
        let message = Message::Text(serde_json::to_string(&ws_resp)?);
        Ok(message)
    }

    /// Create ClearDirDone message
    pub fn new_clear_dir_done_message(file_metas: Vec<FileMeta>) -> Result<tungstenite::Message> {
        let ws_resp = WsResponse::ClearDirDone(file_metas);
        let message = Message::Text(serde_json::to_string(&ws_resp)?);
        Ok(message)
    }

    /// Create ReceiverBusy message
    pub fn new_receiver_busy_message() -> Result<Message> {
        let message = Message::Close(Some(tungstenite::protocol::CloseFrame {
            code: tungstenite::protocol::frame::coding::CloseCode::Abnormal,
            reason: std::borrow::Cow::Borrowed("receiver is busy"),
        }));
        Ok(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_meta() {
        let file_meta = FileMeta::new("test.txt".to_string(), EntryType::File, 1024);
        assert_eq!(file_meta.rel_path(), "test.txt");
        assert_eq!(file_meta.is_file(), true);
        assert_eq!(file_meta.is_dir(), false);
        assert_eq!(file_meta.is_sym_link(), false);
        assert_eq!(file_meta.file_size(), 1024);
    }

    #[test]
    fn test_file_chunk() {
        let file_chunk = FileChunk::new(0, vec![1, 2, 3]);
        assert_eq!(file_chunk.offset(), 0);
        assert_eq!(file_chunk.payload(), &[1, 2, 3]);
    }

    #[test]
    fn test_file_entry() {
        let payload = vec![1, 2, 3u8];
        let file_meta = FileMeta::new("test.txt".to_string(), EntryType::File, 1024);
        let file_chunk = FileChunk::new(0, payload.clone());
        let file_entry = FileEntry::new(file_meta.clone(), Some(file_chunk.clone()));
        assert_eq!(file_entry.file_offset(), Some(0));
        assert_eq!(file_entry.file_content(), Some(payload.as_slice()));
        assert_eq!(file_entry.rel_path(), "test.txt");
        assert_eq!(file_entry.is_file(), true);
        assert_eq!(file_entry.is_dir(), false);
        assert_eq!(file_entry.is_sym_link(), false);
        assert_eq!(file_entry.file_meta(), file_meta);
    }

    #[test]
    fn test_ws_request() {
        let file_meta = FileMeta::new("test.txt".to_string(), EntryType::File, 1024);
        let file_chunk = FileChunk::new(0, vec![1, 2, 3]);
        let file_entry = FileEntry::new(file_meta.clone(), Some(file_chunk.clone()));
        let ws_req = WsRequest::CreateFile(file_meta.clone());
        let message = WsRequest::new_create_file_message(file_meta.clone()).unwrap();
        assert_eq!(message, Message::Text(serde_json::to_string(&ws_req).unwrap()));
        let ws_req = WsRequest::WriteFile(file_entry.clone());
        let message = WsRequest::new_write_file_message(file_entry.clone()).unwrap();
        assert_eq!(message, Message::Text(serde_json::to_string(&ws_req).unwrap()));
    }

    #[test]
    fn test_ws_response() {
        let file_meta = FileMeta::new("test.txt".to_string(), EntryType::File, 1024);
        let file_chunk = FileChunk::new(0, vec![1, 2, 3]);
        let file_entry = FileEntry::new(file_meta.clone(), Some(file_chunk.clone()));
        let ws_resp = WsResponse::CreateSuccess(file_meta.clone());
        let message = WsResponse::new_create_success_message(file_meta.clone()).unwrap();
        assert_eq!(message, Message::Text(serde_json::to_string(&ws_resp).unwrap()));
        let ws_resp = WsResponse::CreateFailed(file_meta.clone());
        let message = WsResponse::new_create_failed_message(file_meta.clone()).unwrap();
        assert_eq!(message, Message::Text(serde_json::to_string(&ws_resp).unwrap()));
        let ws_resp = WsResponse::WriteSuccess(file_meta.clone());
        let message = WsResponse::new_write_success_message(file_meta.clone()).unwrap();
        assert_eq!(message, Message::Text(serde_json::to_string(&ws_resp).unwrap()));
        let ws_resp = WsResponse::WriteFailed(file_meta.clone());
        let message = WsResponse::new_write_failed_message(file_meta.clone()).unwrap();
        assert_eq!(message, Message::Text(serde_json::to_string(&ws_resp).unwrap()));
        let ws_resp = WsResponse::ClearDirDone(vec![file_meta.clone()]);
        let message = WsResponse::new_clear_dir_done_message(vec![file_meta.clone()]).unwrap();
        assert_eq!(message, Message::Text(serde_json::to_string(&ws_resp).unwrap()));
    }
}
