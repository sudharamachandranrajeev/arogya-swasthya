//! Importing records from a file (FHIR JSON, an IPS, a C-CDA document). The
//! file is read into a review list kept in the encrypted database, so it
//! survives leaving the screen; the user ticks what to keep and those are
//! saved as one change.

use std::io::Read;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use swasthya_core::import::{self, Found};
use swasthya_core::shl::{self, Link};
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
        .add_filter(
            "Health records (FHIR JSON or NDJSON, C-CDA)",
            &["json", "ndjson", "xml", "ccd", "cda"],
        );
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
    keep(&state, person, file_name, |s, person| {
        import::read(s, &bytes, person)
    })
}

/// Read the review list with `read` and keep it for the review screen.
fn keep(
    state: &App,
    person: String,
    file_name: String,
    read: impl FnOnce(&swasthya_core::Store, &str) -> Result<Found, import::Problem>,
) -> CommandResult<Option<Pending>> {
    let read = state.with(|s| {
        let found = match read(s, &person) {
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

/// What a pasted SMART Health Link says (label, passcode, expiry), before
/// anything is fetched.
#[tauri::command]
pub fn link_info(link: String) -> CommandResult<Link> {
    shl::parse(&link)
}

/// Fetch the records a SMART Health Link points at and read them into a
/// review list for `person`. This contacts the server named in the link,
/// and only that server; it never sees the records unencrypted.
#[tauri::command]
pub async fn import_link(
    state: State<'_, App>,
    person: String,
    link: String,
    passcode: Option<String>,
) -> CommandResult<Option<Pending>> {
    state.with(|_| Ok(()))?;
    let link = shl::parse(&link)?;
    if link.exp.is_some_and(|exp| exp < now().as_second()) {
        return Err("This link has expired. Ask for a new one.".into());
    }
    let label = link
        .label
        .clone()
        .unwrap_or_else(|| "a SMART Health Link".into());
    let resources = tauri::async_runtime::spawn_blocking(move || fetch(&link, passcode.as_deref()))
        .await
        .map_err(err)??;
    keep(&state, person, label, |s, person| {
        import::review(s, resources, person)
    })
}

const RECIPIENT: &str = "Arogya Swasthya (personal health record)";

/// Everything the link holds, decrypted, as FHIR resources.
fn fetch(link: &Link, passcode: Option<&str>) -> CommandResult<Vec<Value>> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .https_only(true)
        .http_status_as_error(false)
        .timeout_global(Some(std::time::Duration::from_secs(30)))
        .max_redirects(3)
        .build()
        .into();
    let offline = |e: ureq::Error| format!("The link's server could not be reached ({e}).");
    let read = |mut r: ureq::http::Response<ureq::Body>| -> CommandResult<Vec<u8>> {
        r.body_mut()
            .with_config()
            .limit(import::MAX_BYTES as u64)
            .read_to_vec()
            .map_err(|e| format!("The link's server sent something unreadable ({e})."))
    };
    let get = |url: &str| -> CommandResult<String> {
        let r = agent
            .get(url)
            .query("recipient", RECIPIENT)
            .call()
            .map_err(offline)?;
        if !r.status().is_success() {
            return Err(gone(r.status().as_u16()));
        }
        String::from_utf8(read(r)?)
            .map_err(|_| "The link's server sent something unreadable.".into())
    };
    if link.direct {
        let (plain, cty) = shl::decrypt(&get(&link.url)?, &link.key)?;
        return shl::resources(cty.as_deref().unwrap_or("application/fhir+json"), &plain);
    }
    let body = shl::manifest_request(RECIPIENT, passcode);
    let r = agent
        .post(&link.url)
        .header("content-type", "application/json")
        .send(serde_json::to_vec(&body).map_err(err)?)
        .map_err(offline)?;
    let status = r.status().as_u16();
    let answer: Value = serde_json::from_slice(&read(r)?).unwrap_or(Value::Null);
    match status {
        200..=299 => {}
        401 => {
            let left = answer["remainingAttempts"]
                .as_u64()
                .map(|n| format!(" {n} tries left."))
                .unwrap_or_default();
            return Err(format!("The passcode was not accepted.{left}"));
        }
        s => return Err(gone(s)),
    }
    let mut out = Vec::new();
    for f in shl::manifest_files(&answer)? {
        let jwe = match (f.embedded, f.location) {
            (Some(e), _) => e,
            (None, Some(url)) => get(&url)?,
            (None, None) => continue,
        };
        let (plain, _) = shl::decrypt(&jwe, &link.key)?;
        out.extend(shl::resources(&f.content_type, &plain)?);
    }
    Ok(out)
}

fn gone(status: u16) -> String {
    match status {
        404 | 410 => "This link is no longer active. Ask for a new one.".into(),
        429 => "The link's server is busy. Try again in a minute.".into(),
        s => format!("The link's server refused the request (status {s})."),
    }
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
