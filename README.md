# RadioMoon

Radio automation in Rust with a MariaDB / MySQL database, for **Windows, Linux and macOS** (Greek and English interface).

It plays your songs, jingles, spots and sweepers by clocks and a weekly schedule, mixes them automatically and sends the result to the internet (Icecast / Shoutcast). It runs as one program on a PC, or as a **server** (no screen and no sound card needed) that you control from a browser or from the **RadioMoon client** on any computer.

![The studio](https://raw.githubusercontent.com/wiki/nikosbatsikas/radiomoon/images/screens/client-studio.png)

## Documentation

Everything is in the **[Wiki](https://github.com/nikosbatsikas/radiomoon/wiki)**:

- [First Steps](https://github.com/nikosbatsikas/radiomoon/wiki/First-Steps) · [Installation](https://github.com/nikosbatsikas/radiomoon/wiki/Installation) · [Server Installation](https://github.com/nikosbatsikas/radiomoon/wiki/Server-Installation)
- [Studio Screen](https://github.com/nikosbatsikas/radiomoon/wiki/Studio-Screen) · [Web Studio](https://github.com/nikosbatsikas/radiomoon/wiki/Web-Studio) · [Client](https://github.com/nikosbatsikas/radiomoon/wiki/Client)
- [Scheduling](https://github.com/nikosbatsikas/radiomoon/wiki/Scheduling) · [Internet Broadcast](https://github.com/nikosbatsikas/radiomoon/wiki/Internet-Broadcast) · [Users and Two-step Login](https://github.com/nikosbatsikas/radiomoon/wiki/Users-and-Two-step-Login)
- Design guides: [Cloud Hosting Design](https://github.com/nikosbatsikas/radiomoon/wiki/Cloud-Hosting-Design) · [Internet Radio Design](https://github.com/nikosbatsikas/radiomoon/wiki/Internet-Radio-Design) · [Hosting Several Stations](https://github.com/nikosbatsikas/radiomoon/wiki/Hosting-Several-Stations)
- [Troubleshooting](https://github.com/nikosbatsikas/radiomoon/wiki/Troubleshooting) · [Development and Releases](https://github.com/nikosbatsikas/radiomoon/wiki/Development)

![The web studio](https://raw.githubusercontent.com/wiki/nikosbatsikas/radiomoon/images/screens/web-studio.png)

## Download

Get the newest version from the **[Releases](https://github.com/nikosbatsikas/radiomoon/releases)** page.

| Platform | Package |
|---|---|
| Windows x64, x86, arm64 | installer (`-setup.exe`) or portable (`-portable.zip`) |
| Linux x64, arm64 | portable `.tar.gz`, `.deb`, `.rpm` |
| macOS arm64 | `.dmg` |
| Linux server (headless) | `RadioMoon-Server-<ver>-linux-glibc-<x64\|arm64>.tar.gz`, or `-alpine-` for Alpine |

The Windows x64 packages include the MariaDB database. On every other system install `mariadb-server` (the server package installs it for you).

## Quick start

- **Desktop:** install, then follow [First Steps](https://github.com/nikosbatsikas/radiomoon/wiki/First-Steps): import songs, make a clock, press **AUTO**.
- **Server:** `tar xzf RadioMoon-Server-<ver>-linux-glibc-x64.tar.gz && cd RadioMoon-Server && sudo ./install.sh`, then open `http://<server>:8080/` (user `admin`, the password is in `/var/lib/radiomoon/webserver.json`) or use the client.

**Docker:** `docker compose up -d` with the `docker-compose.yml` of this repository starts the server and its database (see the [wiki page](https://github.com/nikosbatsikas/radiomoon/wiki/Docker)).

## Contact

Nikos Batsikas - [nikos@velikanet.net](mailto:nikos@velikanet.net). Questions, bug reports and ideas are welcome (or open an issue on GitHub).

## License

Copyright (c) 2026 Nikos Batsikas. All rights reserved. See the `LICENSE` file. Third-party libraries keep their own licenses. The changes of every version are in [CHANGELOG.md](CHANGELOG.md).
