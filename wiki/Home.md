# RadioMoon Wiki

**RadioMoon** is radio automation software in Rust with a MariaDB / MySQL database for **Windows, Linux and macOS**, with an English and Greek interface. It plays songs, jingles, spots and sweepers by clocks and a weekly schedule, mixes them automatically, sends the result to the internet (Icecast and Shoutcast) and keeps the logs and reports of the station.

It runs in two ways:

| | **Desktop program** | **Server edition** |
|---|---|---|
| Where | one PC with a sound card | a Linux server, virtual machine or container, **no sound card needed** |
| Studio | the program window | the **web studio** in any browser, or the **RadioMoon client** window on any computer |
| Plays | on the sound card of the PC (and to the internet) | to the internet (and to a sound card, if the server has one) |
| Good for | a studio PC | cloud radios, internet radios, a studio that is not where the music is |

![The studio of the RadioMoon client](images/screens/client-studio.png)

## Start here

- [[First Steps]]: from an empty program to the first automatic hour
- [[Installation]]: Windows, macOS and Linux desktop
- [[Server Installation]]: the headless server (Debian, Ubuntu, AlmaLinux, Alpine, Raspberry Pi)
- [[Docker]]: the server and its database in containers
- [[Cloud Hosting Design]]: how to run a radio in the cloud, and [[Hosting Several Stations]]
- [[Internet Radio Design]]: streaming, relays, backup servers, live guests

## Using the program

- [[Studio Screen]] · [[Web Studio]] · [[Client]]
- [[Libraries]] · [[Scheduling]] · [[Spots and Campaigns]] · [[Live Rebroadcast]]
- [[Internet Broadcast]] · [[Effects CLAP]]
- [[Users and Two-step Login]] · [[Backup and Restore]]
- [[Move to a Server]] · [[Updating]] · [[Import from Jazler]]
- [[Troubleshooting]] · [[Development]] (languages, releases, building)

The newest version is on the **Releases** page of the repository; what changed is in `CHANGELOG.md`.

## Contact

Nikos Batsikas: nikos@velikanet.net
