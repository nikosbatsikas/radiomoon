# Installation (desktop program)

Download the newest version from the **Releases** page of the repository.

| Platform | Package |
|---|---|
| Windows x64, x86, arm64 | `RadioMoon-<ver>-windows-<arch>-setup.exe` (installer) or `...-portable.zip` |
| Linux x64, arm64 | `RadioMoon-<ver>-linux-<arch>-portable.tar.gz`, `radiomoon_<ver>-1_<arch>.deb`, `radiomoon-<ver>-1.<arch>.rpm` |
| macOS (Apple Silicon) | `RadioMoon-<ver>-macos-arm64.dmg` |

## The database

RadioMoon keeps everything in a MariaDB / MySQL database.

- **Windows x64:** MariaDB is included. The setup can install it as the Windows service `RadioMoonDB` (port 3307, local connections only, starts with Windows, data in `%PROGRAMDATA%\RadioMoon\data`). The portable folder has its own `mariadb` folder.
- **Everything else:** install `mariadb-server` (or `mysql-server`) and enter its address in **Control Panel > Database server**.

## Portable or installed

- **Portable:** the folder contains `portable.flag`; all the data (database, backups, reports, settings, `config.json`) stays in the folder. Copy the folder to move the station.
- **Installed:** the data is per user: `%PROGRAMDATA%\RadioMoon` (Windows), `~/Library/Application Support/RadioMoon` (macOS), `~/.local/share/RadioMoon` (Linux). Uninstalling or upgrading never deletes it.

## The Windows setup

The setup asks you to accept the licence, installs into *Program Files* and keeps the data in *ProgramData*. It is in English; the program itself is English or Greek.

## First start

Set the language and the station name in **Control Panel > General**, choose the sound cards in **Settings > Sound outputs**, then follow [[First Steps]].

## Sound outputs

**Main output** is the on-air sound (it also goes to the internet broadcast). **Monitor / headphones** is the preview. **Instant jingles output** can be a separate card.

![Sound outputs](images/screens/cp-outputs.png)

**Auto volume** keeps all songs at the same loudness (target level, maximum boost for quiet songs, maximum reduction for loud songs).
