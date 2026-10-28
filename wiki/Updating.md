# Updating

## The server updates itself

Administrators can update the server from the **web studio** (the **Update** button) or from the **client** (Control Panel > Settings > **Server update**).

![Server update](images/screens/cp-update.png)

1. **Look for a new version**: the server asks GitHub for the newest release that has a package for its own system (glibc or Alpine, x64 or arm64) and shows what is new.
2. **Update the server now**: it downloads the package, checks that the new program starts and reports the right version (if not, nothing is changed), puts the new program, its `lang` folder and the clients it offers in place, and starts the new program. The database is upgraded when it starts, after a backup. The **broadcast stops for a moment**.

The program folder must belong to the service user. A server installed with an older package needs `sudo ./install.sh upgrade` once from the newest package; after that the page can update it.

## The client follows the server

When a client connects to a server of another version it downloads the client of that server and replaces itself. See [[Client]].

## By hand

Unpack the new package and run `sudo ./install.sh upgrade`.

## Versions and the database

An older database opened by a newer program is upgraded after a safety backup (`pre-upgrade-<old>-to-<new>.rmbak`). A newer database opened by an older program is refused.
