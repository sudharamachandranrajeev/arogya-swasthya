# Changelog

## [Unreleased]

The records vault:

- Documents for each person: letters, reports, scans, photos and recordings, each file encrypted
  with its own key. Look at them inside the app, save a copy out, edit what they are, delete with
  Undo.
- Search across records and the words in documents. PDFs with their own text are searchable at
  once. Photos and scanned PDFs are read by OCR, and recordings transcribed (desktop), only after
  you agree to a one-time model download; everything runs on the device.
- Import a FHIR R4 or IPS file, a C-CDA document, or a SMART Health Link (with passcode). Every
  record goes to a review list first, with ones you already have marked and unticked. One Undo
  removes the whole import.

Tracking and reports:

- Track vital signs (blood pressure, pulse, weight, temperature, oxygen level, blood sugar,
  breathing rate, height, waist) several at once, and symptoms on a 0 to 10 scale. Readings
  outside plausible limits are refused as typing slips.
- Trends: a chart for every reading, result and symptom recorded at least twice, with the lab's
  normal range shaded, a readout by mouse or arrow keys, and a table view. Patterns shows
  readings that changed together, with how many days it rests on and how strong it is, and says
  it does not show a cause.
- Medicine reminders: times on each medicine, a Today list to mark doses taken or skipped, a
  second reminder 30 minutes later, quiet hours, and no medicine names unless you choose. A
  second "taken" within two hours asks first. On a computer the app stays in the tray and starts
  at login while reminders are on; phones use system alarms for the next 48 hours.
- Calculators: BMI (with the lower NICE cut-offs), waist to height, eGFR (CKD-EPI 2021, no race
  term), pack-years and alcohol units, filled in from your records.
- Doctor report: a PDF with the sections and period you choose and a note, made on the device
  with Typst.
- Export a patient summary (IPS) or every record as a FHIR Bundle from Settings → Export.
  Import reads NDJSON files too.

## [0.1.0]

The first release (alpha). Installers for Windows, Linux and Android.

Releases and updates:

- Windows installer (no admin rights needed) and a portable zip. Linux AppImage and Arch package.
  Android APK. Every release lists SHA-256 checksums.
- Checks for a new version at start and asks before installing. Backs up your records first.
  Every download is checked against the project's signature (desktop) or checksum and signing
  key (Android).
- Go back to the version before the last update (desktop), restoring the matching backup if
  needed.
- The portable version tells you when a new version is out, since it cannot update itself.
- Restore a backup: daily, made by you, before an update, before a restore, or a file from
  anywhere. The app shows what it holds (people, records, last change) before replacing
  anything, and backs up your current records first. A backup from another device opens with
  that device's recovery key.
- Report a problem / Suggest a feature: opens a GitHub form with version, system, look settings
  and the last three error messages filled in, with words from your records taken out.
- First-run step: check for updates, or not.

The foundation:

- Records for several people: conditions, allergies, procedures, vaccines, medicines and test
  results, stored as FHIR R4. Suggestions as you type, dates as a day, a month or a year.
- Results shown against the lab's own normal range.
- Overview and timeline for each person.
- Emergency numbers for the country you live in or are visiting, with your own corrections.
- Encrypted database (SQLCipher) with a key in the system keyring or Android Keystore, a recovery
  key shown once at setup, PIN lock with growing waits, fingerprint or face on phones.
- Daily encrypted backups (14 kept), a second folder on computers, and Save a backup as….
- 30-day trash and Undo.
- Sage, Aurora and Journal themes, light or dark, calm movement, and a theme editor that checks
  contrast.
