# Live Rebroadcast

**Control Panel > Scheduling > Live rebroadcast**.

![Live rebroadcast](images/screens/cp-live.png)

- **Sources:** add as many as you need. A source is a **sound card input** or an **internet radio stream** (a URL, MP3 or AAC over HTTP or HTTPS; HLS is not supported), with its own level. **Go live** puts it on the air at once.
- **Schedule:** each entry has a source, the days, a start and an end time (HH:MM, over midnight is allowed) and whether to mute the automation. The source goes on the air at the start and off at the end.
- **LIVE button** in the studio header: click to start or stop the selected source; right-click to choose another. Stopping a scheduled source by hand keeps it off until its slot ends.
- The live audio is mixed into the main output, so it is also in the **internet broadcast**. The player shows a blinking **LIVE** with the name of the source. With *Stop automation* the playlist stops and the studio goes to MANUAL while the source is live; afterwards the previous mode returns and AUTO continues with the next item.

## A guest or a presenter from another place

Give the presenter an Icecast (or Shoutcast) mount on a streaming server. Add the URL of the mount as a source, and schedule it for the time of the show. See [[Internet Radio Design]].
