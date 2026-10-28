# Effects (CLAP)

**Control Panel > Settings > Effects (CLAP)** loads **CLAP** plug-ins (the open plug-in standard, cleveraudio.org) on the on-air mix: the sound goes through the chain **before the sound card and before the internet broadcast**.

![Effects](images/screens/cp-clap.png)

- **Scan** the CLAP folders (`~/.clap`, `/usr/lib/clap`, `Common Files\CLAP`, `CLAP_PATH`, the plug-in folder of the program). A file that cannot be opened (made for another system, for example a Windows `.clap` on a Linux server, or a glibc plug-in on Alpine) is listed with the reason, in red.
- **Add** effects, set their parameters with sliders, **bypass**, change the order, remove.
- **Open window** opens the window of a plug-in that has one of its own (on the computer where the plug-in runs, so not for a server without a screen).
- **Upload plugin files...** copies `.clap` files to the plug-in folder, or uploads them to the server (`plugins/clap` of its data).
- The chain is saved in the settings and loaded at every start.

On a server the plug-ins run on the server; build or take the plug-in for the **same system and libc** as the server.
