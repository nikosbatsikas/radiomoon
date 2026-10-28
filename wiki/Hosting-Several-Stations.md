# Hosting Several Stations

For a company that hosts radios for customers. One `radiomoon-server` process is **one station**: its own database, data folder and web port. To host several stations run **one instance per station**.

> The installer (`install.sh`) sets up **one** instance. The layout below is how to run more by hand with the options the server already has (`--bind`, `--data`, `--db-name`, `--db-user`, `--db-password`). Try it on a test machine before you use it for customers.

## 1. What each station has

| | Station A | Station B |
|---|---|---|
| Web port | `127.0.0.1:8081` | `127.0.0.1:8082` |
| Data folder | `/var/lib/radiomoon-a` | `/var/lib/radiomoon-b` |
| Database | `rm_a` | `rm_b` |
| Database user | `rm_a` | `rm_b` |
| Public name | `a.radios.example.com` | `b.radios.example.com` |

The stations share the machine, MariaDB and the reverse proxy, but **not** their data. The web login (`webserver.json`), the users, the settings and the songs of each one are separate.

## 2. Make a station

```sh
# database and user
mariadb -e "CREATE DATABASE rm_a CHARACTER SET utf8mb4;
            CREATE USER 'rm_a'@'127.0.0.1' IDENTIFIED BY 'a-long-secret';
            GRANT ALL ON rm_a.* TO 'rm_a'@'127.0.0.1'; FLUSH PRIVILEGES;"
# data folder
install -d -o radiomoon -g radiomoon /var/lib/radiomoon-a
# create the tables and remember the database settings
su -s /bin/sh radiomoon -c "RADIOMOON_DATA=/var/lib/radiomoon-a /opt/radiomoon/radiomoon-server --setup \
    --db-host 127.0.0.1 --db-port 3306 --db-user rm_a --db-password a-long-secret --db-name rm_a"
```

A systemd **template unit** `/etc/systemd/system/radiomoon@.service` runs any station by its name:

```ini
[Unit]
Description=RadioMoon station %i
After=network-online.target mariadb.service
Wants=network-online.target

[Service]
User=radiomoon
Group=radiomoon
Environment=RADIOMOON_DATA=/var/lib/radiomoon-%i
EnvironmentFile=/etc/radiomoon/%i.env
ExecStart=/opt/radiomoon/radiomoon-server
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

with `/etc/radiomoon/a.env` holding `RADIOMOON_BIND=127.0.0.1:8081`. Then `systemctl enable --now radiomoon@a`. The password of the web studio of the station is in `/var/lib/radiomoon-a/webserver.json`.

## 3. The reverse proxy

One name per station, HTTPS for all (Caddy):

```
a.radios.example.com { reverse_proxy 127.0.0.1:8081 }
b.radios.example.com { reverse_proxy 127.0.0.1:8082 }
```

A customer uses the RadioMoon client with `https://a.radios.example.com`.

## 4. Isolation

- Better isolation: **one container or VM per station** (LXC, Proxmox, a small cloud instance). Then the standard installer is used unchanged inside each one and a problem of one station cannot touch another.
- On one machine: one database user per station (as above), one data folder per station, and limits per service (`MemoryMax=`, `CPUQuota=` in the unit).
- Do not give customers a shell on the machine. They need only their web studio or client.

## 5. Sizing a host

Count the **stream encoders** (every stream server entry of every station is one MP3 encoder) and the **upstream bandwidth**: sum of the bitrates. 20 stations with one 128 kbps output each send 20 × 16 kB/s = 320 kB/s (about 2.6 Mbit/s) of source traffic. The listeners are on the streaming server, not on this host: see [[Internet Radio Design]].

## 6. Updates and backups

- **Updates:** the `Update` button updates the program of the **machine**, which every instance shares; all the stations restart on the new version. Plan a window and tell the customers. (Run one container per station if they must update separately.)
- **Backups:** every station makes its own backup into its data folder. Back up `/var/lib/radiomoon-*/media`, `/var/lib/radiomoon-*/backups` and the databases (`mariadb-dump`) off the machine every night.
- Also see the checklist in [[Cloud Hosting Design]].

## 7. Not included

A central panel to create stations, usage numbers or invoices does not exist; the commands above are what a provider would script. Ideas for the future are open for discussion.
