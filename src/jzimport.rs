//! Import of a Jazler RadioStar 2 database (Access .mdb): music library, artists, categories,
//! spots and campaigns, playlists, instant-jingle pallets and the play history.
use crate::mdb::{date_text, Mdb, Row, Val};
use crate::model::*;
use std::collections::HashMap;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct Opts {
    pub songs: bool,
    pub spots: bool,
    pub playlists: bool,
    pub pallets: bool,
    pub history: bool,
    pub history_days: u32,
    /// Start of the audio file paths in the old database ...
    pub replace_from: String,
    /// ... and what to put there instead (e.g. "D:\" -> "E:\Music\").
    pub replace_to: String,
}

impl Default for Opts {
    fn default() -> Self {
        Opts { songs: true, spots: true, playlists: true, pallets: true, history: true, history_days: 90, replace_from: String::new(), replace_to: String::new() }
    }
}

#[derive(Default)]
pub struct Imported {
    pub tracks: Vec<Track>,
    pub cats: Vec<Category>,
    pub playlists: Vec<Playlist>,
    pub campaigns: Vec<Campaign>,
    pub pallets: Vec<Pallet>,
    /// (track id, "YYYY-MM-DD HH:MM:SS")
    pub history: Vec<(u64, String)>,
    pub report: Vec<String>,
}

fn rows(m: &Mdb, name: &str) -> Vec<Row> {
    m.table(name).map(|t| m.rows(&t)).unwrap_or_default()
}
fn s(r: &Row, k: &str) -> String {
    r.get(k).map(|v| v.text()).unwrap_or_default()
}
fn n(r: &Row, k: &str) -> i64 {
    r.get(k).map(|v| v.int()).unwrap_or(0)
}
fn f(r: &Row, k: &str) -> f64 {
    r.get(k).map(|v| v.float()).unwrap_or(0.0)
}
fn b(r: &Row, k: &str) -> bool {
    r.get(k).map(|v| v.bool()).unwrap_or(false)
}
fn date_of(r: &Row, k: &str) -> String {
    match r.get(k) {
        Some(Val::Date(d)) => date_text(*d),
        _ => String::new(),
    }
}
fn ms(sec: f64) -> u64 {
    if sec > 0.0 {
        (sec * 1000.0).round() as u64
    } else {
        0
    }
}

/// "Not Entered", "-" and similar placeholders of the old program mean "empty".
fn clean(x: &str) -> String {
    let t = x.trim();
    if t == "-" || t.eq_ignore_ascii_case("not entered") || t.eq_ignore_ascii_case("no comments") || t.eq_ignore_ascii_case("no artist") {
        String::new()
    } else {
        t.to_string()
    }
}

fn fix_path(p: &str, from: &str, to: &str) -> String {
    let p = p.trim();
    let from = from.trim();
    if from.is_empty() {
        return p.to_string();
    }
    let k = from.chars().count();
    let head: String = p.chars().take(k).collect();
    if head.to_lowercase() == from.to_lowercase() {
        let rest: String = p.chars().skip(k).collect();
        format!("{to}{rest}")
    } else {
        p.to_string()
    }
}

/// Visual Basic colour ("&H00BBGGRR&" or a decimal number) to RGB.
fn color(x: &str) -> [u8; 3] {
    let t = x.trim().trim_matches('&');
    let v: u32 = if let Some(h) = t.strip_prefix('H').or_else(|| t.strip_prefix('h')) { u32::from_str_radix(h.trim_end_matches('&'), 16).unwrap_or(0xFFFFFF) } else { t.parse().unwrap_or(0xFFFFFF) };
    [(v & 255) as u8, ((v >> 8) & 255) as u8, ((v >> 16) & 255) as u8]
}

fn day_part(dt: &str) -> String {
    dt.chars().take(10).collect()
}
fn hm_part(dt: &str) -> String {
    dt.chars().skip(11).take(5).collect()
}

/// Number of items found in the file, for the preview before the import.
pub fn preview(path: &Path) -> Result<Vec<(String, usize)>, String> {
    let m = Mdb::open(path)?;
    if m.table("snDatabase").is_none() && m.table("Media").is_some() {
        let media = rows(&m, "Media");
        let count = |f: &dyn Fn(i64) -> bool| media.iter().filter(|r| f(n(r, "MediaType"))).count();
        return Ok(vec![
            ("MegaMix songs".to_string(), count(&|t| t == 0)),
            ("MegaMix jingles / effects".to_string(), count(&|t| t != 0 && t != 6)),
            ("MegaMix commercials".to_string(), count(&|t| t == 6)),
            ("Artists".to_string(), rows(&m, "Artist").len()),
            ("Playlists".to_string(), rows(&m, "PlayListBase").len()),
        ]);
    }
    let mut v = Vec::new();
    for (label, table) in [("Songs", "snDatabase"), ("Artists", "snArtists"), ("Spots", "Com_Files"), ("Campaigns", "Com_List"), ("Playlists", "Playlists_Titles"), ("Jingle pallet buttons", "PalleteData"), ("Play history entries", "snMusicBroadcasts")] {
        let c = m.table(table).map(|t| m.rows(&t).len()).unwrap_or(0);
        v.push((label.to_string(), c));
    }
    if v[0].1 == 0 && m.table("snDatabase").is_none() {
        return Err("This does not look like a Jazler RadioStar 2 database (table snDatabase not found).".into());
    }
    Ok(v)
}

/// Reads the database. `existing` maps lower-case file paths of the current library to their ids
/// (such files are not added twice); new tracks get ids starting at `first_id`.
pub fn read(path: &Path, o: &Opts, existing: &HashMap<String, u64>, first_id: u64) -> Result<Imported, String> {
    let m = Mdb::open(path)?;
    if m.table("snDatabase").is_none() && m.table("Media").is_some() {
        return read_megamix(&m, o, existing, first_id);
    }
    if m.table("snDatabase").is_none() {
        return Err("This does not look like a Jazler RadioStar 2 or MegaMix database (tables snDatabase / Media not found).".into());
    }
    let mut out = Imported::default();
    let mut next = first_id;
    let mut by_path: HashMap<String, u64> = existing.clone();
    let mut dup = 0usize;

    // ---- categories ----
    let mut names: HashMap<&str, HashMap<i64, String>> = HashMap::new();
    for (lib, table, namecol, colorcol) in [
        ("song1", "snCat1", "fldMusicType", "fldColor"),
        ("song2", "snCat2", "fldMusicType", "fldColor"),
        ("song3", "snCat3", "fldMusicType", "fldColor"),
        ("vocal", "snVocalPresent", "fldVocalPresent", ""),
        ("props", "snProperties", "fldPropertyName", ""),
        ("spot", "Com_Categories", "fldSpotCategory", ""),
        ("jingle", "Elements_Jingles_Categories", "fldCategoryName", "fldColor"),
        ("sweeper", "Elements_Sweepers_Categories", "fldCategory", ""),
        ("voice", "Elements_VTracks_Voices", "fldVoiceName", ""),
    ] {
        let mut map = HashMap::new();
        let mut count = 0;
        for r in rows(&m, table) {
            let name = clean(&s(&r, namecol));
            if name.is_empty() {
                continue;
            }
            map.insert(n(&r, "AUID"), name.clone());
            out.cats.push(Category { id: 0, lib: lib.to_string(), name, color: if colorcol.is_empty() { [90, 110, 140] } else { color(&s(&r, colorcol)) }, comment: clean(&s(&r, "fldComments")) });
            count += 1;
        }
        let _ = count;
        names.insert(lib, map);
    }
    let nm = |lib: &str, id: i64| -> String { names.get(lib).and_then(|x| x.get(&id)).cloned().unwrap_or_default() };

    // ---- artists ----
    let mut artists: HashMap<i64, String> = HashMap::new();
    for r in rows(&m, "snArtists") {
        let name = format!("{} {}", clean(&s(&r, "fldName")), clean(&s(&r, "fldSurname")));
        artists.insert(n(&r, "AUID"), name.trim().to_string());
    }

    // ---- songs ----
    let mut song_ids: HashMap<i64, u64> = HashMap::new();
    if o.songs || o.playlists || o.history {
        let mut imported = 0usize;
        for r in rows(&m, "snDatabase") {
            let old = n(&r, "AUID");
            let p = fix_path(&s(&r, "fldFilename"), &o.replace_from, &o.replace_to);
            if p.is_empty() {
                continue;
            }
            let key = p.to_lowercase();
            if let Some(id) = by_path.get(&key) {
                song_ids.insert(old, *id);
                dup += 1;
                continue;
            }
            if !o.songs {
                continue;
            }
            let dur = f(&r, "fldDuration");
            let mix = f(&r, "fldMixPos");
            let artist = {
                let a = clean(&s(&r, "fldArtistName"));
                if a.is_empty() { artists.get(&n(&r, "fldArtistCode")).cloned().map(|x| clean(&x)).unwrap_or_default() } else { a }
            };
            let vol = n(&r, "fldVolume");
            let gain = if vol > 0 && vol != 100 { (20.0 * (vol as f64 / 100.0).log10()).clamp(-30.0, 30.0) as f32 } else { 0.0 };
            let year = n(&r, "fldYear");
            let intro = f(&r, "fldIntroPos");
            let t = Track {
                id: next,
                kind: "music".into(),
                title: s(&r, "fldTitle"),
                artist,
                album: clean(&s(&r, "fldAlbum")),
                genre: String::new(),
                year: if year > 0 { year.to_string() } else { String::new() },
                path: p,
                duration_ms: ms(dur),
                cat1: nm("song1", n(&r, "fldCat1a")),
                cat2: nm("song2", n(&r, "fldCat2")),
                cat3: nm("song3", n(&r, "fldCat3")),
                vocal: nm("vocal", n(&r, "fldVocalPresent")),
                props: clean(&s(&r, "fldProperties")),
                enabled: b(&r, "fldEnabled"),
                bookmarked: false,
                intro_ms: if intro > 0.0 && intro < dur { ms(intro) } else { 0 },
                mix_in_ms: ms(f(&r, "fldStartPos")),
                mix_out_ms: if mix > 0.0 && mix < dur - 0.05 { ms(mix) } else { 0 },
                hook_in_ms: 0,
                hook_out_ms: 0,
                gain_db: gain,
                sep_min: n(&r, "fldLeastBroadcast").clamp(0, 10080) as u32,
                plcode: clean(&s(&r, "fldPlaylisterCode")),
                notes: clean(&s(&r, "fldComments")),
                variable_len: false,
                ..Default::default()
            };
            by_path.insert(key, next);
            song_ids.insert(old, next);
            out.tracks.push(t);
            next += 1;
            imported += 1;
        }
        if o.songs {
            out.report.push(format!("Songs: {imported} imported, {dup} already in the library"));
        }
    }

    // ---- spots and campaigns ----
    if o.spots {
        let list = rows(&m, "Com_List");
        let clients: HashMap<i64, String> = rows(&m, "Com_Clients").iter().map(|r| (n(r, "AUID"), clean(&s(r, "fldClientCompanyName")))).collect();
        let mut camp_cat: HashMap<i64, String> = HashMap::new();
        for r in &list {
            camp_cat.insert(n(r, "AUID"), nm("spot", n(r, "fldCategory")));
        }
        let mut spot_ids: HashMap<i64, Vec<u64>> = HashMap::new();
        let mut added = 0usize;
        for r in rows(&m, "Com_Files") {
            let p = fix_path(&s(&r, "fldFilename"), &o.replace_from, &o.replace_to);
            if p.is_empty() {
                continue;
            }
            let key = p.to_lowercase();
            let camp = n(&r, "fldReferencedCom");
            let id = if let Some(id) = by_path.get(&key) {
                *id
            } else {
                let dur = f(&r, "fldDuration");
                let mix = f(&r, "fldMixPos");
                let vol = n(&r, "fldVolume");
                let title = {
                    let a = clean(&s(&r, "fldAlias"));
                    if a.is_empty() { clean(&s(&r, "fldComTitle")) } else { a }
                };
                out.tracks.push(Track {
                    id: next,
                    kind: "spot".into(),
                    title: if title.is_empty() { p.rsplit(['\\', '/']).next().unwrap_or("").to_string() } else { title },
                    path: p,
                    duration_ms: ms(dur),
                    cat1: camp_cat.get(&camp).cloned().unwrap_or_default(),
                    enabled: true,
                    mix_in_ms: ms(f(&r, "fldStartPos")),
                    mix_out_ms: if mix > 0.0 && mix < dur - 0.05 { ms(mix) } else { 0 },
                    gain_db: if vol > 0 && vol != 100 { (20.0 * (vol as f64 / 100.0).log10()).clamp(-30.0, 30.0) as f32 } else { 0.0 },
                    plcode: clean(&s(&r, "fldPlaylisterCode")),
                    ..Default::default()
                });
                by_path.insert(key, next);
                next += 1;
                added += 1;
                next - 1
            };
            spot_ids.entry(camp).or_default().push(id);
        }
        let mut camps = 0usize;
        for r in &list {
            let id = n(r, "AUID");
            let end = date_of(r, "fldExpires");
            let mut slots = Vec::new();
            for tm in s(r, "fldIndexDisplayTimes").split(|c: char| c.is_whitespace() || c == ',' || c == ';').filter(|x| x.len() >= 4 && x.contains(':')) {
                let tm = if tm.len() == 4 { format!("0{tm}") } else { tm.to_string() };
                for wd in 0..7 {
                    slots.push(Slot { date: String::new(), weekday: wd, time: tm.clone(), priority: 1, spot: 0 });
                }
            }
            let client = clients.get(&n(r, "fldClientRef")).cloned().unwrap_or_default();
            let mut notes = clean(&s(r, "fldComments"));
            if !client.is_empty() {
                notes = format!("{notes}{}Client: {client}", if notes.is_empty() { "" } else { "\n" });
            }
            out.campaigns.push(Campaign {
                id: 0,
                name: s(r, "fldTitle"),
                category: camp_cat.get(&id).cloned().unwrap_or_default(),
                start: day_part(&date_of(r, "fldStarts")),
                end: day_part(&end),
                never: end.is_empty() || end.chars().take(4).collect::<String>().parse::<i32>().unwrap_or(0) >= 2099,
                enabled: n(r, "fldStatus") != 0,
                mode: "weekly".into(),
                system: "order".into(),
                spots: spot_ids.get(&id).cloned().unwrap_or_default(),
                slots,
                notes,
            });
            camps += 1;
        }
        out.report.push(format!("Spots: {added} imported; campaigns: {camps}"));
    }

    // ---- playlists ----
    if o.playlists {
        let mut items: HashMap<i64, Vec<(i64, u64)>> = HashMap::new();
        let mut skipped = 0usize;
        for r in rows(&m, "Playlists_Data") {
            match song_ids.get(&n(&r, "fldTrackAUID")) {
                Some(id) => items.entry(n(&r, "fldPlaylistAUID")).or_default().push((n(&r, "fldPlaylistOrder"), *id)),
                None => skipped += 1,
            }
        }
        let mut sched: HashMap<i64, Vec<PSched>> = HashMap::new();
        for r in rows(&m, "Playlists_DateTimes") {
            let dt = date_of(&r, "fldDateTime");
            if dt.is_empty() {
                continue;
            }
            sched.entry(n(&r, "OrigAUID")).or_default().push(PSched { date: day_part(&dt), weekdays: 0, time: hm_part(&dt), immediate: n(&r, "fldImmediate") != 0 });
        }
        let mut count = 0usize;
        for r in rows(&m, "Playlists_Titles") {
            let id = n(&r, "AUID");
            let mut it = items.remove(&id).unwrap_or_default();
            it.sort_by_key(|x| x.0);
            out.playlists.push(Playlist {
                id: 0,
                name: s(&r, "fldTitle"),
                random: false,
                override_prev: false,
                items: it.into_iter().map(|x| PItem { track_id: x.1, ..Default::default() }).collect(),
                sched: sched.remove(&id).unwrap_or_default(),
            });
            count += 1;
        }
        out.report.push(format!("Playlists: {count}{}", if skipped > 0 { format!(" ({skipped} items skipped: song not in the library)") } else { String::new() }));
    }

    // ---- instant jingle pallets ----
    if o.pallets {
        let titles: HashMap<i64, String> = rows(&m, "Pallettes").iter().map(|r| (n(r, "AUID"), s(r, "PalTitle"))).collect();
        let mut by_pal: HashMap<i64, Vec<Row>> = HashMap::new();
        for r in rows(&m, "PalleteData") {
            by_pal.entry(n(&r, "PalleteID")).or_default().push(r);
        }
        let mut ids: Vec<i64> = titles.keys().copied().collect();
        ids.sort();
        let mut nb = 0usize;
        for pid in ids {
            let btns = by_pal.remove(&pid).unwrap_or_default();
            let maxpos = btns.iter().map(|r| n(r, "ButtonPosition")).max().unwrap_or(0).max(0) as usize;
            let (rows_n, cols_n) = (4usize, 4usize);
            let pages = ((maxpos + 1 + rows_n * cols_n - 1) / (rows_n * cols_n)).clamp(1, 5);
            let mut pal = Pallet { id: 0, name: titles[&pid].clone(), pages, rows: rows_n, cols: cols_n, buttons: vec![PBtn { track_id: 0, color: [60, 70, 90], label: String::new() }; pages * rows_n * cols_n] };
            for r in btns {
                let p = fix_path(&s(&r, "Filename"), &o.replace_from, &o.replace_to);
                let pos = n(&r, "ButtonPosition").max(0) as usize;
                if p.is_empty() || pos >= pal.buttons.len() {
                    continue;
                }
                let key = p.to_lowercase();
                let id = if let Some(id) = by_path.get(&key) {
                    *id
                } else {
                    let dur_txt = s(&r, "Duration");
                    let parsed = dur_txt.trim_end_matches('s').replace(',', ".").trim().parse::<f64>().unwrap_or(0.0);
                    let dur = if parsed > 0.0 { parsed } else { f(&r, "MixTime") };
                    out.tracks.push(Track {
                        id: next,
                        kind: "jingle".into(),
                        title: s(&r, "Title"),
                        path: p,
                        duration_ms: ms(dur),
                        enabled: true,
                        mix_in_ms: ms(f(&r, "StartTime")),
                        ..Default::default()
                    });
                    by_path.insert(key, next);
                    next += 1;
                    next - 1
                };
                let col: u32 = s(&r, "Color").parse().unwrap_or(0x00FF_FFFF);
                pal.buttons[pos] = PBtn { track_id: id, color: [(col & 255) as u8, ((col >> 8) & 255) as u8, ((col >> 16) & 255) as u8], label: s(&r, "Title") };
                nb += 1;
            }
            out.pallets.push(pal);
        }
        out.report.push(format!("Jingle pallets: {} with {nb} buttons", out.pallets.len()));
    }

    // ---- play history (keeps the rotation rules working right after the move) ----
    if o.history && o.history_days > 0 {
        let now_days = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64() / 86400.0).unwrap_or(0.0) + 25569.0;
        let from = now_days - o.history_days as f64;
        let mut c = 0usize;
        for r in rows(&m, "snMusicBroadcasts") {
            if let Some(Val::Date(d)) = r.get("fldDateTime") {
                if *d >= from && *d <= now_days + 1.0 {
                    if let Some(id) = song_ids.get(&n(&r, "SongID")) {
                        out.history.push((*id, date_text(*d)));
                        c += 1;
                    }
                }
            }
        }
        out.history.sort_by(|a, b| a.1.cmp(&b.1));
        out.report.push(format!("Play history: {c} entries of the last {} days", o.history_days));
    }
    Ok(out)
}

/// MegaMix database: songs, jingles / effects and commercials (table Media), artists, styles, categories, eras and playlists.
/// Times are in whole seconds. The music sequence rules (MusicSeq*) are not converted.
fn read_megamix(m: &Mdb, o: &Opts, existing: &HashMap<String, u64>, first_id: u64) -> Result<Imported, String> {
    let mut out = Imported::default();
    let mut next = first_id;
    let mut by_path: HashMap<String, u64> = existing.clone();
    let mut names: HashMap<&str, HashMap<i64, String>> = HashMap::new();
    for (lib, table, idcol) in [("song1", "Style", "StyleID"), ("song2", "Category", "CategoryID"), ("song3", "Era", "EraID")] {
        let mut map = HashMap::new();
        for r in rows(m, table) {
            let name = clean(&s(&r, "Title"));
            if name.is_empty() {
                continue;
            }
            map.insert(n(&r, idcol), name.clone());
            out.cats.push(Category { id: 0, lib: lib.to_string(), name, color: [90, 110, 140], comment: String::new() });
        }
        names.insert(lib, map);
    }
    let nm = |lib: &str, id: i64| -> String { names.get(lib).and_then(|x| x.get(&id)).cloned().unwrap_or_default() };
    let artists: HashMap<i64, String> = rows(m, "Artist").iter().map(|r| (n(r, "ArtistID"), clean(&s(r, "Name")))).collect();
    let mut ids: HashMap<i64, u64> = HashMap::new();
    let (mut songs, mut other, mut spots, mut dup) = (0usize, 0usize, 0usize, 0usize);
    for r in rows(m, "Media") {
        let old = n(&r, "MediaID");
        let p = fix_path(&s(&r, "MediaFile"), &o.replace_from, &o.replace_to);
        let p = if p.is_empty() { fix_path(&s(&r, "URLMediaFile"), &o.replace_from, &o.replace_to) } else { p };
        if p.is_empty() {
            continue;
        }
        let key = p.to_lowercase();
        if let Some(id) = by_path.get(&key) {
            ids.insert(old, *id);
            dup += 1;
            continue;
        }
        let typ = n(&r, "MediaType");
        let (kind, want) = match typ {
            0 => ("music", o.songs),
            6 => ("spot", o.spots),
            _ => ("jingle", o.songs),
        };
        if !want {
            continue;
        }
        let dur = f(&r, "DurrationLT");
        let mix = f(&r, "MixTimeLT");
        let intro = f(&r, "IntroLT");
        let music = kind == "music";
        out.tracks.push(Track {
            id: next,
            kind: kind.into(),
            title: s(&r, "Title"),
            artist: if music { artists.get(&n(&r, "ArtistID")).cloned().unwrap_or_default() } else { String::new() },
            album: if music { clean(&s(&r, "CDTitle")) } else { String::new() },
            path: p,
            duration_ms: ms(dur),
            cat1: if music { nm("song1", n(&r, "StyleID")) } else { String::new() },
            cat2: if music { nm("song2", n(&r, "CategoryID")) } else { String::new() },
            cat3: if music { nm("song3", n(&r, "EraID")) } else { String::new() },
            enabled: true,
            intro_ms: if intro > 0.0 && intro < dur { ms(intro) } else { 0 },
            mix_in_ms: ms(f(&r, "StartTimeLT")),
            mix_out_ms: if mix > 0.0 && mix < dur - 0.05 { ms(mix) } else { 0 },
            notes: clean(&s(&r, "Other")),
            ..Default::default()
        });
        by_path.insert(key, next);
        ids.insert(old, next);
        match kind {
            "music" => songs += 1,
            "spot" => spots += 1,
            _ => other += 1,
        }
        next += 1;
    }
    out.report.push(format!("MegaMix: {songs} songs, {other} jingles / effects, {spots} commercials imported, {dup} already in the library"));
    if o.playlists {
        let items = rows(m, "PlayListItems");
        for r in rows(m, "PlayListBase") {
            let pid = n(&r, "PlayListBaseID");
            let mut list: Vec<(i64, u64)> = items.iter().filter(|i| n(i, "PlayListBaseID") == pid).filter_map(|i| ids.get(&n(i, "MediaID")).map(|t| (n(i, "PlayListItemID"), *t))).collect();
            list.sort_by_key(|x| x.0);
            let title = clean(&s(&r, "Title"));
            if title.is_empty() || list.is_empty() {
                continue;
            }
            out.playlists.push(Playlist { name: title, random: b(&r, "RandomizeOnStartup"), items: list.into_iter().map(|x| PItem { track_id: x.1, ..Default::default() }).collect(), ..Default::default() });
        }
        out.report.push(format!("Playlists: {}", out.playlists.len()));
    }
    out.report.push("The music sequence rules (MusicSeq*) and customers are not converted; build the clocks in the Main auto schedule.".to_string());
    Ok(out)
}

/// Reads the Jazler RadioStar 2 database and adds its data to the RadioMoon database. Nothing existing is
/// overwritten: tracks with a known path, and categories / playlists / campaigns / pallets with a known name are skipped.
pub fn apply(db: &crate::db::Db, path: &Path, o: &Opts, progress: &dyn Fn(&str)) -> Result<Vec<String>, String> {
    progress("Reading the database file ...");
    let existing: HashMap<String, u64> = db.track_paths().into_iter().collect();
    let first = db.max_track_id() + 1;
    let imp = read(path, o, &existing, first)?;
    let mut rep = imp.report.clone();
    progress("Writing songs and spots ...");
    let mut n = 0usize;
    for (i, t) in imp.tracks.iter().enumerate() {
        if db.restore_track(t).is_ok() {
            n += 1;
        }
        if i % 500 == 0 {
            progress(&format!("Writing tracks: {} / {}", i, imp.tracks.len()));
        }
    }
    rep.push(format!("Tracks added: {n} of {}", imp.tracks.len()));
    progress("Writing categories and lists ...");
    let have_cats: Vec<(String, String)> = db.docs::<crate::model::Category>("category").unwrap_or_default().into_iter().map(|c| (c.lib, c.name)).collect();
    let mut k = 0;
    for mut c in imp.cats {
        if have_cats.iter().any(|h| h.0 == c.lib && h.1.eq_ignore_ascii_case(&c.name)) {
            continue;
        }
        c.id = 0;
        if db.save_doc("category", &mut c).is_ok() {
            k += 1;
        }
    }
    rep.push(format!("Categories added: {k}"));
    let have: Vec<String> = db.docs::<crate::model::Playlist>("playlist").unwrap_or_default().into_iter().map(|x| x.name.to_lowercase()).collect();
    let mut k = 0;
    for mut x in imp.playlists {
        if have.contains(&x.name.to_lowercase()) {
            continue;
        }
        x.id = 0;
        if db.save_doc("playlist", &mut x).is_ok() {
            k += 1;
        }
    }
    rep.push(format!("Playlists added: {k}"));
    let have: Vec<String> = db.docs::<crate::model::Campaign>("campaign").unwrap_or_default().into_iter().map(|x| x.name.to_lowercase()).collect();
    let mut k = 0;
    for mut x in imp.campaigns {
        if have.contains(&x.name.to_lowercase()) {
            continue;
        }
        x.id = 0;
        if db.save_doc("campaign", &mut x).is_ok() {
            k += 1;
        }
    }
    rep.push(format!("Campaigns added: {k}"));
    let have: Vec<String> = db.docs::<crate::model::Pallet>("pallet").unwrap_or_default().into_iter().map(|x| x.name.to_lowercase()).collect();
    let mut k = 0;
    for mut x in imp.pallets {
        if have.contains(&x.name.to_lowercase()) {
            continue;
        }
        x.id = 0;
        if db.save_doc("pallet", &mut x).is_ok() {
            k += 1;
        }
    }
    rep.push(format!("Pallets added: {k}"));
    if !imp.history.is_empty() {
        progress("Writing play history ...");
        let mut id = db.max_history_id();
        let rows: Vec<(u64, u64, String, String, String)> = imp
            .history
            .iter()
            .map(|(t, at)| {
                id += 1;
                (id, *t, at.clone(), "import".to_string(), String::new())
            })
            .collect();
        match db.restore_history(&rows) {
            Ok(_) => rep.push(format!("History entries added: {}", rows.len())),
            Err(e) => rep.push(format!("History not written: {e}")),
        }
    }
    Ok(rep)
}
