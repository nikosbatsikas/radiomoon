# Development and Releases

## Languages

English and Greek are built in. To add a language, copy `lang\template.json` to `lang\xx.json`, translate it and choose it in Control Panel > General settings. See `lang\README.txt`.

## Security note

`config.json` is stored encrypted so the database password is not readable in a text editor. The key is built into the program: this protects against casual reading, not against someone who has the program.

## Not included

Weather, VST plug-ins (CLAP plug-ins are supported), microphone recording of voice tracks, a client for Windows 7.

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
