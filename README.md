# RadioMoon

Portable radio automation in Rust with a MariaDB/MySQL backend. Windows, Linux and macOS; Greek and English interface.

## Build locally (Windows)

1. Install Rust from <https://rustup.rs> (MSVC toolchain; Visual Studio Build Tools needed).
2. Download the MariaDB ZIP package for Windows x64 from <https://mariadb.org/download/>.
3. In PowerShell, in this folder:
   ```powershell
   .\build_portable.ps1 -MariaDbZip C:\path\to\mariadb-xx.x.x-winx64.zip
   ```
4. Result: `dist\RadioMoon-Portable` (folder) and `dist\RadioMoon-Portable.zip`.

No Rust on your PC? Push this folder to a GitHub repository and let the Release workflow build everything.

## Packages

Made automatically by the Release workflow.

| Platform | Architectures | Portable | Installer |
|---|---|---|---|
| Windows | x64, x86, arm64 | `RadioMoon-<ver>-windows-<arch>-portable.zip` | `...-setup.exe` |
| Linux | x64, arm64 | `RadioMoon-<ver>-linux-<arch>-portable.tar.gz` | `.deb`, `.rpm` |
| macOS | arm64 | `RadioMoon-<ver>-macos-arm64-portable.zip` | `.dmg` |

The bundled database server (MariaDB) is included only in the Windows x64 packages. On every other platform RadioMoon uses a MySQL or MariaDB server that you install (Linux/macOS: `mariadb-server` or `mysql-server`); enter its address in **Control Panel > Database server**.

## Portable or installed

- **Portable:** the folder contains `portable.flag`. All data (database, backups, reports, settings, `config.json`) stays in the program folder. Copy the folder to move the station.
- **Installed:** no `portable.flag`. Data is kept per user in `%PROGRAMDATA%\RadioMoon` (Windows; the program itself is in Program Files), `~/Library/Application Support/RadioMoon` (macOS) or `~/.local/share/RadioMoon` (Linux). Uninstalling or upgrading never deletes it.

## Versioning and releases

The version is in `Cargo.toml` and is shown in the header and in **Control Panel > General settings > About**. The same version is stored in the database.

- An older database opened by a newer program: a safety backup (`pre-upgrade-<old>-to-<new>.rmbak`) is saved in the backups folder, then the database is upgraded. No data is lost.
- A newer database opened by an older program is refused with a message.

To publish a release:

1. Change `version` in `Cargo.toml` and add the changes to `CHANGELOG.md`.
2. Commit, then create the tag `vX.Y.Z` (on GitHub: Releases > Draft a new release > new tag) matching the version.
3. The Release workflow builds every platform, attaches the packages to that release and replaces its description with the matching `## x.y.z` section of `CHANGELOG.md`. It never creates a release itself; if it cannot find one it stops with a message (create the release, then re-run the workflow).

Developers: the database layout is in the `sql/` folder: `sql/schema.sql` (latest full layout) and `sql/migrations/NNNN_name.sql` (upgrade steps). Both are embedded in the program; register new migrations in `src/schema.rs` (see `sql/migrations/README.md`).

## Portable folder layout (Windows x64)

`RadioMoon.exe`, `RadioMoon-MPX-Receiver.exe`, `portable.flag`, `lang\`, `mariadb\` (bundled database server), `data\` (created on first run), `backups\`, `reports\`, `export\`, `logs\`, `config.json` (encrypted).

The bundled database listens only on `127.0.0.1:3307`. Use another host in Control Panel > Database server to share one database between workstations.

## Languages

English and Greek are built in. To add a language, copy `lang\template.json` to `lang\xx.json`, translate it and choose it in Control Panel > General settings. See `lang\README.txt`.

## Internet broadcast

Control Panel > Internet broadcast: add several servers, each Icecast 2 (host, port, mount, user, password) or Shoutcast v1 (host, port, password; the source connects to port+1). MP3, 48-320 kbps. Everything played on the main output is in the stream; the preview (monitor) output is not.

## MPX / FM multiplex

Control Panel > MPX: generates the FM composite signal (stereo pilot, 38 kHz, RDS) and sends it to a 192 kHz sound card and/or over IP (RTP L24 over UDP, or TCP). The separate program `RadioMoon-MPX-Receiver` receives the IP stream and plays it on a sound card at the transmitter site.

Both the generator (Control Panel > MPX > Signal analysis) and the receiver show an analyzer: the spectrum from 0 to 96 kHz with markers at 19, 38 and 57 kHz, the waveform of the composite signal, the levels of the pilot, RDS, L+R and L-R, and the current and maximum peak deviation in kHz and % (100% = 75 kHz). *Reset* clears the maximum.

The receiver also runs on a **Raspberry Pi 3/4/5 with 64-bit Raspberry Pi OS**: download `RadioMoon-MPX-Receiver-<ver>-linux-arm64.tar.gz`, extract it and run `./radiomoon-mpx-receiver` (needs a desktop session and `libasound2`). A 192 kHz capable USB or HAT sound card is required for the composite output.

## Import from Jazler RadioStar 2

Control Panel > Utilities > Import from Jazler RadioStar 2. Select the old `.mdb` database (Access 2000-2003), press **Scan file**, choose what to import (songs, spots and campaigns, playlists, jingle pallets, play history) and press **Start import**.

- If the audio files are on another drive or folder, use the old/new path-start fields.
- Existing data is never overwritten: known file paths and same-named categories, playlists, campaigns and pallets are skipped.
- User passwords are not imported. Newer Access formats must first be saved as Access 2000.

## Backups

Control Panel > Backup & restore: a daily automatic backup (default 03:00, keeps 14), "Back up now", and restore from any `.rmbak` file. A backup contains libraries, schedules, settings and play history (not the audio files). Restoring first saves a safety copy (`before-restore.rmbak`).

## Security note

`config.json` is stored encrypted so the database password is not readable in a text editor. The key is built into the program: this protects against casual reading, not against someone who has the program.

## Studio screen

- **Header:** AUTO / MANUAL / OFFLINE, layout (classic / simple / wide), Control Panel, volume.
- **Deck:** next, countdown, waveform with mix marks, VU, clock, Restart / Loop / Pause / StopNext.
- **Left:** playlist. Double-click plays, headphones = preview, Move Up/Down, "EXACT TIME" items. Delete removes the selected item from the playlist (the library record stays).
- **Centre:** database tabs (Songs, Spots, Jingles, Assets, Sweepers, Voice, Memos).
- **Right:** instant jingle pallets (right-click a button to assign / clear / colour) and Central (history, upcoming ad breaks, problems, memos).
- **Default shortcuts:** F1 next, F2 pause, F3 stop, F4 restart, F5 loop, F6 stopnext, F7 stop jingles.

## Control Panel

| Group | Contents |
|---|---|
| Libraries | Songs (3 categories + vocal + properties, intro, mix points, hooks, volume, song separation, playlister code, mass change, normalize, statistics), Spots (campaigns, breaks, priorities, playlister import), Jingles, Assets, Sweepers, Voice tracks, Memos, Instant jingle pallets, Stitcher |
| Scheduling | Main auto schedule (clocks, 7x24 grid, randomness, artist separation, next-day separation, restrictive rules, classic auto), Playlists, Force clocks, Final log creator, Log viewer |
| Settings | General, Sound outputs, Shortcuts, Users & security, Audio folder mappings, Internet broadcast, MPX, Backup & restore, Database server |
| Utilities | Internet updater files, Import from Jazler RadioStar 2, Integrity check, Broadcasting reports, Reset database |

## First steps

1. Control Panel > Libraries > Songs: import a folder (tick "auto cue" for mix points). Do the same for Jingles and Spots.
2. Scheduling > Main auto schedule: create a clock (rows of songs/jingles/...) and paint it into the weekday x hour grid.
3. Spots > Add new campaign; program it on the break times.
4. Header: press **AUTO**. The next 2 hours are generated and played with automatic mixing.

## Live rebroadcast

Control Panel > Scheduling > Live rebroadcast.

- **Sources:** add as many as you need. Each is a sound card input, or an internet radio stream (enter a URL: MP3 or AAC over HTTP/HTTPS; HLS is not supported), with its own level. *Go live* puts it on the air at once.
- **Schedule:** each entry picks a source, the days, a start and an end time (HH:MM, over midnight allowed) and whether to mute the automation. The source goes on the air at the start and off at the end.
- **LIVE button** in the studio header: click to start or stop the selected source; right-click to choose another. Stopping a scheduled source by hand keeps it off until its slot ends.
- The live audio is mixed into the main output, so it is also in the internet broadcast and MPX. The player shows a blinking **LIVE** with the source name. With *Stop automation* the playlist stops and the studio goes to MANUAL while the source is live; afterwards the previous mode returns and AUTO continues with the next item.

## Not included

Weather, VST plug-ins, microphone recording of voice tracks.
