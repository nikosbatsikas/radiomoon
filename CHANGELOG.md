# Changelog

Versions follow MAJOR.MINOR.PATCH. The version lives in `Cargo.toml`. Pushing a new version to `main` creates the tag and the release automatically (see README.md, "Versioning and releases").

## 0.7.0
- **Server for headless Linux (new):** `radiomoon-server` runs the whole automation without a window and is used from a web browser (player, playlist, library search, instant jingles, mode, volume, live, history; English / Greek). Packages for Debian 10+, Ubuntu 20.04+, Raspberry Pi OS 64-bit, AlmaLinux / Rocky / RHEL 8+, Fedora (one glibc 2.28 build) and Alpine (musl), x64 and arm64, with a systemd unit (OpenRC on Alpine) and `install.sh`.
- The web studio asks for a user and password (`webserver.json` in the data folder; a random password is created at the first start). Options: `--bind`, `--data`, `--db-host/--db-port/--db-user/--db-password/--db-name`.
- The server needs no sound card (a virtual output keeps the stream and the recordings running) and no window libraries.

## 0.6.1
- The automation never runs out of songs: when every song was played too recently for the separation rules, the song played the longest time ago is repeated instead of stopping.
- The second screen function is removed (F10 full screen stays).

## 0.6.0
- The FM multiplex part moved to its own project, [radiomoon-mpx](https://github.com/nikosbatsikas/radiomoon-mpx): the MPX Transmitter and the MPX Receiver are released there (version 1.0.0 has everything of 0.5.30, including FLAC lossless). RadioMoon no longer has the Control Panel > MPX page, and its packages no longer contain the two MPX programs.
- To put RadioMoon on air: enable the internet broadcast and use that stream as the source of the MPX Transmitter. The song titles RadioMoon sends to the server become the RDS RadioText there.
- Saved MPX settings are simply ignored; nothing else changes.

## 0.5.30
- MPX over IP can now be sent as **FLAC lossless** (Control Panel > MPX > Outputs > Format, and in the MPX Transmitter). The composite signal arrives bit-exact, exactly as with L24, at about 3-4 Mbit/s instead of 4.6 Mbit/s (how much smaller depends on the program audio). Every 2 ms packet is a complete, standard FLAC frame, so a lost packet never affects the next one. Works over UDP (RTP) and TCP. L24 stays the default.
- MPX Receiver: detects L24 or FLAC by itself. Lost UDP packets are now concealed by repeating the last 2 ms, which keeps the 19 kHz pilot, the 38 kHz subcarrier and the 57 kHz RDS carrier in phase, so radios do not drop to mono or lose RDS on a lost packet, and the timing stays exact. The status shows the format, the bitrate and the lost / concealed packets.
- FLAC needs MPX Receiver 0.5.30 or newer at the transmitter site; update the receiver first.

## 0.5.29
- Automatic database detection: when the configured port does not answer on this computer, the usual ports (3307 of RadioMoon, 3306 of the system MariaDB / MySQL) are tried and the one that works is saved. If nothing connects, the connection window opens with the port that answers already filled in.

## 0.5.28
- Linux / macOS: a database service of the system (MariaDB / MySQL) is used when there is no bundled server; the program no longer stops with "Database server not found" but tries the connection. The default port is 3306 there (3307 on Windows). The connection window explains how to create a database user for the system server.

## 0.5.27
- Fix of the build error of 0.5.26 (connection window).

## 0.5.26
- If the database cannot be reached at start, a connection window opens (server address, port, user, password, database name) with Test connection, Save and start RadioMoon, and Save only, instead of only showing an error.

## 0.5.25
- The icon next to the name in the header is removed.
- The header buttons (modes, layout, LIVE, About, Control Panel) now get their width from the text, so longer Greek labels are no longer cut off.

## 0.5.24
- The second screen is an option, off by default: switch it on in Control Panel > General settings (Station); then the "2nd screen" button appears in the header, and it can also open at program start.

## 0.5.23
- F10 switches the window to full screen and back (studio and second screen).
- Dual screen: the "2nd screen" button in the header opens an on-air window for the second monitor with the clock, the song on air with its countdown, and the next songs. Move it to the other monitor and press F10 for full screen.

## 0.5.22
- The blue RadioMoon tile in the studio is replaced by the new program icon.

## 0.5.21
- Import of a MegaMix database (.mdb): songs, jingles / effects, commercials, artists, styles, categories, eras and playlists. The import page now detects Jazler RadioStar 2 and MegaMix files automatically.

## 0.5.20
- The Windows installer can install the database (MariaDB) as the Windows service RadioMoonDB (automatic start, local connections only); the program uses the running service, and the uninstaller removes it but keeps the data.
- Auto volume (Control Panel > Sound outputs): keeps all songs at the same loudness with a target level and maximum boost and reduction.
- The new icon is shown in the header of the studio (instead of the drawn moon).

## 0.5.19
- New program icon (sunset moon with sound bars) for the studio, MPX Receiver and Transmitter, installer and packages.

## 0.5.18
- The songs that were already played leave the playlist; the list starts with the song on air.
- Playlist editor: a song can be only once in a playlist (songs already added are hidden in the picker).
- New About window (button in the header) in the style of FM Analyzer: logo, version, authors, features, project link and gift button.

## 0.5.17
- Playlist editor: search the songs stored in the database and add them to the playlist with + or a double-click (or add all shown), choose music, jingles, sweepers, voices or spots.
- A playlist placed in the Main auto schedule now plays only its own songs for the whole painted block, repeating if it is shorter than the block (no clock songs are mixed in).

## 0.5.16
- Studio fits small screens and windows: the program starts maximized and the whole studio (player, playlist, library, jingles, buttons) scales down automatically instead of being cut off at the right and bottom.

## 0.5.15
- Playlists page redesigned: a grid of all playlists (title, duration, items, schedules) on the left, and on the right the numbered items of the selected playlist (duration, intro, jingles tinted blue) with its scheduled times below. Double-click or right-click a playlist to edit or delete it.

## 0.5.14
- Playlists can be called from the Main auto schedule by time: pick a playlist (purple, marked with a triangle) as the brush and paint it on the half-hour grid; it starts at the beginning of each painted block, then the clock continues.

## 0.5.13
- Start in AUTO no longer replays the playlist of the last run: the schedule builds a new playlist, so the songs differ at every start.
- "Add selected" in AUTO mode queues the track right after the current song (and after earlier manual items) instead of after all the generated hours, so it is really played.

## 0.5.12
- New stand-alone program **RadioMoon-MPX-Transmitter**: takes a folder of audio files (random order), an internet stream or a sound input, builds the FM multiplex (stereo, pilot, RDS with RadioText from the playing file) and sends it to a 192 kHz sound card and/or over IP. It runs by itself, like the MPX Receiver, with the MPX analyzer included.
- Included in the Windows installer and portable zip, Linux packages and tar.gz, and macOS app bundle.

## 0.5.11
- Fixed: the build error in the half-hour schedule grid (0.5.9 and 0.5.10 did not compile) and the compiler warnings.
- Auto mode no longer starts with the same songs every time: songs with an equal rank are chosen at random.

## 0.5.10
- Build: compiler errors and warnings of the Linux build are shown as annotations of the workflow run.

## 0.5.9
- Main auto schedule: the weekly grid now has half-hour steps (a clock can start at :00 and at :30; the previous hourly schedule is converted automatically). New look: coloured clock chips, neighbouring slots of the same clock drawn as one block, today and the current half hour highlighted, drag to paint, click a day name to fill the day or an hour to fill it on every day.

## 0.5.8
- Live rebroadcast: cross-fade (0-15 s, default 3 s, setting in Scheduling > Live rebroadcast). At the start the automation fades out while the live source fades in; at the end the live source fades out while the automation fades in with its next item, so both are heard together during the transition.

## 0.5.7
- All cards in the Control Panel span the whole width of the page, like the edit windows; General settings is grouped into cards and its text fields fill the width.

## 0.5.6
- First release made automatically by the auto-tag workflow. Contains everything of 0.5.4 (edit windows keep the keyboard focus) and 0.5.5 (LIVE shown in the player, live source stops the automation and resumes it afterwards).

## 0.5.5
- Live rebroadcast: the player shows a blinking LIVE (with the source name) while a live source is on the air. "Stop automation" now really stops the playlist and switches to MANUAL during the live source; when it ends, the previous mode returns and AUTO plays the next item.

## 0.5.4
- Fixed: in the edit windows (song title, campaign, clock, playlist, force clock ...) every text field accepted one letter and then lost the keyboard focus. The window changed its identity whenever the title changed.

## 0.5.3
- New look: colours and fonts of FM Analyzer (dark instrument palette, egui's own fonts with DejaVu as the Greek fallback).
- The tabs fit the window: the library tabs of the studio use the full width, the Control Panel menu shrinks its rows to fit the window height.

## 0.5.2
- Live rebroadcast: any number of sources (sound inputs and internet streams), each with its own level, and a weekly schedule (days, start and end time, mute automation per entry; over midnight is supported). New page Scheduling > Live rebroadcast. Right-click the LIVE button in the studio to choose a source. The single source of 0.5.0/0.5.1 is converted automatically.

## 0.5.1
- Fixed: the program closed when MPX was started (the new MPX analyzer read past the end of the spectrum data).

## 0.5.0
- MPX analyzer in the generator (Control Panel > MPX) and in the MPX receiver: spectrum 0-96 kHz with 19/38/57 kHz markers, composite waveform, levels of pilot, RDS, L+R and L-R, peak and maximum deviation (kHz and %).
- New: live rebroadcast. A LIVE button in the studio header puts a sound card input or an internet radio stream (MP3/AAC URL) on the air, mixed into the main output, internet broadcast and MPX. Option to mute the automation while live (it keeps running silently so the schedule stays in time). Settings: Control Panel > Sound outputs.
- Release now also publishes the MPX receiver alone for Linux x64 and arm64 (Raspberry Pi 3/4/5, 64-bit OS).
- Windows installer: the program goes to Program Files (needs administrator rights) and the data (database, backups, settings) to %PROGRAMDATA%\RadioMoon, shared by all users. An existing %LOCALAPPDATA%\RadioMoon folder is still used if present.
- Database layout moved to a `sql/` folder: `schema.sql` (latest) and `migrations/` (upgrade steps per version), embedded in the program.
- New: Control Panel > Utilities > Import from Jazler RadioStar 2. Reads the old .mdb database (Access 2000-2003) directly on every platform and imports songs, spots, categories, playlists, campaigns, jingle pallets and play history, with a path-replace option. Existing data is never overwritten.

## 0.4.3
- The MSI installer was removed (WiX 7 requires accepting a maintenance-fee EULA). Windows editions: setup .exe and portable .zip.

## 0.4.2
- Audio item editor: order is Data, Playback, Waveform, then the play buttons; everything fits in the window without a scroll bar.
- Studio: the Delete button removes the selected item from the playlist (the song stays in the library).

## 0.4.1
- Control Panel: smaller fonts; all dialogs open with the same size and position (content scrolls inside).
- Audio item editor: data and playback settings and the play buttons are at the top, the waveform is below; smaller window.
- Browse buttons for all folder and file paths.
- Fixed: Control Panel was drawn with a light background when Windows uses the light theme.

## 0.4.0
- The MPX receiver is now a separate program: RadioMoon-MPX-Receiver.exe (radiomoon-mpx-receiver on Linux). It is no longer part of RadioMoon.
- Internet broadcast: any number of Icecast / Shoutcast servers at the same time, each with its own bitrate and settings.
- AUTO mode starts playing the first song as soon as it is selected (and when the program starts in AUTO).
- Control Panel: dark backgrounds and white text.

## 0.3.0
- MPX / FM multiplex: built-in stereo encoder (19 kHz pilot, 38 kHz, RDS PS + RadioText from the playing song) with output to a 192 kHz sound card and/or over IP (RTP L24 over UDP, or raw L24 over TCP). Control Panel page "MPX / FM multiplex".

## 0.2.1
- Clearer text: embedded DejaVu Sans font (Latin + Greek), brighter text colours, stronger borders, larger sizes.
- Executable and window now have the RadioMoon icon.
- Library lists use a proper column table (code, artist, title, category, duration).
- Faster: missing-file checks cached, idle redraw rate lowered, `panic = abort` in release builds.

## 0.2.0
- New professional look: dark theme, card layout, full-size Control Panel with a navigation sidebar.
- Audio editor with waveform: click or drag to set mix start, mix end, intro end and hooks; zoom; listen from any position or to the mix end.
- Import dialog: choose whether to read ID3 tags or use the file name (optionally "Artist - Title").
- Multilingual interface: English and Greek built in; add a language by dropping a `lang/xx.json` file.
- Internet broadcast to Icecast 2 and Shoutcast v1 servers (MP3), with song titles sent to the server.
- Automatic daily database backups, manual backups, and restore.
- The program version is stored in the database; when a newer program opens an older database it makes a safety backup and upgrades it without losing data. An older program refuses to open a newer database.
- All durations are shown in minutes:seconds.
- `config.json` is encrypted.
- Installed mode keeps data in the user's application-data folder; portable mode (a `portable.flag` file) keeps everything next to the program.
- Builds and packages for Windows (x64, x86, arm64), Linux (x64, arm64) and macOS (arm64, x64), portable and installer.
- Playout engine reworked around one mixer (needed for the stream); instants, sweepers and voice tracks are part of the mix.

## 0.1.0
- First version: studio screen, libraries, clocks, spots, playlists, final log, instant jingles, reports.
