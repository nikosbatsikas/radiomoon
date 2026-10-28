# First Steps

From an empty program to the first automatic hour.

## 1. Put the music in

**Control Panel > Libraries > Songs > Import**: choose the folder of your songs. Tick **auto cue** to let RadioMoon find the mix points (where a song starts and where the next one may begin). Do the same for **Jingles**, **Spots**, **Sweepers** and **Voice tracks**.

![Songs library](images/screens/cp-songs.png)

- The file tags (artist, title) are read when you import; you can change them later in the edit window of a song.
- With a server, the files are uploaded to it (one folder per library under `media`) and the work on them (waveform, auto cue, tags, normalize) is done by the server.

## 2. Make a clock and a schedule

**Control Panel > Scheduling > Main auto schedule**.

1. Make a **clock**: the rows of one hour (for example song, song, jingle, song, spot break).
2. In the palette on the left pick the clock as a *brush*, then click or drag over the cells of the grid (weekday × half hour). *none* erases. Click a day name to fill the day, an hour label to fill that hour on every day.

![Main auto schedule](images/screens/cp-clocks.png)

## 3. Spots

**Spots > Add new campaign**: choose the spots, the days and the break times, and the priority. See [[Spots and Campaigns]].

## 4. Press AUTO

In the header of the studio press **AUTO**. The next two hours are built from the schedule and played, with automatic mixing between the songs. **MANUAL** plays only what you put in the playlist, **OFFLINE** stops the automation.

## 5. Go on the internet

**Control Panel > Settings > Internet broadcast**: add your Icecast or Shoutcast server. See [[Internet Broadcast]].

## Next

[[Studio Screen]] explains every part of the studio window. For a server in the cloud read [[Server Installation]] and [[Cloud Hosting Design]].
