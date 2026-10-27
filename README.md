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

The bundled database server (MariaDB) is included only in the Windows x64 packages. The Windows installer can install it as the Windows service `RadioMoonDB` (port 3307, local connections only, starts with Windows, data in `%PROGRAMDATA%\RadioMoon\data`); the service is removed when the program is uninstalled, the data is kept. On every other platform RadioMoon uses a MySQL or MariaDB server that you install (Linux/macOS: `mariadb-server` or `mysql-server`); enter its address in **Control Panel > Database server**.

## Server for headless Linux (web studio)

`RadioMoon-Server-<version>-linux-glibc-<x64|arm64>.tar.gz` runs on Debian 10+, Ubuntu 20.04+, Raspberry Pi OS 64-bit, AlmaLinux / Rocky / RHEL 8+ and Fedora; the `-alpine-` package is for Alpine. Unpack it and run `sudo ./install.sh`: it installs MariaDB, creates the database, installs the service (systemd, OpenRC on Alpine) and starts it.
To upgrade later, unpack the new package and run `sudo ./install.sh upgrade` (settings, database and songs are kept). To remove: `sudo ./install.sh uninstall` (keeps the data and the database) or `sudo ./install.sh uninstall --purge` (removes everything).
Open `http://<server>:8080/` in a browser. User `admin`; the password is in `/var/lib/radiomoon/webserver.json` (created at the first start). Settings (`RADIOMOON_BIND`) are in `/etc/radiomoon/server.env`.
Run by hand: `radiomoon-server --bind 0.0.0.0:8080 --data /var/lib/radiomoon --db-user radiomoon --db-password ...`.

The server needs no sound card (it can run in a container or a virtual machine and send only the internet stream). A watchdog restarts it if it ever stops responding, and the installer tunes MariaDB for it.

**RadioMoon client** (`radiomoon-client`, in the Windows setup as "RadioMoon Client"): the same studio and control panel as the desktop program, but the server plays. Nothing else is needed than the address of the server (`http://server:8080`), a user name and a password: the database of the server is reached through the server itself (no database port is opened).

- **Login window:** centred card with *Remember password* and a drop-down list of the servers used before. The window opens maximized.
- **Connection:** the combo box at the bottom of the studio (and in the login window) chooses *Normal*, *Low bandwidth* or *Very low bandwidth*: the client asks the server less often and does not download waveforms. The status bar shows the download / upload rate.
- **One folder per server:** the settings and the cache of each server are separate, so two servers never show each other's data.
- **Uploads:** songs added in the client are uploaded to the server, into one folder per library inside `media` (`songs`, `spots`, `jingles`, `assets`, `sweepers`, `voice`). CLAP plugins are uploaded to the server too.
- **Light traffic:** the server does the heavy work on songs: normalize volume (in the background), the waveform (a few KB), auto cue and reading tags. A song is downloaded only for the preview.
- **Cache:** songs downloaded to listen to or to draw the waveform are kept in a local cache. Files older than a day are removed automatically and the cache stays under 1 GB; **Control Panel > Reset database** shows its size and has a *Clear the cache now* button.
- Closing the client never stops the music or the internet broadcast of the server.

## Portable or installed

- **Portable:** the folder contains `portable.flag`. All data (database, backups, reports, settings, `config.json`) stays in the program folder. Copy the folder to move the station.
- **Installed:** no `portable.flag`. Data is kept per user in `%PROGRAMDATA%\RadioMoon` (Windows; the program itself is in Program Files), `~/Library/Application Support/RadioMoon` (macOS) or `~/.local/share/RadioMoon` (Linux). Uninstalling or upgrading never deletes it.

## Versioning and releases

The version is in `Cargo.toml` and is shown in the header and in **Control Panel > General settings > About**. The same version is stored in the database.

- An older database opened by a newer program: a safety backup (`pre-upgrade-<old>-to-<new>.rmbak`) is saved in the backups folder, then the database is upgraded. No data is lost.
- A newer database opened by an older program is refused with a message.

To publish a release (automatic): change `version` in `Cargo.toml`, add its `## x.y.z` section to `CHANGELOG.md` and push to `main`. The *auto-tag* workflow creates the tag and the release and starts the build. (In *Settings > Actions > General > Workflow permissions* choose *Read and write*.) By hand, instead:

1. Change `version` in `Cargo.toml` and add the changes to `CHANGELOG.md`.
2. Commit, then create the tag `vX.Y.Z` (on GitHub: Releases > Draft a new release > new tag) matching the version.
3. The Release workflow builds every platform, attaches the packages to that release and replaces its description with the matching `## x.y.z` section of `CHANGELOG.md`. It never creates a release itself; if it cannot find one it stops with a message (create the release, then re-run the workflow).

Developers: the database layout is in the `sql/` folder: `sql/schema.sql` (latest full layout) and `sql/migrations/NNNN_name.sql` (upgrade steps). Both are embedded in the program; register new migrations in `src/schema.rs` (see `sql/migrations/README.md`).

## Portable folder layout (Windows x64)

`RadioMoon.exe`, `portable.flag`, `lang\`, `mariadb\` (bundled database server), `data\` (created on first run), `backups\`, `reports\`, `export\`, `logs\`, `config.json` (encrypted).

The bundled database listens only on `127.0.0.1:3307`. Use another host in Control Panel > Database server to share one database between workstations.

## Languages

English and Greek are built in. To add a language, copy `lang\template.json` to `lang\xx.json`, translate it and choose it in Control Panel > General settings. See `lang\README.txt`.

## Internet broadcast

Control Panel > Internet broadcast: add several servers, each Icecast 2 (host, port, mount, user, password), Shoutcast v1 (host, port, password) or Shoutcast v2 (host, port, password, stream ID). The Shoutcast source connects to port+1. MP3, 48-320 kbps. The *Show password* box next to the password shows it while you type. The title of the playing song is sent to every server. Everything played on the main output is in the stream; the preview (monitor) output is not.

## FM / MPX

The FM multiplex programs (MPX Transmitter and MPX Receiver: stereo, RDS, L24 or lossless FLAC over IP) are a separate project since RadioMoon 0.6.0: [radiomoon-mpx](https://github.com/nikosbatsikas/radiomoon-mpx). To put RadioMoon on air, enable its internet broadcast and use that stream as the MPX Transmitter's source; the song titles RadioMoon sends to the server become the RDS RadioText.

## Import from Jazler RadioStar 2 / MegaMix

Control Panel > Utilities > Import from Jazler / MegaMix. The type of the database is detected automatically; for MegaMix the songs, jingles / effects, commercials, artists, styles / categories / eras and playlists are imported (the music sequence rules are not converted). Select the old `.mdb` database (Access 2000-2003), press **Scan file**, choose what to import (songs, spots and campaigns, playlists, jingle pallets, play history) and press **Start import**.

- If the audio files are on another drive or folder, use the old/new path-start fields.
- On a client, tick the option to **upload the files to the server and change the paths**: every imported file is copied to the server and the library points to the copy.
- Existing data is never overwritten: known file paths and same-named categories, playlists, campaigns and pallets are skipped.
- User passwords are not imported. Newer Access formats must first be saved as Access 2000.

## Backups

Control Panel > Backup & restore: a daily automatic backup (default 03:00, keeps 14), "Back up now", and restore from any `.rmbak` file. A backup contains libraries, schedules, settings and play history (not the audio files). Restoring first saves a safety copy (`before-restore.rmbak`).

## Security note

`config.json` is stored encrypted so the database password is not readable in a text editor. The key is built into the program: this protects against casual reading, not against someone who has the program.

## Studio screen

- **Header:** AUTO / MANUAL / OFFLINE, layout (classic / simple / wide), Control Panel, volume.
- **Deck:** next, countdown, waveform with mix marks, VU, Restart / Loop / Pause / StopNext, and a neon clock (round minutes outside, seconds ring inside).
- **Left:** playlist. The rows have the colours of the library tabs (songs blue, spots green, jingles yellow, assets grey, sweepers purple, voice red). Double-click plays, headphones = preview, Move Up/Down, "EXACT TIME" items. Delete removes the selected item from the playlist (the library record stays).
- **Centre:** database tabs (Songs, Spots, Jingles, Assets, Sweepers, Voice, Memos).
- **Right:** instant jingle pallets (right-click a button to assign / clear / colour) and Central (history, upcoming ad breaks, problems, memos).
- **Default shortcuts:** F1 next, F2 pause, F3 stop, F4 restart, F5 loop, F6 stopnext, F7 stop jingles.

## Control Panel

| Group | Contents |
|---|---|
| Libraries | Songs (3 categories + vocal + properties, intro, mix points, hooks, volume, song separation, playlister code, mass change, normalize, statistics), Spots (campaigns, breaks, priorities, playlister import), Jingles, Assets, Sweepers, Voice tracks, Memos, Instant jingle pallets, Stitcher |
| Scheduling | Main auto schedule (clocks, weekday x half-hour grid, randomness, artist separation, next-day separation, restrictive rules, classic auto), Playlists, Force clocks, Live rebroadcast, Final log creator, Log viewer |
| Settings | General, Sound outputs, Effects (CLAP), Shortcuts, Users & security, Audio folder mappings, Internet broadcast, Backup & restore, Database server |
| Utilities | Internet updater files, Import from Jazler / MegaMix, Integrity check, Broadcasting reports, Reset database |

- **Main auto schedule:** pick a brush in the palette on the left (a clock or a playlist, with a search box; *none* erases) and click or drag over the cells of the grid. Click a day name to fill the day, an hour label to fill that hour on every day.
- **Station IDs:** make a campaign with the spot of the station ID and give it priority *1 Station ID* on the break times; it plays first in the break. Or put a *jingle* row in a clock.
- **Delete from a library:** you are asked whether the audio files are deleted too.
- **Reset database:** deletes the selected libraries, or everything except the settings.
- **Effects (CLAP):** loads `.clap` plugins on the main output (on a server the plugins are uploaded to it). If a plugin does not load, the reason is shown in the list.

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
- The live audio is mixed into the main output, so it is also in the internet broadcast. The player shows a blinking **LIVE** with the source name. With *Stop automation* the playlist stops and the studio goes to MANUAL while the source is live; afterwards the previous mode returns and AUTO continues with the next item.

## Not included

Weather, VST plug-ins (CLAP plug-ins are supported), microphone recording of voice tracks, a client for Windows 7.
