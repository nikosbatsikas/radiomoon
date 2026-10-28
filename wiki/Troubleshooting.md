# Troubleshooting

| Problem | What to do |
|---|---|
| I do not know the web password | `grep password /var/lib/radiomoon/webserver.json` on the server. |
| The page does not open | Is the service running (`systemctl status radiomoon-server`, Alpine: `rc-service radiomoon-server status`)? Is the port open in the firewall and in the cloud provider's security group? Is `RADIOMOON_BIND` in `/etc/radiomoon/server.env` right? |
| "Wrong user name, password or code" | Check the user and password. With two-step login the code must be the **current** one and typed right after the password; the clock of the phone must be right. After 5 wrong codes wait 5 minutes. |
| The client says the versions differ | It tries to install the client of the server. If that fails (no package for this system, no network) update the client by hand from the **Download client** button. |
| The server cannot update itself | The program folder is not writable for the service user: run `sudo ./install.sh upgrade` once from the newest package. The update also needs the server to reach `github.com`. |
| No sound on a server | A cloud server has no sound card: the sound goes to the **internet broadcast**. Listen to the stream, or in the client choose an output of your own PC for previews. |
| A CLAP plug-in is red in the list | It was made for another system or libc; the list says why. |
| `Missing file` in Central | The audio file was moved or deleted. Use **Utilities > Integrity check**. |
| The stream does not connect | Check the type, host, port, mount and password. Shoutcast connects to port + 1. The status line of the server says what failed. |
| Listeners show "-" | The statistics page of the stream server cannot be read (password or address); the stream itself can still be fine. |
| MariaDB does not start (Alpine) | The installer switches off `skip-networking`; check `/etc/my.cnf.d`. |

**Logs:** systemd `journalctl -u radiomoon-server`; Alpine `/var/log/radiomoon-server.log`; desktop program: the `logs` folder.
