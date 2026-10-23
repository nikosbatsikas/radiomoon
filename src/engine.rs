//! Scheduling engine: clocks, selection rules, break building, playlists, force clocks and final logs.
use crate::model::*;
use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, Timelike};
use rand::seq::SliceRandom;
use rand::Rng;
use std::collections::{HashMap, HashSet};

const TOL_S: i64 = 25;

#[derive(Default)]
pub struct Data {
    pub tracks: Vec<Track>,
    pub idx: HashMap<u64, usize>,
    pub s: Settings,
    pub clocks: Vec<Clock>,
    pub force: Vec<ForceClock>,
    pub playlists: Vec<Playlist>,
    pub campaigns: Vec<Campaign>,
    pub breaks: Vec<BreakDef>,
    pub cats: Vec<Category>,
    pub bpl: Vec<BreakPl>,
    pub finals: Vec<FinalLog>,
    pub memos: Vec<Memo>,
    pub pallets: Vec<Pallet>,
    pub users: Vec<User>,
}

impl Data {
    pub fn reindex(&mut self) {
        self.idx = self.tracks.iter().enumerate().map(|(i, t)| (t.id, i)).collect();
    }
    pub fn track(&self, id: u64) -> Option<&Track> {
        self.idx.get(&id).and_then(|i| self.tracks.get(*i))
    }
    pub fn by_code(&self, kind: &str, code: &str) -> Option<&Track> {
        self.tracks.iter().find(|t| t.kind == kind && t.code() == code)
    }
    pub fn cat_names(&self, lib: &str) -> Vec<String> {
        self.cats.iter().filter(|c| c.lib == lib).map(|c| c.name.clone()).collect()
    }
}

pub fn dstr(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

pub fn parse_hm(s: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(s.trim(), "%H:%M").ok()
}

pub fn parse_date(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok()
}

pub fn virtual_break(time: &str, dur_ms: u64, date: &str) -> Track {
    Track {
        kind: "break".into(),
        title: format!("Commercial break {time}"),
        path: time.to_string(),
        duration_ms: dur_ms,
        enabled: true,
        src: "break".into(),
        reason: date.to_string(),
        ..Default::default()
    }
}

pub fn stitch_track(kind: &str) -> Track {
    let title = match kind {
        "stitch_hooks" => "Next songs hooks",
        "stitch_time" => "Time announcement",
        _ => "News",
    };
    Track { kind: kind.into(), title: title.into(), duration_ms: 20_000, enabled: true, src: "auto".into(), ..Default::default() }
}

/// Builds the real contents (spots) of one commercial break.
pub fn break_tracks(d: &Data, date: NaiveDate, time: &str) -> Vec<Track> {
    let ds = dstr(date);
    let wd = date.weekday().num_days_from_monday() as i8;
    let mut rng = rand::thread_rng();
    let mut items: Vec<(i32, Track, bool)> = Vec::new();
    for c in d.campaigns.iter().filter(|c| c.enabled) {
        if !c.start.is_empty() && ds < c.start {
            continue;
        }
        if !c.never && !c.end.is_empty() && ds > c.end {
            continue;
        }
        if c.mode == "playlister" {
            continue;
        }
        let commercial = c.category.to_lowercase().starts_with("commercial");
        for (si, sl) in c.slots.iter().enumerate() {
            let hit = if c.mode == "detailed" { sl.date == ds && sl.time == time } else { sl.weekday == wd && sl.time == time };
            if !hit {
                continue;
            }
            let id = if sl.spot != 0 {
                sl.spot
            } else if c.spots.is_empty() {
                0
            } else if c.system == "random" {
                c.spots[rng.gen_range(0..c.spots.len())]
            } else {
                c.spots[((date.num_days_from_ce().max(0) as usize) + si) % c.spots.len()]
            };
            if let Some(t) = d.track(id) {
                if t.enabled {
                    let mut t = t.clone();
                    t.src = format!("campaign:{}", c.name);
                    t.reason = format!("Campaign '{}' ({}) priority {} at {}", c.name, c.mode, sl.priority, time);
                    items.push((sl.priority, t, commercial));
                }
            }
        }
    }
    let pl_prio = d.s.prios.iter().find(|p| p.playlister).map(|p| p.num).unwrap_or(4);
    for b in d.bpl.iter().filter(|b| b.date == ds && b.time == time) {
        if let Some(t) = d.by_code("spot", &b.code) {
            if t.enabled {
                let mut t = t.clone();
                t.src = "playlister".into();
                t.reason = format!("Playlister import: code {} at {}", b.code, time);
                items.push((pl_prio, t, true));
            }
        }
    }
    items.sort_by_key(|(p, _, _)| *p);
    let any_commercial = items.iter().any(|(_, _, c)| *c);
    items.retain(|(p, _, _)| any_commercial || !d.s.prios.iter().any(|q| q.num == *p && q.optional));
    items.into_iter().map(|(_, t, _)| t).collect()
}

/// Expands a break reference (virtual "break" track) into its spots.
pub fn expand_break(d: &Data, t: &Track) -> Vec<Track> {
    let date = parse_date(&t.reason).unwrap_or_else(|| chrono::Local::now().date_naive());
    let mut v = break_tracks(d, date, &t.path);
    if let Some(first) = v.first_mut() {
        first.fixed_start = t.fixed_start.clone();
    }
    v
}

pub fn playlist_tracks(d: &Data, p: &Playlist, expand: bool, date: NaiveDate) -> Vec<Track> {
    let mut items = p.items.clone();
    if p.random {
        let ids: Vec<usize> = (0..items.len()).filter(|&i| items[i].break_ref.is_empty() && items[i].overlay_pos.is_empty()).collect();
        let mut vals: Vec<PItem> = ids.iter().map(|&i| items[i].clone()).collect();
        vals.shuffle(&mut rand::thread_rng());
        for (k, &i) in ids.iter().enumerate() {
            items[i] = vals[k].clone();
        }
    }
    let mut out = Vec::new();
    for it in items {
        if !it.break_ref.is_empty() {
            if expand {
                out.extend(break_tracks(d, date, &it.break_ref));
            } else {
                out.push(virtual_break(&it.break_ref, 120_000, &dstr(date)));
            }
        } else if let Some(t) = d.track(it.track_id) {
            if t.enabled {
                let mut t = t.clone();
                t.src = format!("playlist:{}", p.name);
                t.reason = format!("Playlist '{}'", p.name);
                t.overlay_pos = it.overlay_pos.clone();
                t.overlay_custom_ms = it.overlay_custom_ms;
                out.push(t);
            }
        }
    }
    out
}

pub struct Flags {
    pub playlists: bool,
    pub force: bool,
    pub breaks: bool,
}
impl Default for Flags {
    fn default() -> Self {
        Flags { playlists: true, force: true, breaks: true }
    }
}

#[derive(Clone)]
struct Anchor {
    at: NaiveDateTime,
    key: String,
    kind: u8, // 0 break, 1 playlist, 2 force clock
    idx: usize,
    time: String,
    immediate: bool,
}

pub struct Generator {
    pub t: NaiveDateTime,
    t0: NaiveDateTime,
    until: NaiveDateTime,
    prev: Option<Track>,
    artist_last: HashMap<String, NaiveDateTime>,
    plays: HashMap<u64, Vec<NaiveDateTime>>,
    cycle: HashSet<u64>,
    order: HashMap<u64, u32>,
    clock_id: u64,
    row: usize,
    fired: HashSet<String>,
    pub force_clock: Option<u64>,
}

fn field_is(t: &Track, field: &str, value: &str) -> bool {
    match field {
        "cat1" => t.cat1 == value,
        "cat2" => t.cat2 == value,
        "cat3" => t.cat3 == value,
        "vocal" => t.vocal == value,
        "props" => t.props == value,
        _ => false,
    }
}

fn fdesc(r: &ClockRow) -> String {
    let mut v = Vec::new();
    for (n, x) in [("c1", &r.cat1), ("c2", &r.cat2), ("c3", &r.cat3), ("vocal", &r.vocal), ("props", &r.props)] {
        if !x.is_empty() {
            v.push(format!("{n}={x}"));
        }
    }
    if v.is_empty() {
        "any".into()
    } else {
        v.join(", ")
    }
}

impl Generator {
    pub fn new(start: NaiveDateTime, recent: &[(u64, NaiveDateTime)], d: &Data) -> Self {
        let mut g = Generator {
            t: start,
            t0: start,
            until: start,
            prev: None,
            artist_last: HashMap::new(),
            plays: HashMap::new(),
            cycle: HashSet::new(),
            order: HashMap::new(),
            clock_id: u64::MAX,
            row: 0,
            fired: HashSet::new(),
            force_clock: None,
        };
        for (id, at) in recent {
            if let Some(t) = d.track(*id) {
                if t.kind == "music" {
                    g.plays.entry(*id).or_default().push(*at);
                    if !t.artist.is_empty() {
                        let e = g.artist_last.entry(t.artist.to_lowercase()).or_insert(*at);
                        if *at > *e {
                            *e = *at;
                        }
                    }
                }
            }
        }
        g.reshuffle(d);
        g
    }

    /// "Reshuffle all media": new rotation order for every track.
    pub fn reshuffle(&mut self, d: &Data) {
        let mut ids: Vec<u64> = d.tracks.iter().map(|t| t.id).collect();
        ids.shuffle(&mut rand::thread_rng());
        self.order = ids.into_iter().enumerate().map(|(i, id)| (id, i as u32)).collect();
    }

    fn anchors(&self, d: &Data, f: &Flags) -> Vec<Anchor> {
        let mut v = Vec::new();
        for k in 0..2i64 {
            let day = self.t.date() + Duration::days(k);
            let ds = dstr(day);
            let wd = day.weekday().num_days_from_monday();
            if f.breaks {
                for b in &d.breaks {
                    if let Some(tm) = parse_hm(&b.time) {
                        v.push(Anchor {
                            at: day.and_time(tm),
                            key: format!("b:{ds}:{}", b.time),
                            kind: 0,
                            idx: 0,
                            time: b.time.clone(),
                            immediate: (b.immediate_mask >> wd) & 1 == 1,
                        });
                    }
                }
            }
            if f.playlists && d.s.clock_grid.len() == 336 {
                // playlists placed in the main auto schedule: one call at the start of every block of half hours
                for sl in 0..48usize {
                    let id = d.s.clock_grid[wd as usize * 48 + sl];
                    if id == 0 || (sl > 0 && d.s.clock_grid[wd as usize * 48 + sl - 1] == id) {
                        continue;
                    }
                    if let Some(pi) = d.playlists.iter().position(|p| p.id == id) {
                        let time = format!("{:02}:{:02}", sl / 2, (sl % 2) * 30);
                        if let Some(tm) = parse_hm(&time) {
                            v.push(Anchor { at: day.and_time(tm), key: format!("g:{id}:{ds}:{sl}"), kind: 1, idx: pi, time, immediate: false });
                        }
                    }
                }
            }
            if f.playlists {
                for (pi, p) in d.playlists.iter().enumerate() {
                    for (si, ps) in p.sched.iter().enumerate() {
                        let hit = if ps.date.is_empty() { (ps.weekdays >> wd) & 1 == 1 } else { ps.date == ds };
                        if hit {
                            if let Some(tm) = parse_hm(&ps.time) {
                                v.push(Anchor {
                                    at: day.and_time(tm),
                                    key: format!("p:{}:{si}:{ds}", p.id),
                                    kind: 1,
                                    idx: pi,
                                    time: ps.time.clone(),
                                    immediate: ps.immediate,
                                });
                            }
                        }
                    }
                }
            }
            if f.force {
                for (fi, fc) in d.force.iter().enumerate() {
                    if (fc.start_date.is_empty() || ds >= fc.start_date) && (fc.end_date.is_empty() || ds <= fc.end_date) {
                        for ts in &fc.times {
                            if let Some(tm) = parse_hm(ts) {
                                v.push(Anchor {
                                    at: day.and_time(tm),
                                    key: format!("f:{}:{ts}:{ds}", fc.id),
                                    kind: 2,
                                    idx: fi,
                                    time: ts.clone(),
                                    immediate: true,
                                });
                            }
                        }
                    }
                }
            }
        }
        let t0 = self.t0 - Duration::seconds(90);
        v.retain(|a| !self.fired.contains(&a.key) && a.at >= t0 && a.at < self.until);
        v.sort_by_key(|a| a.at);
        v
    }

    fn allowed(&self, d: &Data, t: &Track, level: u8) -> bool {
        if let Some(a) = self.plays.get(&t.id) {
            if t.sep_min > 0 {
                if let Some(last) = a.iter().max() {
                    if (self.t - *last).num_minutes() < t.sep_min as i64 {
                        return false;
                    }
                }
            }
            if level < 2 && d.s.next_day_days > 0 {
                let half = (d.s.next_day_window_min as i64) / 2;
                for p in a {
                    for k in 1..=d.s.next_day_days as i64 {
                        let target = *p + Duration::days(k);
                        if (target - self.t).num_minutes().abs() <= half {
                            return false;
                        }
                    }
                }
            }
        }
        if level < 3 && d.s.artist_sep_min > 0 && !t.artist.is_empty() {
            if let Some(l) = self.artist_last.get(&t.artist.to_lowercase()) {
                if (self.t - *l).num_minutes() < d.s.artist_sep_min as i64 {
                    return false;
                }
            }
        }
        if level < 1 {
            if let Some(p) = &self.prev {
                for r in &d.s.rules {
                    if field_is(p, &r.a_field, &r.a_value) && field_is(t, &r.b_field, &r.b_value) {
                        return false;
                    }
                }
            }
        }
        true
    }

    fn take(&mut self, pick: &Track, reason: String) -> Track {
        self.cycle.insert(pick.id);
        self.plays.entry(pick.id).or_default().push(self.t);
        if !pick.artist.is_empty() {
            self.artist_last.insert(pick.artist.to_lowercase(), self.t);
        }
        self.prev = Some(pick.clone());
        let mut t = pick.clone();
        t.src = "auto".into();
        t.reason = reason;
        t
    }

    fn pick_comp(&mut self, d: &Data, clock: &Clock, rem: i64) -> Option<Track> {
        let mut c: Vec<&Track> = d
            .tracks
            .iter()
            .filter(|t| t.kind == "music" && t.enabled && (clock.backup_cat.is_empty() || t.cat1 == clock.backup_cat) && self.allowed(d, t, 2))
            .collect();
        if c.is_empty() {
            c = d.tracks.iter().filter(|t| t.kind == "music" && t.enabled).collect();
        }
        if c.is_empty() {
            return None;
        }
        let tol = TOL_S * 1000;
        let fit: Vec<&Track> = c.iter().cloned().filter(|t| (t.eff_ms() as i64) <= rem + tol).collect();
        let pick: &Track = if fit.is_empty() {
            c.iter().min_by_key(|t| t.eff_ms()).copied()?
        } else {
            fit.iter().min_by_key(|t| (rem - t.eff_ms() as i64).abs()).copied()?
        };
        Some(self.take(pick, format!("Compensation track: {} s left until the next scheduled event", rem / 1000)))
    }

    fn pick_song(&mut self, d: &Data, row: &ClockRow, clock: &Clock, rem: Option<i64>, base: &str) -> Option<Track> {
        let filt = |t: &Track| -> bool {
            t.kind == "music"
                && t.enabled
                && (row.cat1.is_empty() || t.cat1 == row.cat1)
                && (row.cat2.is_empty() || t.cat2 == row.cat2)
                && (row.cat3.is_empty() || t.cat3 == row.cat3)
                && (row.vocal.is_empty() || t.vocal == row.vocal)
                && (row.props.is_empty() || t.props == row.props)
        };
        for level in 0..4u8 {
            let mut cands: Vec<&Track> = d.tracks.iter().filter(|t| filt(*t) && self.allowed(d, *t, level)).collect();
            if cands.is_empty() {
                continue;
            }
            cands.shuffle(&mut rand::thread_rng()); // equal ranks are decided randomly: not the same songs at every start
            cands.sort_by_key(|t| self.order.get(&t.id).copied().unwrap_or(0));
            let mut fresh: Vec<&Track> = cands.iter().cloned().filter(|t| !self.cycle.contains(&t.id)).collect();
            if fresh.is_empty() {
                for t in &cands {
                    self.cycle.remove(&t.id);
                }
                fresh = cands.clone();
            }
            let n = if d.s.randomness == 0 { 1 } else { d.s.randomness.min(fresh.len()) };
            let k = rand::thread_rng().gen_range(0..n.max(1));
            let pick = fresh[k];
            if let Some(r) = rem {
                if pick.eff_ms() as i64 > r + TOL_S * 1000 {
                    if let Some(c) = self.pick_comp(d, clock, r) {
                        return Some(c);
                    }
                }
            }
            let why = format!("{base}: filters [{}], randomness {}, relax level {level}", fdesc(row), d.s.randomness);
            return Some(self.take(pick, why));
        }
        None
    }

    fn row_items(&mut self, d: &Data, c: &Clock, row: &ClockRow, idx: usize, rem: Option<i64>) -> Vec<Track> {
        let base = format!("Clock '{}' row {}", c.name, idx + 1);
        match row.element.as_str() {
            "song" => self.pick_song(d, row, c, rem, &base).into_iter().collect(),
            "track" => d
                .track(row.track_id)
                .filter(|t| t.enabled)
                .map(|t| {
                    let mut t = t.clone();
                    t.src = "auto".into();
                    t.reason = format!("{base}: specific track");
                    t
                })
                .into_iter()
                .collect(),
            "stitch_hooks" | "stitch_time" | "stitch_news" => {
                let mut t = stitch_track(&row.element);
                t.reason = base;
                vec![t]
            }
            kind => {
                let cands: Vec<&Track> = d
                    .tracks
                    .iter()
                    .filter(|t| t.kind == kind && t.enabled && (row.cat1.is_empty() || t.cat1 == row.cat1))
                    .collect();
                if cands.is_empty() {
                    return vec![];
                }
                let mut pool: Vec<&Track> = cands.iter().cloned().filter(|t| !self.cycle.contains(&t.id)).collect();
                if pool.is_empty() {
                    for t in &cands {
                        self.cycle.remove(&t.id);
                    }
                    pool = cands.clone();
                }
                let pick = pool[rand::thread_rng().gen_range(0..pool.len())];
                self.cycle.insert(pick.id);
                let mut t = pick.clone();
                t.src = "auto".into();
                t.reason = format!("{base}: {kind} from category '{}'", row.cat1);
                if kind == "sweeper" || kind == "voice" {
                    t.overlay_pos = if row.overlay_pos.is_empty() { "start".into() } else { row.overlay_pos.clone() };
                }
                vec![t]
            }
        }
    }

    fn next_items(&mut self, d: &Data, rem: Option<i64>) -> Vec<Track> {
        let wd = self.t.date().weekday().num_days_from_monday() as usize;
        let hr = self.t.hour() as usize;
        let slot = if self.t.minute() >= 30 { 1 } else { 0 };
        let g = &d.s.clock_grid;
        let cid = self.force_clock.unwrap_or_else(|| (if g.len() == 168 { g.get(wd * 24 + hr) } else { g.get(wd * 48 + hr * 2 + slot) }).copied().unwrap_or(0));
        if cid != self.clock_id {
            self.clock_id = cid;
            self.row = 0;
        }
        let synthetic: Clock;
        let found = d.clocks.iter().find(|c| c.id == cid).filter(|c| !c.rows.is_empty());
        let c: &Clock = match found {
            Some(c) => c,
            None => {
                synthetic = Clock {
                    name: "(no clock assigned: any song)".into(),
                    rows: vec![ClockRow { element: "song".into(), ..Default::default() }],
                    ..Default::default()
                };
                &synthetic
            }
        };
        let idx = self.row % c.rows.len();
        self.row += 1;
        let row = c.rows[idx].clone();
        self.row_items(d, c, &row, idx, rem)
    }

    fn force_pass(&mut self, d: &Data, clock_id: u64, title: &str) -> Vec<Track> {
        let Some(c) = d.clocks.iter().find(|c| c.id == clock_id) else { return vec![] };
        let mut v = Vec::new();
        for (i, row) in c.rows.iter().enumerate() {
            for mut t in self.row_items(d, c, row, i, None) {
                t.src = format!("force:{title}");
                t.reason = format!("Force clock '{title}' ({}) row {}", c.name, i + 1);
                v.push(t);
            }
        }
        v
    }

    fn push(&mut self, t: Track, out: &mut Vec<Track>) {
        if !t.is_overlay() {
            let ms = t.eff_ms().max(1000);
            self.t = self.t + Duration::milliseconds(ms as i64);
        }
        out.push(t);
    }

    fn fire(&mut self, d: &Data, a: &Anchor, expand: bool, out: &mut Vec<Track>) {
        self.fired.insert(a.key.clone());
        let day = a.at.date();
        let mut items: Vec<Track> = match a.kind {
            0 => {
                let ex = break_tracks(d, day, &a.time);
                if expand {
                    ex
                } else {
                    let dur: u64 = ex.iter().map(|t| t.duration_ms).sum();
                    vec![virtual_break(&a.time, dur.max(30_000), &dstr(day))]
                }
            }
            1 => playlist_tracks(d, &d.playlists[a.idx], expand, day),
            _ => {
                let fc = &d.force[a.idx];
                self.force_pass(d, fc.clock_id, &fc.title)
            }
        };
        if items.is_empty() {
            return;
        }
        if a.immediate {
            items[0].fixed_start = a.at.format("%Y-%m-%d %H:%M:%S").to_string();
        }
        for t in items {
            self.push(t, out);
        }
    }

    /// Generates items until the virtual time reaches `until`.
    pub fn fill_until(&mut self, d: &Data, until: NaiveDateTime, f: &Flags, expand: bool) -> Vec<Track> {
        self.until = until;
        let mut out = Vec::new();
        let mut guard = 0;
        while self.t < until && guard < 30_000 {
            guard += 1;
            let anchors = self.anchors(d, f);
            let next = anchors.first().cloned();
            if let Some(a) = &next {
                if self.t + Duration::seconds(TOL_S) >= a.at {
                    self.fire(d, a, expand, &mut out);
                    continue;
                }
            }
            let rem = next.as_ref().map(|a| (a.at - self.t).num_milliseconds());
            let before = out.len();
            let items = self.next_items(d, rem);
            for t in items {
                self.push(t, &mut out);
            }
            if out.len() == before {
                self.t = self.t + Duration::seconds(30);
            }
        }
        out
    }
}

pub fn to_final(items: &[Track], date: &str) -> FinalLog {
    FinalLog {
        id: 0,
        date: date.into(),
        items: items
            .iter()
            .map(|t| LogItem {
                track_id: t.id,
                break_time: if t.id != 0 {
                    String::new()
                } else if t.kind == "break" {
                    format!("break:{}", t.path)
                } else {
                    t.kind.clone()
                },
                overlay_pos: t.overlay_pos.clone(),
                overlay_custom_ms: t.overlay_custom_ms,
                src: t.src.clone(),
                reason: t.reason.clone(),
                fixed_start: t.fixed_start.clone(),
            })
            .collect(),
    }
}

pub fn from_final(d: &Data, f: &FinalLog) -> Vec<Track> {
    let mut out = Vec::new();
    for it in &f.items {
        let mut t = if let Some(rest) = it.break_time.strip_prefix("break:") {
            virtual_break(rest, 120_000, &f.date)
        } else if it.break_time.starts_with("stitch_") {
            stitch_track(&it.break_time)
        } else if let Some(t) = d.track(it.track_id) {
            t.clone()
        } else {
            continue;
        };
        t.overlay_pos = it.overlay_pos.clone();
        t.overlay_custom_ms = it.overlay_custom_ms;
        t.src = it.src.clone();
        if t.kind != "break" {
            t.reason = it.reason.clone();
        }
        t.fixed_start = it.fixed_start.clone();
        out.push(t);
    }
    out
}

pub fn create_final(d: &Data, date: NaiveDate, f: &Flags, recent: &[(u64, NaiveDateTime)]) -> FinalLog {
    let start = date.and_hms_opt(0, 0, 0).unwrap();
    let mut g = Generator::new(start, recent, d);
    let items = g.fill_until(d, start + Duration::days(1), f, false);
    to_final(&items, &dstr(date))
}

pub fn auto_playlist(d: &Data, clock_id: u64, start: NaiveDateTime, hours: u32, recent: &[(u64, NaiveDateTime)]) -> Vec<PItem> {
    let mut g = Generator::new(start, recent, d);
    g.force_clock = Some(clock_id);
    let none = Flags { playlists: false, force: false, breaks: false };
    let items = g.fill_until(d, start + Duration::hours(hours as i64), &none, true);
    items
        .into_iter()
        .filter(|t| t.id != 0)
        .map(|t| PItem { track_id: t.id, break_ref: String::new(), overlay_pos: t.overlay_pos, overlay_custom_ms: t.overlay_custom_ms })
        .collect()
}
