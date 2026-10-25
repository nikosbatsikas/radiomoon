//! Data model shared by the database, the scheduling engine and the screens.
use serde::{Deserialize, Serialize};

pub trait HasId {
    fn id(&self) -> u64;
    fn set_id(&mut self, id: u64);
}
macro_rules! has_id {
    ($($t:ty),*) => { $(impl HasId for $t {
        fn id(&self) -> u64 { self.id }
        fn set_id(&mut self, i: u64) { self.id = i; }
    })* };
}

/// Library kinds (value of Track.kind).
pub const KINDS: [(&str, &str); 7] = [
    ("music", "Songs"),
    ("spot", "Spots"),
    ("jingle", "Jingles"),
    ("asset", "Assets"),
    ("sweeper", "Sweepers"),
    ("voice", "Voice tracks"),
    ("memo", "Memos"),
];

pub const POSITIONS: [(&str, &str); 6] = [
    ("start", "Start of song"),
    ("intro", "Before intro"),
    ("end", "Before end"),
    ("bridge", "Bridge at end"),
    ("independent", "Independent"),
    ("custom", "Custom position"),
];

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Track {
    pub id: u64,
    pub kind: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub year: String,
    pub path: String,
    pub duration_ms: u64,
    pub cat1: String,
    pub cat2: String,
    pub cat3: String,
    pub vocal: String,
    pub props: String,
    pub enabled: bool,
    pub bookmarked: bool,
    pub intro_ms: u64,
    pub mix_in_ms: u64,
    pub mix_out_ms: u64,
    pub hook_in_ms: u64,
    pub hook_out_ms: u64,
    pub gain_db: f32,
    pub sep_min: u32,
    pub plcode: String,
    pub notes: String,
    pub variable_len: bool,
    // runtime only (not stored in the library table)
    #[serde(skip)]
    pub src: String,
    #[serde(skip)]
    pub reason: String,
    #[serde(skip)]
    pub overlay_pos: String,
    #[serde(skip)]
    pub overlay_custom_ms: u64,
    #[serde(skip)]
    pub fixed_start: String,
}

impl Track {
    /// Length that counts on the timeline (from mix-in to mix-out).
    pub fn eff_ms(&self) -> u64 {
        let end = if self.mix_out_ms > 0 && self.mix_out_ms <= self.duration_ms { self.mix_out_ms } else { self.duration_ms };
        end.saturating_sub(self.mix_in_ms.min(end))
    }
    /// Playlister code (manual code, or the automatic code = library id).
    pub fn code(&self) -> String {
        if self.plcode.is_empty() {
            self.id.to_string()
        } else {
            self.plcode.clone()
        }
    }
    pub fn is_overlay(&self) -> bool {
        !self.overlay_pos.is_empty()
    }
    pub fn label(&self) -> String {
        if self.artist.is_empty() {
            self.title.clone()
        } else {
            format!("{} - {}", self.artist, self.title)
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Category {
    pub id: u64,
    pub lib: String, // song1 song2 song3 vocal props jingle spot asset sweeper voice memo
    pub name: String,
    pub color: [u8; 3],
    pub comment: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ClockRow {
    pub element: String, // song jingle sweeper voice asset track stitch_hooks stitch_time stitch_news
    pub cat1: String,
    pub cat2: String,
    pub cat3: String,
    pub vocal: String,
    pub props: String,
    pub track_id: u64,
    pub overlay_pos: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Clock {
    pub id: u64,
    pub name: String,
    pub color: [u8; 3],
    pub comment: String,
    pub backup_cat: String,
    pub rows: Vec<ClockRow>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ForceClock {
    pub id: u64,
    pub title: String,
    pub clock_id: u64,
    pub start_date: String,
    pub end_date: String,
    pub times: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PItem {
    pub track_id: u64,
    pub break_ref: String,
    pub overlay_pos: String,
    pub overlay_custom_ms: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PSched {
    pub date: String,   // "" = repeating weekly
    pub weekdays: u8,   // bit 0 = Monday
    pub time: String,
    pub immediate: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Playlist {
    pub id: u64,
    pub name: String,
    pub random: bool,
    pub override_prev: bool,
    pub items: Vec<PItem>,
    pub sched: Vec<PSched>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Slot {
    pub date: String,   // detailed mode
    pub weekday: i8,    // weekly mode, 0 = Monday
    pub time: String,
    pub priority: i32,
    pub spot: u64,      // 0 = chosen by the campaign's programming system
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Campaign {
    pub id: u64,
    pub name: String,
    pub category: String,
    pub start: String,
    pub end: String,
    pub never: bool,
    pub enabled: bool,
    pub mode: String,   // weekly detailed playlister
    pub system: String, // order specific random
    pub spots: Vec<u64>,
    pub slots: Vec<Slot>,
    pub notes: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BreakDef {
    pub id: u64,
    pub time: String,
    pub name: String,
    pub immediate_mask: u8,
    pub visible: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BreakPl {
    pub id: u64,
    pub date: String,
    pub time: String,
    pub code: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Memo {
    pub id: u64,
    pub category: String,
    pub title: String,
    pub body: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PBtn {
    pub track_id: u64,
    pub color: [u8; 3],
    pub label: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Pallet {
    pub id: u64,
    pub name: String,
    pub pages: usize,
    pub rows: usize,
    pub cols: usize,
    pub buttons: Vec<PBtn>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct User {
    pub id: u64,
    pub name: String,
    pub salt: String,
    pub hash: String,
    pub timeout_min: u32,
    pub admin: bool,
    pub perms: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LogItem {
    pub track_id: u64,
    pub break_time: String, // set for break references
    pub overlay_pos: String,
    pub overlay_custom_ms: u64,
    pub src: String,
    pub reason: String,
    pub fixed_start: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FinalLog {
    pub id: u64,
    pub date: String,
    pub items: Vec<LogItem>,
}

has_id!(Category, Clock, ForceClock, Playlist, Campaign, BreakDef, BreakPl, Memo, Pallet, User, FinalLog);

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Priority {
    pub num: i32,
    pub name: String,
    pub time_announce: bool,
    pub playlister: bool,
    pub optional: bool,
}
impl Default for Priority {
    fn default() -> Self {
        Priority { num: 1, name: String::new(), time_announce: false, playlister: false, optional: false }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Rule {
    pub a_field: String, // cat1 cat2 cat3 vocal props
    pub a_value: String,
    pub b_field: String,
    pub b_value: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StitchCfg {
    pub hooks_open: String,
    pub hooks_close: String,
    pub hooks_sep: String,
    pub hooks_default: String,
    pub hooks_count: usize,
    pub time_open: String,
    pub time_close: String,
    pub time_dir: String,
    pub time_default: String,
    pub news_open: String,
    pub news_close: String,
    pub news_voice: String,
    pub news_default: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct StreamCfg {
    pub enabled: bool,
    pub autostart: bool,
    pub kind: String, // icecast | shoutcast
    pub host: String,
    pub port: u16,
    pub mount: String,
    pub user: String,
    pub password: String,
    pub bitrate: u32,
    pub name: String,
    pub genre: String,
    pub description: String,
    pub url: String,
    pub public: bool,
}
impl Default for StreamCfg {
    fn default() -> Self {
        StreamCfg {
            enabled: false,
            autostart: false,
            kind: "icecast".into(),
            host: "localhost".into(),
            port: 8000,
            mount: "/radiomoon".into(),
            user: "source".into(),
            password: String::new(),
            bitrate: 128,
            name: "RadioMoon".into(),
            genre: "Various".into(),
            description: String::new(),
            url: String::new(),
            public: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct BackupCfg {
    pub enabled: bool,
    pub dir: String,
    pub time: String,
    pub keep: usize,
}
impl Default for BackupCfg {
    fn default() -> Self {
        BackupCfg { enabled: true, dir: "backups".into(), time: "03:00".into(), keep: 14 }
    }
}

/// A source for the live rebroadcast: a sound card input, or an internet stream when `url` is set.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct LiveSource {
    pub name: String,
    pub device: String,
    pub url: String,
    pub gain_db: f32,
}

/// A scheduled live rebroadcast: the source goes on the air between `start` and `end` (HH:MM) on the chosen days.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct LiveSlot {
    pub enabled: bool,
    pub source: String,
    /// Monday .. Sunday
    pub days: [bool; 7],
    pub start: String,
    pub end: String,
    pub mute: bool,
}

impl Default for LiveSlot {
    fn default() -> Self {
        LiveSlot { enabled: true, source: String::new(), days: [true; 7], start: "12:00".into(), end: "13:00".into(), mute: true }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub station: String,
    pub station_details: String,
    pub logo_path: String,
    pub start_mode: String,
    pub classic_auto: bool,
    pub classic_decks: usize,
    pub auto_hours: u32,
    pub time_offset_s: i32,
    pub default_gain_db: f32,
    pub autocue_db: f32,
    pub spot_autocue_db: f32,
    pub randomness: usize,
    pub artist_sep_min: u32,
    pub next_day_days: u32,
    pub next_day_window_min: u32,
    pub rules: Vec<Rule>,
    pub cat_names: Vec<String>,
    /// one clock id per half hour: weekday * 48 + hour * 2 + (minute >= 30); older versions stored 168 hourly values
    pub clock_grid: Vec<u64>,
    pub prios: Vec<Priority>,
    pub out_main: String,
    pub out_monitor: String,
    pub out_instants: String,
    /// Live rebroadcast: sound input (empty = default) or stream URL
    pub live_device: String,
    pub live_url: String,
    pub live_gain_db: f32,
    pub live_mute_automation: bool,
    /// automatic volume (AGC) of the songs: target level (dB RMS), maximum boost and cut (dB)
    pub auto_volume: bool,
    pub auto_volume_target_db: f32,
    pub auto_volume_boost_db: f32,
    pub auto_volume_cut_db: f32,
    /// seconds of cross-fade when a live source starts and ends
    pub live_fade_s: f32,
    pub live_sources: Vec<LiveSource>,
    pub live_slots: Vec<LiveSlot>,
    pub custom_log: bool,
    pub custom_log_path: String,
    pub custom_log_format: String,
    pub export_dir: String,
    pub export_next: bool,
    pub export_history: bool,
    pub export_art: bool,
    pub msg_kinds: Vec<(String, String)>,
    pub folder_maps: Vec<(String, String)>,
    pub users_enabled: bool,
    pub shortcuts: Vec<(String, String)>,
    pub stitch: StitchCfg,
    pub plcode_prefix: Vec<(String, String)>,
    pub alert_emails: String,
    pub report_style: String,
    pub report_format: String,
    pub report_template: String,
    pub last_pallet: u64,
    pub layout: String,
    pub language: String,
    pub stream: StreamCfg,
    pub streams: Vec<StreamCfg>,
    pub backup: BackupCfg,
}

impl Default for Settings {
    fn default() -> Self {
        let pr = |n: i32, name: &str, ta: bool, pl: bool, op: bool| Priority { num: n, name: name.into(), time_announce: ta, playlister: pl, optional: op };
        Settings {
            station: "RadioMoon FM".into(),
            station_details: String::new(),
            logo_path: String::new(),
            start_mode: "manual".into(),
            classic_auto: false,
            classic_decks: 11,
            auto_hours: 2,
            time_offset_s: 0,
            default_gain_db: 0.0,
            autocue_db: -40.0,
            spot_autocue_db: -90.0,
            randomness: 0,
            artist_sep_min: 60,
            next_day_days: 1,
            next_day_window_min: 120,
            rules: vec![],
            cat_names: vec!["Category 1".into(), "Category 2".into(), "Category 3".into()],
            clock_grid: vec![0; 336],
            prios: vec![
                pr(1, "Station ID", false, false, false),
                pr(2, "Break start announcement", false, false, true),
                pr(3, "Time announcement", true, false, false),
                pr(4, "Commercials", false, true, false),
                pr(5, "Weather", false, false, false),
                pr(6, "News", false, false, false),
                pr(7, "Break end announcement", false, false, true),
            ],
            out_main: String::new(),
            out_monitor: String::new(),
            out_instants: String::new(),
            live_device: String::new(),
            live_url: String::new(),
            live_gain_db: 0.0,
            live_mute_automation: true,
            auto_volume: false,
            auto_volume_target_db: -18.0,
            auto_volume_boost_db: 9.0,
            auto_volume_cut_db: 12.0,
            live_fade_s: 3.0,
            live_sources: Vec::new(),
            live_slots: Vec::new(),
            custom_log: false,
            custom_log_path: "logs".into(),
            custom_log_format: "{date} {time} {kind} {artist} - {title}".into(),
            export_dir: "export".into(),
            export_next: true,
            export_history: true,
            export_art: true,
            msg_kinds: vec![],
            folder_maps: vec![],
            users_enabled: false,
            shortcuts: vec![
                ("next".into(), "F1".into()),
                ("pause".into(), "F2".into()),
                ("stop".into(), "F3".into()),
                ("restart".into(), "F4".into()),
                ("loop".into(), "F5".into()),
                ("stopnext".into(), "F6".into()),
                ("stopjingles".into(), "F7".into()),
            ],
            stitch: StitchCfg { hooks_count: 3, ..Default::default() },
            plcode_prefix: vec![],
            alert_emails: String::new(),
            report_style: "default".into(),
            report_format: "csv".into(),
            report_template: "{date};{time};{artist};{title};{duration}".into(),
            last_pallet: 0,
            layout: "classic".into(),
            language: "en".into(),
            stream: StreamCfg::default(),
            streams: Vec::new(),
            backup: BackupCfg::default(),
        }
    }
}

pub const PERMS: [(&str, &str); 24] = [
    ("lib_songs", "Songs library"),
    ("lib_spots", "Spots & commercials library"),
    ("lib_jingles", "Jingles library"),
    ("lib_assets", "Assets library"),
    ("lib_sweepers", "Sweepers library"),
    ("lib_voice", "Voice tracks library"),
    ("lib_memos", "Memos library"),
    ("lib_instants", "Instant jingles"),
    ("auto_schedule", "Auto schedule (clocks)"),
    ("playlists", "Playlists"),
    ("final_log", "Log editor (final log)"),
    ("log_viewer", "Log viewer"),
    ("force_clocks", "Force clocks"),
    ("settings", "Settings"),
    ("folder_maps", "Folder mappings"),
    ("users", "Users & security"),
    ("utilities", "Utilities (updater, integrity, reports)"),
    ("db_reset", "Database reset"),
    ("studio_delete", "Studio: delete tracks"),
    ("studio_add", "Studio: add / insert items"),
    ("studio_instants", "Studio: edit instant jingles"),
    ("studio_modes", "Studio: change mode"),
    ("studio_memo", "Studio: edit memos"),
    ("close", "Close the program"),
];
