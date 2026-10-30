//! Documents: any file a person keeps with their records (letters, scans,
//! photos, recordings). The file is encrypted into the vault folder as it is
//! read; the record is a FHIR DocumentReference that points at it.

use std::io::Read;
use std::path::PathBuf;

use serde_json::Value;
use swasthya_core::files;
use tauri::ipc::Response;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::{DialogExt, FilePath};

use crate::{App, CommandResult, err, get, now};

/// The vault folder, next to the database.
pub const FILES_DIR: &str = "files";
/// Largest file shown inside the app; bigger ones are saved out instead.
const PREVIEW_MAX: u64 = 50 * 1024 * 1024;

impl App {
    pub fn files_dir(&self) -> PathBuf {
        self.dir.join(FILES_DIR)
    }

    /// Where a file can be read from: the vault, or else the copy next to the
    /// second backup folder (after a restore on a new device, say).
    fn file_home(&self, id: &str) -> CommandResult<PathBuf> {
        let main = self.files_dir();
        if files::path(&main, id).map_err(err)?.exists() {
            return Ok(main);
        }
        if let Some(folder) = self.with(|s| Ok(get::<String>(s, "backup.folder")))? {
            let copy = PathBuf::from(folder).join(FILES_DIR);
            if files::path(&copy, id).map_err(err)?.exists() {
                return Ok(copy);
            }
        }
        Err("This document's file is not on this device. It may be in the backup folder of the device it was added on.".into())
    }
}

/// Ask for a file, encrypt it into the vault and save `doc` (a
/// DocumentReference without its content) pointing at it. Returns the saved
/// record, or nothing when the user cancelled.
#[tauri::command]
pub async fn add_document(
    app: AppHandle,
    state: State<'_, App>,
    doc: Value,
) -> CommandResult<Option<Value>> {
    // Fail before the dialog if the data is locked.
    state.with(|_| Ok(()))?;
    let dialog = app.dialog().file().set_title("Choose a document to add");
    let Some(path) = tauri::async_runtime::spawn_blocking(move || dialog.blocking_pick_file())
        .await
        .map_err(err)?
    else {
        return Ok(None);
    };
    let name = file_name(&path);
    let content_type = content_type(&name);
    let dir = state.files_dir();
    let src = open_picked(&app, path).map_err(|e| format!("That file could not be read ({e})."))?;
    let sealed = tauri::async_runtime::spawn_blocking(move || files::seal(&dir, src))
        .await
        .map_err(err)?
        .map_err(|e| format!("Could not store the document: {e}"))?;
    let saved = state.with(|s| s.add_document(doc, &sealed, content_type, &name, now()));
    if saved.is_err() {
        let _ = files::remove(&state.files_dir(), &sealed.id);
    }
    saved.map(Some)
}

/// The plain bytes of a document, to show it inside the app.
#[tauri::command]
pub fn document_bytes(state: State<App>, id: String) -> CommandResult<Response> {
    let row = state.with(|s| s.file(&id))?;
    if row.size > PREVIEW_MAX {
        return Err("This file is too large to show here. Save a copy to open it.".into());
    }
    let dir = state.file_home(&id)?;
    files::read(&dir, &id, &row.key)
        .map(Response::new)
        .map_err(|e| format!("Could not open the document: {e}"))
}

/// Save the plain document to a place the user picked in the save dialog.
#[tauri::command]
pub fn save_document_to(
    app: AppHandle,
    state: State<App>,
    id: String,
    target: String,
) -> CommandResult<()> {
    let row = state.with(|s| s.file(&id))?;
    let dir = state.file_home(&id)?;
    let out = create_target(&app, &target).map_err(|e| format!("Could not save it there: {e}"))?;
    files::open(&dir, &id, &row.key, std::io::BufWriter::new(out))
        .map(drop)
        .map_err(|e| format!("Could not save the document: {e}"))
}

pub(crate) fn file_name(path: &FilePath) -> String {
    let s = path.to_string();
    let last = s.rsplit(['/', '\\']).next().unwrap_or("document");
    let name: String = last.chars().filter(|c| !c.is_control()).take(200).collect();
    if name.is_empty() {
        "document".into()
    } else {
        name
    }
}

/// The media type, from the file name. Unknown types are kept as bytes.
fn content_type(name: &str) -> &'static str {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "heic" => "image/heic",
        "tif" | "tiff" => "image/tiff",
        "txt" => "text/plain",
        "md" => "text/markdown",
        "rtf" => "application/rtf",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "odt" => "application/vnd.oasis.opendocument.text",
        "json" => "application/json",
        "xml" => "application/xml",
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "wav" => "audio/wav",
        "ogg" | "oga" => "audio/ogg",
        "mp4" | "m4v" => "video/mp4",
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        _ => "application/octet-stream",
    }
}

#[cfg(mobile)]
pub(crate) fn open_picked(
    app: &AppHandle,
    path: FilePath,
) -> std::io::Result<Box<dyn Read + Send>> {
    use tauri_plugin_fs::{FsExt, OpenOptions};
    let mut opts = OpenOptions::new();
    opts.read(true);
    Ok(Box::new(app.fs().open(path, opts)?))
}

#[cfg(desktop)]
pub(crate) fn open_picked(
    _app: &AppHandle,
    path: FilePath,
) -> std::io::Result<Box<dyn Read + Send>> {
    let p = path.into_path().map_err(std::io::Error::other)?;
    Ok(Box::new(std::fs::File::open(p)?))
}

#[cfg(mobile)]
fn create_target(app: &AppHandle, target: &str) -> std::io::Result<std::fs::File> {
    use tauri_plugin_fs::{FsExt, OpenOptions};
    let path: FilePath = target.parse().map_err(std::io::Error::other)?;
    let mut opts = OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    app.fs().open(path, opts)
}

#[cfg(desktop)]
fn create_target(_app: &AppHandle, target: &str) -> std::io::Result<std::fs::File> {
    std::fs::File::create(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_types_come_from_the_picked_path() {
        let p = FilePath::Path(PathBuf::from("/data/Scans/Letter 2026.PDF"));
        assert_eq!(file_name(&p), "Letter 2026.PDF");
        assert_eq!(content_type(&file_name(&p)), "application/pdf");
        assert_eq!(content_type("noext"), "application/octet-stream");
    }
}
