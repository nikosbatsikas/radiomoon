# Docker

The server edition runs in a container: the **RadioMoon server** and a **MariaDB** database, started together with Docker Compose. No sound card is needed; the container only sends the stream to your Icecast / Shoutcast server (see [[Internet Broadcast]]).

## Start

Download `docker-compose.yml` from the repository (or copy it from below) and run:

```sh
docker compose up -d
```

Then open `http://<computer>:8080/` and sign in as `admin`. The password:

- is the value of `WEB_PASSWORD` if you set it (`WEB_PASSWORD=secret docker compose up -d`, at the **first** start), or
- is random: `docker compose exec radiomoon cat /data/webserver.json`.

Use the [[Client]] or the [[Web Studio]] from here on; the first steps are in [[First Steps]].

## The image

`ghcr.io/nikosbatsikas/radiomoon-server:<version>` (and `:latest`), for **x64 and arm64** (a Raspberry Pi 4/5 with a 64-bit system works). It is published by the release workflow. If the package is private, make it public (GitHub > the repository's Packages > the package > Package settings) or run `docker login ghcr.io`.

## What is where

| | |
|---|---|
| Port | `8080` (the web studio and the client). Put an HTTPS reverse proxy in front for the internet: [[Cloud Hosting Design]] |
| Volume `data` (`/data`) | settings, `webserver.json`, the songs (`media`), the backups, CLAP plugins |
| Volume `db` | the MariaDB database |
| `DB_PASSWORD` | password of the database user (default `radiomoon`; change it before the first start) |
| `TZ` | time zone of the logs and the clocks, for example `Europe/Athens` |

Settings of the server container (environment): `RADIOMOON_DB_HOST` (default `db`), `RADIOMOON_DB_PORT`, `RADIOMOON_DB_USER`, `RADIOMOON_DB_PASSWORD`, `RADIOMOON_DB_NAME`, `RADIOMOON_WEB_PASSWORD`, `RADIOMOON_BIND`. To use a MariaDB / MySQL you already have, point `RADIOMOON_DB_HOST` to it and remove the `db` service.

## Update

The **Update** button of the web studio does not replace the program inside a container; it says so. Update the image instead:

```sh
docker compose pull
docker compose up -d
```

The database is upgraded when the new version starts, after a safety backup (see [[Updating]]).

## Backup

Back up the two volumes (or `docker compose exec radiomoon ...` and the **Backup & restore** page: the `.rmbak` files are in `/data/backups`). The backup of RadioMoon does not contain the songs: they are in `/data/media`. See [[Backup and Restore]].

## Build the image yourself

The release workflow makes the image from the Alpine server program. From a release package:

```sh
tar xzf RadioMoon-Server-<ver>-linux-alpine-x64.tar.gz
mkdir -p dk/ctx/amd64 && cp RadioMoon-Server/radiomoon-server dk/ctx/amd64/
cp -r RadioMoon-Server/lang RadioMoon-Server/clients dk/
cp packaging/docker/Dockerfile packaging/docker/entrypoint.sh dk/     # from the repository
docker build --build-arg TARGETARCH=amd64 -t radiomoon-server dk
```

## Limits

- Sound cards of the host are not used: this is the headless server (no local sound output).
- One container is one station. For several stations run several `radiomoon` services, each with its own `data` volume, database name and port: [[Hosting Several Stations]].
- CLAP plugins must be built for Alpine (musl); glibc plugins do not load. The Effects page lists why.
