use std::path::Path;
use log::error;
use tokio::fs;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use ws_common::{AppError, FileEntry, FileMeta, Result};

/// Remote all files and directories in `output_dir`
pub async fn clear_dir(output_dir: String) -> Result<()> {
    let mut entries = fs::read_dir(&output_dir).await.map_err(AppError::FailedReadDir)?;
    while let Some(entry) = entries.next_entry().await.map_err(AppError::DirEntryError)? {
        let path = entry.path();
        // TODO: skip PID file
        if path.is_dir() {
            if let Err(err) = fs::remove_dir_all(path.as_path()).await {
                error!("failed to remove dir: {}, error: {}", path.display(), err);
            }
        } else {
            if let Err(err) = fs::remove_file(path.as_path()).await {
                error!("failed to remove file: {}, error: {}", path.display(), err);
            }
        }
    }
    Ok(())
}

/// Create file or directory, if it's a file, create with size info from `file_meta`
pub async fn create_file(output_dir: String, file_meta: FileMeta) -> Result<()> {
    let target_path = Path::new(&output_dir).join(&file_meta.rel_path());
    if file_meta.is_dir() {
        if !target_path.exists() {
            fs::create_dir_all(target_path).await.map_err(AppError::FailedCreateDir)?;
        }
        return Ok(());
    }
    // file_meta is file, create file with size
    if !target_path.exists() {
        let parent_dir = target_path.parent().unwrap();
        if !parent_dir.exists() {
            fs::create_dir_all(parent_dir).await.map_err(AppError::FailedCreateDir)?;
        }
    }
    // create & truncate file
    fs::File::create(&target_path).await.map_err(AppError::FailedCreateFile)?;

    Ok(())
}

/// Write file content to local
pub async fn write_file(output_dir: String, file_entry: FileEntry) -> Result<()> {
    if !file_entry.is_file() {
        return Ok(());
    }
    if file_entry.file_content().is_none() {
        error!("[Receiver] file content is empty, shouldn't happen: {:?}", file_entry);
        return Err(AppError::EmptyPayload);
    }

    let target_path = Path::new(&output_dir).join(&file_entry.rel_path());
    if !target_path.exists() {
        error!("[Receiver] file not exists, shouldn't happen: {:?}", target_path);
        return Err(AppError::FileNotExist(target_path.to_str().unwrap_or("").to_string()));
    }
    if !target_path.is_file() {
        error!("[Receiver] target path is not file, shouldn't happen: {:?}", target_path);
        return Err(AppError::FileNotExist(target_path.to_str().unwrap_or("").to_string()));
    }

    let mut file = OpenOptions::new()
        .append(true)
        .open(&target_path).await.map_err(AppError::FailedOpenFile)?;
    file.write(file_entry.file_content().unwrap_or(&[])).await.map_err(AppError::FailedWriteFile)?;

    Ok(())
}

