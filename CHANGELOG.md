# Changelog

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
