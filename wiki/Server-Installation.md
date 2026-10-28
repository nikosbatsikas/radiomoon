# Server Installation

The **server edition** is a program without a window: `radiomoon-server`. It plays, mixes and streams; you control it from a browser ([[Web Studio]]) or from the [[Client]]. It needs **no sound card**, so it runs well in a cloud virtual machine, a Proxmox LXC container or a Raspberry Pi.

## Which package

| System | Package |
|---|---|
| Debian 10+, Ubuntu 20.04+, Raspberry Pi OS 64-bit, AlmaLinux / Rocky / RHEL 8+, Fedora | `RadioMoon-Server-<ver>-linux-glibc-<x64\|arm64>.tar.gz` |
| Alpine Linux | `RadioMoon-Server-<ver>-linux-alpine-<x64\|arm64>.tar.gz` |

## Install

```sh
tar xzf RadioMoon-Server-<ver>-linux-glibc-x64.tar.gz
cd RadioMoon-Server
sudo ./install.sh
```

The installer, as root:

1. installs MariaDB (and the audio libraries) with the package manager (`apt`, `dnf` or `apk`) and tunes it for a small server;
2. creates the database `radiomoon` and the database user `radiomoon`, reachable **only from the server itself**;
3. creates the system user `radiomoon`, the folders `/opt/radiomoon` (program), `/var/lib/radiomoon` (data) and `/etc/radiomoon` (settings);
4. installs and starts the service (systemd, or OpenRC on Alpine).

At the end it tells you how to read the web password:

```sh
grep password /var/lib/radiomoon/webserver.json
```

Open `http://<server>:8080/` in a browser and sign in as `admin`.

## Settings of the service

`/etc/radiomoon/server.env`:

```
RADIOMOON_BIND=0.0.0.0:8080
```

To run by hand: `radiomoon-server --bind 0.0.0.0:8080 --data /var/lib/radiomoon --db-user radiomoon --db-password ... --db-name radiomoon`. `radiomoon-server --help` lists all the options.

## Folders

| Folder | What |
|---|---|
| `/opt/radiomoon` | the program, `lang`, and `clients` (the client programs that the web page offers) |
| `/var/lib/radiomoon` | data: `webserver.json` (web login), settings, `media` (uploaded songs, one folder per library), `backups`, `plugins/clap` |
| `/etc/radiomoon/server.env` | address and port of the web studio |
| `/var/log/radiomoon-server.log` | log on Alpine (on systemd use `journalctl -u radiomoon-server`) |

## Upgrade, uninstall

- **Upgrade:** unpack the new package and run `sudo ./install.sh upgrade`. Settings, database and songs are kept; the tables are upgraded after a safety backup. Or use [[Updating]] from the web page.
- **Remove:** `sudo ./install.sh uninstall` keeps the data and the database; `sudo ./install.sh uninstall --purge` removes everything (asks first).

## Security

The web studio is plain HTTP. On the internet put it behind a reverse proxy with HTTPS and turn on the two-step login. See [[Cloud Hosting Design]] and [[Users and Two-step Login]].

## A watchdog

The server restarts itself if its main loop ever stops responding.
