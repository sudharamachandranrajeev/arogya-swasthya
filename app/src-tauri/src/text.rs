//! Reading the words in documents, in the background, so search finds them.
//! PDFs with their own text and text files are read straight away. Photos
//! and scans need the reading models (about 12 MB), and recordings need the
//! transcript model (about 60 MB, desktop only for now). Each is downloaded
//! once, only after the user says yes; documents never leave the device.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use sha2::{Digest, Sha256};
use swasthya_core::extract::{self, Reader, Text};
use swasthya_core::files;
use tauri::{AppHandle, Manager, State};

use crate::{App, CommandResult, err, get};

const MODELS_DIR: &str = "models";

/// A model file to download, checked by size and SHA-256 before use.
pub(crate) struct ModelFile {
    pub url: &'static str,
    pub name: &'static str,
    pub sha256: &'static str,
    pub size: u64,
}

/// The reading (OCR) models.
const MODELS: [ModelFile; 2] = [
    ModelFile {
        url: "https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.onnx",
        name: "text-detection.onnx",
        sha256: "a917b23dbd9524b465df7e922641b3ff2981623df4ded5a0234004ef2fee7cfe",
        size: 2_499_479,
    },
    ModelFile {
        url: "https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.onnx",
        name: "text-recognition.onnx",
        sha256: "86c145c2edb96c8caed5b1ebb8f44d706408922211451c309c625157dd6061c5",
        size: 9_713_177,
    },
];
/// Larger files are not read for text.
const MAX_READ: u64 = 200 * 1024 * 1024;

static RUNNING: AtomicBool = AtomicBool::new(false);
static READER: Mutex<Option<Arc<Reader>>> = Mutex::new(None);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextStatus {
    /// Reading photos and scans: "on", "off", or nothing when not chosen.
    ocr: Option<String>,
    /// Transcribing recordings, the same way.
    transcribe: Option<String>,
    /// This device can transcribe (desktop).
    can_transcribe: bool,
    /// Documents being read now (not those waiting for a yes).
    waiting: usize,
}

impl App {
    pub(crate) fn models_dir(&self) -> PathBuf {
        self.dir.join(MODELS_DIR)
    }
}

fn is_media(content_type: &str) -> bool {
    content_type.starts_with("audio/") || content_type.starts_with("video/")
}

fn on(s: &swasthya_core::Store, key: &str) -> bool {
    get::<String>(s, key).as_deref() == Some("on")
}

/// What is switched on, and how many documents wait to be read. Also
/// starts reading any that can be.
#[tauri::command]
pub fn text_status(app: AppHandle, state: State<App>) -> CommandResult<TextStatus> {
    let status = state.with(|s| {
        let (ocr_on, talk_on) = (on(s, "ocr.mode"), on(s, "transcribe.mode"));
        let waiting = s
            .files_without_text()?
            .iter()
            .filter(|r| {
                if is_media(&r.content_type) {
                    talk_on
                } else {
                    ocr_on
                }
            })
            .count();
        Ok(TextStatus {
            ocr: get::<String>(s, "ocr.mode"),
            transcribe: get::<String>(s, "transcribe.mode"),
            can_transcribe: cfg!(desktop),
            waiting,
        })
    })?;
    start(&app);
    Ok(status)
}

/// Turn reading photos and scans on (downloading the models if needed) or off.
#[tauri::command]
pub async fn set_ocr(app: AppHandle, state: State<'_, App>, on: bool) -> CommandResult<()> {
    switch(&app, &state, "ocr.mode", on, &MODELS).await
}

/// Turn transcripts of recordings on (downloading the model if needed) or off.
#[tauri::command]
pub async fn set_transcribe(app: AppHandle, state: State<'_, App>, on: bool) -> CommandResult<()> {
    #[cfg(desktop)]
    return switch(
        &app,
        &state,
        "transcribe.mode",
        on,
        std::slice::from_ref(&crate::transcribe::MODEL),
    )
    .await;
    #[cfg(mobile)]
    {
        let _ = (app, state, on);
        Err("Transcripts are not available on phones yet.".into())
    }
}

async fn switch(
    app: &AppHandle,
    state: &App,
    key: &str,
    on: bool,
    models: &'static [ModelFile],
) -> CommandResult<()> {
    if on {
        let dir = state.models_dir();
        tauri::async_runtime::spawn_blocking(move || download(&dir, models))
            .await
            .map_err(err)??;
    }
    state.with(|s| s.set_setting(key, &if on { "on" } else { "off" }))?;
    start(app);
    Ok(())
}

/// Read the words of every document not read yet, on a background thread.
/// Does nothing if a reader is already running.
pub fn start(app: &AppHandle) {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        read_all(app.state::<App>().inner());
        RUNNING.store(false, Ordering::SeqCst);
    });
}

fn read_all(state: &App) {
    let mut tried = std::collections::HashSet::new();
    loop {
        let Ok((rows, ocr_on, talk_on)) = state.with(|s| {
            Ok((
                s.files_without_text()?,
                on(s, "ocr.mode"),
                on(s, "transcribe.mode"),
            ))
        }) else {
            return;
        };
        let Some(row) = rows.into_iter().find(|r| !tried.contains(&r.id)) else {
            return;
        };
        tried.insert(row.id.clone());
        let media = is_media(&row.content_type);
        let text = if row.size > MAX_READ {
            Text::Unsupported
        } else if media && !(cfg!(desktop) && talk_on) {
            // Phones cannot transcribe yet; desktops wait for a yes.
            if cfg!(desktop) {
                Text::NeedsOcr
            } else {
                Text::Unsupported
            }
        } else {
            match state
                .file_home(&row.id)
                .ok()
                .and_then(|dir| files::read(&dir, &row.id, &row.key).ok())
            {
                None => Text::Unsupported,
                #[cfg(desktop)]
                Some(bytes) if media => crate::transcribe::text_of(&state.models_dir(), bytes),
                Some(bytes) => {
                    let ocr = ocr_on.then(|| reader(&state.models_dir())).flatten();
                    extract::text_of(&row.content_type, &bytes, ocr.as_deref())
                }
            }
        };
        let words = match text {
            Text::Found(t) => t,
            Text::Unsupported => String::new(),
            // Left unread until the user turns on what it needs.
            Text::NeedsOcr => continue,
        };
        let _ = state.with(|s| s.set_file_text(&row.id, &words));
    }
}

/// The reading models, loaded once they are on this device.
fn reader(dir: &Path) -> Option<Arc<Reader>> {
    let mut slot = READER.lock().ok()?;
    if slot.is_none() {
        let r = Reader::load(&dir.join(MODELS[0].name), &dir.join(MODELS[1].name)).ok()?;
        *slot = Some(Arc::new(r));
    }
    slot.clone()
}

/// Fetch any model not yet here, checking its size and SHA-256.
fn download(dir: &Path, models: &[ModelFile]) -> CommandResult<()> {
    std::fs::create_dir_all(dir).map_err(err)?;
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .https_only(true)
        .timeout_global(Some(std::time::Duration::from_secs(300)))
        .build()
        .into();
    for &ModelFile {
        url,
        name,
        sha256: sha,
        size,
    } in models
    {
        let dest = dir.join(name);
        if dest.exists() {
            continue;
        }
        let failed =
            |e: String| format!("The model could not be downloaded ({e}). Try again later.");
        let bytes = agent
            .get(url)
            .call()
            .map_err(|e| failed(e.to_string()))?
            .body_mut()
            .with_config()
            .limit(size + 1)
            .read_to_vec()
            .map_err(|e| failed(e.to_string()))?;
        let got = Sha256::digest(&bytes);
        let hex: String = got.iter().map(|b| format!("{b:02x}")).collect();
        if bytes.len() as u64 != size || hex != sha {
            return Err(failed("it did not match the expected file".into()));
        }
        let tmp = dest.with_extension("partial");
        std::fs::write(&tmp, &bytes).map_err(err)?;
        std::fs::rename(&tmp, &dest).map_err(err)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Inner;
    use jiff::Timestamp;
    use serde_json::json;
    use swasthya_core::Store;

    fn app_with(dir: &Path) -> (App, String) {
        let mut store = Store::memory().unwrap();
        let me = store
            .save(
                json!({"resourceType": "Patient", "name": [{"text": "Test Person"}]}),
                Timestamp::UNIX_EPOCH,
            )
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();
        let inner = Inner {
            store: Some(store),
            ..Default::default()
        };
        let app = App {
            inner: Mutex::new(inner),
            dir: dir.to_path_buf(),
        };
        (app, me)
    }

    fn add(app: &App, me: &str, bytes: &[u8], content_type: &str, name: &str) {
        let sealed = files::seal(&app.files_dir(), bytes).unwrap();
        let doc = json!({"resourceType": "DocumentReference", "subject": {"reference": format!("Patient/{me}")}, "description": name});
        app.with(|s| s.add_document(doc, &sealed, content_type, name, Timestamp::UNIX_EPOCH))
            .unwrap();
    }

    #[test]
    fn reads_documents_in_the_background_and_waits_for_ocr_when_it_is_off() {
        let dir = tempfile::tempdir().unwrap();
        let (app, me) = app_with(dir.path());
        add(
            &app,
            &me,
            b"Clinic letter about vitamin D",
            "text/plain",
            "letter.txt",
        );
        add(
            &app,
            &me,
            include_bytes!("../../../core/tests/ocr-sample.png"),
            "image/png",
            "scan.png",
        );
        add(&app, &me, b"\x00\x01", "audio/mpeg", "voice.mp3");
        read_all(&app);
        let found = |w: &str| app.with(|s| s.search(Some(&me), w, 5)).unwrap().len();
        assert_eq!(found("vitamin"), 1);
        assert_eq!(found("ferritin"), 0);
        // The photo waits for OCR to be turned on; on a desktop the
        // recording waits for transcripts too.
        let waiting = app.with(|s| s.files_without_text()).unwrap();
        let mut kinds: Vec<_> = waiting.iter().map(|r| r.content_type.as_str()).collect();
        kinds.sort();
        if cfg!(desktop) {
            assert_eq!(kinds, ["audio/mpeg", "image/png"]);
        } else {
            assert_eq!(kinds, ["image/png"]);
        }
        // With transcripts on, a file that is not really a recording is
        // marked read (with no words) rather than tried again and again.
        app.with(|s| s.set_setting("transcribe.mode", &"on"))
            .unwrap();
        read_all(&app);
        assert_eq!(app.with(|s| s.files_without_text()).unwrap().len(), 1);

        let Some(models) = std::env::var_os("OCR_MODELS") else {
            eprintln!("OCR_MODELS not set: skipping the OCR part");
            return;
        };
        let to = app.models_dir();
        std::fs::create_dir_all(&to).unwrap();
        for m in &MODELS {
            std::fs::copy(Path::new(&models).join(m.name), to.join(m.name)).unwrap();
        }
        app.with(|s| s.set_setting("ocr.mode", &"on")).unwrap();
        read_all(&app);
        assert_eq!(found("ferritin"), 1);
        assert!(app.with(|s| s.files_without_text()).unwrap().is_empty());
    }
    /// Fetches about 12 MB from the model server: run by hand with
    /// `cargo test -p arogya-swasthya -- --ignored downloads`.
    #[test]
    #[ignore]
    fn downloads_and_checks_the_models() {
        let dir = tempfile::tempdir().unwrap();
        download(dir.path(), &MODELS).unwrap();
        for m in &MODELS {
            assert_eq!(
                std::fs::metadata(dir.path().join(m.name)).unwrap().len(),
                m.size
            );
        }
        assert!(reader(dir.path()).is_some());
    }
}
