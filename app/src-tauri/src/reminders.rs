//! Medicine reminders. Times come from each medicine being taken now
//! (FHIR `dosage[0].timing.repeat.timeOfDay`); what was taken or skipped is
//! kept in a dose log. A reminder fires at the dose time and, if the dose is
//! still not marked, once more half an hour later unless that falls in the
//! quiet hours. By default a reminder says only "Time for a medicine", so
//! nothing private shows on a locked screen.
//!
//! Desktops check every 30 seconds while the app runs (in the tray when the
//! window is closed, started at login). Phones schedule the next two days
//! with the system's alarms, which also survive a restart.

use std::collections::{BTreeMap, HashSet};

use jiff::civil::{Date, Time};
use jiff::{SignedDuration, Timestamp, ToSpan, Zoned};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use swasthya_core::store::Store;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_notification::NotificationExt;

use crate::{App, CommandResult, err, get};

const SETTINGS: &str = "reminders";
const LOG: &str = "reminders.log";
/// Days of the dose log kept.
const KEEP_DAYS: i64 = 90;
/// The second reminder, if the dose is still not marked.
const NUDGE_AFTER: SignedDuration = SignedDuration::from_mins(30);
/// A reminder this late is not sent at all (the computer was asleep, say).
const STALE_AFTER: SignedDuration = SignedDuration::from_mins(30);
/// Phones: how far ahead reminders are scheduled.
#[cfg(mobile)]
const AHEAD: SignedDuration = SignedDuration::from_hours(48);
/// Most times a day one medicine can have.
const MAX_TIMES: usize = 8;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub on: bool,
    /// Show the medicine's name in the reminder (seen on a locked screen).
    pub names: bool,
    /// No second reminders from the first time to the second ("22:00", "07:00").
    pub quiet: Option<(String, String)>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Mark {
    /// "taken" or "skipped".
    pub status: String,
    pub at: Timestamp,
}

/// Day ("2026-10-30") to "medicine id|HH:MM" to what was done.
type Log = BTreeMap<String, BTreeMap<String, Mark>>;

/// A medicine being taken now that has times set.
#[derive(Clone, Debug)]
struct Med {
    id: String,
    person: String,
    person_name: String,
    name: String,
    times: Vec<Time>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Dose {
    med: String,
    person: String,
    person_name: String,
    name: String,
    date: String,
    time: String,
    status: Option<String>,
    at: Option<Timestamp>,
    /// When this medicine was last marked taken, for the double-dose check.
    last_taken: Option<Timestamp>,
}

#[derive(Debug, PartialEq)]
struct Alert {
    at: Timestamp,
    /// Unique per dose and reminder, so each is sent once.
    key: String,
    title: String,
    body: String,
}

fn hhmm(t: Time) -> String {
    format!("{:02}:{:02}", t.hour(), t.minute())
}

/// "08:00", "8:00" or "08:00:00".
fn parse_time(s: &str) -> Option<Time> {
    let mut it = s.trim().split(':');
    let h: i8 = it.next()?.parse().ok()?;
    let m: i8 = it.next()?.parse().ok()?;
    let sec: i8 = it.next().map_or(Some(0), |x| x.parse().ok())?;
    if it.next().is_some() {
        return None;
    }
    Time::new(h, m, sec, 0).ok()
}

fn text<'a>(v: &'a Value, ptr: &str) -> &'a str {
    v.pointer(ptr).and_then(Value::as_str).unwrap_or("")
}

/// Every medicine being taken now, with times, for everyone.
fn meds(store: &Store, today: Date) -> Vec<Med> {
    let mut out = Vec::new();
    for p in store.people().unwrap_or_default() {
        let person = text(&p, "/id").to_string();
        let person_name = text(&p, "/name/0/text").to_string();
        for r in store
            .records(&person, "MedicationStatement")
            .unwrap_or_default()
        {
            if text(&r, "/status") != "active" {
                continue;
            }
            // Ended before today (only a full date counts).
            if let Ok(end) = text(&r, "/effectivePeriod/end").parse::<Date>()
                && end < today
            {
                continue;
            }
            let mut times: Vec<Time> = r
                .pointer("/dosage/0/timing/repeat/timeOfDay")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|t| t.as_str().and_then(parse_time))
                .collect();
            times.sort();
            times.dedup();
            times.truncate(MAX_TIMES);
            if times.is_empty() {
                continue;
            }
            out.push(Med {
                id: text(&r, "/id").to_string(),
                person: person.clone(),
                person_name: person_name.clone(),
                name: text(&r, "/medicationCodeableConcept/text").to_string(),
                times,
            });
        }
    }
    out
}

fn dose_key(med: &str, time: Time) -> String {
    format!("{med}|{}", hhmm(time))
}

fn marked<'a>(log: &'a Log, date: Date, med: &str, time: Time) -> Option<&'a Mark> {
    log.get(&date.to_string())?.get(&dose_key(med, time))
}

fn last_taken(log: &Log, med: &str) -> Option<Timestamp> {
    let prefix = format!("{med}|");
    log.values()
        .flat_map(|day| day.iter())
        .filter(|(k, m)| k.starts_with(&prefix) && m.status == "taken")
        .map(|(_, m)| m.at)
        .max()
}

/// Today's doses for everyone, in time order.
fn doses(meds: &[Med], log: &Log, today: Date) -> Vec<Dose> {
    let mut out: Vec<Dose> = meds
        .iter()
        .flat_map(|m| {
            m.times.iter().map(move |&t| {
                let mark = marked(log, today, &m.id, t);
                Dose {
                    med: m.id.clone(),
                    person: m.person.clone(),
                    person_name: m.person_name.clone(),
                    name: m.name.clone(),
                    date: today.to_string(),
                    time: hhmm(t),
                    status: mark.map(|x| x.status.clone()),
                    at: mark.map(|x| x.at),
                    last_taken: last_taken(log, &m.id),
                }
            })
        })
        .collect();
    out.sort_by(|a, b| (&a.time, &a.person_name, &a.name).cmp(&(&b.time, &b.person_name, &b.name)));
    out
}

fn quiet(settings: &Settings, t: Time) -> bool {
    let Some((from, to)) = &settings.quiet else {
        return false;
    };
    let (Some(from), Some(to)) = (parse_time(from), parse_time(to)) else {
        return false;
    };
    if from <= to {
        from <= t && t < to
    } else {
        // Over midnight: 22:00 to 07:00.
        t >= from || t < to
    }
}

/// Reminders due from `from` up to (not including) `to`, for doses not yet
/// marked. `now` sets the time zone and the days looked at.
fn alerts(
    meds: &[Med],
    log: &Log,
    settings: &Settings,
    many_people: bool,
    now: &Zoned,
    from: Timestamp,
    to: Timestamp,
) -> Vec<Alert> {
    let mut out = Vec::new();
    if !settings.on {
        return out;
    }
    let tz = now.time_zone();
    let today = now.date();
    let first = today.yesterday().unwrap_or(today);
    let days = (to.duration_since(from).as_hours() / 24) + 3;
    for m in meds {
        for day in first.series(1.day()).take(days as usize) {
            for &t in &m.times {
                if marked(log, day, &m.id, t).is_some() {
                    continue;
                }
                let Ok(due) = day.to_datetime(t).to_zoned(tz.clone()) else {
                    continue;
                };
                let base = format!("{day}|{}", dose_key(&m.id, t));
                let nudge = due.timestamp() + NUDGE_AFTER;
                let nudge_local = nudge.to_zoned(tz.clone()).time();
                for (at, again) in [(due.timestamp(), false), (nudge, true)] {
                    if at < from || at >= to || (again && quiet(settings, nudge_local)) {
                        continue;
                    }
                    let (title, body) = words(m, hhmm(t), again, settings.names, many_people);
                    out.push(Alert {
                        at,
                        key: format!("{base}|{}", u8::from(again)),
                        title,
                        body,
                    });
                }
            }
        }
    }
    out.sort_by_key(|a| a.at);
    out
}

fn words(m: &Med, time: String, again: bool, names: bool, many_people: bool) -> (String, String) {
    let what = if names {
        let who = if many_people && !m.person_name.is_empty() {
            format!(" ({})", m.person_name)
        } else {
            String::new()
        };
        format!("{}{who}", m.name)
    } else {
        "a medicine".to_string()
    };
    if again {
        (
            "Medicine reminder".into(),
            format!(
                "The {time} dose of {what} is not marked yet. Open Arogya Swasthya to mark it taken or skipped."
            ),
        )
    } else {
        (
            "Medicine reminder".into(),
            format!("Time for {what} ({time}). Open Arogya Swasthya to mark it taken or skipped."),
        )
    }
}

/// A stable number for a reminder, as phones want.
#[cfg_attr(desktop, allow(dead_code))]
fn alert_id(key: &str) -> i32 {
    let h = Sha256::digest(key.as_bytes());
    i32::from_be_bytes([h[0] & 0x7f, h[1], h[2], h[3]])
}

fn settings_of(store: &Store) -> Settings {
    get(store, SETTINGS).unwrap_or_default()
}

fn log_of(store: &Store) -> Log {
    get(store, LOG).unwrap_or_default()
}

fn many_people(store: &Store) -> bool {
    store.people().map(|p| p.len() > 1).unwrap_or(false)
}

/// Mark a dose taken or skipped, or (`None`) clear the mark. Drops days
/// older than [`KEEP_DAYS`].
fn mark(
    log: &mut Log,
    today: Date,
    date: Date,
    med: &str,
    time: Time,
    status: Option<&str>,
    now: Timestamp,
) {
    let day = log.entry(date.to_string()).or_default();
    match status {
        Some(s) => {
            day.insert(
                dose_key(med, time),
                Mark {
                    status: s.into(),
                    at: now,
                },
            );
        }
        None => {
            day.remove(&dose_key(med, time));
        }
    }
    let oldest = today
        .checked_sub(KEEP_DAYS.days())
        .unwrap_or(today)
        .to_string();
    log.retain(|d, v| !v.is_empty() && *d >= oldest);
}

fn check_quiet(q: &Option<(String, String)>) -> CommandResult<()> {
    if let Some((a, b)) = q
        && (parse_time(a).is_none() || parse_time(b).is_none())
    {
        return Err("Quiet hours: use times like 22:00 and 07:00.".into());
    }
    Ok(())
}

// ---- commands ----

#[tauri::command]
pub fn reminder_settings(state: State<App>) -> CommandResult<Settings> {
    state.with(|s| Ok(settings_of(s)))
}

#[tauri::command]
pub async fn set_reminders(
    app: AppHandle,
    state: State<'_, App>,
    settings: Settings,
) -> CommandResult<()> {
    check_quiet(&settings.quiet)?;
    if settings.on {
        // Android 13 and later ask the user once.
        let asked = app.notification().request_permission().map_err(err)?;
        if asked != tauri_plugin_notification::PermissionState::Granted {
            return Err("Notifications are turned off for Arogya Swasthya. Allow them in the system settings, then try again.".into());
        }
    }
    state.with(|s| s.set_setting(SETTINGS, &settings))?;
    refresh(&app);
    Ok(())
}

/// Today's doses for everyone.
#[tauri::command]
pub fn doses_today(state: State<App>) -> CommandResult<Vec<Dose>> {
    let today = Zoned::now().date();
    state.with(|s| Ok(doses(&meds(s, today), &log_of(s), today)))
}

/// Mark one dose "taken" or "skipped", or clear it with `None`.
#[tauri::command]
pub fn mark_dose(
    app: AppHandle,
    state: State<App>,
    med: String,
    date: String,
    time: String,
    status: Option<String>,
) -> CommandResult<()> {
    let date: Date = date.parse().map_err(|_| "That dose has no valid date.")?;
    let time = parse_time(&time).ok_or("That dose has no valid time.")?;
    if let Some(s) = &status
        && s != "taken"
        && s != "skipped"
    {
        return Err("A dose is marked taken or skipped.".into());
    }
    let now = Zoned::now();
    state.with(|s| {
        let mut log = log_of(s);
        mark(
            &mut log,
            now.date(),
            date,
            &med,
            time,
            status.as_deref(),
            now.timestamp(),
        );
        s.set_setting(LOG, &log)
    })?;
    refresh(&app);
    Ok(())
}

/// Bring reminders up to date after medicines change.
#[tauri::command]
pub fn refresh_reminders(app: AppHandle) {
    refresh(&app);
}

/// Read what reminders need. Works while the app lock is on: reminders run
/// in the background and say nothing private unless the user chose names.
fn snapshot(app: &AppHandle) -> Option<(Settings, Vec<Med>, Log, bool)> {
    let state = app.state::<App>();
    let inner = state.lock_inner().ok()?;
    let store = inner.store.as_ref()?;
    let today = Zoned::now().date();
    Some((
        settings_of(store),
        meds(store, today),
        log_of(store),
        many_people(store),
    ))
}

/// Phones: schedule the next two days. Desktops: show or hide the tray icon
/// and turn start-at-login on or off.
pub fn refresh(app: &AppHandle) {
    let Some((settings, meds, log, many)) = snapshot(app) else {
        return;
    };
    #[cfg(mobile)]
    {
        // Off the main thread, which the phone's side of the plugin needs;
        // one at a time, so two refreshes do not mix their schedules.
        static ONE: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let app = app.clone();
        std::thread::spawn(move || {
            use tauri_plugin_notification::Schedule;
            let _one = ONE.lock();
            let n = app.notification();
            let _ = n.cancel_all();
            let now = Zoned::now();
            let from = now.timestamp() + SignedDuration::from_secs(5);
            for a in alerts(&meds, &log, &settings, many, &now, from, from + AHEAD) {
                let Ok(date) = jiff_to_time(a.at) else {
                    continue;
                };
                let _ = n
                    .builder()
                    .id(alert_id(&a.key))
                    .title(a.title)
                    .body(a.body)
                    .schedule(Schedule::At {
                        date,
                        repeating: false,
                        allow_while_idle: true,
                    })
                    .show();
            }
        });
    }
    #[cfg(desktop)]
    {
        let _ = (meds, log, many);
        desktop::apply(app, settings.on);
    }
}

#[cfg(mobile)]
fn jiff_to_time(t: Timestamp) -> Result<time::OffsetDateTime, time::error::ComponentRange> {
    time::OffsetDateTime::from_unix_timestamp(t.as_second())
}

#[cfg(desktop)]
pub mod desktop {
    //! The tray icon, start at login and the 30-second check.

    use std::sync::atomic::{AtomicBool, Ordering};

    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::TrayIconBuilder;
    use tauri_plugin_autostart::ManagerExt;

    use super::*;

    const TRAY: &str = "reminders";
    /// Started at login: stay in the tray.
    pub const BACKGROUND_ARG: &str = "--background";
    /// Closing the window hides it while reminders are on.
    pub static KEEP_RUNNING: AtomicBool = AtomicBool::new(false);

    pub fn show(app: &AppHandle) {
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.show();
            let _ = w.unminimize();
            let _ = w.set_focus();
        }
    }

    pub fn apply(app: &AppHandle, on: bool) {
        KEEP_RUNNING.store(on, Ordering::SeqCst);
        let auto = app.autolaunch();
        if auto.is_enabled().unwrap_or(false) != on {
            let _ = if on { auto.enable() } else { auto.disable() };
        }
        match (on, app.tray_by_id(TRAY)) {
            (true, None) => {
                let _ = tray(app);
            }
            (false, Some(_)) => {
                let _ = app.remove_tray_by_id(TRAY);
            }
            _ => {}
        }
    }

    fn tray(app: &AppHandle) -> tauri::Result<()> {
        let open = MenuItem::with_id(app, "open", "Open Arogya Swasthya", true, None::<&str>)?;
        let quit = MenuItem::with_id(app, "quit", "Quit (no more reminders)", true, None::<&str>)?;
        let menu = Menu::with_items(app, &[&open, &quit])?;
        let mut b = TrayIconBuilder::with_id(TRAY)
            .tooltip("Arogya Swasthya: medicine reminders on")
            .menu(&menu)
            .on_menu_event(|app, e| match e.id().as_ref() {
                "open" => show(app),
                "quit" => app.exit(0),
                _ => {}
            });
        if let Some(icon) = app.default_window_icon() {
            b = b.icon(icon.clone());
        }
        b.build(app)?;
        Ok(())
    }

    /// Check every 30 seconds; send each reminder once.
    pub fn start(app: AppHandle) {
        std::thread::spawn(move || {
            let mut sent = HashSet::new();
            loop {
                if let Some((settings, meds, log, many)) = snapshot(&app) {
                    let now = Zoned::now();
                    let to = now.timestamp() + SignedDuration::from_secs(1);
                    for a in alerts(&meds, &log, &settings, many, &now, to - STALE_AFTER, to) {
                        if sent.insert(a.key) {
                            let _ = app
                                .notification()
                                .builder()
                                .title(a.title)
                                .body(a.body)
                                .show();
                        }
                    }
                }
                std::thread::sleep(std::time::Duration::from_secs(30));
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn med(times: &[&str]) -> Med {
        Med {
            id: "m1".into(),
            person: "p1".into(),
            person_name: "Asha".into(),
            name: "Metformin 500 mg".into(),
            times: times.iter().map(|t| parse_time(t).unwrap()).collect(),
        }
    }

    fn at(s: &str) -> Zoned {
        format!("{s}[Asia/Kolkata]").parse().unwrap()
    }

    fn on() -> Settings {
        Settings {
            on: true,
            ..Settings::default()
        }
    }

    #[test]
    fn times_parse_with_or_without_seconds() {
        assert_eq!(parse_time("8:05"), Some(Time::constant(8, 5, 0, 0)));
        assert_eq!(parse_time("20:00:00"), Some(Time::constant(20, 0, 0, 0)));
        assert_eq!(parse_time("25:00"), None);
        assert_eq!(parse_time("8"), None);
        assert_eq!(parse_time("8:00:00:00"), None);
    }

    #[test]
    fn only_active_medicines_with_times_get_reminders() {
        let mut s = Store::memory().unwrap();
        let now = Timestamp::now();
        let p = s
            .save(
                serde_json::json!({"resourceType":"Patient","name":[{"text":"Asha"}]}),
                now,
            )
            .unwrap();
        let pid = p["id"].as_str().unwrap();
        let ms = |status: &str, times: Value, end: Option<&str>| {
            let mut r = serde_json::json!({
                "resourceType":"MedicationStatement","status":status,
                "subject":{"reference":format!("Patient/{pid}")},
                "medicationCodeableConcept":{"text":"X"},
                "dosage":[{"timing":{"repeat":{"timeOfDay":times}}}]
            });
            if let Some(e) = end {
                r["effectivePeriod"] = serde_json::json!({"end": e});
            }
            r
        };
        s.save(
            ms(
                "active",
                serde_json::json!(["20:00:00", "08:00:00", "08:00"]),
                None,
            ),
            now,
        )
        .unwrap();
        s.save(ms("stopped", serde_json::json!(["08:00:00"]), None), now)
            .unwrap();
        s.save(ms("active", serde_json::json!([]), None), now)
            .unwrap();
        s.save(
            ms(
                "active",
                serde_json::json!(["09:00:00"]),
                Some("2020-01-01"),
            ),
            now,
        )
        .unwrap();
        let found = meds(&s, "2026-10-30".parse().unwrap());
        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0].times,
            vec![Time::constant(8, 0, 0, 0), Time::constant(20, 0, 0, 0)]
        );
        assert_eq!(found[0].person_name, "Asha");
    }

    #[test]
    fn a_dose_reminds_then_nudges_once_unless_marked() {
        let m = [med(&["08:00"])];
        let now = at("2026-10-30T07:00");
        let day_end = now.timestamp() + SignedDuration::from_hours(12);
        let got = alerts(
            &m,
            &Log::new(),
            &on(),
            false,
            &now,
            now.timestamp(),
            day_end,
        );
        let times: Vec<String> = got
            .iter()
            .map(|a| a.at.to_zoned(now.time_zone().clone()).time().to_string())
            .collect();
        assert_eq!(times, ["08:00:00", "08:30:00"]);
        assert!(got[0].body.contains("Time for a medicine (08:00)"));
        assert!(
            !got[0].body.contains("Metformin"),
            "names stay hidden by default"
        );

        let mut log = Log::new();
        let today = now.date();
        mark(
            &mut log,
            today,
            today,
            "m1",
            Time::constant(8, 0, 0, 0),
            Some("taken"),
            now.timestamp(),
        );
        assert!(alerts(&m, &log, &on(), false, &now, now.timestamp(), day_end).is_empty());
        assert!(
            alerts(
                &m,
                &Log::new(),
                &Settings::default(),
                false,
                &now,
                now.timestamp(),
                day_end
            )
            .is_empty()
        );
    }

    #[test]
    fn quiet_hours_stop_the_nudge_but_not_the_reminder() {
        let m = [med(&["22:15"])];
        let now = at("2026-10-30T21:00");
        let s = Settings {
            quiet: Some(("22:30".into(), "07:00".into())),
            ..on()
        };
        let got = alerts(
            &m,
            &Log::new(),
            &s,
            false,
            &now,
            now.timestamp(),
            now.timestamp() + SignedDuration::from_hours(3),
        );
        assert_eq!(got.len(), 1);
        assert!(got[0].key.ends_with("|0"));
        assert!(quiet(&s, Time::constant(3, 0, 0, 0)));
        assert!(!quiet(&s, Time::constant(12, 0, 0, 0)));
    }

    #[test]
    fn names_show_only_when_chosen_and_people_only_when_several() {
        let m = med(&["08:00"]);
        assert!(
            words(&m, "08:00".into(), false, true, false)
                .1
                .contains("Time for Metformin 500 mg (08:00)")
        );
        assert!(
            words(&m, "08:00".into(), true, true, true)
                .1
                .contains("Metformin 500 mg (Asha) is not marked")
        );
    }

    #[test]
    fn a_dose_just_after_midnight_counts_yesterdays_late_dose() {
        let m = [med(&["23:50"])];
        let now = at("2026-10-31T00:05");
        let got = alerts(
            &m,
            &Log::new(),
            &on(),
            false,
            &now,
            now.timestamp() - STALE_AFTER,
            now.timestamp() + SignedDuration::from_secs(1),
        );
        assert_eq!(got.len(), 1);
        assert!(got[0].key.starts_with("2026-10-30|m1|23:50|0"));
    }

    #[test]
    fn todays_doses_show_marks_and_the_last_time_taken() {
        let m = [med(&["20:00", "08:00"])];
        let now = at("2026-10-30T09:00");
        let today = now.date();
        let mut log = Log::new();
        mark(
            &mut log,
            today,
            today,
            "m1",
            Time::constant(8, 0, 0, 0),
            Some("taken"),
            now.timestamp(),
        );
        let d = doses(&m, &log, today);
        assert_eq!(
            d.iter().map(|x| x.time.as_str()).collect::<Vec<_>>(),
            ["08:00", "20:00"]
        );
        assert_eq!(d[0].status.as_deref(), Some("taken"));
        assert_eq!(d[1].status, None);
        assert_eq!(d[1].last_taken, Some(now.timestamp()));

        // Clearing undoes it; old days are dropped.
        mark(
            &mut log,
            today,
            today,
            "m1",
            Time::constant(8, 0, 0, 0),
            None,
            now.timestamp(),
        );
        assert!(log.is_empty());
        mark(
            &mut log,
            today,
            "2026-01-01".parse().unwrap(),
            "m1",
            Time::constant(8, 0, 0, 0),
            Some("taken"),
            now.timestamp(),
        );
        assert!(log.is_empty(), "more than 90 days old");
    }

    #[test]
    fn reminder_ids_are_stable_and_positive() {
        assert_eq!(alert_id("a"), alert_id("a"));
        assert!(alert_id("2026-10-30|m1|08:00|0") >= 0);
        assert_ne!(alert_id("x|0"), alert_id("x|1"));
    }

    #[test]
    fn quiet_hours_must_be_times() {
        assert!(check_quiet(&Some(("22:00".into(), "7am".into()))).is_err());
        assert!(check_quiet(&Some(("22:00".into(), "07:00".into()))).is_ok());
        assert!(check_quiet(&None).is_ok());
    }
}
