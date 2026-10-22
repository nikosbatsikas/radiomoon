# 0.4.3
- The MSI installer was removed (WiX 7 requires accepting a maintenance-fee EULA). Windows editions: setup .exe and portable .zip.

# 0.4.2
- Audio item editor: order is Data, Playback, Waveform, then the play buttons; everything fits in the window without a scroll bar.
- Studio: the Delete button removes the selected item from the playlist (the song stays in the library).

# 0.4.1
- Control Panel: smaller fonts; all dialogs open with the same size and position (content scrolls inside).
- Audio item editor: data and playback settings and the play buttons are at the top, the waveform is below; smaller window.
- Browse buttons for all folder and file paths.
- Fixed: Control Panel was drawn with a light background when Windows uses the light theme.

# 0.4.0
- The MPX receiver is now a separate program: RadioMoon-MPX-Receiver.exe (radiomoon-mpx-receiver on Linux). It is no longer part of RadioMoon.
- Internet broadcast: any number of Icecast / Shoutcast servers at the same time, each with its own bitrate and settings.
- AUTO mode starts playing the first song as soon as it is selected (and when the program starts in AUTO).
- Control Panel: dark backgrounds and white text.

# 0.3.0
- MPX / FM multiplex: built-in stereo encoder (19 kHz pilot, 38 kHz, RDS PS + RadioText from the playing song) with output to a 192 kHz sound card and/or over IP (RTP L24 over UDP, or raw L24 over TCP). Control Panel page "MPX / FM multiplex".

# 0.2.1
- Clearer text: embedded DejaVu Sans font (Latin + Greek), brighter text colours, stronger borders, larger sizes.
- Executable and window now have the RadioMoon icon.
- Library lists use a proper column table (code, artist, title, category, duration).
- Faster: missing-file checks cached, idle redraw rate lowered, `panic = abort` in release builds.

# Changelog

Versions follow MAJOR.MINOR.PATCH. The version lives in `Cargo.toml`; a release is made by pushing a tag
`vMAJOR.MINOR.PATCH` that matches it (see README.txt, "Versioning and releases").

## 0.5.0
- Database layout moved to a `sql/` folder: `schema.sql` (latest) and `migrations/` (upgrade steps per version), embedded in the program.
- New: Control Panel > Utilities > Import from Jazler RadioStar 2. Reads the old .mdb database (Access 2000-2003) directly on every platform and imports songs, spots, categories, playlists, campaigns, jingle pallets and play history, with a path-replace option. Existing data is never overwritten.

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
