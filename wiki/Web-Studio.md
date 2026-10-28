# Web Studio

Every server edition has a studio in the browser: `http://<server>:8080/`.

![Sign in](images/screens/web-login.png)

## Sign in

The page asks for the **user name**, the **password** and, if the user has the [[two-step login|Users and Two-step Login]], the **6 digit code** of the authenticator app. The `admin` user and its password are in `/var/lib/radiomoon/webserver.json`; other users are the RadioMoon users of **Control Panel > Users & security**.

A sign-in stays valid for the browser session. A wrong password waits a moment before it answers, to slow down guessing.

## The page

![Web studio](images/screens/web-studio.png)

- **On air**: the title, time and a progress bar; the buttons Restart, Pause, Next, Stop, Loop, Stop after track, Live; the volume.
- **Mode**: Auto, Manual, Offline.
- **Playlist** with the order buttons and Play / Remove for every row; **Rebuild automatic** and **Clear playlist**.
- **Library**: choose the kind (music, jingle, spot ...), search, add *Next up* or to the end.
- **Instant jingles**: the pallets and their buttons.
- **Internet broadcast**: starts and stops the streams and shows their status.
- **Played**: the history.
- English or Greek.

The browser studio is for control. Importing, the schedule and the settings are in the [[Client]] or the desktop program.

## Download the client

The **Download client** button lists the client programs that came with the server (Windows, Linux, macOS).

![Download client](images/screens/web-download.png)

## Update (administrators)

The **Update** button shows the version of the server and the newest version, and updates the server. See [[Updating]].

![Server update](images/screens/web-update.png)
