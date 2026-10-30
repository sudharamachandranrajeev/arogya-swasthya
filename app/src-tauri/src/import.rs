//! Importing records from a file (FHIR JSON, an IPS). The file is read into
//! a review list kept in the encrypted database, so it survives leaving the
//! screen; the user ticks what to keep and those are saved as one change.

use std::io::Read;

use serde::{Deserialize, Serialize};
use swasthya_core::import::{self, Found};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::docs::{file_name, open_picked};
use crate::{App, CommandResult, err, get, now};

const PENDING: &str = "import.pending";

/// A file read and waiting for review.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pending {
    pub person: String,
    pub file_name: String,
    #[serde(flatten)]
    pub found: Found,
}

/// Ask for a file and read it into a review list for `person`, replacing
/// any list not yet finished. Nothing when the user cancelled.
#[tauri::command]
pub async fn import_pick(
    app: AppHandle,
    state: State<'_, App>,
    person: String,
) -> CommandResult<Option<Pending>> {
    state.with(|_| Ok(()))?;
    let dialog = app
        .dialog()
        .file()
        .set_title("Choose a health record file to import")
        .add_filter("Health records (FHIR JSON)", &["json"]);
    let Some(path) = tauri::async_runtime::spawn_blocking(move || dialog.blocking_pick_file())
        .await
        .map_err(err)?
    else {
        return Ok(None);
    };
    let file_name = file_name(&path);
    let mut bytes = Vec::new();
    open_picked(&app, path)
        .and_then(|src| {
            src.take(import::MAX_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
        })
        .map_err(|e| format!("That file could not be read ({e})."))?;
    let read = state.with(|s| {
        let found = match import::read_fhir(s, &bytes, &person) {
            Ok(found) => found,
            Err(import::Problem::Store(e)) => return Err(e),
            Err(problem) => return Ok(Err(capital(&problem.to_string()))),
        };
        let pending = Pending {
            person: person.clone(),
            file_name,
            found,
        };
        s.set_setting(PENDING, &Some(&pending))?;
        Ok(Ok(pending))
    })?;
    read.map(Some)
}

/// The review list not yet finished, if any.
#[tauri::command]
pub fn import_pending(state: State<App>) -> CommandResult<Option<Pending>> {
    state.with(|s| Ok(get::<Pending>(s, PENDING)))
}

/// Save the records at `picks` (places in the review list) as one change,
/// one Undo, and end the review. Returns how many were saved.
#[tauri::command]
pub fn import_accept(state: State<App>, picks: Vec<usize>) -> CommandResult<usize> {
    state.with(|s| {
        let Some(p) = get::<Pending>(s, PENDING) else {
            return Ok(0);
        };
        let chosen: Vec<_> = p
            .found
            .candidates
            .into_iter()
            .enumerate()
            .filter(|(i, _)| picks.contains(i))
            .map(|(_, c)| c.resource)
            .collect();
        let n = chosen.len();
        if n > 0 {
            let label = format!("import of {n} record{}", if n == 1 { "" } else { "s" });
            s.save_all(chosen, &label, now())?;
        }
        s.set_setting(PENDING, &None::<Pending>)?;
        Ok(n)
    })
}

/// Drop the review list without saving anything.
#[tauri::command]
pub fn import_discard(state: State<App>) -> CommandResult<()> {
    state.with(|s| s.set_setting(PENDING, &None::<Pending>))
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect::<String>() + ".")
        .unwrap_or_default()
}
