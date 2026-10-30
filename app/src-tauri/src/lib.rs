//! Arogya Swasthya's app shell: opens the encrypted store with the key kept by
//! this device, guards it with an optional app lock (PIN, or fingerprint/face
//! on phones), keeps daily backups, and answers the UI's commands.

mod data;
mod docs;
mod import;
mod text;
mod update;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use jiff::{SignedDuration, Timestamp};
use serde::Serialize;
use serde_json::Value;
use swasthya_core::keys::{self, Key};
use swasthya_core::store::{self, Store, Trashed};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_swasthya_device::{DeviceExt, KeyStorage};

const DB_FILE: &str = "swasthya.db";
const BACKUP_DIR: &str = "backups";
/// Wrong PINs allowed before each wait starts.
const FREE_TRIES: u32 = 5;
const MAX_WAIT_SECS: i64 = 15 * 60;

type CommandResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[derive(Default)]
struct Inner {
    store: Option<Store>,
    key: Option<Key>,
    key_storage: Option<KeyStorage>,
    /// Why the store is not open, in plain words.
    problem: Option<String>,
    /// The app lock is on: the store is open but nothing may read it.
    locked: bool,
    wrong_pins: u32,
    wait_until: Option<Timestamp>,
    /// Something the user should hear about once, such as a failed restore
    /// at start.
    notice: Option<String>,
}

struct App {
    inner: Mutex<Inner>,
    dir: PathBuf,
}

impl App {
    fn lock_inner(&self) -> CommandResult<std::sync::MutexGuard<'_, Inner>> {
        self.inner.lock().map_err(err)
    }

    /// Run `f` on the open, unlocked store.
    fn with<T>(&self, f: impl FnOnce(&mut Store) -> store::Result<T>) -> CommandResult<T> {
        let mut inner = self.lock_inner()?;
        if inner.locked {
            return Err("Arogya Swasthya is locked.".into());
        }
        let store = inner
            .store
            .as_mut()
            .ok_or("Your data is not open yet: enter your recovery key.")?;
        f(store).map_err(err)
    }

    /// Open the store with the key from this device.
    fn open(&self, app: &AppHandle) {
        let db = self.dir.join(DB_FILE);
        let first = !db.exists();
        let mut inner = self.inner.lock().unwrap();
        match app.device().db_key(&self.dir, first) {
            Ok(k) => match (self.apply_staged_restore(&k.key), Store::open(&db, &k.key)) {
                (notice, Ok(store)) => {
                    let locked = lock_on(&store);
                    *inner = Inner {
                        store: Some(store),
                        key: Some(k.key),
                        key_storage: Some(k.storage),
                        locked,
                        notice,
                        ..Inner::default()
                    }
                }
                (_, Err(e)) => {
                    inner.problem = Some(format!("The saved key does not open your data ({e})."))
                }
            },
            Err(e) => inner.problem = Some(format!("{e}.")),
        }
    }

    /// Daily backups (app folder, then the user's folder) and trash clean-up.
    /// Each runs at most once a day; both are safe to call often.
    fn maintain(&self) {
        let now = Timestamp::now();
        let Ok(mut inner) = self.inner.lock() else {
            return;
        };
        let Inner {
            store: Some(store),
            key: Some(key),
            ..
        } = &mut *inner
        else {
            return;
        };
        let _ = store.daily_backup(&self.dir.join(BACKUP_DIR), key, now);
        let vault = self.dir.join(docs::FILES_DIR);
        if let Some(folder) = get::<String>(store, "backup.folder") {
            let _ = store.daily_backup(Path::new(&folder), key, now);
            // Documents are not inside the database: copy any new ones too.
            let copies = Path::new(&folder).join(docs::FILES_DIR);
            for id in store.file_ids().unwrap_or_default() {
                let _ = swasthya_core::files::mirror(&vault, &copies, &id);
            }
        }
        let _ = store.purge(now);
        for id in store.drop_stale_files(now).unwrap_or_default() {
            let _ = swasthya_core::files::remove(&vault, &id);
        }
    }
}

/// A setting, treating a cleared one (stored as null) as unset.
fn get<T: serde::de::DeserializeOwned>(store: &Store, key: &str) -> Option<T> {
    store.setting::<Option<T>>(key).ok().flatten().flatten()
}

/// The app lock is on when the user set a PIN or turned on biometric unlock.
fn lock_on(store: &Store) -> bool {
    get::<String>(store, "lock.pin").is_some()
        || get::<bool>(store, "lock.biometric").unwrap_or(false)
}

fn now() -> Timestamp {
    Timestamp::now()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusView {
    /// The store is open (it may still be behind the app lock).
    open: bool,
    locked: bool,
    problem: Option<String>,
    /// The user has not yet confirmed they saved the recovery key.
    recovery_pending: bool,
    key_storage: Option<KeyStorage>,
    has_pin: bool,
    biometric: bool,
    /// Seconds before another PIN may be tried.
    wait_secs: i64,
    platform: &'static str,
    notice: Option<String>,
}

#[tauri::command]
fn status(state: State<App>) -> CommandResult<StatusView> {
    let mut inner = state.lock_inner()?;
    let notice = inner.notice.take();
    let flag = |k: &str| {
        inner
            .store
            .as_ref()
            .and_then(|s| get::<bool>(s, k))
            .unwrap_or(false)
    };
    let wait_secs = inner
        .wait_until
        .map(|t| (t - now()).get_seconds().max(0))
        .unwrap_or(0);
    Ok(StatusView {
        open: inner.store.is_some(),
        locked: inner.locked,
        problem: inner.problem.clone(),
        recovery_pending: inner.store.is_some() && !flag("recovery.saved"),
        key_storage: inner.key_storage,
        has_pin: inner
            .store
            .as_ref()
            .and_then(|s| get::<String>(s, "lock.pin"))
            .is_some(),
        biometric: flag("lock.biometric"),
        wait_secs,
        platform: std::env::consts::OS,
        notice,
    })
}

/// The key in groups of four, for writing down. Only while unlocked.
#[tauri::command]
fn recovery_key(state: State<App>) -> CommandResult<String> {
    let inner = state.lock_inner()?;
    match (&inner.key, inner.locked) {
        (Some(key), false) => Ok(keys::recovery_text(key)),
        _ => Err("Arogya Swasthya is locked.".into()),
    }
}

#[tauri::command]
fn recovery_saved(state: State<App>) -> CommandResult<()> {
    state.with(|s| s.set_setting("recovery.saved", &true))
}

/// Open the data with the recovery key, then keep the key on this device
/// again. The recovery key also clears the app lock.
#[tauri::command]
fn unlock_with(app: AppHandle, state: State<App>, recovery: String) -> CommandResult<()> {
    let key = keys::parse_recovery(&recovery)
        .ok_or("A recovery key is 64 letters and digits (0-9, a-f).")?;
    let store = Store::open(&state.dir.join(DB_FILE), &key)
        .map_err(|_| "That recovery key does not open this data.")?;
    let storage = app.device().set_db_key(&state.dir, &key).map_err(err)?;
    *state.lock_inner()? = Inner {
        store: Some(store),
        key: Some(key),
        key_storage: Some(storage),
        ..Inner::default()
    };
    state.maintain();
    Ok(())
}

/// Set, change or (with `None`) remove the PIN. Only while unlocked.
#[tauri::command]
fn set_pin(state: State<App>, pin: Option<String>) -> CommandResult<()> {
    match pin {
        Some(pin) => {
            let hash = keys::pin_hash(&pin).map_err(err)?;
            state.with(|s| s.set_setting("lock.pin", &hash))
        }
        None => state.with(|s| s.set_setting("lock.pin", &None::<String>)),
    }
}

/// Try a PIN. After [`FREE_TRIES`] wrong ones each further try waits longer,
/// doubling up to [`MAX_WAIT_SECS`]. The recovery key always works.
#[tauri::command]
fn unlock_pin(state: State<App>, pin: String) -> CommandResult<bool> {
    let ok = state.lock_inner()?.try_pin(&pin, now())?;
    if ok {
        state.maintain();
    }
    Ok(ok)
}

/// Wait before the next PIN try after `wrong` wrong ones in a row: none for
/// the first [`FREE_TRIES`], then 30 s doubling up to [`MAX_WAIT_SECS`].
fn pin_wait(wrong: u32) -> Option<SignedDuration> {
    let extra = wrong.checked_sub(FREE_TRIES)?;
    Some(SignedDuration::from_secs(
        (30i64 << extra.min(10)).min(MAX_WAIT_SECS),
    ))
}

impl Inner {
    /// Check a PIN, keeping count of wrong ones. Unlocks on a match.
    fn try_pin(&mut self, pin: &str, now: Timestamp) -> CommandResult<bool> {
        if let Some(t) = self.wait_until
            && t > now
        {
            return Err(format!(
                "Too many wrong PINs. Try again in {} seconds.",
                (t - now).get_seconds() + 1
            ));
        }
        let hash = self
            .store
            .as_ref()
            .and_then(|s| get::<String>(s, "lock.pin"))
            .ok_or("No PIN is set.")?;
        if keys::pin_check(pin, &hash) {
            self.locked = false;
            self.wrong_pins = 0;
            self.wait_until = None;
            return Ok(true);
        }
        self.wrong_pins += 1;
        self.wait_until = pin_wait(self.wrong_pins).map(|d| now + d);
        Ok(false)
    }
}

/// Unlock with fingerprint or face (phones only).
#[tauri::command]
fn unlock_biometric(app: AppHandle, state: State<App>) -> CommandResult<()> {
    biometric_check(&app, "Unlock your health records")?;
    state.lock_inner()?.locked = false;
    state.maintain();
    Ok(())
}

/// Turn biometric unlock on (after one successful check) or off.
#[tauri::command]
fn set_biometric(app: AppHandle, state: State<App>, on: bool) -> CommandResult<()> {
    if on {
        biometric_check(&app, "Turn on fingerprint or face unlock")?;
    }
    state.with(|s| s.set_setting("lock.biometric", &on))
}

#[cfg(mobile)]
fn biometric_check(app: &AppHandle, reason: &str) -> CommandResult<()> {
    use tauri_plugin_biometric::{AuthOptions, BiometricExt};
    let options = AuthOptions {
        allow_device_credential: true,
        ..AuthOptions::default()
    };
    app.biometric()
        .authenticate(reason.into(), options)
        .map_err(err)
}

#[cfg(desktop)]
fn biometric_check(_app: &AppHandle, _reason: &str) -> CommandResult<()> {
    Err("Fingerprint and face unlock are for phones; use a PIN on this computer.".into())
}

/// Lock now. Does nothing unless a PIN or biometric unlock is set.
#[tauri::command]
fn lock(state: State<App>) -> CommandResult<bool> {
    let mut inner = state.lock_inner()?;
    inner.locked = inner.store.as_ref().is_some_and(lock_on);
    Ok(inner.locked)
}

/// Settings the UI may read and write. Everything else (PIN, backup folder,
/// recovery state) has its own command with its own checks.
const UI_SETTINGS: &[&str] = &["places", "terms.accepted", "setup.done", "person.current"];

fn ui_setting(key: &str) -> CommandResult<()> {
    UI_SETTINGS
        .contains(&key)
        .then_some(())
        .ok_or_else(|| format!("{key} is not a setting the screens can change."))
}

#[tauri::command]
fn get_setting(state: State<App>, key: String) -> CommandResult<Option<Value>> {
    ui_setting(&key)?;
    state.with(|s| Ok(get::<Value>(s, &key)))
}

#[tauri::command]
fn put_setting(state: State<App>, key: String, value: Value) -> CommandResult<()> {
    ui_setting(&key)?;
    state.with(|s| s.set_setting(&key, &value))
}

#[tauri::command]
fn people(state: State<App>) -> CommandResult<Vec<Value>> {
    state.with(|s| s.people())
}

#[tauri::command]
fn records(state: State<App>, person: String, kind: String) -> CommandResult<Vec<Value>> {
    state.with(|s| s.records(&person, &kind))
}

#[tauri::command]
fn timeline(state: State<App>, person: String) -> CommandResult<Vec<Value>> {
    state.with(|s| s.timeline(&person))
}

/// Records holding every word typed, best first; one person or everyone.
#[tauri::command]
fn search(
    state: State<App>,
    person: Option<String>,
    words: String,
) -> CommandResult<Vec<store::Hit>> {
    state.with(|s| s.search(person.as_deref(), &words, 50))
}

#[tauri::command]
fn save_record(state: State<App>, resource: Value) -> CommandResult<Value> {
    state.with(|s| s.save(resource, now()))
}

#[tauri::command]
fn delete_record(state: State<App>, id: String) -> CommandResult<()> {
    state.with(|s| s.delete(&id, now()))
}

#[tauri::command]
fn restore_record(state: State<App>, id: String) -> CommandResult<()> {
    state.with(|s| s.restore(&id, now()))
}

#[tauri::command]
fn trash(state: State<App>) -> CommandResult<Vec<Trashed>> {
    state.with(|s| s.trash())
}

/// Undo the last change. Returns what was undone, or nothing.
#[tauri::command]
fn undo(state: State<App>) -> CommandResult<Option<String>> {
    state.with(|s| Ok(s.undo()?.map(|u| u.label)))
}

/// Everything (or one person's records) as a FHIR Bundle.
#[tauri::command]
fn export_bundle(state: State<App>, person: Option<String>) -> CommandResult<Value> {
    state.with(|s| s.export_bundle(person.as_deref(), now()))
}

/// Desktop: re-check the daily jobs every hour while the app runs.
#[cfg(desktop)]
fn upkeep_loop(app: AppHandle) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(3600));
            app.state::<App>().maintain();
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if std::env::args().any(|a| a == "--version") {
        println!("Arogya Swasthya {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    let mut builder = tauri::Builder::default();
    #[cfg(desktop)]
    {
        builder = builder
            .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.unminimize();
                    let _ = w.set_focus();
                }
            }))
            .plugin(tauri_plugin_updater::Builder::new().build());
    }
    #[cfg(mobile)]
    {
        builder = builder
            .plugin(tauri_plugin_biometric::init())
            .plugin(tauri_plugin_fs::init());
    }
    builder
        .plugin(tauri_plugin_swasthya_device::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(dir.join(BACKUP_DIR))?;
            // The database and backups are encrypted; keep their folders private too.
            #[cfg(unix)]
            for d in [dir.clone(), dir.join(BACKUP_DIR)] {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(d, std::fs::Permissions::from_mode(0o700));
            }
            app.manage(App {
                inner: Mutex::new(Inner::default()),
                dir,
            });
            let handle = app.handle().clone();
            let state = app.state::<App>();
            state.open(&handle);
            // Backups need no unlock: they copy the encrypted data as it is.
            state.maintain();
            text::start(&handle);
            #[cfg(desktop)]
            upkeep_loop(handle);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            status,
            recovery_key,
            recovery_saved,
            unlock_with,
            set_pin,
            unlock_pin,
            unlock_biometric,
            set_biometric,
            lock,
            data::backups,
            data::set_backup_folder,
            data::backup_now,
            data::save_backup_to,
            data::pick_backup,
            data::inspect_backup,
            data::restore_backup,
            docs::add_document,
            docs::document_bytes,
            docs::save_document_to,
            import::import_pick,
            import::import_pending,
            import::import_accept,
            import::import_discard,
            import::link_info,
            import::import_link,
            text::ocr_status,
            text::set_ocr,
            update::update_status,
            update::set_update_mode,
            update::open_page,
            update::check_update,
            update::install_update,
            update::rollback_plan,
            update::rollback,
            get_setting,
            put_setting,
            people,
            records,
            timeline,
            search,
            save_record,
            delete_record,
            restore_record,
            trash,
            undo,
            export_bundle,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Arogya Swasthya");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_pin(pin: &str) -> Inner {
        let store = Store::memory().unwrap();
        store
            .set_setting("lock.pin", &keys::pin_hash(pin).unwrap())
            .unwrap();
        let locked = lock_on(&store);
        Inner {
            store: Some(store),
            locked,
            ..Inner::default()
        }
    }

    #[test]
    fn waits_grow_after_five_wrong_pins_and_stop_at_fifteen_minutes() {
        assert_eq!(pin_wait(4), None);
        assert_eq!(pin_wait(5), Some(SignedDuration::from_secs(30)));
        assert_eq!(pin_wait(6), Some(SignedDuration::from_secs(60)));
        assert_eq!(pin_wait(9), Some(SignedDuration::from_secs(480)));
        assert_eq!(pin_wait(10), Some(SignedDuration::from_secs(MAX_WAIT_SECS)));
        assert_eq!(
            pin_wait(500),
            Some(SignedDuration::from_secs(MAX_WAIT_SECS))
        );
    }

    #[test]
    fn right_pin_unlocks_and_wrong_ones_lock_out_for_a_while() {
        let t0: Timestamp = "2026-10-29T10:00:00Z".parse().unwrap();
        let mut inner = with_pin("2468");
        assert!(inner.locked);
        for _ in 0..4 {
            assert_eq!(inner.try_pin("1111", t0), Ok(false));
        }
        assert_eq!(inner.wait_until, None);
        assert_eq!(inner.try_pin("1111", t0), Ok(false));
        // Even the right PIN waits until the time is up.
        let err = inner
            .try_pin("2468", t0 + SignedDuration::from_secs(10))
            .unwrap_err();
        assert!(err.contains("Try again in 21 seconds"), "{err}");
        assert!(inner.locked);
        assert_eq!(
            inner.try_pin("2468", t0 + SignedDuration::from_secs(30)),
            Ok(true)
        );
        assert!(!inner.locked);
        assert_eq!((inner.wrong_pins, inner.wait_until), (0, None));
    }

    #[test]
    fn removed_pin_and_biometric_switch_the_lock_off() {
        let inner = with_pin("2468");
        let store = inner.store.as_ref().unwrap();
        store.set_setting("lock.pin", &None::<String>).unwrap();
        assert!(!lock_on(store));
        store.set_setting("lock.biometric", &true).unwrap();
        assert!(lock_on(store));
        store.set_setting("lock.biometric", &false).unwrap();
        assert!(!lock_on(store));
    }

    #[test]
    fn no_pin_set_is_an_error_not_an_unlock() {
        let mut inner = Inner {
            store: Some(Store::memory().unwrap()),
            locked: true,
            ..Inner::default()
        };
        assert_eq!(
            inner.try_pin("0000", Timestamp::now()),
            Err("No PIN is set.".into())
        );
        assert!(inner.locked);
    }

    #[test]
    fn screens_can_only_touch_their_own_settings() {
        assert!(ui_setting("places").is_ok());
        for key in [
            "lock.pin",
            "lock.biometric",
            "backup.folder",
            "recovery.saved",
        ] {
            assert!(ui_setting(key).is_err(), "{key}");
        }
    }
}
