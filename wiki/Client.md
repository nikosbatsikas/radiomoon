# Client

The **RadioMoon client** (`radiomoon-client`; in the Windows setup *RadioMoon Client*) is the same studio and control panel as the desktop program, but the **server plays**. Closing the client never stops the music or the internet broadcast.

Download it from the **Download client** button of the web studio, or from the Releases page.

![Client sign-in](images/screens/client-login.png)

## Connect

Enter the **server address** (`http://server:8080`, or `https://...` behind a reverse proxy), the **user name**, the **password** and, if the user has it, the **two-step code**. *Remember the password on this computer* keeps the password (encrypted); the list next to the address has the servers used before. The window opens maximized.

Nothing else is needed: no database address and no open database port. The database of the server is reached through the server itself.

## Same version as the server

The client and the server must be the same version. If they are not, the client downloads the client of the server (newer or older), replaces itself and starts again ("Updating the client to version ..."). If that is not possible it connects anyway and the status line says that the versions differ.

## Connection quality

*Normal*, *Low bandwidth* or *Very low bandwidth* (in the sign-in window and at the bottom of the studio). The status bar shows the download and upload rate.

## What the client does and what the server does

- The server does the heavy work: waveform (a few KB), auto cue, tags and **normalize volume**.
- A song is downloaded only when you preview it, into a cache of this computer (files older than a day are removed; at most 1 GB). **Control Panel > Reset database** shows its size and has *Clear the cache now*.
- Songs you add are **uploaded** to the server, into one folder per library under `media`.
- Each server has its own folder for settings and cache, so two servers never show each other's data.

## Sound outputs

**Control Panel > Settings > Sound outputs** shows the sound cards **of the server** (what it plays on; none on a cloud server) and, in a separate card, the **output of this computer for listening** to songs and previews.

![Sound outputs of the client](images/screens/cp-outputs.png)

## Server update

**Control Panel > Settings > Server update** (administrators): see [[Updating]].
