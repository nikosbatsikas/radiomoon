# Changelog

Versions follow MAJOR.MINOR.PATCH. The version lives in `Cargo.toml`. Pushing a new version to `main` creates the tag and the release automatically (see README.md, "Versioning and releases").

## 0.14.22

- The web studio shows the number of listeners (the sum of all internet broadcast servers, in the header, and per server in the broadcast line).
- The About windows and the README have the contact e-mail.
- The wiki has real screenshots (made by the wiki-screenshots workflow).

## 0.14.21

- The web studio has the same blue look as the client sign-in, and a **Sign out** button.
- Web studio: the scrollbars are dark, the page no longer scrolls sideways and the playlist rows fit their box.
- Client sign-in: the Connect button is inside the card.
- New: the Wiki (`wiki/`) with the manual and the design guides for cloud hosting and internet radio.

## 0.14.20

- **Server update:** the server can update itself to the newest release. In the **web studio** (administrator) the **Update** button, and in the **client** Control panel > Settings > **Server update**: look for the newest version, read what is new, **Update the server now**. The server downloads the package of its system (glibc or Alpine, x64 or arm64) from GitHub, checks that the new program starts and reports the new version, replaces itself (with the `lang` folder and the clients that it offers), starts the new program in the same process and the database is upgraded at the start (a backup is made first). The broadcast stops for a moment. The program folder must belong to the service user: a server installed with an older package needs `./install.sh upgrade` once (it now gives the folder to the user `radiomoon`). The client installs the version of its server the next time it is opened.
- **Move to a server (program without a server):** Control panel > Settings > **Move to a server**. Enter the address, user name and password (and the two-step code) of a RadioMoon server of the same version: the database of this program is sent and REPLACES the database of the server, then the audio files of the library are uploaded one by one and the songs get their new place on the server. The sound outputs and the folders of this computer are not taken over; this program is not changed.
- **Two-step login:** the window of the user shows a **QR code** to scan with the authenticator app (the key and the address are still shown).
- **Sound outputs in the client:** the page shows the **sound outputs of the server** (its sound cards; the changes are sent to the server) and, in its own card, the **output of this computer for listening** to songs and previews. Before, choosing an output in the client tried to open the sound card names of the server on the client computer, and the listening stopped.

## 0.14.19

- **The client follows the server:** when the client connects to a server of a different version (newer or older), it downloads the client that came with that server, replaces itself and starts again ("Updating the client to version ..."). If that is not possible (no package for the computer, no network) it connects anyway and the status line says that the versions differ.
- **Web studio sign-in page:** the browser no longer shows its grey password box: the web studio opens a sign-in page with the same look as the client (moon, equalizer bars, user name, password and the two-step code). The client download buttons work from the page.
- **No compiler warnings:** unused code and imports, an unneeded `mut` and the deprecated `child_ui` were removed; the new *warnings* workflow shows the warnings of the window and the server editions on every change.
- **Two-step login (MFA) for the users of a server:** Control panel > Users & security > edit a user > **Two-step login**. The window shows a key (and the `otpauth://` address) for an authenticator app (Google Authenticator, Microsoft Authenticator, Authy, 2FAS ...); the user is turned on after the code of the app is typed to confirm it. Then the user signs in with the password followed by the 6 digit code: in the **client** there is a *Two-step code* field, in the **web studio** the password box takes `password123456`. A checked sign-in stays valid for 12 hours (the browser and the client send the same credentials with every request); after 5 wrong codes the user is locked for 5 minutes. The server was restarted: sign in again with a new code. The key is stored in the user's record; the administrator account of `webserver.json` has no two-step login.
- **Listeners in the header:** moved a little to the right so it does not touch the volume knob; "1 listener" in the singular.
- **Client of a server:** the pages **Audio folder mappings** and **Database server**, and the **Folder** field of the automatic backup, are not shown any more: they belong to the server (its paths, its database and its backups), not to the client. The program without a server keeps them.

## 0.14.18

- **Plugin windows (CLAP):** Control panel > Effects (CLAP) has an **Open window** button for plugins that have a window of their own (for example the RadioMoon MPX Transmitter plugin, whose settings are in the Transmitter window). The plugin opens its window as a separate floating window. With a server, the button is not shown: the plugin runs on the server, which has no screen.

## 0.14.17
- The Ogg Opus and Ogg FLAC broadcast formats of 0.14.15 were taken out: their libraries did not build on every platform (the AlmaLinux server). The broadcast is MP3 for Icecast and Shoutcast, as in 0.14.14. The release build still sets `CMAKE_POLICY_VERSION_MINIMUM` and shows the build errors of Windows, macOS and the server in the run summary.

## 0.14.14
- The listeners in the header are a little higher, level with the volume slider.
- **Internet broadcast page:** every server that is on air shows its own number of listeners (*Listeners now*).
- The workflow *cleanup-releases* deletes the old runs of the workflows too (keeps the last 20 of each).
- The README no longer has the FM / MPX section.

## 0.14.13
- **Listeners:** the header of the studio shows how many listeners are connected to the internet broadcasts now (the sum of all servers). It is read from the station servers every 20 seconds: Icecast 2 (`status-json.xsl`), Shoutcast v2 (`stats`) and Shoutcast v1 (`7.html`). A client shows the numbers that the server reads.
- **Fix:** deleting items from a library no longer stops the automation: only the item that is on air stops it. The deleted items leave the playlist and the rest keeps playing.
- The *Internet updater* page was removed from Utilities.

## 0.14.12
- Fix of a compile error of the server in 0.14.11 (the changes of 0.14.11 are the same).

## 0.14.11
- **Less traffic between the client and the server:** the heavy work on a song is done by the server and only the result travels.
  - *Normalize volume* (mass change) runs on the server in the background: the client sends one command and downloads nothing.
  - The waveform (studio and song editor) is made by the server and sent as a few KB instead of the whole song; the server remembers it.
  - *Auto cue* and *Read tags* in the song editor run on the server. The song is downloaded only when you press the preview play button.
- **Server:** sending a song, the waveform and the analyses are done by the worker threads, not by the main thread that plays the music (a big file could delay it).

## 0.14.10
- **Client cache:** the songs that the client downloads (to listen, for the waveform) are cleaned automatically: files older than a day are removed and the cache stays under 1 GB. *Reset* page: shows the size of the cache and a *Clear the cache now* button.
- **Normalize volume** (mass change) on a client no longer keeps a copy of every analysed song on the disk.

## 0.14.9
- **Fix:** closing the client stopped the internet broadcast on the server (the client sent a *stop broadcast* command while it was closing). A client never stops the broadcast when it closes now.

## 0.14.8
- **Internet broadcast:** Shoutcast v2 (DNAS 2.x) servers with a stream ID (the source connects to the next port, the title is updated per stream); *Show password* option next to the password field.

## 0.14.7
- **Studio playlist:** the row colours are the same as the library tabs (songs blue, spots green, jingles yellow, assets grey, sweepers purple, voice red).

## 0.14.6
- **Main auto schedule redesigned:** instead of a wall of coloured buttons there is a brush palette on the left (current brush, search box, clocks and playlists in separate lists) and the weekly grid next to it.
- **Studio clock:** round neon minutes outside and a thin green seconds ring inside (the dotted-seconds clock was removed).
- **Fix:** the edit windows (playlist, campaign, ...) opened only the first time; afterwards they stayed behind the control panel, so clicks did nothing. They are now always on top.
- **Fix:** all campaigns showed the same times: the marked break times of the campaign edited before were reused. The times and days now come from the campaign being edited, and the campaign list shows the break times of each campaign.

## 0.14.5
- **New studio clock** with round neon minutes; the time, the day and the date are in the centre.

## 0.14.3
- **Client:** the window is asked to maximize until the system confirms it; the login card is taller so that the *Connect* button is inside it; the list of servers has a single arrow.
- **Uploads to the server go to one folder per library:** `media/songs`, `media/spots`, `media/jingles`, `media/assets`, `media/sweepers`, `media/voice` (before: `media/music` ...). Files uploaded before stay where they are.

## 0.14.2
- **Reset database > Delete everything except the settings:** new button (with a confirmation checkbox) that deletes all libraries, clocks, playlists, campaigns, categories, breaks, the playlist and the play history. The settings and the user accounts stay.
- **Client:** the window opens maximized, and the server address has a drop-down list of the servers used before (user name, and the password when it was remembered). The button *Upload the songs to the server* was removed from the Jazler import page: the upload is done by the option of the import.
- **Fixed: upload at the Jazler import did not change the paths.** The server did not know the songs that the client had just added to the database, so it answered "no such item" and the path stayed as it was. The server now reads the library again when it does not know an item, and it no longer reads the whole library again for every uploaded file.
- **Faster database and client:**
  - The installer (install and upgrade) tunes MariaDB: commits do not wait for the disk every time (`innodb_flush_log_at_trx_commit=2`), a buffer in memory (a quarter of the memory, 128 MB - 1 GB) and the number of connections. The settings are applied at once, without a restart.
  - The client no longer reads the whole library again every minute: the server tells when the library or the documents changed (`data_rev`), and the client reads them again only then (otherwise every 15 minutes).
  - The database tunnel sends the big answers (the list of songs when the client starts) in fewer, bigger parts.

## 0.14.0
- **Import from Jazler / MegaMix with a server:** new option *Upload the audio files to the server at the end of the import and change the paths in the database*. When the import is finished the client uploads the audio files to the server and the library points to the copies on the server (the same as the button *Upload the songs to the server*, but started by the import). Importing a folder of songs with a server already uploads the files and adds them with their path on the server.
- **Low bandwidth connections (client):** new *Connection* setting in the login window and in the status bar of the studio: *Normal*, *Low bandwidth* (the state of the server is read every 1.5 s, the library every 5 minutes, no waveform download) and *Very low bandwidth* (every 4 s, the library every 15 minutes). The buttons of the studio are sent at once in every mode. The studio window already shrinks to fit a small screen.

## 0.13.0
- **Delete from the library asks about the files:** *Delete selected* now opens a question. A checkbox deletes the audio files from the disk too (with a server: from the disk of the server). A file that another item of the library still uses is kept. The server answers the number of files that were deleted.
- **New login window of the client:** a centred login card with the moon logo on an animated dark-blue background, labelled fields, and the option **Remember the password on this computer** (the password is kept encrypted in the settings of the client; leave it off on a shared computer).

## 0.12.4
- **Fixed the freezes of the server ("busy"):** a song with length 0:00 (not analysed or damaged) was chosen again and again by the automatic playlist to fill the last seconds before a scheduled event, and the playlist grew with thousands of copies of it (every check made it longer, until the server did not answer). Songs without a length are no longer chosen by the automatic playlist, the length of an item is counted as at least 1 second everywhere, and the playlist stops growing at 3000 waiting items. Use *Control panel > Library > Integrity* to find the songs with length 0:00.

## 0.12.3
- The server writes in its log which step is slow (`slow step: ...`) when the main loop takes 3 seconds or more, and it writes the playlist to the database only when the list really changed (before it was written at every check, which made the server answer "busy" on slow computers).

## 0.12.2
- (no Windows 7 client: it was tried and dropped)

## 0.12.1
- **Server watchdog:** when the main loop of the server stops for 60 seconds (the web page answers "error" and the music stops until the service is restarted), the server now writes the cause in the log (`WATCHDOG: ... stuck in [...]`) and restarts itself, so the broadcast continues. Slow requests and slow playout steps (3 s or more) are written in the log.
- The connections to the database have time limits (connect 8 s, read/write 40 s), so a database connection that was lost no longer makes the server wait for ever.

## 0.12.0
- **The client connects with only the server address, user name and password.** The database of the server is no longer opened from the client computer: the client carries its database connection inside the HTTP port of the server (a tunnel, signed in with the same user). `RADIOMOON_REMOTE_DB`, the port 3306 and the "Database of the server" fields are not needed any more, and the client can never read the database of another server by mistake. The server must be updated to 0.12.0 too.

## 0.11.3
- **Client with several servers:** every server now has its own folder on the computer (`RadioMoon-Client/servers/<address>`) for its settings and its cache of songs. Before, two servers shared them, so the second server showed the settings of the first and played cached songs with the same number. Delete the old `cache` folder once; the first connection to each server starts with fresh settings.

## 0.11.2
- **CLAP scan explains why a plugin is not loaded:** a plugin file that cannot be opened (built for another system, e.g. a Windows `.clap` on a Linux server, or a glibc plugin on Alpine) or is not a CLAP plugin is now listed with the reason, in red, after the scan.

## 0.11.1
- **Client in sync with the server:** the client now reads the playlist from the memory of the server (`/api/log`) and not from the database, so the playlist, the song on air and the *Next* button always agree with what the server plays (before: "- off air -" and an empty playlist). The playlist is read again only when it changes.
- **Upload CLAP plugins:** Control panel > Effects (CLAP) > **Upload plugin files...**. With a server, the `.clap` files are uploaded to the server (folder `plugins/clap` of its data) and the plugin list is refreshed; without a server they are copied to the plugin folder of the program.

## 0.11.0
- **CLAP plugin hosting (new):** Control panel > Settings > **Effects (CLAP)**. CLAP effects (the open plugin standard, cleveraudio.org) are loaded on the on-air mix: the sound goes through the chain before the sound card and before the internet broadcast. Scan the CLAP folders (`~/.clap`, `/usr/lib/clap`, `Common Files\CLAP`, `CLAP_PATH`, ...), add effects, set their parameters with sliders, bypass, reorder and remove them. The chain is saved in the settings and loaded at every start.
- With a server, the plugins run **on the server** (they must be installed there); the client edits the chain through the server.
- The windows of the plugins are not hosted yet (parameters are edited with the sliders). Instruments and plugins without audio input are refused.

## 0.10.1
- **Internet broadcast in the client and the web page:** the broadcast that runs on the server is now shown (status of every Icecast / Shoutcast server in the status bar of the client, in the "Internet broadcast" page and in the web page). **Start / Stop / Restart broadcast** in the client and the new button in the web page control the server; before, the client tried to start a broadcast on its own computer.

## 0.10.0
- **The client is part of the server editions only:** it is no longer in the Windows setup, the .deb / .rpm or the portable packages. Every server package (`RadioMoon-Server-...tar.gz`) contains the clients for Windows (x64, x86, arm64), Linux (x64, arm64) and macOS, and the web page of the server has a **Download client** button.
- **About** button in the web page of the server.
- **Waveform in the client:** the waveform of the item on air is made from the song of the server (before, the client showed "analysing waveform..." for ever).
- **Bandwidth in the status bar of the client:** download and upload speed to the server.
- **Import from other programs, server edition:** after importing a Jazler / MegaMix database in the client, the button "Upload the songs to the server" uploads the songs of this computer to the server and the library points to the copies on the server.

## 0.9.2
- Server installer on Alpine: MariaDB is restarted only once (the second restart waited for the database that was still starting and the installer stopped at "Stopping mariadb"). A restart that does not finish in 90 seconds is forced.

## 0.9.1
- **Server installer:** `./install.sh upgrade` replaces the program and updates the database tables (after a backup) and keeps the settings, the web password and the songs; `./install.sh uninstall` removes the program and the service and keeps the data and the database; `./install.sh uninstall --purge` removes everything. Running `./install.sh` on a computer where the server is already installed upgrades it.

## 0.9.0
- **The client is now the real studio:** `radiomoon-client` shows the same studio and the same control panel as the desktop program, connected to a RadioMoon server. The server plays; the buttons of the studio are sent to it and the playlist, mode, volume and playing position follow the server. Closing the client never stops the broadcast.
- **Upload songs to the server:** "Add single file" and "Import folder" in the library of the control panel upload the files to the server (folder `media`), which reads the tags and adds them to the library.
- **Listen to the audio of the server:** previews and the waveform editor download the song from the server (cached on the client computer).
- Login by RadioMoon users: their permissions apply in the client.
- Server installer: `RADIOMOON_REMOTE_DB=1 ./install.sh` lets the database accept the client from another computer.

## 0.8.2
- Server installer: the Alpine (BusyBox) `sed` call that enables MariaDB networking now works.

## 0.8.2
- Alpine service: the log file `/var/log/radiomoon-server.log` and the data folder are created for the `radiomoon` user at start (the service failed with "Permission denied"). The BusyBox `sed` call of the installer that enables MariaDB networking now works.

## 0.8.1
- **Server installer (Alpine and others):** MariaDB on Alpine is started with networking switched off; the installer now turns it on for 127.0.0.1, waits until the database answers, creates the database and user, and saves the connection (new option `--setup`). Before, the server stayed without a database login. Errors are no longer hidden.

## 0.8.0
- **RadioMoon client (new):** `radiomoon-client` (Windows: "RadioMoon Client") is the studio window for a RadioMoon server: now playing, transport, volume, playlist (play, move, remove), library search with Add / Next, instant jingles, history and mode. The server plays; closing the client never stops the broadcast.
- **Login:** the client (and the web studio) signs in with the web studio account of the server (`webserver.json`) or with a RadioMoon user and password; the permissions of that user apply (add, delete, change the mode...). A wrong password is reported at once and slows down guessing.
- The server is built with `--features server,vendored-ssl` (OpenSSL included).

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
