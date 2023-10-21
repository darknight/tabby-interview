mod model;
mod error;

use walkdir::{DirEntry, WalkDir};
use log::{debug, error, warn};

pub use model::{EntryType, FileEntry, FileMeta, FileChunk, WsRequest, WsResponse};
pub use error::{AppError, Result};

/// Walk directory and return file meta infos
pub fn walk_dir(base_dir: String, contain_symlink: bool) -> Vec<(FileMeta, DirEntry)> {
    let mut meta_infos = Vec::new();

    // the first item yielded by `WalkDir` is the root directory itself, so we skip it
    for dir_entry in WalkDir::new(base_dir.as_str()).into_iter().skip(1) {
        if let Err(err) = dir_entry {
            error!("[Sender] walk dir error: {}", err);
            continue;
        }
        let dir_entry = dir_entry.unwrap();

        let entry_type = if dir_entry.file_type().is_dir() { EntryType::Dir } else if dir_entry.file_type().is_file() { EntryType::File } else { EntryType::SymLink };

        // since the entry is from `from_dir`, we can safely unwrap here
        let rel_path = dir_entry.path().strip_prefix(base_dir.as_str()).unwrap().to_str();
        if rel_path.is_none() {
            warn!("[Sender] invalid rel path: {:?}", rel_path);
            continue;
        }
        let rel_path = rel_path.unwrap().to_string();
        let file_meta = FileMeta::new(rel_path, entry_type.clone());
        debug!("file meta: {:?}", file_meta);
        if !contain_symlink && entry_type == EntryType::SymLink {
            continue;
        }
        meta_infos.push((file_meta, dir_entry));
    }
    meta_infos
}
