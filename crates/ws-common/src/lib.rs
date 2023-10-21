mod model;
mod error;

use std::collections::BTreeMap;
use walkdir::{DirEntry, WalkDir};
use log::{debug, error};

pub use model::{EntryType, FileEntry, FileMeta, FileChunk, WsRequest, WsResponse};
pub use error::{AppError, Result};

/// Walk directory and return file meta infos
pub fn walk_dir(base_dir: String, contain_symlink: bool) -> BTreeMap<FileMeta, DirEntry> {
    let mut meta_infos = BTreeMap::new();

    // the first item yielded by `WalkDir` is the root directory itself, so we skip it
    for dir_entry in WalkDir::new(base_dir.as_str()).into_iter().skip(1) {
        if let Err(err) = dir_entry {
            error!("[Common] walk dir error: {}", err);
            continue;
        }
        let dir_entry = dir_entry.unwrap();

        let mut file_size = 0u64;
        let entry_type = if dir_entry.file_type().is_dir() { EntryType::Dir } else if dir_entry.file_type().is_file() { EntryType::File } else { EntryType::SymLink };
        if entry_type != EntryType::Dir {
            if let Ok(metadata) = dir_entry.metadata() {
                file_size = metadata.len();
            } else {
                error!("[Common] Failed to get metadata for file: {:?}", dir_entry.path());
                continue;
            }
        }

        // since the entry is from `from_dir`, we can safely unwrap here
        let rel_path = dir_entry.path().strip_prefix(base_dir.as_str()).unwrap().to_str();
        if rel_path.is_none() {
            error!("[Common] invalid rel path: {:?}", rel_path);
            continue;
        }
        let rel_path = rel_path.unwrap().to_string();

        let file_meta = FileMeta::new(rel_path, entry_type.clone(), file_size);
        debug!("[Common] file meta: {:?}", file_meta);
        if !contain_symlink && entry_type == EntryType::SymLink {
            continue;
        }
        meta_infos.insert(file_meta, dir_entry);
    }
    meta_infos
}
