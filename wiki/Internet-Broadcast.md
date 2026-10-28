# Internet Broadcast

**Control Panel > Settings > Internet broadcast** sends the on-air mix to one or more servers **at the same time**.

![Internet broadcast](images/screens/cp-stream.png)

## Server types

| Type | Settings |
|---|---|
| **Icecast 2** | host, port, mount, user, password |
| **Shoutcast v1** | host, port, password |
| **Shoutcast v2** | host, port, password, **stream ID** |

The Shoutcast source connects to the port **+1** of the listener port. The format is **MP3**, 48 to 320 kbps. Each server has its own name, genre, description and URL, and a *public* option. The **Show password** box next to the password shows it while you type.

Per server: **enabled**, **start automatically** (with the program) and a status line. The title of the song that is playing is sent to every server. Everything played on the main output is in the stream; the preview (monitor) output is not.

## Listeners

The number of listeners is read from the stream servers every 20 seconds (Icecast `status-json.xsl`, Shoutcast v2 `stats`, Shoutcast v1 `7.html`). The page shows it for each server and the **header of the studio shows the sum**.

## Good to know

- Each server entry has its **own MP3 encoder**. Two servers at 128 kbps use twice the CPU and twice the upstream bandwidth of one; see [[Cloud Hosting Design]] for the numbers.
- A server on a cloud machine without a sound card sends exactly the same stream.
- Closing a [[Client]] never stops the broadcast.

Designs for relays, backup servers and listeners: [[Internet Radio Design]].
