# RadioMoon

Radio automation in Rust with a MariaDB/MySQL database. Windows, Linux and macOS; Greek and English interface (more languages can be added).

RadioMoon plays your songs, jingles, spots and sweepers by clocks and a weekly schedule, mixes them automatically, sends the result to the internet (Icecast / Shoutcast) and keeps the logs and reports of the station. It runs as one program on a PC, or as a **server** (no screen, no sound card needed) that you control from a browser or from the **RadioMoon client** on any computer.

- [Download](#download) · [Server](#server-for-headless-linux-and-the-client) · [Studio](#studio-screen) · [Control Panel](#control-panel) · [Internet broadcast](#internet-broadcast) · [Backups](#backups) · [Releases](#versioning-and-releases) · [What's new](#whats-new)

## Documentation

The detailed manual is the **[Wiki](https://github.com/nikosbatsikas/radiomoon/wiki)** (the source is the `wiki/` folder): installation, the server edition, the client and web studio, scheduling, internet broadcast, two-step login, backups, updating, and the design guides for running a radio in the cloud: *Cloud Hosting Design*, *Hosting Several Stations* and *Internet Radio Design*.

## Download

Get the newest version from the **Releases** page of the repository. The last 10 good releases are kept; older ones are removed automatically (see [Versioning and releases](#versioning-and-releases)).

| Platform | Package | Files |
|---|---|---|
| Windows x64, x86, arm64 | installer and portable | `RadioMoon-<ver>-windows-<arch>-setup.exe`, `RadioMoon-<ver>-windows-<arch>-portable.zip` |
| Linux x64, arm64 | portable, `.deb`, `.rpm` | `RadioMoon-<ver>-linux-<arch>-portable.tar.gz`, `radiomoon_<ver>-1_<arch>.deb`, `radiomoon-<ver>-1.<arch>.rpm` |
| macOS arm64 | disk image | `RadioMoon-<ver>-macos-arm64.dmg` |
| Linux server (headless) | tar.gz with installer | `RadioMoon-Server-<ver>-linux-glibc-<x64\|arm64>.tar.gz`, `RadioMoon-Server-<ver>-linux-alpine-<x64\|arm64>.tar.gz` |

The packages are made automatically by the *release* workflow.

The bundled database server (MariaDB) is included only in the Windows x64 packages. The Windows installer can install it as the Windows service `RadioMoonDB` (port 3307, local connections only, starts with Windows, data in `%PROGRAMDATA%\RadioMoon\data`); the service is removed when the program is uninstalled, the data is kept. On every other platform RadioMoon uses a MySQL or MariaDB server that you install (Linux/macOS: `mariadb-server` or `mysql-server`); enter its address in **Control Panel > Database server**.

### Portable or installed

- **Portable:** the folder contains `portable.flag`. All data (database, backups, reports, settings, `config.json`) stays in the program folder. Copy the folder to move the station.
- **Installed:** no `portable.flag`. Data is kept per user in `%PROGRAMDATA%\RadioMoon` (Windows; the program itself is in Program Files), `~/Library/Application Support/RadioMoon` (macOS) or `~/.local/share/RadioMoon` (Linux). Uninstalling or upgrading never deletes it.

Portable folder layout (Windows x64): `RadioMoon.exe`, `portable.flag`, `lang\`, `mariadb\` (bundled database server, listens only on `127.0.0.1:3307`), `data\` (created on first run), `backups\`, `reports\`, `export\`, `logs\`, `config.json` (encrypted). To share one database between workstations, use another host in Control Panel > Database server.

## Server for headless Linux and the client

`RadioMoon-Server-<version>-linux-glibc-<x64|arm64>.tar.gz` runs on Debian 10+, Ubuntu 20.04+, Raspberry Pi OS 64-bit, AlmaLinux / Rocky / RHEL 8+ and Fedora; the `-alpine-` package is for Alpine. Unpack it and run `sudo ./install.sh`: it installs MariaDB (tuned for the server), creates the database, installs the service (systemd, OpenRC on Alpine) and starts it.

- **Upgrade:** unpack the new package and run `sudo ./install.sh upgrade` (settings, database and songs are kept).
- **Remove:** `sudo ./install.sh uninstall` (keeps the data and the database) or `sudo ./install.sh uninstall --purge` (removes everything).
- **Web studio:** open `http://<server>:8080/` in a browser. User `admin`; the password is in `/var/lib/radiomoon/webserver.json` (created at the first start). Settings (`RADIOMOON_BIND`) are in `/etc/radiomoon/server.env`.
- **By hand:** `radiomoon-server --bind 0.0.0.0:8080 --data /var/lib/radiomoon --db-user radiomoon --db-password ...`.
- The server needs **no sound card**: it can run in a container (LXC) or a virtual machine and only send the internet stream. A watchdog restarts it if it ever stops responding.

**RadioMoon client** (`radiomoon-client`; in the Windows setup it is "RadioMoon Client"): the same studio and control panel as the desktop program, but the server plays. You only need the address of the server (`http://server:8080`), a user name and a password. The database of the server is reached through the server itself: no database port is opened.

- **Login window:** *Remember password* and a drop-down list of the servers used before. The window opens maximized.
- **Connection:** the combo box in the login window and at the bottom of the studio chooses *Normal*, *Low bandwidth* or *Very low bandwidth*; the status bar shows the download / upload rate.
- **One folder per server:** the settings and the cache of each server are separate, so two servers never show each other's data.
- **Uploads:** songs added in the client are uploaded to the server, into one folder per library inside `media` (`songs`, `spots`, `jingles`, `assets`, `sweepers`, `voice`). CLAP plugins are uploaded too.
- **Light traffic:** the server does the heavy work on songs: *normalize volume* (in the background), the waveform (a few KB), *auto cue* and *read tags*. A song is downloaded only for the preview.
- **Cache:** downloaded songs are kept in a local cache. Files older than a day are removed automatically and the cache stays under 1 GB; **Control Panel > Reset database** shows its size and has a *Clear the cache now* button.
- Closing the client never stops the music or the internet broadcast of the server.

## First steps

1. Control Panel > Libraries > Songs: import a folder (tick "auto cue" for mix points). Do the same for Jingles and Spots.
2. Scheduling > Main auto schedule: create a clock (rows of songs / jingles / ...) and paint it into the weekday x half-hour grid.
3. Spots > Add new campaign; program it on the break times.
4. Header: press **AUTO**. The next 2 hours are generated and played with automatic mixing.

## Studio screen

- **Header:** AUTO / MANUAL / OFFLINE, layout (classic / simple / wide), LIVE, Control Panel, volume, and the number of **listeners** that are connected to the internet broadcasts (the sum of all servers).
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
| Utilities | Import from Jazler / MegaMix, Integrity check, Broadcasting reports, Reset database |

- **Main auto schedule:** pick a brush in the palette on the left (a clock or a playlist, with a search box; *none* erases) and click or drag over the cells of the grid. Click a day name to fill the day, an hour label to fill that hour on every day.
- **Station IDs:** make a campaign with the spot of the station ID and give it priority *1 Station ID* on the break times; it plays first in the break. Or put a *jingle* row in a clock.
- **Delete from a library:** you are asked whether the audio files are deleted too. Deleting items never interrupts the automation unless the item is on air.
- **Reset database:** deletes the selected libraries, or everything except the settings.
- **Effects (CLAP):** loads `.clap` plugins on the main output (on a server the plugins are uploaded to it). If a plugin does not load, the reason is shown in the list.

## Internet broadcast

Control Panel > Internet broadcast: add several servers, each **Icecast 2** (host, port, mount, user, password), **Shoutcast v1** (host, port, password) or **Shoutcast v2** (host, port, password, stream ID). The Shoutcast source connects to port+1. MP3, 48-320 kbps. The *Show password* box next to the password field shows it while you type. The title of the playing song is sent to every server. Everything played on the main output is in the stream; the preview (monitor) output is not.

The number of listeners is read from the station servers every 20 seconds (Icecast `status-json.xsl`, Shoutcast v2 `stats`, Shoutcast v1 `7.html`) and shown in the header of the studio.

## Live rebroadcast

Control Panel > Scheduling > Live rebroadcast.

- **Sources:** add as many as you need. Each is a sound card input, or an internet radio stream (enter a URL: MP3 or AAC over HTTP/HTTPS; HLS is not supported), with its own level. *Go live* puts it on the air at once.
- **Schedule:** each entry picks a source, the days, a start and an end time (HH:MM, over midnight allowed) and whether to mute the automation. The source goes on the air at the start and off at the end.
- **LIVE button** in the studio header: click to start or stop the selected source; right-click to choose another. Stopping a scheduled source by hand keeps it off until its slot ends.
- The live audio is mixed into the main output, so it is also in the internet broadcast. The player shows a blinking **LIVE** with the source name. With *Stop automation* the playlist stops and the studio goes to MANUAL while the source is live; afterwards the previous mode returns and AUTO continues with the next item.

## Import from Jazler RadioStar 2 / MegaMix

Control Panel > Utilities > Import from Jazler / MegaMix. The type of the database is detected automatically; for MegaMix the songs, jingles / effects, commercials, artists, styles / categories / eras and playlists are imported (the music sequence rules are not converted). Select the old `.mdb` database (Access 2000-2003), press **Scan file**, choose what to import (songs, spots and campaigns, playlists, jingle pallets, play history) and press **Start import**.

- If the audio files are on another drive or folder, use the old/new path-start fields.
- On a client, tick the option to **upload the files to the server and change the paths**: every imported file is copied to the server and the library points to the copy.
- Existing data is never overwritten: known file paths and same-named categories, playlists, campaigns and pallets are skipped.
- User passwords are not imported. Newer Access formats must first be saved as Access 2000.

## Backups

Control Panel > Backup & restore: a daily automatic backup (default 03:00, keeps 14), "Back up now", and restore from any `.rmbak` file. A backup contains libraries, schedules, settings and play history (not the audio files). Restoring first saves a safety copy (`before-restore.rmbak`).

## Languages

English and Greek are built in. To add a language, copy `lang\template.json` to `lang\xx.json`, translate it and choose it in Control Panel > General settings. See `lang\README.txt`.

## Updating the server

Administrators update the server from the web studio (**Update** button) or from the client (Control panel > Settings > **Server update**): the server finds the newest release on GitHub, installs the package of its own system and restarts. The first time on a server installed with an older package, run `./install.sh upgrade` from the newest package once (it lets the server update itself later).

## Two-step login (MFA)

Each user of a server can have a two-step login: Control panel > Users & security > edit the user > *Turn on two-step login*, add the key to an authenticator app (Google Authenticator, Microsoft Authenticator, Authy, 2FAS ...) and type the code to confirm. After that the user types the password followed by the 6 digit code: the client has a *Two-step code* field, the web studio takes `password123456` in the password box. The sign-in then stays valid for 12 hours; 5 wrong codes lock the user for 5 minutes. The `webserver.json` administrator has no two-step login.

## Security note

`config.json` is stored encrypted so the database password is not readable in a text editor. The key is built into the program: this protects against casual reading, not against someone who has the program.

## Not included

Weather, VST plug-ins (CLAP plug-ins are supported), microphone recording of voice tracks, a client for Windows 7.

## What's new

The full list is in [CHANGELOG.md](CHANGELOG.md). The releases that are online now:

| Version | Main changes |
|---|---|
| 0.14.13 | Listeners of the internet broadcasts in the header; deleting library items no longer stops AUTO; *Internet updater* page removed |
| 0.14.12 | Fix of a server compile error of 0.14.11 |
| 0.14.10 | Client cache cleanup and *Clear the cache now*; normalize without keeping copies |
| 0.14.9 | Closing the client no longer stops the broadcast of the server |
| 0.14.8 | Shoutcast v2 and *Show password* |
| 0.14.7 | Playlist row colours like the library tabs; only the round-minutes clock |
| 0.14.6 | New neon studio clock; new Main auto schedule page with a brush palette; edit windows always on top; campaign times fixed |
| 0.14.3 | One media folder per library; client window maximized |
| 0.14.2 | Reset everything except the settings; server history in the client; Jazler import path fix; faster database |
| 0.14.0 | Jazler import with upload to the server; low bandwidth connection modes |
| 0.13.0 | Delete from the library asks about the audio files; new client login window with *Remember password* |

0.14.11 added the server-side normalize / waveform / auto cue (less traffic for clients) but its server build failed; use 0.14.12 or newer.

## Versioning and releases

The version is in `Cargo.toml` and is shown in the header and in **Control Panel > General settings > About**. The same version is stored in the database.

- An older database opened by a newer program: a safety backup (`pre-upgrade-<old>-to-<new>.rmbak`) is saved in the backups folder, then the database is upgraded. No data is lost.
- A newer database opened by an older program is refused with a message.

To publish a release (automatic): change `version` in `Cargo.toml`, add its `## x.y.z` section to `CHANGELOG.md` and push to `main`. The *auto-tag* workflow creates the tag and the release and starts the build. (In *Settings > Actions > General > Workflow permissions* choose *Read and write*.) By hand, instead:

1. Change `version` in `Cargo.toml` and add the changes to `CHANGELOG.md`.
2. Commit, then create the tag `vX.Y.Z` (on GitHub: Releases > Draft a new release > new tag) matching the version.
3. The *release* workflow builds every platform, attaches the packages to that release and replaces its description with the matching `## x.y.z` section of `CHANGELOG.md`. It never creates a release itself; if it cannot find one it stops with a message (create the release, then re-run the workflow).

The *cleanup-releases* workflow keeps the last 10 good releases (published, with their packages) and their tags and deletes the older ones. It also deletes the old runs of the workflows (the last 20 runs of each workflow stay). It runs after every successful release and can be started by hand (Actions > cleanup-releases > Run workflow; *dry run* only lists what would be deleted).

## Build locally (Windows)

No Rust on your PC? Push the folder to a GitHub repository and let the *release* workflow build everything. To build yourself:

1. Install Rust from <https://rustup.rs> (MSVC toolchain; Visual Studio Build Tools needed).
2. Download the MariaDB ZIP package for Windows x64 from <https://mariadb.org/download/>.
3. In PowerShell, in this folder:
   ```powershell
   .\build_portable.ps1 -MariaDbZip C:\path\to\mariadb-xx.x.x-winx64.zip
   ```
4. Result: `dist\RadioMoon-Portable` (folder) and `dist\RadioMoon-Portable.zip`.

Developers: the database layout is in the `sql/` folder: `sql/schema.sql` (latest full layout) and `sql/migrations/NNNN_name.sql` (upgrade steps). Both are embedded in the program; register new migrations in `src/schema.rs` (see `sql/migrations/README.md`).

## Contact

Nikos Batsikas - [nikos@velikanet.net](mailto:nikos@velikanet.net). Questions, bug reports and ideas are welcome (or open an issue on GitHub).

## License

Copyright (c) 2026 Nikos Batsikas. All rights reserved. See the `LICENSE` file. Third-party libraries keep their own licenses.
