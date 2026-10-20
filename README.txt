RadioMoon  (portable radio automation, Rust + MariaDB/MySQL)

BUILD LOCALLY (Windows)
 1. Install Rust: https://rustup.rs  (MSVC toolchain; Visual Studio Build Tools needed)
 2. Download the MariaDB "ZIP" package for Windows x64 from https://mariadb.org/download/
 3. In PowerShell, in this folder:
        .\build_portable.ps1 -MariaDbZip C:\path\to\mariadb-xx.x.x-winx64.zip
 4. Result: dist\RadioMoon-Portable (folder) and dist\RadioMoon-Portable.zip
 No Rust on your PC? Upload this folder to a GitHub repository and run the
 "Build portable Windows package" workflow (Actions tab); download the zip it produces.

PACKAGES (made automatically, see "Versioning and releases")
   Platform         Architectures      Portable                          Installer
   Windows          x64, x86, arm64    RadioMoon-<ver>-windows-<arch>-portable.zip    ...-setup.exe
   Linux            x64, arm64         RadioMoon-<ver>-linux-<arch>-portable.tar.gz   .deb and .rpm
   macOS            arm64, x64         RadioMoon-<ver>-macos-<arch>-portable.zip      .dmg
 The bundled database server (MariaDB) is included only in the Windows x64 packages. On every other
 platform RadioMoon uses a MySQL or MariaDB server that you install; enter its address in
 Control Panel > Database server. (Linux/macOS: install mariadb-server or mysql-server.)

PORTABLE OR INSTALLED
   Portable: the folder contains a file called portable.flag. All data (database, backups, reports,
             settings, config.json) stays in the program folder - copy the folder to move the station.
   Installed: no portable.flag. Data is kept per user in
             Windows %LOCALAPPDATA%\RadioMoon, macOS ~/Library/Application Support/RadioMoon,
             Linux ~/.local/share/RadioMoon. Uninstalling or upgrading never deletes it.

VERSIONING AND RELEASES
   The program version is in Cargo.toml (version = "0.2.0") and is shown in the header and in
   Control Panel > General settings > About. The same version is stored in the database.
   - Opening an older database with a newer program: a safety backup (pre-upgrade-<old>-to-<new>.rmbak)
     is saved in the backups folder, then the database is upgraded. No data is lost.
   - Opening a newer database with an older program is refused with a message.
   To publish a release:
     1. Change `version` in Cargo.toml and add the changes to CHANGELOG.md.
     2. git commit, then  git tag v0.2.1  and  git push --tags
     3. The "Release" workflow (GitHub Actions) builds every platform and attaches the packages and
        SHA256SUMS.txt to the GitHub release. The tag must match the version in Cargo.toml.
   Developers: when the database layout changes, raise SCHEMA_VERSION and add the SQL to MIGRATIONS in
   src/db.rs.

PORTABLE FOLDER LAYOUT (Windows x64)
   RadioMoon.exe, portable.flag, lang\, mariadb\ (bundled database server)
   data\ (the database, created on first run), backups\, reports\, export\, logs\, config.json (encrypted)
The bundled database listens only on 127.0.0.1:3307. Use another host in Control Panel > Database server
to share one database between several workstations.

LANGUAGES
   English and Greek are built in. To add a language copy lang\template.json to lang\xx.json, translate it,
   and choose it in Control Panel > General settings. See lang\README.txt.

INTERNET BROADCAST
   Control Panel > Internet broadcast: Icecast 2 (host, port, mount, user, password) or Shoutcast v1
   (host, port, password; the source connects to port+1). MP3, 48-320 kbps. Start / stop there, or start
   automatically with the program. Everything played through the main output is in the stream
   (songs, spots, sweepers, voice tracks and instant jingles on the main output); the preview (monitor)
   output is not.

BACKUPS
   Control Panel > Backup & restore: a daily automatic backup (default 03:00, keeps 14), "Back up now",
   and restore from any .rmbak file. A backup contains the libraries, schedules, settings and play history
   (not the audio files). Restoring first saves a safety copy (before-restore.rmbak).

SECURITY NOTE
   config.json is stored encrypted so the database password is not readable in a text editor. The key is
   built into the program, so this protects against casual reading, not against someone who has the program.

STUDIO SCREEN
   Header: AUTO / MANUAL / OFFLINE, Layout (classic / simple / wide), Control Panel, volume
   Deck: Next, countdown, waveform with mix marks, VU, clock, Restart / Loop / Pause / StopNext
   Left: playlist. Double-click plays, headphones = preview, Move Up/Down, "EXACT TIME" items
   Centre: database tabs (Songs, Spots, Jingles, Assets, Sweepers, Voice, Memos)
   Right: Instant jingle pallets (right-click a button to assign / clear / colour) and
          Central (history, upcoming ad breaks, problems, memos)
   Default shortcuts: F1 next, F2 pause, F3 stop, F4 restart, F5 loop, F6 stopnext, F7 stop jingles.

CONTROL PANEL
   Libraries   Songs (3 categories + vocal + properties, intro, mix points, hooks, volume,
               song separation, playlister code, mass change, normalize, statistics),
               Spots (campaigns, breaks, priorities, playlister import), Jingles, Assets,
               Sweepers, Voice tracks, Memos, Instant jingle pallets, Stitcher
   Scheduling  Main auto schedule (clocks, 7x24 grid, randomness, artist separation,
               next-day separation, restrictive rules, classic auto), Playlists, Force clocks,
               Final log creator, Log viewer (shows why each track was chosen)
   Settings    General, Sound outputs (main / monitor / instants), Shortcuts,
               Users & security, Audio folder mappings, Database server
   Utilities   Internet updater files, Integrity check, Broadcasting reports, Reset database

FIRST STEPS
 1. Control Panel > Libraries > Songs: import a folder (tick "auto cue" for mix points).
    Do the same for Jingles and Spots.
 2. Scheduling > Main auto schedule: create a clock (rows of songs/jingles/...), paint it
    into the weekday x hour grid.
 3. Spots > Add new campaign; programme it on the break times.
 4. Header: press AUTO. The next 2 hours are generated and played with automatic mixing.

NOT INCLUDED: RDS, weather, stream encoders, rebroadcast of live inputs, VST plug-ins,
import of other automation databases, microphone recording of voice tracks.
