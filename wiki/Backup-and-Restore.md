# Backup and Restore

**Control Panel > Settings > Backup & restore**.

![Backup](images/screens/cp-backup.png)

- **Automatic backup:** every day at a time (default 03:00), keeps the last 14. A backup is also made before the database is upgraded to a new version.
- **Back up now** and **Restore** from any `.rmbak` file. Restoring first saves a safety copy, `before-restore.rmbak`.
- A backup contains the libraries, schedules, playlists, campaigns, users, settings and the play history. **It does not contain the audio files.**

On a server the backups are on the **server**, in its `backups` folder; in the client the folder field is not shown. Keep a copy somewhere else too: a snapshot of the machine and a copy of `/var/lib/radiomoon/media` (the songs) and `/var/lib/radiomoon/backups`. See [[Cloud Hosting Design]].
