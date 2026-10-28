# Move to a Server

A radio that runs on a desktop PC can be moved to a RadioMoon server in the cloud.

**Control Panel > Settings > Move to a server** (desktop program only).

![Move to a server](images/screens/cp-move.png)

1. Install the server ([[Server Installation]]), **the same version** as the desktop program.
2. In the desktop program enter the **server address**, an **administrator** user and password (and the two-step code if it has one).
3. Tick *I understand that the database of the server is replaced* and press **Send the database and the songs**.

What happens:

- The whole database is sent and **replaces** the database of the server: libraries, schedules, playlists, campaigns, users, settings and history. The sound outputs and the folders of the PC are not taken over.
- Then the audio files of the library are **uploaded one by one**, each into the folder of its library on the server, and the songs get their new place. A file that cannot be uploaded is counted in the final message; the others go on.
- The desktop program is not changed.

When it says *Finished*, connect with the [[Client]].

Other ways in: [[Import from Jazler]] with the upload option, and a backup restored on the server (**Backup & restore**).
