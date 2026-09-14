# Tenth

![Tenth logo](assets/tenth-mark.svg)

**Time, billed cleanly.**

Tenth is a minimal native desktop application for tracking consulting time by project. It keeps exact elapsed time and rounds billable time up to 0.1-hour (6-minute) increments.

## Run it

Install Rust, then from this folder run:

```sh
cargo run --release
```

The first build downloads and compiles the GUI dependencies, so it takes longer than later launches.

## How it works

1. Select **+ New project** to add a project.
2. Select the project and click **Start timer**.
3. Click **Stop & save** when finished.
4. Open **Timesheet** to review Saturday–Friday totals and entry notes, edit an entry with its clearly labeled **View / edit** action, add missed time manually, copy a summary, or export a CSV timesheet.

Add an optional note before starting a timer or when logging time manually. Notes stay attached to their individual entries, appear in recent and weekly entry lists, and can be updated from the entry editor. Copied summaries and CSV exports include any notes logged that week.

Only one timer can run at a time. The active timer is saved immediately, so it continues correctly if the application is closed and opened again.

Use the theme control at the right side of the top bar to switch between light and dark appearances. Tenth remembers the selection for the next launch.

On macOS, Tenth lives in the menu bar while the app is running. The menu-bar title shows the live elapsed time, and its menu lets you start the selected project, stop and save, or restore the main window. Select **Compact timer ↗** below the running timer to show a translucent always-on-top reminder with the same essential controls on all platforms. On Windows and Linux, native minimization also opens the reminder. On macOS, the native yellow minimize button retains menu-bar tracking; use **Compact timer ↗** for the floating reminder. The reminder opens near the main window on its current display and can be dragged to another screen. It becomes more opaque on hover or focus; **Restore app** returns to the full window. macOS retains the menu-bar controls as well.

The Timer view shows today’s saved billable hours across all projects and for the selected project. Editing only an entry’s note or project preserves the original exact timestamps and elapsed duration.

## Data storage

Data is stored as a readable JSON file in the operating system's per-user application-data directory. Tenth intentionally keeps the original Hour Tracker location so existing installations retain their data after upgrading:

- macOS: `~/Library/Application Support/com.HourTracker.Hour-Tracker/tracker.json`
- Windows: `%LOCALAPPDATA%\\HourTracker\\Hour Tracker\\data\\tracker.json`
- Linux: `$XDG_DATA_HOME/hour-tracker/tracker.json` (or `~/.local/share/hour-tracker/tracker.json`)

Each save uses a temporary file and rename to reduce the chance of a partial write. Set `TENTH_DATA_FILE` to an absolute JSON path to use a separate data file, for example when testing the UI without touching your timesheets.

## Brand

The name, logo rationale, palette, and usage guidance live in [BRAND.md](BRAND.md). The production-ready vector mark is in [assets/tenth-mark.svg](assets/tenth-mark.svg).

Weekly CSV reports are written to a `reports` folder next to `tracker.json`. The app shows the exact path after export.

## Checks

```sh
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```
