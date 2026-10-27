//! Control Panel: libraries, scheduling, settings and utilities.
use crate::app::{fmt_ms, fmt_precise, hash_pw, parse_time, App, BUILD, VERSION};
use crate::backup;
use crate::rfd;
use crate::i18n::tr;
use crate::kit::{self, Icon};
use crate::engine::{self, Flags};
use crate::export;
use crate::library::{self, ImportOpts};
use crate::model::*;
use crate::player;
use chrono::{Datelike, Duration, Local, NaiveDateTime, Timelike};
use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Rect, RichText, Sense, Stroke};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

#[derive(Clone, Copy, PartialEq, Default)]
pub enum Page {
    #[default]
    Songs,
    Clap,
    Spots,
    Jingles,
    Assets,
    Sweepers,
    Voice,
    Memos,
    Instants,
    Stitcher,
    Clocks,
    Playlists,
    Force,
    FinalLog,
    LogViewer,
    General,
    Outputs,
    Live,
    Shortcuts,
    Users,
    Folders,
    Server,
    Stream,
    Backup,
    Updater,
    Jazler,
    Integrity,
    Reports,
    Reset,
}

const NAV: [(&str, &[(Page, &str, &str)]); 4] = [
    (
        "Libraries",
        &[
            (Page::Songs, "Songs", "lib_songs"),
            (Page::Spots, "Spots / Commercials", "lib_spots"),
            (Page::Jingles, "Jingles", "lib_jingles"),
            (Page::Assets, "Assets", "lib_assets"),
            (Page::Sweepers, "Sweepers", "lib_sweepers"),
            (Page::Voice, "Voice tracks", "lib_voice"),
            (Page::Memos, "Memos", "lib_memos"),
            (Page::Instants, "Instant jingles", "lib_instants"),
            (Page::Stitcher, "Stitcher", "lib_assets"),
        ],
    ),
    (
        "Scheduling",
        &[
            (Page::Clocks, "Main auto schedule", "auto_schedule"),
            (Page::Playlists, "Playlists", "playlists"),
            (Page::Force, "Force clocks", "force_clocks"),
            (Page::Live, "Live rebroadcast", "auto_schedule"),
            (Page::FinalLog, "Final log creator", "final_log"),
            (Page::LogViewer, "Log viewer", "log_viewer"),
        ],
    ),
    (
        "Settings",
        &[
            (Page::General, "General", "settings"),
            (Page::Outputs, "Sound outputs", "settings"),
            (Page::Clap, "Effects (CLAP)", "settings"),
            (Page::Shortcuts, "Shortcuts", "settings"),
            (Page::Users, "Users & security", "users"),
            (Page::Folders, "Audio folder mappings", "folder_maps"),
            (Page::Stream, "Internet broadcast", "settings"),
            (Page::Backup, "Backup & restore", "settings"),
            (Page::Server, "Database server", "settings"),
        ],
    ),
    (
        "Utilities",
        &[
            (Page::Updater, "Internet updater", "utilities"),
            (Page::Jazler, "Import from Jazler / MegaMix", "utilities"),
            (Page::Integrity, "Integrity check", "utilities"),
            (Page::Reports, "Broadcasting reports", "utilities"),
            (Page::Reset, "Reset database", "db_reset"),
        ],
    ),
];

#[derive(Default)]
pub struct ControlState {
    pub page: Page,
    pub clap_view: Option<crate::clap_host::ClapView>,
    pub clap_filter: String,
    pub clap_msg: String,
    pub clap_sent: Option<std::time::Instant>,
    pub clap_poll: Option<std::time::Instant>,
    /// items to delete from the library, waiting for the answer of the question (and whether the files are deleted too)
    pub del_ask: Option<(Vec<u64>, bool)>,
    /// Jazler import with a server: upload the audio files to the server when the import is finished and change the paths
    pub jz_upload: bool,
    pub jz_upload_pending: bool,
    // library
    pub lib_filter: String,
    pub lib_cat: String,
    pub lib_sel: Vec<u64>,
    pub edit: Option<Track>,
    pub mass: Option<MassEdit>,
    pub stats_from: String,
    pub stats_to: String,
    pub stats: Vec<(String, u64)>,
    pub new_cat: String,
    pub cat_lib: String,
    // spots
    pub camp: Option<Campaign>,
    pub camp_prio: i32,
    pub camp_days: [bool; 7],
    pub camp_times: Vec<String>,
    pub spots_msg: String,
    pub bdate: String,
    pub brk_text: String,
    // clocks
    pub clock: Option<Clock>,
    pub grid_brush: u64,
    pub grid_q: String,
    pub new_row: ClockRow,
    pub rule: Rule,
    // playlists
    pub pl: Option<Playlist>,
    pub pl_time: String,
    pub pl_date: String,
    pub pl_days: [bool; 7],
    pub pl_clock: u64,
    pub pl_hours: u32,
    pub pl_sel: u64,
    pub pl_q: String,
    pub pl_kind: String,
    // force
    pub fc: Option<ForceClock>,
    pub fc_time: String,
    // final log
    pub fl_date: String,
    pub fl_flags: [bool; 3],
    pub fl_items: Vec<Track>,
    pub fl_msg: String,
    // log viewer
    pub lv_from: String,
    pub lv_to: String,
    pub lv_rows: Vec<crate::db::HistRow>,
    pub lv_sel: Option<usize>,
    // users
    pub user: Option<User>,
    pub user_pw: String,
    // jazler import
    pub jz_path: String,
    pub jz_opts: Option<crate::jzimport::Opts>,
    pub jz_preview: Vec<(String, usize)>,
    pub jz_status: std::sync::Arc<std::sync::Mutex<String>>,
    pub jz_report: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    pub jz_busy: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub jz_done: std::sync::Arc<std::sync::atomic::AtomicBool>,
    // utilities
    pub integ_kind: String,
    pub integ_res: Vec<Track>,
    pub rep_from: String,
    pub rep_to: String,
    pub reset_kinds: Vec<String>,
    pub reset_all_ok: bool,
    pub map_from: String,
    pub map_to: String,
    pub msg: String,
    pub memo: Option<Memo>,
    pub cleanup_days: u32,
    pub pallet: Option<Pallet>,
    pub import_pending: Option<(PathBuf, ImportOpts)>,
    pub edit_wave: crate::waveform::WaveStore,
    pub edit_wave_for: u64,
    pub edit_marker: u8,
    pub edit_cursor: u64,
    pub wave_zoom: f32,
    pub wave_off: f32,
    pub restore_target: Option<PathBuf>,
}

#[derive(Default, Clone)]
pub struct MassEdit {
    pub cat1: Option<String>,
    pub cat2: Option<String>,
    pub cat3: Option<String>,
    pub vocal: Option<String>,
    pub props: Option<String>,
    pub enabled: Option<bool>,
    pub sep: Option<u32>,
    pub normalize: bool,
    pub bookmark: Option<bool>,
}

fn kind_of(p: Page) -> Option<&'static str> {
    Some(match p {
        Page::Songs => "music",
        Page::Spots => "spot",
        Page::Jingles => "jingle",
        Page::Assets => "asset",
        Page::Sweepers => "sweeper",
        Page::Voice => "voice",
        _ => return None,
    })
}

fn today() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

const DAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

/// Time field in minutes:seconds (m:ss.d). Keeps what you type until you leave the field.
fn ms_edit(ui: &mut egui::Ui, label: &str, v: &mut u64) {
    ui.horizontal(|ui| {
        ui.label(tr(label));
        let id = ui.id().with(("time", label));
        let stored: Option<String> = ui.memory(|m| m.data.get_temp::<String>(id));
        let mut text = stored.clone().unwrap_or_else(|| fmt_precise(*v));
        let resp = ui.add(egui::TextEdit::singleline(&mut text).desired_width(86.0).hint_text(tr("m:ss.d")));
        if resp.changed() {
            ui.memory_mut(|m| m.data.insert_temp(id, text.clone()));
            if let Some(ms) = parse_time(&text) {
                *v = ms;
            }
        } else if !resp.has_focus() && stored.is_some() {
            ui.memory_mut(|m| m.data.remove::<String>(id));
        }
    });
}

/// Same size and position for every dialog of the Control Panel.
/// 2:56:39 style total of a playlist
fn hms(ms: u64) -> String {
    let s = ms / 1000;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

fn dialog<'a>(ctx: &egui::Context, title: impl Into<String>) -> egui::Window<'a> {
    let title: String = title.into();
    // The window id must not depend on text the user edits (a title such as "Edit: <song title>"): a new id on every
    // key press would rebuild the window and drop the keyboard focus after each letter.
    let key = title.split(|c| c == ':' || c == '(').next().unwrap_or("").trim().to_string();
    let sc = ctx.screen_rect();
    let size = vec2((sc.width() - 120.0).clamp(560.0, 860.0), (sc.height() - 100.0).clamp(420.0, 700.0));
    egui::Window::new(title).id(egui::Id::new(("rm_dialog", key))).order(egui::Order::Foreground).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).fixed_size(size)
}

/// Text field with a Browse button (folder or file).
fn path_row(ui: &mut egui::Ui, label: &str, v: &mut String, folder: bool) -> bool {
    let mut ch = false;
    ui.horizontal(|ui| {
        ui.label(tr(label));
        let w = (ui.available_width() - 110.0).max(80.0);
        ch |= ui.add(egui::TextEdit::singleline(v).desired_width(w)).changed();
        if ui.button(tr("Browse...")).clicked() {
            let d = rfd::FileDialog::new();
            let picked = if folder { d.pick_folder() } else { d.add_filter("audio", &library::EXTS).pick_file() };
            if let Some(p) = picked {
                *v = p.to_string_lossy().to_string();
                ch = true;
            }
        }
    });
    ch
}

fn text_row(ui: &mut egui::Ui, label: &str, v: &mut String) -> bool {
    let mut ch = false;
    ui.horizontal(|ui| {
        ui.label(tr(label));
        ch = ui.add(egui::TextEdit::singleline(v).desired_width((ui.available_width() - 8.0).max(60.0))).changed();
    });
    ch
}

fn combo(ui: &mut egui::Ui, id: &str, label: &str, cur: &mut String, opts: &[String]) {
    ui.horizontal(|ui| {
        ui.label(tr(label));
        egui::ComboBox::from_id_salt(id).selected_text(if cur.is_empty() { tr("(any)") } else { cur.clone() }).show_ui(ui, |ui| {
            ui.selectable_value(cur, String::new(), tr("(any)"));
            for o in opts {
                ui.selectable_value(cur, o.clone(), o);
            }
        });
    });
}

impl App {
    pub fn control_panel(&mut self, ctx: &egui::Context) {
        if self.cp.stats_from.is_empty() {
            let t = today();
            self.cp.stats_from = t.clone();
            self.cp.stats_to = t.clone();
            self.cp.bdate = t.clone();
            self.cp.fl_date = (Local::now() + Duration::days(1)).format("%Y-%m-%d").to_string();
            self.cp.fl_flags = [true, true, true];
            self.cp.lv_from = t.clone();
            self.cp.lv_to = t.clone();
            self.cp.rep_from = t.clone();
            self.cp.rep_to = t.clone();
            self.cp.pl_date = t;
            self.cp.pl_hours = 3;
            self.cp.cleanup_days = 365;
        }
        let screen = ctx.screen_rect();
        let size = vec2((screen.width() - 80.0).clamp(900.0, 1600.0), (screen.height() - 80.0).max(560.0));
        let mut close = false;
        let head_bg = kit::INSET;
        egui::Window::new(tr("rm_control_panel"))
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size(size)
            .frame(egui::Frame::window(&ctx.style()).fill(kit::PANEL).inner_margin(0.0))
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                let head_h = 54.0;
                let (hr, _) = ui.allocate_exact_size(vec2(size.x, head_h), Sense::hover());
                let p = ui.painter().clone();
                p.rect_filled(hr, egui::Rounding { nw: 10.0, ne: 10.0, sw: 0.0, se: 0.0 }, head_bg);
                p.line_segment([hr.left_bottom(), hr.right_bottom()], Stroke::new(1.0_f32, kit::BORDER));
                kit::draw_moon(&p, pos2(hr.left() + 30.0, hr.center().y), 11.0, Color32::WHITE, head_bg);
                p.text(pos2(hr.left() + 52.0, hr.center().y), Align2::LEFT_CENTER, tr("Control Panel"), FontId::proportional(17.0), Color32::WHITE);
                let cur_name = NAV.iter().flat_map(|g| g.1.iter()).find(|x| x.0 == self.cp.page).map(|x| x.1).unwrap_or("");
                p.text(pos2(hr.right() - 84.0, hr.center().y), Align2::RIGHT_CENTER, tr(cur_name), FontId::proportional(13.0), kit::GRAY);
                if kit::button_at(ui, Rect::from_min_size(pos2(hr.right() - 64.0, hr.top() + 10.0), vec2(48.0, 34.0)), "cp_close", Some(Icon::Clear), "", Color32::from_rgb(120, 40, 45), Color32::WHITE).clicked() {
                    close = true;
                }
                let body_h = size.y - head_h;
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(vec2(232.0, body_h), egui::Layout::top_down(egui::Align::LEFT), |ui| {
                        let side = ui.max_rect();
                        ui.painter().rect_filled(side, egui::Rounding { nw: 0.0, ne: 0.0, sw: 10.0, se: 0.0 }, kit::SIDEBAR);
                        // all entries fit the height of the window: the row height shrinks when the window is low
                        let n_items: usize = NAV.iter().map(|g| g.1.len()).sum();
                        let fixed = NAV.len() as f32 * (10.0 + 18.0) + 12.0 + 26.0 + if self.data.s.users_enabled { 34.0 } else { 0.0 };
                        let item_h = ((body_h - fixed - 8.0) / n_items as f32).clamp(20.0, 34.0);
                        egui::ScrollArea::vertical().id_salt("cpnav").auto_shrink([false, false]).show(ui, |ui| {
                            for (group, pages) in NAV.iter() {
                                ui.add_space(10.0);
                                let (gr, _) = ui.allocate_exact_size(vec2(216.0, 18.0), Sense::hover());
                                ui.painter().text(pos2(gr.left() + 18.0, gr.center().y), Align2::LEFT_CENTER, tr(group).to_uppercase(), FontId::proportional(11.0), kit::GRAY);
                                for (pg, name, perm) in pages.iter() {
                                    let allowed = self.can(perm);
                                    let (rect, resp) = ui.allocate_exact_size(vec2(216.0, item_h), Sense::click());
                                    let sel = self.cp.page == *pg;
                                    let painter = ui.painter().clone();
                                    if sel {
                                        painter.rect_filled(rect.shrink2(vec2(8.0, 1.0)), 6.0, kit::ACCENT.gamma_multiply(0.28));
                                        painter.rect_filled(Rect::from_min_size(rect.min + vec2(8.0, item_h * 0.18), vec2(3.0, item_h * 0.64)), 2.0, kit::ACCENT);
                                    } else if resp.hovered() && allowed {
                                        painter.rect_filled(rect.shrink2(vec2(8.0, 1.0)), 6.0, kit::CONTROL_HOVER);
                                    }
                                    let col = if !allowed { Color32::from_gray(150) } else if sel { Color32::WHITE } else { Color32::WHITE };
                                    painter.text(pos2(rect.left() + 26.0, rect.center().y), Align2::LEFT_CENTER, tr(name), FontId::proportional(13.0), col);
                                    if resp.clicked() && allowed {
                                        self.cp.page = *pg;
                                        self.cp.msg.clear();
                                    }
                                }
                            }
                            ui.add_space(12.0);
                            if self.data.s.users_enabled {
                                let (rect, _) = ui.allocate_exact_size(vec2(216.0, 34.0), Sense::hover());
                                if kit::button_at(ui, rect.shrink2(vec2(10.0, 0.0)), "cp_logout", None, "Log out", kit::DARK, Color32::WHITE).clicked() {
                                    self.logout();
                                }
                            }
                            let (vr, _) = ui.allocate_exact_size(vec2(216.0, 26.0), Sense::hover());
                            ui.painter().text(pos2(vr.left() + 18.0, vr.center().y), Align2::LEFT_CENTER, format!("RadioMoon {VERSION} ({BUILD})"), FontId::proportional(11.5), Color32::from_gray(170));
                        });
                    });
                    ui.allocate_ui_with_layout(vec2(size.x - 232.0, body_h), egui::Layout::top_down(egui::Align::LEFT), |ui| {
                        ui.spacing_mut().item_spacing = vec2(8.0, 7.0);
                        egui::Frame::none().fill(kit::PANEL).inner_margin(20.0).show(ui, |ui| {
                            egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                                ui.set_min_width(size.x - 232.0 - 70.0);
                                let page = self.cp.page;
                                match page {
                                    Page::Songs | Page::Spots | Page::Jingles | Page::Assets | Page::Sweepers | Page::Voice => self.page_library(ui, kind_of(page).unwrap_or("music")),
                                    Page::Memos => self.page_memos(ui),
                                    Page::Instants => self.page_instants(ui),
                                    Page::Stitcher => self.page_stitcher(ui),
                                    Page::Clocks => self.page_clocks(ui),
                                    Page::Playlists => self.page_playlists(ui),
                                    Page::Force => self.page_force(ui),
                                    Page::FinalLog => self.page_final(ui),
                                    Page::LogViewer => self.page_logviewer(ui),
                                    Page::General => self.page_general(ui),
                                    Page::Outputs => self.page_outputs(ui),
                                    Page::Clap => self.page_clap(ui),
                                    Page::Live => self.page_live(ui),
                                    Page::Shortcuts => self.page_shortcuts(ui),
                                    Page::Users => self.page_users(ui),
                                    Page::Folders => self.page_folders(ui),
                                    Page::Server => self.page_server(ui),
                                    Page::Stream => self.page_stream(ui),
                                    Page::Backup => self.page_backup(ui),
                                    Page::Updater => self.page_updater(ui),
                                    Page::Jazler => self.page_jazler(ui),
                                    Page::Integrity => self.page_integrity(ui),
                                    Page::Reports => self.page_reports(ui),
                                    Page::Reset => self.page_reset(ui),
                                }
                                if !self.cp.msg.is_empty() {
                                    ui.add_space(10.0);
                                    ui.colored_label(kit::GOOD, &self.cp.msg.clone());
                                }
                            });
                        });
                    });
                });
            });
        if close {
            self.cp_open = false;
        }
        self.edit_windows(ctx);
    }

    // ---------------------------------------------------------------- libraries
    fn start_import(&mut self, path: PathBuf, kind: &'static str) {
        let o = ImportOpts {
            kind: kind.to_string(),
            cat1: String::new(),
            autocue: false,
            autocue_db: if kind == "spot" { self.data.s.spot_autocue_db } else { self.data.s.autocue_db },
            bookmark: false,
            gain_db: self.data.s.default_gain_db,
            read_tags: true,
            parse_filename: true,
        };
        self.cp.import_pending = Some((path, o));
    }

    fn cat_lib_of(kind: &str) -> &'static str {
        match kind {
            "spot" => "spot",
            "jingle" => "jingle",
            "asset" => "asset",
            "sweeper" => "sweeper",
            "voice" => "voice",
            _ => "song1",
        }
    }

    fn page_library(&mut self, ui: &mut egui::Ui, kind: &'static str) {
        let title = KINDS.iter().find(|k| k.0 == kind).map(|k| k.1).unwrap_or("Library");
        kit::page_title(ui, title, "");
        let cat_lib = Self::cat_lib_of(kind);
        let cats = self.data.cat_names(cat_lib);
        ui.add_space(2.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(tr("Filter:")).strong());
            ui.add(egui::TextEdit::singleline(&mut self.cp.lib_filter).desired_width(180.0));
            combo(ui, "libcat", "Category:", &mut self.cp.lib_cat, &cats);
        });
        ui.horizontal_wrapped(|ui| {
            if kit::primary_button(ui, "Add single file...").clicked() {
                if let Some(p) = rfd::FileDialog::new().add_filter("audio", &library::EXTS).pick_file() {
                    self.start_import(p, kind);
                }
            }
            if kit::primary_button(ui, "Import folder...").clicked() {
                if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                    self.start_import(dir, kind);
                }
            }
            if ui.button(tr("Edit categories")).clicked() {
                self.cp.cat_lib = cat_lib.to_string();
            }
            if ui.button(tr("Export to playlister")).clicked() {
                let list: Vec<&Track> = self.data.tracks.iter().filter(|t| t.kind == kind).collect();
                match export::write_lines(&self.base, &format!("playlister_{kind}.txt"), &export::playlister(&list)) {
                    Ok(p) => self.cp.msg = format!("Exported to {}", p.display()),
                    Err(e) => self.cp.msg = e,
                }
            }
            if ui.button(tr("Print list (CSV)")).clicked() {
                let mut lines = vec!["Code;Artist;Title;Category;Duration".to_string()];
                for t in self.data.tracks.iter().filter(|t| t.kind == kind) {
                    lines.push(format!("{};{};{};{};{}", t.code(), export::csv_esc(&t.artist), export::csv_esc(&t.title), export::csv_esc(&t.cat1), fmt_ms(t.duration_ms)));
                }
                match export::write_lines(&self.base, &format!("list_{kind}.csv"), &lines) {
                    Ok(p) => self.cp.msg = format!("Saved {}", p.display()),
                    Err(e) => self.cp.msg = e,
                }
            }
        });
        if kind == "music" {
            ui.horizontal_wrapped(|ui| {
                ui.label(tr("Statistics from"));
                ui.add(egui::TextEdit::singleline(&mut self.cp.stats_from).desired_width(80.0));
                ui.label(tr("to"));
                ui.add(egui::TextEdit::singleline(&mut self.cp.stats_to).desired_width(80.0));
                if ui.button(tr("Broadcast statistics")).clicked() {
                    let counts: HashMap<u64, u64> = self.db.play_counts(&self.cp.stats_from, &self.cp.stats_to).unwrap_or_default().into_iter().collect();
                    let mut v: Vec<(String, u64)> = self.data.tracks.iter().filter(|t| t.kind == "music").map(|t| (t.label(), counts.get(&t.id).copied().unwrap_or(0))).collect();
                    v.sort_by(|a, b| b.1.cmp(&a.1));
                    self.cp.stats = v;
                }
            });
        }
        self.cat_editor(ui);
        ui.separator();
        let f = self.cp.lib_filter.to_lowercase();
        let cf = self.cp.lib_cat.clone();
        let ids: Vec<u64> = self
            .data
            .tracks
            .iter()
            .filter(|t| t.kind == kind)
            .filter(|t| f.is_empty() || t.title.to_lowercase().contains(&f) || t.artist.to_lowercase().contains(&f))
            .filter(|t| cf.is_empty() || t.cat1 == cf)
            .map(|t| t.id)
            .collect();
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("{} items, {} selected", ids.len(), self.cp.lib_sel.len()));
            if ui.button(tr("Select all shown")).clicked() {
                self.cp.lib_sel = ids.clone();
            }
            if ui.button(tr("Clear selection")).clicked() {
                self.cp.lib_sel.clear();
            }
            if ui.button(tr("Mass change...")).clicked() && !self.cp.lib_sel.is_empty() {
                self.cp.mass = Some(MassEdit::default());
            }
            if ui.button(tr("Delete selected")).clicked() && !self.cp.lib_sel.is_empty() {
                self.cp.del_ask = Some((self.cp.lib_sel.clone(), false));
            }
        });
        if let Some((ids, mut files)) = self.cp.del_ask.clone() {
            let mut answer: Option<bool> = None;
            egui::Window::new(tr("Delete from the library")).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ui.ctx(), |ui| {
                ui.label(format!("{} {}", ids.len(), tr("items will be deleted from the library.")));
                ui.add_space(4.0);
                ui.checkbox(&mut files, tr("Also delete the audio files from the disk (with a server: from the disk of the server)"));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if kit::danger_button(ui, "Delete").clicked() {
                        answer = Some(true);
                    }
                    if ui.button(tr("Cancel")).clicked() {
                        answer = Some(false);
                    }
                });
            });
            match answer {
                Some(true) => {
                    self.cp.del_ask = None;
                    let m = self.delete_library_items(&ids, files);
                    self.cp.msg = m;
                    self.cp.lib_sel.clear();
                }
                Some(false) => self.cp.del_ask = None,
                None => self.cp.del_ask = Some((ids, files)),
            }
        }
        let mut edit_id = None;
        let mut toggle = None;
        // column table: header + zebra rows, fixed column widths
        let total_w = ui.available_width().max(520.0);
        let cw = [86.0, (total_w - 86.0 - 130.0 - 80.0) * 0.4, (total_w - 86.0 - 130.0 - 80.0) * 0.6, 130.0, 80.0];
        let (hr, _) = ui.allocate_exact_size(vec2(total_w, 26.0), Sense::hover());
        ui.painter().rect_filled(hr, 5.0, Color32::from_rgb(30, 36, 50));
        let mut x = hr.left() + 10.0;
        for (i, h) in ["Code", "Artist", "Title", "Category", "Duration"].iter().enumerate() {
            ui.painter().text(pos2(x, hr.center().y), Align2::LEFT_CENTER, tr(h), FontId::proportional(12.0), Color32::WHITE);
            x += cw[i];
        }
        let row_h = 26.0;
        let avail_h = (ui.available_height() - 40.0).clamp(220.0, 900.0);
        egui::ScrollArea::vertical().id_salt("libtbl").max_height(avail_h).auto_shrink([false, false]).show_rows(ui, row_h, ids.len(), |ui, range| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for r in range {
                if let Some(t) = self.data.track(ids[r]) {
                    let (rect, resp) = ui.allocate_exact_size(vec2(total_w, row_h), Sense::click());
                    let sel = self.cp.lib_sel.contains(&t.id);
                    let bg = if sel { kit::ACCENT.gamma_multiply(0.45) } else if resp.hovered() { Color32::from_rgb(52, 60, 80) } else if r % 2 == 0 { Color32::from_rgb(12, 14, 18) } else { Color32::from_rgb(19, 22, 29) };
                    ui.painter().rect_filled(rect, 3.0, bg);
                    let col = if t.enabled { Color32::WHITE } else { Color32::from_rgb(255, 130, 130) };
                    let code = format!("{}{}", if t.bookmarked { "* " } else { "" }, t.code());
                    let cells = [code, t.artist.clone(), t.title.clone(), t.cat1.clone(), fmt_ms(t.duration_ms)];
                    let mut x = rect.left() + 10.0;
                    for (i, c) in cells.iter().enumerate() {
                        let clip = Rect::from_min_size(pos2(x, rect.top()), vec2(cw[i] - 8.0, row_h));
                        ui.painter().with_clip_rect(clip).text(pos2(x, rect.center().y), Align2::LEFT_CENTER, c, FontId::proportional(11.0), col);
                        x += cw[i];
                    }
                    if resp.clicked() {
                        toggle = Some((t.id, ui.input(|i| i.modifiers.ctrl || i.modifiers.shift)));
                    }
                    if resp.double_clicked() {
                        edit_id = Some(t.id);
                    }
                }
            }
        });
        if let Some((id, multi)) = toggle {
            if multi {
                if let Some(pos) = self.cp.lib_sel.iter().position(|x| *x == id) {
                    self.cp.lib_sel.remove(pos);
                } else {
                    self.cp.lib_sel.push(id);
                }
            } else {
                self.cp.lib_sel = vec![id];
            }
        }
        if let Some(id) = edit_id {
            self.cp.edit = self.data.track(id).cloned();
        }
        ui.label(tr("Double-click an item to edit it. Ctrl/Shift-click selects several items."));
        if kind == "spot" {
            self.spots_section(ui);
        }
        if !self.cp.stats.is_empty() && kind == "music" {
            ui.separator();
            ui.label(tr("Broadcast statistics (plays in the period):"));
            egui::ScrollArea::vertical().id_salt("stats").max_height(160.0).show(ui, |ui| {
                for (l, n) in self.cp.stats.iter().take(200) {
                    ui.label(format!("{n:>4}  {l}"));
                }
            });
        }
    }

    fn cat_editor(&mut self, ui: &mut egui::Ui) {
        if self.cp.cat_lib.is_empty() {
            return;
        }
        let lib = self.cp.cat_lib.clone();
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(tr("Categories")).strong());
                if lib.starts_with("song") {
                    for (i, l) in ["song1", "song2", "song3", "vocal", "props"].iter().enumerate() {
                        let name = match i {
                            0..=2 => self.data.s.cat_names.get(i).cloned().unwrap_or_default(),
                            3 => "Vocal".into(),
                            _ => "Properties".into(),
                        };
                        if ui.selectable_label(lib == *l, name).clicked() {
                            self.cp.cat_lib = l.to_string();
                        }
                    }
                }
                if ui.button(tr("Close")).clicked() {
                    self.cp.cat_lib.clear();
                }
            });
            if lib.starts_with("song") && lib != "vocal" && lib != "props" {
                let idx = lib.trim_start_matches("song").parse::<usize>().unwrap_or(1) - 1;
                let mut name = self.data.s.cat_names.get(idx).cloned().unwrap_or_default();
                let mut resp = None;
                ui.horizontal(|ui| {
                    ui.label(tr("Name of this category field:"));
                    resp = Some(ui.text_edit_singleline(&mut name));
                });
                if let Some(r) = resp {
                    if r.changed() {
                        if let Some(n) = self.data.s.cat_names.get_mut(idx) {
                            *n = name.clone();
                        }
                    }
                    if r.lost_focus() {
                        self.db.save_settings(&self.data.s);
                    }
                }
            }
            let mut del = None;
            let mut upd: Option<Category> = None;
            for c in self.data.cats.iter().filter(|c| c.lib == lib) {
                ui.horizontal(|ui| {
                    let mut x = c.clone();
                    let mut changed = ui.text_edit_singleline(&mut x.name).lost_focus();
                    changed |= ui.color_edit_button_srgb(&mut x.color).changed();
                    changed |= ui.add(egui::TextEdit::singleline(&mut x.comment).hint_text(tr("comment")).desired_width(160.0)).lost_focus();
                    if changed {
                        upd = Some(x);
                    }
                    if ui.small_button(tr("remove")).clicked() {
                        del = Some(c.id);
                    }
                });
            }
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.cp.new_cat).hint_text(tr("new category")).desired_width(160.0));
                if ui.button(tr("Add")).clicked() && !self.cp.new_cat.trim().is_empty() {
                    let mut c = Category { lib: lib.clone(), name: self.cp.new_cat.trim().to_string(), color: [90, 90, 90], ..Default::default() };
                    self.save_doc("category", &mut c);
                    self.cp.new_cat.clear();
                }
            });
            if let Some(mut x) = upd {
                self.save_doc("category", &mut x);
            }
            if let Some(id) = del {
                self.del_doc(id);
            }
        });
    }

    // ---------------------------------------------------------------- memos
    fn page_memos(&mut self, ui: &mut egui::Ui) {
        ui.heading(tr("Memos"));
        let cats = self.data.cat_names("memo");
        if ui.button(tr("New memo")).clicked() {
            self.cp.memo = Some(Memo::default());
        }
        let mut open = None;
        let mut del = None;
        for m in &self.data.memos {
            ui.horizontal(|ui| {
                ui.label(format!("[{}] {}", m.category, m.title));
                if ui.small_button(tr("edit")).clicked() {
                    open = Some(m.clone());
                }
                if ui.small_button(tr("delete")).clicked() {
                    del = Some(m.id);
                }
            });
        }
        if open.is_some() {
            self.cp.memo = open;
        }
        if let Some(id) = del {
            self.del_doc(id);
        }
        if ui.button(tr("Import text file as memo...")).clicked() {
            if let Some(p) = rfd::FileDialog::new().add_filter("text", &["txt"]).pick_file() {
                if let Ok(body) = std::fs::read_to_string(&p) {
                    let title = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                    let mut m = Memo { title, body, ..Default::default() };
                    self.save_doc("memo", &mut m);
                }
            }
        }
        ui.separator();
        ui.label(tr("Memo categories"));
        self.cp.cat_lib = "memo".into();
        let _ = cats;
        self.cat_editor(ui);
    }

    // ---------------------------------------------------------------- instant jingles
    fn page_instants(&mut self, ui: &mut egui::Ui) {
        ui.heading(tr("Instant jingles - pallets"));
        ui.label(tr("Create pallets (one per producer), then right-click a button in the studio to assign the selected library item."));
        if ui.button(tr("New pallet")).clicked() {
            self.cp.pallet = Some(Pallet { name: "New pallet".into(), pages: 1, rows: 4, cols: 4, ..Default::default() });
        }
        let mut edit = None;
        let mut del = None;
        for p in &self.data.pallets {
            ui.horizontal(|ui| {
                ui.label(format!("{} ({} page(s), {}x{})", p.name, p.pages, p.rows, p.cols));
                if ui.small_button(tr("edit")).clicked() {
                    edit = Some(p.clone());
                }
                if ui.small_button(tr("delete")).clicked() {
                    del = Some(p.id);
                }
            });
        }
        if edit.is_some() {
            self.cp.pallet = edit;
        }
        if let Some(id) = del {
            self.del_doc(id);
            self.pallet_idx = 0;
        }
        ui.separator();
        ui.label(tr("Audio output of instant jingles: Settings > Sound outputs."));
    }

    // ---------------------------------------------------------------- stitcher
    fn page_stitcher(&mut self, ui: &mut egui::Ui) {
        ui.heading(tr("The Stitcher"));
        ui.label(tr("Merges separate audio files into one stream. Add 'Stitcher' rows to a clock to broadcast them."));
        let mut changed = false;
        let pick = |ui: &mut egui::Ui, label: &str, v: &mut String| -> bool {
            let mut ch = false;
            ui.horizontal(|ui| {
                ui.label(label);
                ch |= ui.add(egui::TextEdit::singleline(v).desired_width(420.0)).lost_focus();
                if ui.small_button(tr("...")).clicked() {
                    if let Some(p) = rfd::FileDialog::new().pick_file() {
                        *v = p.to_string_lossy().to_string();
                        ch = true;
                    }
                }
            });
            ch
        };
        let c = &mut self.data.s.stitch;
        ui.label(RichText::new(tr("Next songs hooks")).strong());
        changed |= pick(ui, "Opening audio", &mut c.hooks_open);
        changed |= pick(ui, "Separator effect", &mut c.hooks_sep);
        changed |= pick(ui, "Closing audio", &mut c.hooks_close);
        changed |= pick(ui, "Default audio (if hooks are missing)", &mut c.hooks_default);
        ui.horizontal(|ui| {
            ui.label(tr("Number of hooks"));
            changed |= ui.add(egui::DragValue::new(&mut c.hooks_count).range(1..=8)).changed();
        });
        ui.label(tr("Songs need hook-in / hook-out marks (edit a song)."));
        ui.separator();
        ui.label(RichText::new(tr("Real time announcement")).strong());
        changed |= pick(ui, "Opening audio", &mut c.time_open);
        changed |= pick(ui, "Closing audio", &mut c.time_close);
        changed |= pick(ui, "Default audio", &mut c.time_default);
        path_row(ui, "Folder with h00..h23 and m00..m59 audio files", &mut c.time_dir, true);
        ui.separator();
        ui.label(RichText::new(tr("News")).strong());
        changed |= pick(ui, "Opening audio", &mut c.news_open);
        changed |= pick(ui, "News voice-over file", &mut c.news_voice);
        changed |= pick(ui, "Closing audio", &mut c.news_close);
        changed |= pick(ui, "Default audio", &mut c.news_default);
        if changed || ui.button(tr("Save")).clicked() {
            self.save_settings();
        }
    }

    // ---------------------------------------------------------------- spots
    fn spots_section(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        ui.heading(tr("Campaigns"));
        ui.horizontal_wrapped(|ui| {
            if ui.button(tr("Add new campaign")).clicked() {
                self.cp.camp = Some(Campaign { name: "New campaign".into(), enabled: true, never: true, mode: "weekly".into(), system: "order".into(), start: today(), category: "Commercials".into(), ..Default::default() });
                self.cp.camp_prio = 4;
                self.cp.camp_days = [true; 7];
                self.cp.camp_times.clear();
            }
        });
        let mut edit = None;
        let mut del = None;
        for c in &self.data.campaigns {
            ui.horizontal(|ui| {
                let end = if c.never { "never".to_string() } else { c.end.clone() };
                let mut tms: Vec<String> = c.slots.iter().map(|s| s.time.clone()).collect();
                tms.sort();
                tms.dedup();
                let tshow = if tms.is_empty() { "-".to_string() } else { tms.join(" ") };
                ui.label(format!("{}{} [{}] {} -> {} ({} spots, {} slots) @ {}", if c.enabled { "" } else { "(off) " }, c.name, c.mode, c.start, end, c.spots.len(), c.slots.len(), tshow));
                if ui.small_button(tr("edit")).clicked() {
                    edit = Some(c.clone());
                }
                if ui.small_button(tr("plays report")).clicked() {
                    edit = None;
                    self.cp.spots_msg = c.id.to_string();
                }
                if ui.small_button(tr("delete")).clicked() {
                    del = Some(c.id);
                }
            });
        }
        if let Some(c) = edit {
            // the marked break times / days belong to the campaign being edited, not to the one edited before
            let mut tms: Vec<String> = c.slots.iter().map(|s| s.time.clone()).collect();
            tms.sort();
            tms.dedup();
            self.cp.camp_times = tms;
            self.cp.camp_prio = c.slots.first().map(|s| s.priority).unwrap_or(4);
            let mut days = [false; 7];
            for s in &c.slots {
                if s.weekday >= 0 && (s.weekday as usize) < 7 {
                    days[s.weekday as usize] = true;
                }
            }
            self.cp.camp_days = if days.iter().any(|d| *d) { days } else { [true; 7] };
            self.cp.camp = Some(c);
        }
        if let Some(id) = del {
            self.del_doc(id);
        }
        if let Ok(id) = self.cp.spots_msg.parse::<u64>() {
            self.cp.spots_msg.clear();
            self.campaign_report(id);
        }
        ui.separator();
        ui.heading(tr("Commercial breaks & priorities"));
        ui.horizontal_wrapped(|ui| {
            ui.label(tr("Add break at (HH:MM):"));
            ui.add(egui::TextEdit::singleline(&mut self.cp.brk_text).desired_width(60.0));
            if ui.button(tr("Add")).clicked() && engine::parse_hm(&self.cp.brk_text).is_some() {
                let mut b = BreakDef { time: self.cp.brk_text.trim().to_string(), visible: true, ..Default::default() };
                self.save_doc("break", &mut b);
                self.cp.brk_text.clear();
            }
            ui.label(tr("Break view date:"));
            ui.add(egui::TextEdit::singleline(&mut self.cp.bdate).desired_width(80.0));
        });
        let mut bdel = None;
        let mut bupd = None;
        let bd = engine::parse_date(&self.cp.bdate);
        egui::ScrollArea::vertical().id_salt("breaks").max_height(220.0).show(ui, |ui| {
            let mut brks = self.data.breaks.clone();
            brks.sort_by(|a, b| a.time.cmp(&b.time));
            for b in brks {
                ui.horizontal(|ui| {
                    let mut x = b.clone();
                    ui.label(&b.time);
                    let mut ch = false;
                    ui.label(tr("start immediately on:"));
                    for d in 0..7 {
                        let mut on = (x.immediate_mask >> d) & 1 == 1;
                        if ui.checkbox(&mut on, tr(DAYS[d])).changed() {
                            if on {
                                x.immediate_mask |= 1 << d;
                            } else {
                                x.immediate_mask &= !(1 << d);
                            }
                            ch = true;
                        }
                    }
                    if let Some(date) = bd {
                        let items = engine::break_tracks(&self.data, date, &b.time);
                        let tot: u64 = items.iter().map(|t| t.duration_ms).sum();
                        ui.label(format!("{} spots, {}", items.len(), fmt_ms(tot)));
                    }
                    if ch {
                        bupd = Some(x);
                    }
                    if ui.small_button(tr("remove")).clicked() {
                        bdel = Some(b.id);
                    }
                });
            }
        });
        if let Some(mut x) = bupd {
            self.save_doc("break", &mut x);
        }
        if let Some(id) = bdel {
            self.del_doc(id);
        }
        ui.label(RichText::new(tr("Priorities (lower number plays first)")).strong());
        let mut changed = false;
        let mut remove = None;
        for (i, p) in self.data.s.prios.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                changed |= ui.add(egui::DragValue::new(&mut p.num).range(1..=99)).changed();
                changed |= ui.add(egui::TextEdit::singleline(&mut p.name).desired_width(200.0)).lost_focus();
                changed |= ui.checkbox(&mut p.time_announce, tr("time announcement")).changed();
                changed |= ui.checkbox(&mut p.playlister, tr("playlister")).changed();
                changed |= ui.checkbox(&mut p.optional, tr("optional play")).changed();
                if ui.small_button(tr("remove")).clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            self.data.s.prios.remove(i);
            changed = true;
        }
        if ui.button(tr("Add priority")).clicked() {
            let n = self.data.s.prios.iter().map(|p| p.num).max().unwrap_or(0) + 1;
            self.data.s.prios.push(Priority { num: n, name: "New priority".into(), ..Default::default() });
            changed = true;
        }
        if changed {
            self.save_settings();
        }
        ui.separator();
        ui.heading(tr("Playlister (traffic software) import / export"));
        ui.horizontal_wrapped(|ui| {
            ui.label(tr("Text file lines: TIME,CODE,TITLE - file name YYYYMMDD.txt or the date above."));
            if ui.button(tr("Import breaks playlist...")).clicked() {
                if let Some(p) = rfd::FileDialog::new().add_filter("text", &["txt", "asc"]).pick_file() {
                    self.import_break_playlist(&p);
                }
            }
            if ui.button(tr("Export break durations")).clicked() {
                if let Some(date) = bd {
                    let mut lines = Vec::new();
                    for b in &self.data.breaks {
                        let items = engine::break_tracks(&self.data, date, &b.time);
                        let tot: u64 = items.iter().map(|t| t.duration_ms).sum();
                        lines.push(format!("{},{}", b.time, tot / 1000));
                    }
                    match export::write_lines(&self.base, &format!("break_durations_{}.txt", self.cp.bdate), &lines) {
                        Ok(p) => self.cp.msg = format!("Saved {}", p.display()),
                        Err(e) => self.cp.msg = e,
                    }
                }
            }
            if ui.button(tr("Daily spots report")).clicked() {
                if let Some(date) = bd {
                    let mut lines = vec!["Break;Priority source;Spot".to_string()];
                    for b in &self.data.breaks {
                        for t in engine::break_tracks(&self.data, date, &b.time) {
                            lines.push(format!("{};{};{}", b.time, export::csv_esc(&t.reason), export::csv_esc(&t.title)));
                        }
                    }
                    match export::write_lines(&self.base, &format!("daily_spots_{}.csv", self.cp.bdate), &lines) {
                        Ok(p) => self.cp.msg = format!("Saved {}", p.display()),
                        Err(e) => self.cp.msg = e,
                    }
                }
            }
            if ui.button(tr("Clean up old playlister spots")).clicked() {
                let cutoff = (Local::now() - Duration::days(60)).format("%Y-%m-%d").to_string();
                let recent: std::collections::HashSet<String> = self.data.bpl.iter().filter(|b| b.date >= cutoff).map(|b| b.code.clone()).collect();
                let olds: Vec<Track> = self.data.tracks.iter().filter(|t| t.kind == "spot" && t.enabled && !recent.contains(&t.code())).cloned().collect();
                let n = olds.len();
                for mut t in olds {
                    t.enabled = false;
                    let _ = self.db.update_track(&t);
                }
                self.reload_tracks();
                self.cp.msg = format!("{n} spots without playlister scheduling in the last 60 days were disabled");
            }
        });
    }

    fn import_break_playlist(&mut self, p: &Path) {
        let Ok(text) = std::fs::read_to_string(p) else {
            self.cp.msg = tr("Could not read the file");
            return;
        };
        let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let date = if stem.len() == 8 && stem.chars().all(|c| c.is_ascii_digit()) {
            format!("{}-{}-{}", &stem[0..4], &stem[4..6], &stem[6..8])
        } else {
            self.cp.bdate.clone()
        };
        for b in self.data.bpl.clone().into_iter().filter(|b| b.date == date) {
            let _ = self.db.del_doc(b.id);
        }
        let times: Vec<String> = self.data.breaks.iter().map(|b| b.time.clone()).collect();
        let (mut n, mut bad) = (0, 0);
        for line in text.lines() {
            let f: Vec<&str> = line.split(',').collect();
            if f.len() < 2 {
                continue;
            }
            let time = f[0].trim().chars().take(5).collect::<String>();
            if !times.contains(&time) || self.data.by_code("spot", f[1].trim()).is_none() {
                bad += 1;
                continue;
            }
            let mut b = BreakPl { date: date.clone(), time, code: f[1].trim().to_string(), ..Default::default() };
            let _ = self.db.save_doc("bpl", &mut b);
            n += 1;
        }
        self.reload_docs();
        self.cp.msg = format!("Imported {n} lines for {date}; {bad} lines rejected (unknown break time or code)");
    }

    fn campaign_report(&mut self, id: u64) {
        let Some(c) = self.data.campaigns.iter().find(|c| c.id == id).cloned() else { return };
        let to = today();
        let rows = self.db.history_range(&c.start, &to).unwrap_or_default();
        let mut lines = vec![format!("Actual plays of campaign '{}' ({} - {})", c.name, c.start, to), "Date;Time;Spot".to_string()];
        for r in rows.iter().rev().filter(|r| c.spots.contains(&r.track_id)) {
            let (d, t) = r.at.split_at(10.min(r.at.len()));
            lines.push(format!("{d};{};{}", t.trim(), export::csv_esc(&r.title)));
        }
        lines.push(String::new());
        lines.push("Programmed schedule".into());
        for s in &c.slots {
            let when = if c.mode == "detailed" { s.date.clone() } else { DAYS.get(s.weekday.max(0) as usize).unwrap_or(&"?").to_string() };
            lines.push(format!("{when};{};priority {}", s.time, s.priority));
        }
        match export::write_lines(&self.base, &format!("campaign_{}.csv", c.name.replace(' ', "_")), &lines) {
            Ok(p) => self.cp.msg = format!("Saved {}", p.display()),
            Err(e) => self.cp.msg = e,
        }
    }

    // ---------------------------------------------------------------- clocks
    fn page_clocks(&mut self, ui: &mut egui::Ui) {
        ui.heading(tr("Main auto schedule"));
        ui.horizontal_wrapped(|ui| {
            if ui.button(tr("Create new clock")).clicked() {
                self.cp.clock = Some(Clock { name: "New clock".into(), color: [60, 120, 200], ..Default::default() });
            }
            let mut classic = self.data.s.classic_auto;
            if ui.checkbox(&mut classic, tr("Classic auto: keep a fixed number of items ahead")).changed() {
                self.data.s.classic_auto = classic;
                self.save_settings();
                self.gen = None;
            }
            let (mut decks, mut hours) = (self.data.s.classic_decks, self.data.s.auto_hours);
            if ui.add(egui::DragValue::new(&mut decks).range(1..=60).prefix(tr("decks: "))).changed() {
                self.data.s.classic_decks = decks;
                self.save_settings();
            }
            if ui.add(egui::DragValue::new(&mut hours).range(1..=24).prefix(tr("Auto mode: hours generated ahead: "))).changed() {
                self.data.s.auto_hours = hours;
                self.save_settings();
            }
        });
        let mut edit = None;
        let mut del = None;
        for c in &self.data.clocks {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                ui.painter().rect_filled(r, 2.0, Color32::from_rgb(c.color[0], c.color[1], c.color[2]));
                ui.label(format!("{} ({} rows)", c.name, c.rows.len()));
                if ui.small_button(tr("edit")).clicked() {
                    edit = Some(c.clone());
                }
                if ui.small_button(tr("delete")).clicked() {
                    del = Some(c.id);
                }
            });
        }
        if edit.is_some() {
            self.cp.clock = edit;
        }
        if let Some(id) = del {
            self.del_doc(id);
        }
        ui.separator();
        ui.label(RichText::new(tr("Clocks schedule (weekday x half hour). Pick a clock or a playlist (\u{25B6}, played at the start of its block), then click or drag over the cells. Click a day name to fill the whole day, an hour to fill that hour on every day.")).strong());
        let mut names: Vec<(u64, String, [u8; 3])> = self.data.clocks.iter().map(|c| (c.id, c.name.clone(), c.color)).collect();
        // playlists can be called at a time from the same schedule (purple)
        names.extend(self.data.playlists.iter().map(|p| (p.id, format!("\u{25B6} {}", p.name), [120, 80, 170])));
        ui.horizontal_top(|ui| {
        // ---- left: the brush palette (clocks and playlists), with a search box
        ui.allocate_ui_with_layout(egui::vec2(230.0, 640.0), egui::Layout::top_down(egui::Align::LEFT), |ui| {
            egui::Frame::none().fill(kit::INSET).rounding(8.0).inner_margin(8.0).show(ui, |ui| {
                ui.set_width(214.0);
                ui.label(RichText::new(tr("BRUSH")).small().color(kit::TEXT_DIM));
                let cur = names.iter().find(|n| n.0 == self.cp.grid_brush).cloned();
                let (cr, _) = ui.allocate_exact_size(egui::vec2(214.0, 34.0), Sense::hover());
                let ccol = cur.as_ref().map(|c| Color32::from_rgb(c.2[0], c.2[1], c.2[2])).unwrap_or(kit::CONTROL);
                ui.painter().rect_filled(cr, 6.0, ccol);
                let cfg = if (ccol.r() as u32 * 299 + ccol.g() as u32 * 587 + ccol.b() as u32 * 114) / 1000 > 150 { Color32::BLACK } else { Color32::WHITE };
                let ctxt = cur.as_ref().map(|c| c.1.clone()).unwrap_or_else(|| tr("(none) - erase"));
                ui.painter().text(cr.center(), Align2::CENTER_CENTER, ctxt, FontId::proportional(14.0), cfg);
                ui.add_space(6.0);
                ui.add(egui::TextEdit::singleline(&mut self.cp.grid_q).desired_width(214.0).hint_text(tr("search clock or playlist...")));
                ui.add_space(4.0);
                let q = self.cp.grid_q.to_lowercase();
                egui::ScrollArea::vertical().id_salt("brushlist").max_height(540.0).auto_shrink([false, false]).show(ui, |ui| {
                    let mut row = |ui: &mut egui::Ui, id: u64, label: &str, col: Color32, brush: &mut u64| {
                        let (r, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 26.0), Sense::click());
                        let on = *brush == id;
                        if on {
                            ui.painter().rect_filled(r, 5.0, kit::ACCENT.gamma_multiply(0.28));
                        } else if resp.hovered() {
                            ui.painter().rect_filled(r, 5.0, kit::CONTROL_HOVER);
                        }
                        ui.painter().rect_filled(Rect::from_min_size(pos2(r.left() + 6.0, r.top() + 5.0), egui::vec2(16.0, 16.0)), 4.0, col);
                        let g = ui.painter().layout_no_wrap(label.to_string(), FontId::proportional(13.0), kit::TEXT);
                        let clip = ui.painter().with_clip_rect(Rect::from_min_max(pos2(r.left() + 28.0, r.top()), pos2(r.right() - 4.0, r.bottom())));
                        clip.galley(pos2(r.left() + 30.0, r.center().y - g.size().y / 2.0), g, kit::TEXT);
                        if resp.clicked() {
                            *brush = id;
                        }
                    };
                    row(ui, 0, &tr("(none) - erase"), kit::CONTROL, &mut self.cp.grid_brush);
                    ui.add_space(4.0);
                    ui.label(RichText::new(tr("CLOCKS")).small().color(kit::TEXT_DIM));
                    for c in self.data.clocks.iter().filter(|c| q.is_empty() || c.name.to_lowercase().contains(&q)) {
                        row(ui, c.id, &c.name, Color32::from_rgb(c.color[0], c.color[1], c.color[2]), &mut self.cp.grid_brush);
                    }
                    ui.add_space(4.0);
                    ui.label(RichText::new(tr("PLAYLISTS (played at the start of the block)")).small().color(kit::TEXT_DIM));
                    for pl in self.data.playlists.iter().filter(|p| q.is_empty() || p.name.to_lowercase().contains(&q)) {
                        row(ui, pl.id, &format!("\u{25B6} {}", pl.name), Color32::from_rgb(120, 80, 170), &mut self.cp.grid_brush);
                    }
                });
            });
        });
        ui.add_space(8.0);
        // ---- right: the weekly grid
        let gw = ui.available_width().max(420.0);
        ui.allocate_ui_with_layout(egui::vec2(gw, 20.0), egui::Layout::top_down(egui::Align::LEFT), |ui| {
        // 48 slots a day: index = weekday * 48 + hour * 2 + (minute >= 30)
        let mut grid = self.data.s.clock_grid.clone();
        if grid.len() == 168 {
            grid = (0..336).map(|i| grid[(i / 48) * 24 + (i % 48) / 2]).collect(); // older versions: one clock per hour
        }
        grid.resize(336, 0);
        let mut changed = false;
        let (label_w, head_h, row_h) = (46.0f32, 26.0f32, 14.0f32);
        let total_w = ui.available_width().max(400.0);
        let col_w = ((total_w - label_w) / 7.0).max(60.0);
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(label_w + col_w * 7.0, head_h + row_h * 48.0), egui::Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 8.0, kit::INSET);
        let clock_of = |id: u64| names.iter().find(|n| n.0 == id);
        // day headers
        let now = Local::now();
        let (now_d, now_slot) = (now.weekday().num_days_from_monday() as usize, (now.hour() * 2 + now.minute() / 30) as usize);
        for d in 0..7 {
            let x0 = rect.left() + label_w + d as f32 * col_w;
            let hr = Rect::from_min_size(pos2(x0, rect.top()), egui::vec2(col_w, head_h));
            let today = d == now_d;
            painter.text(hr.center(), Align2::CENTER_CENTER, tr(DAYS[d]), FontId::proportional(13.5), if today { kit::ACCENT } else { kit::TEXT });
            if today {
                painter.rect_filled(Rect::from_min_size(pos2(x0 + 6.0, rect.top() + head_h - 3.0), egui::vec2(col_w - 12.0, 2.0)), 1.0, kit::ACCENT);
            }
        }
        // hour labels and the cells (neighbouring half hours with the same clock are drawn as one block)
        for h in 0..24 {
            let y = rect.top() + head_h + h as f32 * 2.0 * row_h;
            painter.text(pos2(rect.left() + label_w - 8.0, y + 1.0), Align2::RIGHT_TOP, format!("{h:02}:00"), FontId::proportional(11.0), kit::TEXT_DIM);
            if h > 0 {
                painter.line_segment([pos2(rect.left() + label_w - 4.0, y), pos2(rect.right() - 4.0, y)], Stroke::new(1.0_f32, kit::BORDER.gamma_multiply(0.7)));
            }
        }
        for d in 0..7 {
            let x0 = rect.left() + label_w + d as f32 * col_w;
            let mut i = 0;
            while i < 48 {
                let cid = grid[d * 48 + i];
                let mut j = i + 1;
                while j < 48 && grid[d * 48 + j] == cid && (j % 2 != 0 || cid != 0) {
                    j += 1;
                }
                // blocks of an assigned clock continue over the hour lines, empty slots are not merged
                let top = rect.top() + head_h + i as f32 * row_h;
                let block = Rect::from_min_max(pos2(x0 + 2.0, top + 1.0), pos2(x0 + col_w - 2.0, top + (j - i) as f32 * row_h - 1.0));
                match clock_of(cid) {
                    Some(c) => {
                        let col = Color32::from_rgb(c.2[0], c.2[1], c.2[2]);
                        painter.rect_filled(block, 4.0, col.gamma_multiply(0.85));
                        let fg = if (col.r() as u32 * 299 + col.g() as u32 * 587 + col.b() as u32 * 114) / 1000 > 150 { Color32::BLACK } else { Color32::WHITE };
                        let galley_pos = pos2(block.center().x, block.top() + (block.height().min(row_h * 2.0)) / 2.0);
                        painter.text(galley_pos, Align2::CENTER_CENTER, &c.1, FontId::proportional(11.0), fg);
                    }
                    None => {
                        painter.rect_filled(block, 3.0, kit::CARD);
                    }
                }
                i = j;
            }
        }
        // the current half hour
        let ny = rect.top() + head_h + now_slot as f32 * row_h;
        painter.rect_stroke(Rect::from_min_size(pos2(rect.left() + label_w + now_d as f32 * col_w + 1.0, ny), egui::vec2(col_w - 2.0, row_h)), 3.0, Stroke::new(1.5_f32, Color32::WHITE));
        // painting with the mouse: click, drag, day header, hour label
        let pressed = resp.clicked() || resp.dragged() || resp.drag_started();
        if pressed {
            if let Some(pos) = resp.interact_pointer_pos() {
                let (rx, ry) = (pos.x - rect.left(), pos.y - rect.top());
                let brush = self.cp.grid_brush;
                if rx >= label_w && ry >= head_h {
                    let d = (((rx - label_w) / col_w) as usize).min(6);
                    let i = (((ry - head_h) / row_h) as usize).min(47);
                    if grid[d * 48 + i] != brush {
                        grid[d * 48 + i] = brush;
                        changed = true;
                    }
                } else if rx >= label_w && resp.clicked() {
                    let d = (((rx - label_w) / col_w) as usize).min(6);
                    for i in 0..48 {
                        grid[d * 48 + i] = brush;
                    }
                    changed = true;
                } else if ry >= head_h && resp.clicked() {
                    let h = (((ry - head_h) / (row_h * 2.0)) as usize).min(23);
                    for d in 0..7 {
                        grid[d * 48 + h * 2] = brush;
                        grid[d * 48 + h * 2 + 1] = brush;
                    }
                    changed = true;
                }
            }
        }
        if changed {
            self.data.s.clock_grid = grid;
            self.save_settings();
        } else if self.data.s.clock_grid.len() != 336 {
            self.data.s.clock_grid = grid; // convert an old hourly schedule once
            self.save_settings();
        }
        });
        });
        ui.separator();
        ui.heading(tr("Song selection settings"));
        let mut ch = false;
        let s = &mut self.data.s;
        ui.horizontal_wrapped(|ui| {
            ch |= ui.add(egui::DragValue::new(&mut s.randomness).range(0..=50).prefix(tr("Selection randomness (0 = strict): "))).changed();
            ch |= ui.add(egui::DragValue::new(&mut s.artist_sep_min).range(0..=1440).prefix(tr("Artist separation (min): "))).changed();
            ch |= ui.add(egui::DragValue::new(&mut s.next_day_days).range(0..=14).prefix(tr("Same time next day(s): "))).changed();
            ch |= ui.add(egui::DragValue::new(&mut s.next_day_window_min).range(0..=720).prefix(tr("window (min): "))).changed();
        });
        ui.label(RichText::new(tr("Restrictive rules: a song matching A can not be followed by a song matching B")).strong());
        let mut rem = None;
        for (i, r) in s.rules.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("{} = '{}'  can not be followed by  {} = '{}'", r.a_field, r.a_value, r.b_field, r.b_value));
                if ui.small_button(tr("remove")).clicked() {
                    rem = Some(i);
                }
            });
        }
        if let Some(i) = rem {
            s.rules.remove(i);
            ch = true;
        }
        let fields = ["cat1", "cat2", "cat3", "vocal", "props"];
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt("ra").selected_text(self.cp.rule.a_field.clone()).show_ui(ui, |ui| {
                for f in fields {
                    ui.selectable_value(&mut self.cp.rule.a_field, f.to_string(), f);
                }
            });
            ui.add(egui::TextEdit::singleline(&mut self.cp.rule.a_value).desired_width(90.0).hint_text(tr("value A")));
            ui.label(tr("not followed by"));
            egui::ComboBox::from_id_salt("rb").selected_text(self.cp.rule.b_field.clone()).show_ui(ui, |ui| {
                for f in fields {
                    ui.selectable_value(&mut self.cp.rule.b_field, f.to_string(), f);
                }
            });
            ui.add(egui::TextEdit::singleline(&mut self.cp.rule.b_value).desired_width(90.0).hint_text(tr("value B")));
            if ui.button(tr("Add rule")).clicked() && !self.cp.rule.a_field.is_empty() && !self.cp.rule.b_field.is_empty() {
                s.rules.push(self.cp.rule.clone());
                ch = true;
            }
        });
        if ch {
            self.save_settings();
            self.gen = None;
        }
        if ui.button(tr("Reshuffle all media")).clicked() {
            self.gen = None;
            self.cp.msg = tr("Rotation order will be reshuffled the next time the auto mode generates songs");
        }
    }

    fn page_playlists(&mut self, ui: &mut egui::Ui) {
        ui.heading(tr("Playlists"));
        ui.horizontal_wrapped(|ui| {
            if ui.button(tr("New playlist")).clicked() {
                self.cp.pl = Some(Playlist { name: "New playlist".into(), ..Default::default() });
            }
            ui.label(tr("Auto playlist from clock:"));
            let names: Vec<(u64, String)> = self.data.clocks.iter().map(|c| (c.id, c.name.clone())).collect();
            egui::ComboBox::from_id_salt("apc").selected_text(names.iter().find(|n| n.0 == self.cp.pl_clock).map(|n| n.1.clone()).unwrap_or("(choose)".into())).show_ui(ui, |ui| {
                for (id, n) in &names {
                    ui.selectable_value(&mut self.cp.pl_clock, *id, n);
                }
            });
            ui.add(egui::DragValue::new(&mut self.cp.pl_hours).range(1..=24).suffix(tr(" h")));
            ui.add(egui::TextEdit::singleline(&mut self.cp.pl_date).desired_width(80.0));
            ui.add(egui::TextEdit::singleline(&mut self.cp.pl_time).desired_width(50.0).hint_text(tr("HH:MM")));
            if ui.button(tr("Create auto playlist")).clicked() && self.cp.pl_clock != 0 {
                if let (Some(d), Some(t)) = (engine::parse_date(&self.cp.pl_date), engine::parse_hm(&self.cp.pl_time)) {
                    let recent = self.recent_naive();
                    let items = engine::auto_playlist(&self.data, self.cp.pl_clock, d.and_time(t), self.cp.pl_hours, &recent);
                    let mut p = Playlist { name: format!("Auto {} {}", self.cp.pl_date, self.cp.pl_time), items, ..Default::default() };
                    p.sched.push(PSched { date: self.cp.pl_date.clone(), weekdays: 0, time: self.cp.pl_time.clone(), immediate: false });
                    self.save_doc("playlist", &mut p);
                } else {
                    self.cp.msg = tr("Enter the date (YYYY-MM-DD) and time (HH:MM) of the auto playlist");
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            if ui.button(tr("Import text file(s) by file names...")).clicked() {
                if let Some(files) = rfd::FileDialog::new().add_filter("text", &["txt", "m3u"]).pick_files() {
                    for f in files {
                        self.import_playlist_file(&f);
                    }
                }
            }
        });
        let pls = self.data.playlists.clone();
        let dur_of = |d: &engine::Data, p: &Playlist| -> u64 { p.items.iter().filter_map(|it| d.track(it.track_id)).map(|t| t.eff_ms()).sum() };
        let mut edit: Option<Playlist> = None;
        let mut del: Option<u64> = None;
        let mut new_sel = self.cp.pl_sel;
        if !pls.iter().any(|p| p.id == new_sel) {
            new_sel = pls.first().map(|p| p.id).unwrap_or(0);
        }
        let sel_pl = pls.iter().find(|p| p.id == new_sel).cloned();
        ui.horizontal(|ui| {
            ui.add_enabled_ui(sel_pl.is_some(), |ui| {
                if ui.button(tr("Edit selected")).clicked() {
                    edit = sel_pl.clone();
                }
                if ui.button(tr("Delete selected")).clicked() {
                    del = sel_pl.as_ref().map(|p| p.id);
                }
            });
        });
        ui.add_space(4.0);
        let total = ui.available_width();
        let right_w = 330.0f32.min(total * 0.42);
        let h = (ui.available_height() - 6.0).max(320.0);
        let purple = Color32::from_rgb(0x5a, 0x2d, 0x91);
        ui.horizontal_top(|ui| {
            // ---- left grid: all the playlists
            ui.allocate_ui_with_layout(vec2((total - right_w - 12.0).max(200.0), h), egui::Layout::top_down(egui::Align::LEFT), |ui| {
                let w = ui.available_width();
                let (hr, _) = ui.allocate_exact_size(vec2(w, 22.0), Sense::hover());
                let cols = [0.0_f32, w * 0.52, w * 0.66, w * 0.80];
                for (i, t) in ["Title", "Duration", "Items", "Scheduled"].iter().enumerate() {
                    ui.painter().text(pos2(hr.left() + cols[i] + 8.0, hr.center().y), Align2::LEFT_CENTER, tr(t), FontId::proportional(12.0), kit::TEXT_DIM);
                }
                ui.painter().line_segment([pos2(hr.left(), hr.bottom()), pos2(hr.right(), hr.bottom())], Stroke::new(1.0_f32, kit::BORDER));
                egui::ScrollArea::vertical().id_salt("plgrid").max_height(h - 26.0).auto_shrink([false, false]).show(ui, |ui| {
                    for p in &pls {
                        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click());
                        let on = p.id == new_sel;
                        if on {
                            ui.painter().rect_filled(r, 2.0, purple);
                        } else if resp.hovered() {
                            ui.painter().rect_filled(r, 2.0, kit::CONTROL_HOVER);
                        }
                        let fg = if on { Color32::WHITE } else { kit::TEXT };
                        let pt = |x: f32, txt: String, bold: bool| {
                            ui.painter().text(pos2(r.left() + x + 8.0, r.center().y), Align2::LEFT_CENTER, txt, FontId::proportional(if bold { 13.5 } else { 12.5 }), fg);
                        };
                        pt(cols[0], p.name.clone(), true);
                        pt(cols[1], hms(dur_of(&self.data, p)), true);
                        pt(cols[2], p.items.len().to_string(), false);
                        pt(cols[3], if p.sched.is_empty() { "-".to_string() } else { p.sched.len().to_string() }, false);
                        if resp.clicked() {
                            new_sel = p.id;
                        }
                        if resp.double_clicked() {
                            edit = Some(p.clone());
                        }
                        resp.context_menu(|ui| {
                            if ui.button(tr("edit")).clicked() {
                                edit = Some(p.clone());
                                ui.close_menu();
                            }
                            if ui.button(tr("delete")).clicked() {
                                del = Some(p.id);
                                ui.close_menu();
                            }
                        });
                    }
                });
            });
            // ---- right panel: the items of the selected playlist and its scheduled times
            ui.allocate_ui_with_layout(vec2(right_w, h), egui::Layout::top_down(egui::Align::LEFT), |ui| {
                egui::Frame::none().fill(kit::INSET).rounding(6.0).inner_margin(6.0).show(ui, |ui| {
                    ui.set_width(right_w - 14.0);
                    match &sel_pl {
                        None => {
                            ui.label(RichText::new(tr("No playlist selected")).color(kit::TEXT_DIM));
                        }
                        Some(p) => {
                            ui.label(RichText::new(format!("{} ({})", p.name, hms(dur_of(&self.data, p)))).color(kit::WARN).italics());
                            egui::ScrollArea::vertical().id_salt("plitems_r").max_height((h - 150.0).max(120.0)).auto_shrink([false, false]).show(ui, |ui| {
                                for (i, it) in p.items.iter().enumerate() {
                                    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::hover());
                                    let t = self.data.track(it.track_id);
                                    let jingle = it.break_ref.is_empty() && t.map(|t| t.kind != "music").unwrap_or(false);
                                    let bg = if !it.break_ref.is_empty() { Color32::from_rgb(0x3a, 0x2f, 0x14) } else if jingle { Color32::from_rgb(0x1c, 0x24, 0x4a) } else { kit::CARD };
                                    ui.painter().rect_filled(r.shrink2(vec2(0.0, 1.0)), 3.0, bg);
                                    let (title, dur, intro) = if !it.break_ref.is_empty() {
                                        (format!("Break {}", it.break_ref), String::new(), 0)
                                    } else if let Some(t) = t {
                                        (t.label(), format!("Dur: {}", fmt_ms(t.eff_ms())), t.intro_ms)
                                    } else {
                                        ("(missing)".to_string(), String::new(), 0)
                                    };
                                    ui.painter().text(pos2(r.left() + 6.0, r.top() + 4.0), Align2::LEFT_TOP, format!("{}", i + 1), FontId::proportional(10.0), kit::TEXT_DIM);
                                    ui.painter().text(pos2(r.left() + 24.0, r.top() + 3.0), Align2::LEFT_TOP, dur, FontId::proportional(10.0), kit::GOOD);
                                    if intro > 0 {
                                        ui.painter().text(pos2(r.left() + 110.0, r.top() + 3.0), Align2::LEFT_TOP, format!("INTRO: {:.1}s", intro as f64 / 1000.0), FontId::proportional(10.0), kit::BAD);
                                    }
                                    let g = ui.painter().layout_no_wrap(title, FontId::proportional(14.0), kit::TEXT);
                                    let clip = ui.painter().with_clip_rect(Rect::from_min_max(pos2(r.left() + 6.0, r.top()), pos2(r.right() - 4.0, r.bottom())));
                                    clip.galley(pos2(r.left() + 24.0, r.top() + 16.0), g, kit::TEXT);
                                }
                            });
                            ui.add_space(6.0);
                            ui.label(RichText::new(tr("Scheduled Times")).small().color(kit::TEXT_DIM));
                            egui::Frame::none().fill(kit::PANEL).rounding(4.0).inner_margin(6.0).show(ui, |ui| {
                                ui.set_min_height(60.0);
                                ui.set_width(ui.available_width());
                                if p.sched.is_empty() {
                                    ui.label(RichText::new("-").color(kit::TEXT_DIM));
                                }
                                for s in &p.sched {
                                    let when = if s.date.is_empty() { (0..7).filter(|d| (s.weekdays >> d) & 1 == 1).map(|d| tr(DAYS[d])).collect::<Vec<_>>().join(",") } else { s.date.clone() };
                                    ui.label(format!("{when}  {}{}", s.time, if s.immediate { "  (immediately)" } else { "" }));
                                }
                            });
                        }
                    }
                });
            });
        });
        self.cp.pl_sel = new_sel;
        if edit.is_some() {
            self.cp.pl = edit;
        }
        if let Some(id) = del {
            self.del_doc(id);
        }
    }

    fn recent_naive(&self) -> Vec<(u64, NaiveDateTime)> {
        self.db
            .recent_plays(48)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(id, s)| NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S").ok().map(|d| (id, d)))
            .collect()
    }

    fn import_playlist_file(&mut self, f: &Path) {
        let Ok(text) = std::fs::read_to_string(f) else { return };
        let by_path: HashMap<String, u64> = self.data.tracks.iter().map(|t| (t.path.to_lowercase(), t.id)).collect();
        let by_name: HashMap<String, u64> = self
            .data
            .tracks
            .iter()
            .filter_map(|t| Path::new(&t.path).file_name().map(|n| (n.to_string_lossy().to_lowercase(), t.id)))
            .collect();
        let mut p = Playlist { name: f.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(), ..Default::default() };
        let (mut ok, mut miss) = (0, 0);
        for line in text.lines().map(|l| l.trim()).filter(|l| !l.is_empty() && !l.starts_with('#')) {
            let name = Path::new(line).file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
            if let Some(id) = by_path.get(&line.to_lowercase()).or_else(|| by_name.get(&name)) {
                p.items.push(PItem { track_id: *id, ..Default::default() });
                ok += 1;
            } else {
                miss += 1;
            }
        }
        self.save_doc("playlist", &mut p);
        self.cp.msg = format!("Imported {ok} files, {miss} not found in the songs library");
    }

    // ---------------------------------------------------------------- force clocks
    fn page_force(&mut self, ui: &mut egui::Ui) {
        ui.heading(tr("Force clocks schedule"));
        ui.label(tr("Plays a clock once at the given days and times (also used to schedule one specific song)."));
        if ui.button(tr("Add new")).clicked() {
            self.cp.fc = Some(ForceClock { title: "New force clock".into(), start_date: today(), end_date: today(), ..Default::default() });
        }
        let mut edit = None;
        let mut del = None;
        for f in &self.data.force {
            ui.horizontal(|ui| {
                let cn = self.data.clocks.iter().find(|c| c.id == f.clock_id).map(|c| c.name.clone()).unwrap_or_default();
                ui.label(format!("{}: clock '{}' {} -> {} at {}", f.title, cn, f.start_date, f.end_date, f.times.join(", ")));
                if ui.small_button(tr("edit")).clicked() {
                    edit = Some(f.clone());
                }
                if ui.small_button(tr("delete")).clicked() {
                    del = Some(f.id);
                }
            });
        }
        if edit.is_some() {
            self.cp.fc = edit;
        }
        if let Some(id) = del {
            self.del_doc(id);
        }
    }

    // ---------------------------------------------------------------- final log
    fn page_final(&mut self, ui: &mut egui::Ui) {
        ui.heading(tr("Final log creator"));
        ui.label(tr("Schedules a whole day. Breaks are stored as references, so the spots can still change until broadcast."));
        ui.horizontal_wrapped(|ui| {
            ui.label(tr("Date:"));
            ui.add(egui::TextEdit::singleline(&mut self.cp.fl_date).desired_width(90.0));
            ui.checkbox(&mut self.cp.fl_flags[0], tr("playlists"));
            ui.checkbox(&mut self.cp.fl_flags[1], tr("force clocks"));
            ui.checkbox(&mut self.cp.fl_flags[2], tr("spot breaks"));
        });
        let date = engine::parse_date(&self.cp.fl_date);
        ui.horizontal_wrapped(|ui| {
            if ui.button(tr("Create log for this day")).clicked() {
                if let Some(d) = date {
                    let flags = Flags { playlists: self.cp.fl_flags[0], force: self.cp.fl_flags[1], breaks: self.cp.fl_flags[2] };
                    let recent = self.recent_naive();
                    let f = engine::create_final(&self.data, d, &flags, &recent);
                    self.cp.fl_items = engine::from_final(&self.data, &f);
                    self.cp.fl_msg = format!("Log created: {} items (not saved yet)", self.cp.fl_items.len());
                }
            }
            if ui.button(tr("Edit saved log")).clicked() {
                if let Some(f) = self.data.finals.iter().find(|f| f.date == self.cp.fl_date) {
                    self.cp.fl_items = engine::from_final(&self.data, f);
                    self.cp.fl_msg = tr("Saved log loaded");
                }
            }
            if ui.button(tr("Save & schedule")).clicked() && !self.cp.fl_items.is_empty() {
                let mut f = engine::to_final(&self.cp.fl_items, &self.cp.fl_date);
                if let Some(old) = self.data.finals.iter().find(|x| x.date == self.cp.fl_date) {
                    f.id = old.id;
                }
                self.save_doc("finallog", &mut f);
                self.final_loaded.clear();
                self.cp.fl_msg = format!("Final log of {} saved", self.cp.fl_date);
            }
            if ui.button(tr("Unschedule day")).clicked() {
                if let Some(id) = self.data.finals.iter().find(|f| f.date == self.cp.fl_date).map(|f| f.id) {
                    self.del_doc(id);
                    self.cp.fl_msg = tr("Day unscheduled");
                }
            }
            if ui.button(tr("Print report (CSV)")).clicked() {
                let mut lines = vec!["No;Estimated time;Type;Artist;Title;Duration".to_string()];
                let mut t = date.map(|d| d.and_hms_opt(0, 0, 0).unwrap()).unwrap_or_else(|| Local::now().naive_local());
                for (i, x) in self.cp.fl_items.iter().enumerate() {
                    lines.push(format!("{};{};{};{};{};{}", i + 1, t.format("%H:%M:%S"), x.kind, export::csv_esc(&x.artist), export::csv_esc(&x.title), fmt_ms(x.duration_ms)));
                    if !x.is_overlay() {
                        t = t + Duration::milliseconds(x.eff_ms() as i64);
                    }
                }
                match export::write_lines(&self.base, &format!("finallog_{}.csv", self.cp.fl_date), &lines) {
                    Ok(p) => self.cp.fl_msg = format!("Saved {}", p.display()),
                    Err(e) => self.cp.fl_msg = e,
                }
            }
        });
        ui.colored_label(kit::GOOD, &self.cp.fl_msg.clone());
        ui.label(tr("Saved logs:"));
        ui.label(self.data.finals.iter().map(|f| f.date.clone()).collect::<Vec<_>>().join(", "));
        ui.separator();
        let mut remove = None;
        let mut swap: Option<(usize, usize)> = None;
        egui::ScrollArea::vertical().id_salt("flitems").max_height(420.0).show_rows(ui, 22.0, self.cp.fl_items.len(), |ui, range| {
            for i in range {
                let t = &self.cp.fl_items[i];
                ui.horizontal(|ui| {
                    ui.label(format!("{:>3} {:<10} {}{}", i + 1, t.kind, if t.is_overlay() { "~ " } else { "" }, t.label()));
                    if ui.small_button(tr("up")).clicked() && i > 0 {
                        swap = Some((i, i - 1));
                    }
                    if ui.small_button(tr("down")).clicked() && i + 1 < self.cp.fl_items.len() {
                        swap = Some((i, i + 1));
                    }
                    if ui.small_button(tr("remove")).clicked() {
                        remove = Some(i);
                    }
                });
            }
        });
        if let Some((a, b)) = swap {
            self.cp.fl_items.swap(a, b);
        }
        if let Some(i) = remove {
            self.cp.fl_items.remove(i);
        }
        ui.horizontal(|ui| {
            if ui.button(tr("Insert selected library item at the end")).clicked() {
                if let Some(t) = self.cp.lib_sel.first().and_then(|id| self.data.track(*id)).cloned() {
                    self.cp.fl_items.push(t);
                }
            }
        });
    }

    // ---------------------------------------------------------------- log viewer
    fn page_logviewer(&mut self, ui: &mut egui::Ui) {
        ui.heading(tr("Log viewer"));
        ui.horizontal_wrapped(|ui| {
            ui.label(tr("From"));
            ui.add(egui::TextEdit::singleline(&mut self.cp.lv_from).desired_width(90.0));
            ui.label(tr("to"));
            ui.add(egui::TextEdit::singleline(&mut self.cp.lv_to).desired_width(90.0));
            if ui.button(tr("Show")).clicked() {
                self.cp.lv_rows = self.db.history_range(&self.cp.lv_from, &self.cp.lv_to).unwrap_or_default();
                self.cp.lv_sel = None;
            }
        });
        egui::ScrollArea::vertical().id_salt("lvrows").max_height(320.0).show_rows(ui, 20.0, self.cp.lv_rows.len(), |ui, range| {
            for i in range {
                let r = &self.cp.lv_rows[i];
                let label = format!("{}  [{}]  {}{}", r.at, r.kind, if r.artist.is_empty() { String::new() } else { format!("{} - ", r.artist) }, r.title);
                if ui.selectable_label(self.cp.lv_sel == Some(i), label).clicked() {
                    self.cp.lv_sel = Some(i);
                }
            }
        });
        ui.separator();
        if let Some(r) = self.cp.lv_sel.and_then(|i| self.cp.lv_rows.get(i)) {
            ui.label(RichText::new(tr("Why was this track broadcast?")).strong());
            ui.label(format!("Source: {}", if r.src.is_empty() { "(not recorded)" } else { &r.src }));
            ui.label(format!("Reason: {}", if r.reason.is_empty() { "(not recorded)" } else { &r.reason }));
            if let Some(t) = self.data.track(r.track_id) {
                ui.label(format!("Categories: {} / {} / {}   Separation: {} min   Enabled: {}", t.cat1, t.cat2, t.cat3, t.sep_min, t.enabled));
            }
        }
    }

    // ---------------------------------------------------------------- settings
    fn page_general(&mut self, ui: &mut egui::Ui) {
        kit::page_title(ui, "General settings", "Station, language and defaults");
        let langs = crate::i18n::available(&self.lang_dirs());
        let mut new_lang: Option<String> = None;
        kit::card(ui, "Language", |ui| {
            ui.horizontal(|ui| {
                ui.label(tr("Interface language"));
                let cur = langs.iter().find(|l| l.0 == self.data.s.language).map(|l| l.1.clone()).unwrap_or_else(|| "English".into());
                egui::ComboBox::from_id_salt("lang").selected_text(cur).show_ui(ui, |ui| {
                    for (code, name) in &langs {
                        if ui.selectable_label(self.data.s.language == *code, name).clicked() {
                            new_lang = Some(code.clone());
                        }
                    }
                });
            });
            ui.label(RichText::new(tr("To add a language, put a file such as de.json in the 'lang' folder next to the program (see lang/template.json).")).color(kit::GRAY).small());
        });
        if let Some(c) = new_lang {
            self.data.s.language = c.clone();
            crate::i18n::set_language(&c, &self.lang_dirs());
            self.save_settings();
        }
        let prev = self.db.get_setting("previous_app_version").ok().flatten().unwrap_or_default();
        kit::card(ui, "About", |ui| {
            ui.label(format!("RadioMoon {VERSION}  (build {BUILD})"));
            ui.label(format!("{} {}", tr("Database layout version:"), crate::db::SCHEMA_VERSION));
            if !prev.is_empty() {
                ui.label(format!("{} {prev}", tr("Upgraded from version:")));
            }
            ui.label(format!("{} {}", tr("Data folder:"), self.base.display()));
        });
        ui.add_space(6.0);
        let mut ch = false;
        let s = &mut self.data.s;
        kit::card(ui, "Station", |ui| {
            ui.horizontal(|ui| {
                ui.label(tr("Radio station name"));
                ch |= ui.add(egui::TextEdit::singleline(&mut s.station).desired_width((ui.available_width() - 8.0).max(60.0))).lost_focus();
            });
            ui.horizontal(|ui| {
                ui.label(tr("Start mode"));
                ch |= ui.radio_value(&mut s.start_mode, "manual".to_string(), tr("Manual")).changed();
                ch |= ui.radio_value(&mut s.start_mode, "auto".to_string(), tr("Auto")).changed();
            });
            ui.horizontal(|ui| {
                ch |= ui.add(egui::DragValue::new(&mut s.time_offset_s).range(-3600..=3600).prefix(tr("Time offset (seconds ahead of the clock): "))).changed();
            });
            ui.horizontal_wrapped(|ui| {
                ch |= ui.add(egui::DragValue::new(&mut s.default_gain_db).range(-24.0..=24.0).prefix(tr("Default volume for new files (dB): "))).changed();
                ch |= ui.add(egui::DragValue::new(&mut s.autocue_db).range(-90.0..=-10.0).prefix(tr("Autocue sensitivity songs (dB): "))).changed();
                ch |= ui.add(egui::DragValue::new(&mut s.spot_autocue_db).range(-90.0..=-10.0).prefix(tr("spots (dB, -90 = play whole file): "))).changed();
            });
            ui.label(tr("Station details for reports:"));
            ch |= ui.add(egui::TextEdit::multiline(&mut s.station_details).desired_width(f32::INFINITY).desired_rows(3)).lost_focus();
            ui.horizontal(|ui| {
                ui.label(tr("Alerts e-mail addresses (stored only)"));
                ch |= ui.add(egui::TextEdit::singleline(&mut s.alert_emails).desired_width((ui.available_width() - 8.0).max(60.0))).lost_focus();
            });
        });
        kit::card(ui, "Custom logs (text file per day)", |ui| {
            ch |= ui.checkbox(&mut s.custom_log, tr("Enable custom logs")).changed();
            path_row(ui, "Folder", &mut s.custom_log_path, true);
            text_row(ui, "Line format ({date} {time} {kind} {artist} {title} {album} {code} {duration})", &mut s.custom_log_format);
        });
        if ch || ui.button(tr("Save")).clicked() {
            self.save_settings();
        }
    }

    fn page_live(&mut self, ui: &mut egui::Ui) {
        kit::page_title(ui, "Live rebroadcast", "Put a sound card input or an internet radio stream on the air, by hand (LIVE button) or on a schedule");
        let ins = player::list_inputs();
        let mut ch = false;
        let mut del_src: Option<usize> = None;
        let mut go: Option<usize> = None;
        let mut rename: Option<(String, String)> = None;
        let live_now = self.player.as_ref().map(|p| p.live_active()).unwrap_or(false);
        let sel = self.live_sel;
        kit::card(ui, "Sources", |ui| {
            let list = &mut self.data.s.live_sources;
            for (i, src) in list.iter_mut().enumerate() {
                ui.push_id(i, |ui| {
                    ui.horizontal(|ui| {
                        let old = src.name.clone();
                        ui.label(tr("Name"));
                        if ui.add(egui::TextEdit::singleline(&mut src.name).desired_width(140.0)).changed() {
                            ch = true;
                            rename = Some((old, src.name.clone()));
                        }
                        ui.label(tr("Level (dB)"));
                        ch |= ui.add(egui::Slider::new(&mut src.gain_db, -24.0..=12.0)).changed();
                    });
                    ui.horizontal(|ui| {
                        ui.label(tr("Sound input"));
                        egui::ComboBox::from_id_salt("lsd").width(260.0).selected_text(if src.device.is_empty() { tr("(default)").to_string() } else { src.device.clone() }).show_ui(ui, |ui| {
                            ch |= ui.selectable_value(&mut src.device, String::new(), tr("(default)")).changed();
                            for o in &ins {
                                ch |= ui.selectable_value(&mut src.device, o.clone(), o).changed();
                            }
                        });
                    });
                    ui.horizontal(|ui| {
                        ui.label(tr("Stream URL (leave empty to use the sound input)"));
                        ch |= ui.add(egui::TextEdit::singleline(&mut src.url).desired_width(ui.available_width().max(120.0) - 8.0)).changed();
                    });
                    ui.horizontal(|ui| {
                        let on = live_now && sel == i;
                        if on {
                            if kit::danger_button(ui, "Stop").clicked() {
                                go = Some(usize::MAX);
                            }
                        } else if kit::primary_button(ui, "Go live").clicked() {
                            go = Some(i);
                        }
                        if ui.button(tr("Delete")).clicked() {
                            del_src = Some(i);
                        }
                    });
                    ui.separator();
                });
            }
            if ui.button(tr("Add source")).clicked() {
                let n = list.len() + 1;
                list.push(LiveSource { name: format!("Live {n}"), ..Default::default() });
                ch = true;
            }
        });
        if let Some((o, n)) = rename {
            for sl in self.data.s.live_slots.iter_mut() {
                if sl.source == o {
                    sl.source = n.clone();
                }
            }
        }
        if let Some(i) = del_src {
            let name = self.data.s.live_sources.remove(i).name;
            self.data.s.live_slots.retain(|x| x.source != name);
            ch = true;
        }
        match go {
            Some(usize::MAX) => self.stop_live_source(),
            Some(i) => {
                let m = self.data.s.live_mute_automation;
                self.start_live_source(i, m, None);
            }
            None => {}
        }
        let names: Vec<String> = self.data.s.live_sources.iter().map(|x| x.name.clone()).collect();
        let mut del_slot: Option<usize> = None;
        kit::card(ui, "Schedule", |ui| {
            let days = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
            for (i, sl) in self.data.s.live_slots.iter_mut().enumerate() {
                ui.push_id(("slot", i), |ui| {
                    ui.horizontal(|ui| {
                        ch |= ui.checkbox(&mut sl.enabled, "").changed();
                        egui::ComboBox::from_id_salt("lss").width(130.0).selected_text(sl.source.clone()).show_ui(ui, |ui| {
                            for n in &names {
                                ch |= ui.selectable_value(&mut sl.source, n.clone(), n).changed();
                            }
                        });
                        ui.label(tr("from"));
                        ch |= ui.add(egui::TextEdit::singleline(&mut sl.start).desired_width(48.0)).changed();
                        ui.label(tr("to"));
                        ch |= ui.add(egui::TextEdit::singleline(&mut sl.end).desired_width(48.0)).changed();
                        ch |= ui.checkbox(&mut sl.mute, tr("Stop automation")).changed();
                        if ui.button(tr("Delete")).clicked() {
                            del_slot = Some(i);
                        }
                    });
                    ui.horizontal(|ui| {
                        for (d, n) in days.iter().enumerate() {
                            ch |= ui.selectable_label(sl.days[d], tr(n)).clicked() && {
                                sl.days[d] = !sl.days[d];
                                true
                            };
                        }
                    });
                    ui.separator();
                });
            }
            if ui.add_enabled(!names.is_empty(), egui::Button::new(tr("Add schedule"))).clicked() {
                self.data.s.live_slots.push(LiveSlot { source: names.first().cloned().unwrap_or_default(), ..Default::default() });
                ch = true;
            }
            ui.label(RichText::new(tr("Times are HH:MM. If the end is earlier than the start the slot runs over midnight (the days are those of the start). The source goes on the air at the start and off at the end; the LIVE button in the studio can end it earlier.")).color(kit::GRAY).small());
        });
        if let Some(i) = del_slot {
            self.data.s.live_slots.remove(i);
            ch = true;
        }
        kit::card(ui, "Options", |ui| {
            ch |= ui.checkbox(&mut self.data.s.live_mute_automation, tr("Stop the automation while live (it continues afterwards)")).changed();
            ui.horizontal(|ui| {
                ui.label(tr("Cross-fade at the start and end of the live source (seconds)"));
                ch |= ui.add(egui::Slider::new(&mut self.data.s.live_fade_s, 0.0..=15.0).fixed_decimals(1)).changed();
            });
            ui.label(RichText::new(tr("At the start the automation fades out while the live source fades in, and the other way round at the end, so both are heard together during the transition. With the automation not stopped, only the live source fades in and out.")).color(kit::GRAY).small());
            ui.label(RichText::new(tr("While a source is live the studio shows LIVE in the player. With the automation stopped, the playlist is stopped and the mode switches to MANUAL; when the source ends the previous mode returns and AUTO plays the next item. This applies to sources started by hand; each schedule entry has its own setting. Right-click the LIVE button in the studio to choose a source.")).color(kit::GRAY).small());
        });
        if let Some(p) = self.player.as_ref() {
            if p.live_active() {
                let (pk, st) = p.live_info();
                ui.add(egui::ProgressBar::new(pk.min(1.0)).desired_width(260.0));
                ui.label(st);
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
            }
        }
        if ch {
            self.save_settings();
        }
    }

    fn page_outputs(&mut self, ui: &mut egui::Ui) {
        ui.heading(tr("Sound outputs"));
        let outs = player::list_outputs();
        let mut ch = false;
        let s = &mut self.data.s;
        for (label, id, v) in [("Main output", "om", &mut s.out_main), ("Monitor / headphones (preview)", "omo", &mut s.out_monitor), ("Instant jingles output", "oi", &mut s.out_instants)] {
            ui.horizontal(|ui| {
                ui.label(label);
                egui::ComboBox::from_id_salt(id).selected_text(if v.is_empty() { "(default)".to_string() } else { v.clone() }).show_ui(ui, |ui| {
                    if ui.selectable_value(v, String::new(), tr("(default)")).changed() {
                        ch = true;
                    }
                    for o in &outs {
                        if ui.selectable_value(v, o.clone(), o).changed() {
                            ch = true;
                        }
                    }
                });
            });
        }
        if ch {
            self.save_settings();
            self.apply_outputs();
            self.cp.msg = tr("Outputs applied");
        }
        ui.add_space(6.0);
        let mut av = false;
        kit::card(ui, "Auto volume", |ui| {
            let s = &mut self.data.s;
            av |= ui.checkbox(&mut s.auto_volume, tr("Auto volume: keep all the songs at the same loudness")).changed();
            ui.add_enabled_ui(s.auto_volume, |ui| {
                av |= ui.add(egui::Slider::new(&mut s.auto_volume_target_db, -30.0..=-8.0).suffix(" dB").text(tr("Target level"))).drag_stopped();
                av |= ui.add(egui::Slider::new(&mut s.auto_volume_boost_db, 0.0..=18.0).suffix(" dB").text(tr("Maximum boost (quiet songs)"))).drag_stopped();
                av |= ui.add(egui::Slider::new(&mut s.auto_volume_cut_db, 0.0..=24.0).suffix(" dB").text(tr("Maximum reduction (loud songs)"))).drag_stopped();
            });
            ui.label(RichText::new(tr("The level of each song is followed while it plays and corrected slowly, so quiet songs are raised and loud songs are lowered. It applies to the next song that starts.")).small().color(kit::TEXT_DIM));
        });
        if av {
            self.save_settings();
        }
        ui.label(tr("The live rebroadcast sources and their schedule are in Scheduling > Live rebroadcast."));
        ui.label(tr("Mixing between songs uses each song's mix-in / mix-out points. VST/DSP plugins and A-B interchange output are not supported."));
    }

    fn page_shortcuts(&mut self, ui: &mut egui::Ui) {
        ui.heading(tr("Shortcut keys"));
        ui.label(tr("Key names such as F1..F12, A..Z, Space, Num1, Enter. Active while the studio has the focus."));
        let mut ch = false;
        for (act, key) in self.data.s.shortcuts.iter_mut() {
            ui.horizontal(|ui| {
                ui.label(format!("{act:<12}"));
                ch |= ui.add(egui::TextEdit::singleline(key).desired_width(80.0)).lost_focus();
                if egui::Key::from_name(key).is_none() {
                    ui.colored_label(Color32::from_rgb(255, 120, 90), tr("unknown key"));
                }
            });
        }
        if ch {
            self.save_settings();
        }
    }

    fn page_users(&mut self, ui: &mut egui::Ui) {
        ui.heading(tr("Users & security"));
        let mut en = self.data.s.users_enabled;
        if ui.checkbox(&mut en, tr("Enable user permissions (login at startup)")).changed() {
            if en && self.data.users.is_empty() {
                self.cp.msg = tr("Add at least one administrator first");
            } else {
                self.data.s.users_enabled = en;
                self.save_settings();
            }
        }
        if ui.button(tr("Add new user")).clicked() {
            self.cp.user = Some(User { name: "user".into(), ..Default::default() });
            self.cp.user_pw.clear();
        }
        let mut edit = None;
        let mut del = None;
        for u in &self.data.users {
            ui.horizontal(|ui| {
                ui.label(format!("{}{}  (auto logout {} min)", u.name, if u.admin { " [administrator]" } else { "" }, u.timeout_min));
                if ui.small_button(tr("edit")).clicked() {
                    edit = Some(u.clone());
                }
                if ui.small_button(tr("delete")).clicked() {
                    del = Some(u.id);
                }
            });
        }
        if edit.is_some() {
            self.cp.user = edit;
            self.cp.user_pw.clear();
        }
        if let Some(id) = del {
            if self.data.users.iter().filter(|u| u.admin).count() > 1 || !self.data.users.iter().any(|u| u.id == id && u.admin) {
                self.del_doc(id);
            } else {
                self.cp.msg = tr("You can not delete the last administrator");
            }
        }
    }

    fn page_folders(&mut self, ui: &mut egui::Ui) {
        ui.heading(tr("Audio folder mappings"));
        ui.label(tr("Replaces the start of a stored audio path, e.g. C:\\AUDIO\\  ->  \\\\studio_pc\\AUDIO\\ (also for a workstation)."));
        let mut rem = None;
        for (i, (a, b)) in self.data.s.folder_maps.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("{a}  ->  {b}"));
                if ui.small_button(tr("remove")).clicked() {
                    rem = Some(i);
                }
            });
        }
        if let Some(i) = rem {
            self.data.s.folder_maps.remove(i);
            self.save_settings();
        }
        path_row(ui, "Original path", &mut self.cp.map_from, true);
        path_row(ui, "Replacement path", &mut self.cp.map_to, true);
        if ui.button(tr("Add")).clicked() && !self.cp.map_from.is_empty() {
            self.data.s.folder_maps.push((self.cp.map_from.clone(), self.cp.map_to.clone()));
            self.cp.map_from.clear();
            self.cp.map_to.clear();
            self.save_settings();
        }
    }

    fn page_server(&mut self, ui: &mut egui::Ui) {
        ui.heading(tr("Database server"));
        ui.label(tr("Stored encrypted in config.json in the data folder (changes need a restart). A different database name = a second radio station."));
        let mut c = self.cfg.clone();
        text_row(ui, "Host (127.0.0.1 = bundled server)", &mut c.host);
        ui.horizontal(|ui| {
            ui.label(tr("Port"));
            ui.add(egui::DragValue::new(&mut c.port).range(1..=65535));
        });
        text_row(ui, "User", &mut c.user);
        text_row(ui, "Password", &mut c.password);
        text_row(ui, "Database name", &mut c.database);
        if ui.button(tr("Save config.json")).clicked() {
            crate::config::save(&self.base, &c);
            self.cfg = c;
            self.cp.msg = tr("Saved. Restart RadioMoon to connect with the new settings.");
        }
        ui.separator();
        ui.label(RichText::new(tr("Clean up and shrink database")).strong());
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut self.cp.cleanup_days).range(7..=3650).prefix(tr("Delete broadcast history older than (days): ")));
            if ui.button(tr("Clean up")).clicked() {
                let _ = self.db.cleanup_history(self.cp.cleanup_days);
                self.cp.msg = tr("History cleaned up");
            }
        });
    }

    // ---------------------------------------------------------------- utilities
    fn page_updater(&mut self, ui: &mut egui::Ui) {
        ui.heading(tr("Internet updater"));
        ui.label(tr("Each time a track starts, the files below are written (NowOnAir.txt/.xml, AirPlayNext.xml, AirPlayHistory.xml, CurrentAlbumImage.jpg)."));
        let mut ch = false;
        let s = &mut self.data.s;
        path_row(ui, "Export folder", &mut s.export_dir, true);
        ch |= ui.checkbox(&mut s.export_next, tr("AirPlayNext.xml")).changed();
        ch |= ui.checkbox(&mut s.export_history, tr("AirPlayHistory.xml")).changed();
        ch |= ui.checkbox(&mut s.export_art, tr("CurrentAlbumImage.jpg (cover art)")).changed();
        ui.label(tr("What to announce for each library type (empty = normal metadata, NO-UPDATE = skip, or a custom message):"));
        for (k, name) in KINDS.iter().filter(|k| k.0 != "memo") {
            let pos = s.msg_kinds.iter().position(|m| m.0 == *k);
            let mut v = pos.map(|p| s.msg_kinds[p].1.clone()).unwrap_or_default();
            ui.horizontal(|ui| {
                ui.label(format!("{name:<14}"));
                if ui.text_edit_singleline(&mut v).changed() {
                    match pos {
                        Some(p) => s.msg_kinds[p].1 = v.clone(),
                        None => s.msg_kinds.push((k.to_string(), v.clone())),
                    }
                    ch = true;
                }
            });
        }
        if ch {
            self.save_settings();
        }
        ui.label(tr("Uploading by HTTP/TCP/TuneIn and stream encoders are not part of this version: point your web tools at the export folder."));
    }

    fn page_jazler(&mut self, ui: &mut egui::Ui) {
        use std::sync::atomic::Ordering;
        ui.heading(tr("Import from Jazler / MegaMix"));
        ui.label(tr("Imports songs, spots, jingles, categories, playlists, campaigns, jingle pallets and play history from a Jazler RadioStar 2 or a MegaMix database (.mdb, Access 2000-2003); the type is detected automatically. Existing data is kept; nothing is overwritten or deleted. User passwords are not imported."));
        ui.add_space(6.0);
        let busy = self.cp.jz_busy.load(Ordering::SeqCst);
        if self.cp.jz_done.swap(false, Ordering::SeqCst) {
            self.reload_tracks();
            self.reload_docs();
            if self.cp.jz_upload_pending {
                self.cp.jz_upload_pending = false;
                if self.remote.is_some() {
                    self.remote_upload_local();
                    self.cp.msg = tr("Upload to the server started in the background");
                }
            }
        }
        let mut o = self.cp.jz_opts.clone().unwrap_or_default();
        ui.add_enabled_ui(!busy, |ui| {
            ui.horizontal(|ui| {
                ui.label(tr("Database file (.mdb)"));
                let w = (ui.available_width() - 110.0).max(80.0);
                ui.add(egui::TextEdit::singleline(&mut self.cp.jz_path).desired_width(w));
                if ui.button(tr("Browse...")).clicked() {
                    if let Some(p) = rfd::FileDialog::new().add_filter("Access database", &["mdb", "MDB"]).pick_file() {
                        self.cp.jz_path = p.to_string_lossy().to_string();
                        self.cp.jz_preview.clear();
                    }
                }
            });
            if ui.button(tr("Scan file")).clicked() {
                match crate::jzimport::preview(std::path::Path::new(&self.cp.jz_path)) {
                    Ok(v) => {
                        self.cp.jz_preview = v;
                        self.cp.msg.clear();
                    }
                    Err(e) => {
                        self.cp.jz_preview.clear();
                        self.cp.msg = e;
                    }
                }
            }
            for (k, n) in &self.cp.jz_preview {
                ui.label(format!("{}: {}", tr(k), n));
            }
            ui.separator();
            ui.checkbox(&mut o.songs, tr("Songs and artists"));
            ui.checkbox(&mut o.spots, tr("Spots and campaigns"));
            ui.checkbox(&mut o.playlists, tr("Playlists"));
            ui.checkbox(&mut o.pallets, tr("Jingle pallets"));
            ui.horizontal(|ui| {
                ui.checkbox(&mut o.history, tr("Play history, last days:"));
                ui.add(egui::DragValue::new(&mut o.history_days).range(1..=3650));
            });
            ui.separator();
            ui.label(tr("If the audio files are on a different drive or folder now, replace the start of the old path:"));
            ui.horizontal(|ui| {
                ui.label(tr("Old path start"));
                ui.add(egui::TextEdit::singleline(&mut o.replace_from).desired_width(160.0));
                ui.label(tr("New path start"));
                ui.add(egui::TextEdit::singleline(&mut o.replace_to).desired_width(160.0));
            });
            if self.remote.is_some() {
                ui.checkbox(&mut self.cp.jz_upload, tr("Upload the audio files to the server at the end of the import and change the paths in the database"));
            }
            if ui.button(tr("Start import")).clicked() && !self.cp.jz_path.trim().is_empty() {
                self.cp.jz_upload_pending = self.cp.jz_upload && self.remote.is_some();
                self.cp.jz_busy.store(true, Ordering::SeqCst);
                self.cp.jz_report.lock().unwrap().clear();
                *self.cp.jz_status.lock().unwrap() = "Starting ...".into();
                let (db, path, op) = (self.db.clone(), self.cp.jz_path.clone(), o.clone());
                let (st, rp, bs, dn) = (self.cp.jz_status.clone(), self.cp.jz_report.clone(), self.cp.jz_busy.clone(), self.cp.jz_done.clone());
                std::thread::spawn(move || {
                    let st2 = st.clone();
                    let res = crate::jzimport::apply(&db, std::path::Path::new(&path), &op, &move |m| {
                        *st2.lock().unwrap() = m.to_string();
                    });
                    match res {
                        Ok(r) => {
                            *rp.lock().unwrap() = r;
                            *st.lock().unwrap() = "Import finished".into();
                        }
                        Err(e) => *st.lock().unwrap() = format!("Import failed: {e}"),
                    }
                    dn.store(true, Ordering::SeqCst);
                    bs.store(false, Ordering::SeqCst);
                });
            }
        });
        self.cp.jz_opts = Some(o);
        if busy {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(self.cp.jz_status.lock().unwrap().clone());
            });
        } else {
            let st = self.cp.jz_status.lock().unwrap().clone();
            if !st.is_empty() {
                ui.label(st);
            }
        }
        for l in self.cp.jz_report.lock().unwrap().iter() {
            ui.label(l);
        }
    }

    fn page_integrity(&mut self, ui: &mut egui::Ui) {
        ui.heading(tr("Integrity check"));
        ui.horizontal(|ui| {
            ui.label(tr("Library:"));
            egui::ComboBox::from_id_salt("ik").selected_text(if self.cp.integ_kind.is_empty() { "all".to_string() } else { self.cp.integ_kind.clone() }).show_ui(ui, |ui| {
                ui.selectable_value(&mut self.cp.integ_kind, String::new(), tr("all"));
                for (k, _) in KINDS.iter().filter(|k| k.0 != "memo") {
                    ui.selectable_value(&mut self.cp.integ_kind, k.to_string(), *k);
                }
            });
            if ui.button(tr("Start check")).clicked() {
                let kind = self.cp.integ_kind.clone();
                let missing: Vec<Track> = self
                    .data
                    .tracks
                    .iter()
                    .filter(|t| kind.is_empty() || t.kind == kind)
                    .filter(|t| !self.file_ok(&t.path))
                    .cloned()
                    .collect();
                self.cp.msg = format!("{} inaccessible files found", missing.len());
                self.cp.integ_res = missing;
            }
        });
        if !self.cp.integ_res.is_empty() {
            ui.horizontal(|ui| {
                if ui.button(tr("Save results in text file")).clicked() {
                    let lines: Vec<String> = self.cp.integ_res.iter().map(|t| t.path.clone()).collect();
                    match export::write_lines(&self.base, "inaccessible_files.txt", &lines) {
                        Ok(p) => self.cp.msg = format!("Saved {}", p.display()),
                        Err(e) => self.cp.msg = e,
                    }
                }
                if ui.button(tr("Disable these files")).clicked() {
                    for mut t in self.cp.integ_res.clone() {
                        t.enabled = false;
                        let _ = self.db.update_track(&t);
                    }
                    self.reload_tracks();
                }
                if ui.button(tr("Delete these files from the library")).clicked() {
                    let ids: Vec<u64> = self.cp.integ_res.iter().map(|t| t.id).collect();
                    let _ = self.db.delete_tracks(&ids);
                    self.cp.integ_res.clear();
                    self.reload_tracks();
                }
            });
            egui::ScrollArea::vertical().id_salt("integ").max_height(300.0).show(ui, |ui| {
                for t in &self.cp.integ_res {
                    ui.label(format!("[{}] {}", t.kind, t.path));
                }
            });
        }
    }

    fn page_reports(&mut self, ui: &mut egui::Ui) {
        ui.heading(tr("Broadcasting reports"));
        ui.horizontal_wrapped(|ui| {
            ui.label(tr("From"));
            ui.add(egui::TextEdit::singleline(&mut self.cp.rep_from).desired_width(90.0));
            ui.label(tr("to"));
            ui.add(egui::TextEdit::singleline(&mut self.cp.rep_to).desired_width(90.0));
            ui.radio_value(&mut self.data.s.report_format, "csv".to_string(), tr(".csv"));
            ui.radio_value(&mut self.data.s.report_format, "txt".to_string(), tr(".txt"));
        });
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.data.s.report_style, "default".to_string(), tr("Standard"));
            ui.radio_value(&mut self.data.s.report_style, "custom".to_string(), tr("Custom template"));
        });
        if self.data.s.report_style == "custom" {
            text_row(ui, "Template ({date} {time} {artist} {title} {album} {code} {duration})", &mut self.data.s.report_template);
        }
        if ui.button(tr("Export")).clicked() {
            self.save_settings();
            let rows = self.db.history_range(&self.cp.rep_from, &self.cp.rep_to).unwrap_or_default();
            let csv = self.data.s.report_format == "csv";
            let lines = export::broadcast_report(&rows, &self.data.tracks, &self.data.s, csv);
            let name = format!("broadcast_{}_{}.{}", self.cp.rep_from, self.cp.rep_to, if csv { "csv" } else { "txt" });
            match export::write_lines(&self.base, &name, &lines) {
                Ok(p) => self.cp.msg = format!("{} lines saved to {}", lines.len(), p.display()),
                Err(e) => self.cp.msg = e,
            }
        }
    }

    fn page_reset(&mut self, ui: &mut egui::Ui) {
        ui.heading(tr("Reset database"));
        ui.label(tr("Deletes the selected libraries (the audio files themselves are never touched)."));
        for (k, n) in KINDS.iter().filter(|k| k.0 != "memo") {
            let mut on = self.cp.reset_kinds.iter().any(|x| x == k);
            if ui.checkbox(&mut on, *n).changed() {
                if on {
                    self.cp.reset_kinds.push(k.to_string());
                } else {
                    self.cp.reset_kinds.retain(|x| x != k);
                }
            }
        }
        if ui.button(tr("Reset selected libraries")).clicked() && !self.cp.reset_kinds.is_empty() {
            self.stop();
            let _ = self.db.reset_kinds(&self.cp.reset_kinds);
            self.reload_tracks();
            self.log = self.db.load_log().unwrap_or_default();
            self.cp.msg = tr("Libraries reset");
            self.cp.reset_kinds.clear();
        }
        ui.add_space(14.0);
        ui.separator();
        ui.label(RichText::new(tr("Delete everything except the settings")).strong());
        ui.label(tr("Deletes all libraries, clocks, playlists, campaigns, categories, breaks, the playlist and the play history. Only the settings and the user accounts stay. The audio files are never touched."));
        ui.checkbox(&mut self.cp.reset_all_ok, tr("I understand that this cannot be undone"));
        if ui.add_enabled(self.cp.reset_all_ok, egui::Button::new(RichText::new(tr("Delete everything except the settings")).color(Color32::WHITE)).fill(Color32::from_rgb(190, 50, 50))).clicked() {
            self.cp.reset_all_ok = false;
            self.stop();
            match self.db.reset_all_but_settings() {
                Ok(()) => {
                    if let Some(r) = &self.remote {
                        let _ = r.link.tx.send(crate::remote::Cmd::Act(serde_json::json!({ "do": "clear" })));
                        let _ = r.link.tx.send(crate::remote::Cmd::Act(serde_json::json!({ "do": "reload" })));
                    }
                    self.log.clear();
                    self.reload_docs();
                    self.reload_tracks();
                    self.cp.msg = tr("Everything except the settings was deleted");
                }
                Err(e) => self.cp.msg = format!("{e}"),
            }
        }
    }

    // ---------------------------------------------------------------- edit windows
    fn edit_windows(&mut self, ctx: &egui::Context) {
        self.win_import(ctx);
        self.win_restore(ctx);
        self.win_track(ctx);
        self.win_mass(ctx);
        self.win_campaign(ctx);
        self.win_clock(ctx);
        self.win_playlist(ctx);
        self.win_force(ctx);
        self.win_memo(ctx);
        self.win_pallet(ctx);
        self.win_user(ctx);
    }

    fn win_track(&mut self, ctx: &egui::Context) {
        let Some(mut t) = self.cp.edit.clone() else { return };
        let path = self.local_audio(&t.path);
        if self.cp.edit_wave_for != t.id {
            self.cp.edit_wave_for = t.id;
            crate::waveform::spawn(path.clone(), t.id, self.cp.edit_wave.clone());
            self.cp.edit_cursor = t.mix_in_ms;
            if let Some(p) = self.player.as_mut() {
                p.stop_preview();
            }
        }
        let peaks: std::sync::Arc<Vec<f32>> = match self.cp.edit_wave.lock() {
            Ok(g) if g.0 == t.id => g.1.clone(),
            _ => std::sync::Arc::new(Vec::new()),
        };
        let (pv_active, pv_pos) = self.player.as_ref().map(|p| (p.preview_active(), p.preview_pos_ms())).unwrap_or((false, 0));
        let mut open = true;
        let (mut save, mut cancel) = (false, false);
        let song = t.kind == "music";
        let c1 = self.data.cat_names(Self::cat_lib_of(&t.kind));
        let c2 = self.data.cat_names("song2");
        let c3 = self.data.cat_names("song3");
        let cv = self.data.cat_names("vocal");
        let cp = self.data.cat_names("props");
        let names = self.data.s.cat_names.clone();
        let autocue_db = if t.kind == "spot" { self.data.s.spot_autocue_db } else { self.data.s.autocue_db };
        let mut play: Option<(u64, u64)> = None;
        let mut stop = false;
        let mut marker = self.cp.edit_marker;
        let mut cursor = self.cp.edit_cursor;
        let mut zoom = self.cp.wave_zoom.max(1.0);
        let mut off = self.cp.wave_off;
        dialog(ctx, format!("{}: {}", tr("Edit"), t.title)).open(&mut open).show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            // ---- 1. data ----
            kit::card(ui, "Data", |ui| {
                ui.columns(2, |cols| {
                    text_row(&mut cols[0], "Title", &mut t.title);
                    text_row(&mut cols[0], "Artist", &mut t.artist);
                    text_row(&mut cols[0], "Album", &mut t.album);
                    combo(&mut cols[0], "e1", if song { &names[0] } else { "Category" }, &mut t.cat1, &c1);
                    if song {
                        combo(&mut cols[0], "e2", &names[1], &mut t.cat2, &c2);
                        combo(&mut cols[0], "e3", &names[2], &mut t.cat3, &c3);
                    }
                    cols[1].horizontal(|ui| {
                        ui.label(tr("Genre"));
                        ui.add(egui::TextEdit::singleline(&mut t.genre).desired_width(120.0));
                        ui.label(tr("Year"));
                        ui.add(egui::TextEdit::singleline(&mut t.year).desired_width(50.0));
                    });
                    path_row(&mut cols[1], "File", &mut t.path, false);
                    text_row(&mut cols[1], "Playlister code (empty = automatic code)", &mut t.plcode);
                    if song {
                        combo(&mut cols[1], "ev", "Vocal", &mut t.vocal, &cv);
                        combo(&mut cols[1], "ep", "Properties", &mut t.props, &cp);
                    }
                    cols[1].horizontal(|ui| {
                        ui.label(tr("Notes:"));
                        ui.text_edit_singleline(&mut t.notes);
                    });
                });
            });
            // ---- 2. playback ----
            kit::card(ui, "Playback", |ui| {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut t.enabled, tr("enabled"));
                    ui.checkbox(&mut t.bookmarked, tr("bookmarked"));
                    ui.checkbox(&mut t.variable_len, tr("variable length (file replaced later)"));
                    ui.add(egui::DragValue::new(&mut t.gain_db).range(-30.0..=30.0).prefix(tr("Volume (dB): ")));
                    ui.add(egui::DragValue::new(&mut t.sep_min).range(0..=10080).prefix(tr("Song separation (min): ")));
                });
                ui.columns(2, |cols| {
                    ms_edit(&mut cols[0], "Duration", &mut t.duration_ms);
                    ms_edit(&mut cols[0], "Mix start (skip beginning)", &mut t.mix_in_ms);
                    ms_edit(&mut cols[0], "Mix end (next song starts here)", &mut t.mix_out_ms);
                    if song || t.kind == "sweeper" || t.kind == "voice" {
                        ms_edit(&mut cols[1], "Intro ends at", &mut t.intro_ms);
                    }
                    if song {
                        ms_edit(&mut cols[1], "Hook in", &mut t.hook_in_ms);
                        ms_edit(&mut cols[1], "Hook out", &mut t.hook_out_ms);
                    }
                });
            });
            // ---- 3. waveform ----
            kit::card(ui, "Waveform and mix points", |ui| {
                let dur = t.duration_ms.max(1);
                let view_len = ((dur as f32 / zoom) as u64).max(1);
                let view_start = ((dur.saturating_sub(view_len)) as f32 * off.clamp(0.0, 1.0)) as u64;
                let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width().max(520.0), 100.0), Sense::click_and_drag());
                let p = ui.painter_at(rect);
                p.rect_filled(rect, 6.0, Color32::from_rgb(10, 11, 14));
                let to_x = |ms: u64| rect.left() + ((ms as f32 - view_start as f32) / view_len as f32) * rect.width();
                let mid = rect.center().y;
                if !peaks.is_empty() {
                    let cols = rect.width() as usize;
                    for c in 0..cols {
                        let ms = view_start as f32 + c as f32 / cols as f32 * view_len as f32;
                        let idx = ((ms / dur as f32) * peaks.len() as f32) as usize;
                        if idx >= peaks.len() {
                            break;
                        }
                        let h = peaks[idx].powf(0.7) * rect.height() * 0.46;
                        let x = rect.left() + c as f32 + 0.5;
                        let inside = ms >= t.mix_in_ms as f32 && (t.mix_out_ms == 0 || ms <= t.mix_out_ms as f32);
                        let col = if inside { kit::ACCENT } else { Color32::from_rgb(88, 94, 108) };
                        p.line_segment([pos2(x, mid - h), pos2(x, mid + h)], Stroke::new(1.0_f32, col));
                    }
                } else {
                    p.text(rect.center(), Align2::CENTER_CENTER, tr("analysing waveform..."), FontId::proportional(13.0), kit::GRAY);
                }
                let marks = [
                    (t.mix_in_ms, "Mix start", kit::GOOD),
                    (t.mix_out_ms, "Mix end", kit::WARN),
                    (t.intro_ms, "Intro end", kit::ACCENT),
                    (t.hook_in_ms, "Hook in", Color32::from_rgb(190, 120, 255)),
                    (t.hook_out_ms, "Hook out", Color32::from_rgb(255, 110, 190)),
                ];
                for (i, (ms, name, col)) in marks.iter().enumerate() {
                    if *ms > 0 && *ms >= view_start && *ms <= view_start + view_len {
                        let x = to_x(*ms);
                        let w = if i as u8 == marker { 2.5_f32 } else { 1.5_f32 };
                        p.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], Stroke::new(w, *col));
                        p.text(pos2(x + 4.0, rect.top() + 11.0 + i as f32 * 12.0), Align2::LEFT_CENTER, tr(name), FontId::proportional(11.0), *col);
                    }
                }
                let cur = if pv_active { pv_pos } else { cursor };
                if cur >= view_start && cur <= view_start + view_len {
                    let x = to_x(cur);
                    p.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], Stroke::new(1.5_f32, Color32::WHITE));
                }
                p.text(pos2(rect.left() + 6.0, rect.bottom() - 9.0), Align2::LEFT_CENTER, fmt_ms(view_start), FontId::proportional(11.0), kit::GRAY);
                p.text(pos2(rect.right() - 6.0, rect.bottom() - 9.0), Align2::RIGHT_CENTER, fmt_ms(view_start + view_len), FontId::proportional(11.0), kit::GRAY);
                if let Some(pos) = resp.interact_pointer_pos() {
                    let ms = (view_start as f32 + ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0) * view_len as f32) as u64;
                    if resp.dragged_by(egui::PointerButton::Primary) || resp.clicked_by(egui::PointerButton::Primary) {
                        match marker {
                            0 => t.mix_in_ms = ms,
                            1 => t.mix_out_ms = ms,
                            2 => t.intro_ms = ms,
                            3 => t.hook_in_ms = ms,
                            _ => t.hook_out_ms = ms,
                        }
                    }
                    if resp.clicked_by(egui::PointerButton::Secondary) {
                        cursor = ms;
                    }
                }
                ui.label(RichText::new(tr("Click or drag on the waveform to set the selected mark. Right-click sets the listening position.")).color(kit::GRAY).small());
                ui.horizontal_wrapped(|ui| {
                    ui.label(tr("Set mark:"));
                    ui.radio_value(&mut marker, 0u8, tr("Mix start"));
                    ui.radio_value(&mut marker, 1u8, tr("Mix end"));
                    if song {
                        ui.radio_value(&mut marker, 2u8, tr("Intro end"));
                        ui.radio_value(&mut marker, 3u8, tr("Hook in"));
                        ui.radio_value(&mut marker, 4u8, tr("Hook out"));
                    }
                });
                ui.horizontal(|ui| {
                    ui.add(egui::Slider::new(&mut zoom, 1.0..=32.0).logarithmic(true).text(tr("Zoom")));
                    if zoom > 1.0 {
                        ui.add(egui::Slider::new(&mut off, 0.0..=1.0).show_value(false).text(tr("Position")));
                    }
                });
            });
            // ---- 4. play buttons and actions ----
            ui.horizontal_wrapped(|ui| {
                if kit::primary_button(ui, "Play from start").clicked() {
                    play = Some((0, 0));
                }
                if ui.button(tr("Play from listening position")).clicked() {
                    play = Some((cursor, 0));
                }
                if ui.button(tr("Play from mix start")).clicked() {
                    play = Some((t.mix_in_ms, 0));
                }
                if ui.button(tr("Listen to the mix end")).clicked() {
                    let end = if t.mix_out_ms > 0 { t.mix_out_ms } else { t.duration_ms };
                    play = Some((end.saturating_sub(6000), 8000));
                }
                if ui.button(tr("Stop")).clicked() {
                    stop = true;
                }
                ui.label(format!("{} {}", tr("Position:"), fmt_precise(if pv_active { pv_pos } else { cursor })));
            });
            ui.horizontal(|ui| {
                if ui.button(tr("Auto cue")).clicked() {
                    if let Some((a, b, d)) = crate::waveform::autocue(&path, autocue_db) {
                        t.mix_in_ms = a;
                        t.mix_out_ms = b;
                        t.duration_ms = d;
                    }
                }
                if ui.button(tr("Read tags")).clicked() {
                    let m = library::read_meta(Path::new(&path), true);
                    t.title = m.title;
                    t.artist = m.artist;
                    t.album = m.album;
                    t.genre = m.genre;
                    t.year = m.year;
                }
                if kit::primary_button(ui, "OK").clicked() {
                    save = true;
                }
                if ui.button(tr("Cancel")).clicked() {
                    cancel = true;
                }
            });
        });
        if let Some(pl) = self.player.as_mut() {
            if let Some((from, len)) = play {
                let _ = pl.play_preview(&path, from, len);
            }
            if stop {
                pl.stop_preview();
            }
        }
        self.cp.edit_marker = marker;
        self.cp.edit_cursor = cursor;
        self.cp.wave_zoom = zoom;
        self.cp.wave_off = off;
        if save {
            let _ = self.db.update_track(&t);
            self.reload_tracks();
        }
        if save || cancel || !open {
            self.cp.edit = None;
            self.cp.edit_wave_for = 0;
            if let Some(pl) = self.player.as_mut() {
                pl.stop_preview();
            }
        } else {
            self.cp.edit = Some(t);
        }
    }

    fn win_import(&mut self, ctx: &egui::Context) {
        let Some((path, mut o)) = self.cp.import_pending.clone() else { return };
        let (mut go, mut cancel) = (false, false);
        let cats = self.data.cat_names(Self::cat_lib_of(&o.kind));
        dialog(ctx, tr("Import options")).show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
            ui.label(RichText::new(path.display().to_string()).color(kit::GRAY));
            kit::card(ui, "Audio data", |ui| {
                ui.radio_value(&mut o.read_tags, true, tr("Read title, artist, album, genre and year from the ID3 / file tags"));
                ui.radio_value(&mut o.read_tags, false, tr("Do not read the tags: use the file name as the title"));
                if !o.read_tags {
                    ui.checkbox(&mut o.parse_filename, tr("The file names look like 'Artist - Title'"));
                }
            });
            kit::card(ui, "Options", |ui| {
                combo(ui, "impcat", "Put the new items in category", &mut o.cat1, &cats);
                ui.checkbox(&mut o.autocue, tr("Find the mix start and end automatically (auto cue, slower)"));
                ui.checkbox(&mut o.bookmark, tr("Bookmark the new items"));
            });
            ui.horizontal(|ui| {
                if kit::primary_button(ui, "Import").clicked() {
                    go = true;
                }
                if ui.button(tr("Cancel")).clicked() {
                    cancel = true;
                }
            });
                    });
        });
        if go && self.remote.is_some() {
            self.remote_import(path, o);
            self.cp.msg = tr("Upload to the server started in the background");
            self.cp.import_pending = None;
        } else if go {
            if path.is_dir() {
                let (db, st, done, oo, dir) = (self.db.clone(), self.scan_status.clone(), self.scan_done.clone(), o.clone(), path.clone());
                std::thread::spawn(move || {
                    let n = library::scan(&db, &dir, &oo, &st);
                    if let Ok(mut s) = st.lock() {
                        *s = format!("Import finished: {n} files");
                    }
                    done.store(true, Ordering::SeqCst);
                });
                self.cp.msg = tr("Import started in the background");
            } else {
                library::import_one(&self.db, &path, &o);
                self.reload_tracks();
            }
            self.cp.import_pending = None;
        } else if cancel {
            self.cp.import_pending = None;
        } else {
            self.cp.import_pending = Some((path, o));
        }
    }

    fn win_restore(&mut self, ctx: &egui::Context) {
        let Some(file) = self.cp.restore_target.clone() else { return };
        let (mut go, mut cancel) = (false, false);
        dialog(ctx, tr("Restore database")).show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
            ui.label(file.display().to_string());
            ui.add_space(4.0);
            ui.label(tr("All libraries, schedules and settings are replaced by the content of this backup."));
            ui.label(tr("A safety copy of the current database is saved first (before-restore.rmbak)."));
            ui.horizontal(|ui| {
                if kit::danger_button(ui, "Restore").clicked() {
                    go = true;
                }
                if ui.button(tr("Cancel")).clicked() {
                    cancel = true;
                }
            });
                    });
        });
        if go {
            self.stop();
            let dir = self.backup_dir();
            match backup::import(&self.db, &file, VERSION, &dir) {
                Ok(m) => {
                    self.reload_all();
                    self.log = self.db.load_log().unwrap_or_default();
                    self.gen = None;
                    crate::i18n::set_language(&self.data.s.language.clone(), &self.lang_dirs());
                    self.cp.msg = m;
                }
                Err(e) => self.cp.msg = format!("{e:#}"),
            }
            self.cp.restore_target = None;
        } else if cancel {
            self.cp.restore_target = None;
        }
    }

    // ---------------------------------------------------------------- internet broadcast
    // ---------------------------------------------------------------- CLAP effects
    fn page_clap(&mut self, ui: &mut egui::Ui) {
        use crate::clap_host::ClapOp;
        kit::page_title(ui, "Effects (CLAP)", "CLAP plugins that process the on-air mix before the sound card and the internet broadcast");
        let remote = self.remote.is_some();
        if self.cp.clap_view.is_none() {
            self.cp.clap_view = Some(self.clap_view());
        }
        let mut view = self.cp.clap_view.clone().unwrap_or_default();
        // while the plugin folders are scanned the list is read again every second
        if view.scanning {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
            if self.cp.clap_poll.map(|t| t.elapsed() > std::time::Duration::from_secs(1)).unwrap_or(true) {
                self.cp.clap_poll = Some(std::time::Instant::now());
                view = self.clap_view();
                self.cp.clap_view = Some(view.clone());
            }
        }
        let mut op: Option<ClapOp> = None;
        let mut upload: Option<Vec<std::path::PathBuf>> = None;
        let mut sent_now = false;
        let sent = self.cp.clap_sent;
        if !view.error.is_empty() {
            ui.colored_label(egui::Color32::from_rgb(229, 83, 75), view.error.clone());
        }
        if !self.cp.clap_msg.is_empty() {
            ui.colored_label(egui::Color32::from_rgb(229, 83, 75), self.cp.clap_msg.clone());
        }
        kit::card(ui, "Effect chain (in this order)", |ui| {
            if view.slots.is_empty() {
                ui.label(RichText::new(tr("No effect: the sound goes out unchanged.")).color(kit::GRAY));
            }
            let n = view.slots.len();
            for (i, s) in view.slots.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    let mut on = s.enabled;
                    if ui.checkbox(&mut on, "").on_hover_text(tr("On / bypass")).changed() {
                        op = Some(ClapOp::Enable(i, on));
                    }
                    ui.label(RichText::new(s.name.clone()).strong());
                    ui.label(RichText::new(s.vendor.clone()).color(kit::GRAY));
                    if ui.add_enabled(i > 0, egui::Button::new("\u{25B2}").small()).clicked() {
                        op = Some(ClapOp::Move(i, i - 1));
                    }
                    if ui.add_enabled(i + 1 < n, egui::Button::new("\u{25BC}").small()).clicked() {
                        op = Some(ClapOp::Move(i, i + 1));
                    }
                    if ui.add(egui::Button::new("\u{2715}").small()).on_hover_text(tr("Remove")).clicked() {
                        op = Some(ClapOp::Remove(i));
                    }
                });
                egui::CollapsingHeader::new(format!("{} ({})", tr("Parameters"), s.params.len())).id_salt(("clap_params", i)).show(ui, |ui| {
                    if s.params.is_empty() {
                        ui.label(RichText::new(tr("This plugin has no parameters that can be set here.")).color(kit::GRAY));
                    }
                    for p in s.params.iter_mut() {
                        ui.horizontal(|ui| {
                            ui.add_sized([200.0, 18.0], egui::Label::new(if p.module.is_empty() { p.name.clone() } else { format!("{} / {}", p.module, p.name) }).truncate());
                            let mut sl = egui::Slider::new(&mut p.value, p.min..=p.max.max(p.min + 1e-9));
                            if p.stepped {
                                sl = sl.step_by(1.0);
                            }
                            let r = ui.add(sl);
                            if r.changed() {
                                // through the network a few changes per second are enough; the last value is sent when the mouse is released
                                let due = !remote || !r.dragged() || sent.map(|t| t.elapsed() > std::time::Duration::from_millis(150)).unwrap_or(true);
                                if due {
                                    op = Some(ClapOp::Param(i, p.id, p.value, !r.dragged()));
                                    sent_now = true;
                                }
                            } else if r.drag_stopped() {
                                op = Some(ClapOp::Param(i, p.id, p.value, true));
                            }
                            if ui.small_button("\u{21BA}").on_hover_text(tr("Default value")).clicked() {
                                p.value = p.default;
                                op = Some(ClapOp::Param(i, p.id, p.default, true));
                            }
                        });
                    }
                });
                ui.separator();
            }
        });
        kit::card(ui, "Available plugins", |ui| {
            ui.horizontal(|ui| {
                if ui.add_enabled(!view.scanning, egui::Button::new(tr("Scan for plugins"))).clicked() {
                    op = Some(ClapOp::Scan);
                }
                if ui.button(tr("Upload plugin files...")).on_hover_text(tr("Add .clap files (to the server, when you are connected to one)")).clicked() {
                    if let Some(f) = rfd::FileDialog::new().add_filter("CLAP", &["clap"]).pick_files() {
                        upload = Some(f);
                    }
                }
                if view.scanning {
                    ui.spinner();
                    ui.label(tr("Scanning..."));
                } else if view.scanned {
                    ui.label(format!("{} {}", view.plugins.len(), tr("plugins found")));
                }
                ui.label(tr("Filter:"));
                ui.add(egui::TextEdit::singleline(&mut self.cp.clap_filter).desired_width(160.0));
            });
            let f = self.cp.clap_filter.to_lowercase();
            egui::ScrollArea::vertical().id_salt("clap_list").max_height(260.0).show(ui, |ui| {
                for p in view.plugins.iter().filter(|p| f.is_empty() || p.name.to_lowercase().contains(&f) || p.vendor.to_lowercase().contains(&f)) {
                    ui.horizontal(|ui| {
                        if ui.small_button(tr("Add")).clicked() {
                            op = Some(ClapOp::Add(p.path.clone(), p.id.clone()));
                        }
                        ui.label(RichText::new(p.name.clone()).strong());
                        ui.label(RichText::new(p.vendor.clone()).color(kit::GRAY));
                    });
                }
            });
            ui.label(RichText::new(tr("Plugins are searched in the CLAP folders of the system (~/.clap, /usr/lib/clap, Common Files\\CLAP, ...) and in the folders of the environment variable CLAP_PATH. With a server the plugins of the server are used.")).color(kit::GRAY).small());
        });
        if let Some(f) = upload {
            self.clap_install(f);
        }
        if sent_now {
            self.cp.clap_sent = Some(std::time::Instant::now());
        }
        self.cp.clap_view = Some(view);
        if let Some(o) = op {
            let refresh = !matches!(o, ClapOp::Param(..));
            match self.clap_do(o) {
                Ok(()) => self.cp.clap_msg.clear(),
                Err(e) => self.cp.clap_msg = e,
            }
            if refresh {
                self.cp.clap_view = Some(self.clap_view());
            }
        }
    }

    fn page_stream(&mut self, ui: &mut egui::Ui) {
        kit::page_title(ui, "Internet broadcast", "Send the on-air mix (MP3) to one or more Icecast or Shoutcast (v1 / v2) servers at the same time");
        let mut ch = false;
        let mut remove: Option<usize> = None;
        let running: Vec<(usize, String)> = self.px_streams();
        let n = self.data.s.streams.len();
        for i in 0..n {
            let title = {
                let c = &self.data.s.streams[i];
                let host = if c.host.is_empty() { "-".to_string() } else { format!("{}:{}", c.host, c.port) };
                format!("{} {} - {} ({})", tr("Server"), i + 1, if c.name.is_empty() { host.clone() } else { c.name.clone() }, host)
            };
            let status = running.iter().find(|r| r.0 == i).map(|r| r.1.clone());
            egui::CollapsingHeader::new(RichText::new(title).strong().size(14.0)).id_salt(("srv", i)).default_open(i == 0).show(ui, |ui| {
                kit::card(ui, "", |ui| {
                    let c = &mut self.data.s.streams[i];
                    ch |= ui.checkbox(&mut c.enabled, tr("Use this server")).changed();
                    ui.horizontal(|ui| {
                        ui.label(tr("Server type"));
                        ch |= ui.radio_value(&mut c.kind, "icecast".to_string(), tr("Icecast 2")).changed();
                        ch |= ui.radio_value(&mut c.kind, "shoutcast".to_string(), tr("Shoutcast v1")).changed();
                        ch |= ui.radio_value(&mut c.kind, "shoutcast2".to_string(), tr("Shoutcast v2")).changed();
                    });
                    ch |= text_row(ui, "Host", &mut c.host);
                    ui.horizontal(|ui| {
                        ui.label(tr("Port"));
                        ch |= ui.add(egui::DragValue::new(&mut c.port).range(1..=65534)).changed();
                        if c.kind != "icecast" {
                            ui.label(RichText::new(tr("(the source connects to the next port)")).color(kit::GRAY).small());
                        }
                        if c.kind == "shoutcast2" {
                            ui.label(tr("Stream ID"));
                            ch |= ui.add(egui::DragValue::new(&mut c.sid).range(1..=2147483647)).changed();
                        }
                    });
                    if c.kind == "icecast" {
                        ch |= text_row(ui, "Mount point", &mut c.mount);
                        ch |= text_row(ui, "User", &mut c.user);
                    }
                    ui.horizontal(|ui| {
                        ui.label(tr("Password"));
                        let sid = egui::Id::new(("show_pw", i));
                        let mut show = ui.memory(|m| m.data.get_temp::<bool>(sid).unwrap_or(false));
                        ch |= ui.add(egui::TextEdit::singleline(&mut c.password).password(!show)).changed();
                        if ui.checkbox(&mut show, tr("Show password")).changed() {
                            ui.memory_mut(|m| m.data.insert_temp(sid, show));
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label(tr("Bitrate (kbps)"));
                        egui::ComboBox::from_id_salt(("sbr", i)).selected_text(c.bitrate.to_string()).show_ui(ui, |ui| {
                            for b in [48u32, 64, 96, 128, 160, 192, 256, 320] {
                                ch |= ui.selectable_value(&mut c.bitrate, b, b.to_string()).changed();
                            }
                        });
                    });
                    ch |= text_row(ui, "Station name", &mut c.name);
                    ch |= text_row(ui, "Genre", &mut c.genre);
                    ch |= text_row(ui, "Description", &mut c.description);
                    ch |= text_row(ui, "Web site", &mut c.url);
                    ch |= ui.checkbox(&mut c.public, tr("List the station in the public directory")).changed();
                    ch |= ui.checkbox(&mut c.autostart, tr("Start broadcasting when RadioMoon starts")).changed();
                    match &status {
                        Some(s) => ui.colored_label(Color32::from_rgb(120, 230, 150), format!("{} {s}", tr("Status:"))),
                        None => ui.label(RichText::new(tr("Not broadcasting")).color(kit::GRAY)),
                    };
                    if n > 1 && kit::danger_button(ui, "Remove this server").clicked() {
                        remove = Some(i);
                    }
                });
            });
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if kit::primary_button(ui, "Add server").clicked() {
                let mut c = StreamCfg::default();
                c.enabled = true;
                c.host = String::new();
                self.data.s.streams.push(c);
                ch = true;
            }
        });
        if let Some(i) = remove {
            self.data.s.streams.remove(i);
            ch = true;
            if !self.px_streams().is_empty() {
                self.save_settings();
                self.start_stream();
            }
        }
        if ch {
            self.save_settings();
        }
        let live = !self.px_streams().is_empty();
        kit::card(ui, "Broadcast", |ui| {
            ui.horizontal(|ui| {
                if !live {
                    if kit::primary_button(ui, "Start broadcast").clicked() {
                        self.start_stream();
                    }
                } else {
                    if kit::danger_button(ui, "Stop broadcast").clicked() {
                        self.stop_stream();
                    }
                    if ui.button(tr("Restart broadcast")).clicked() {
                        self.start_stream();
                    }
                }
            });
            ui.label(RichText::new(tr("All servers that are marked \"Use this server\" are started together. The title of the playing song is sent to every server. Jingles, spots and sweepers that are played through the main output are part of the stream.")).color(kit::GRAY).small());
        });
    }

    // ---------------------------------------------------------------- backup & restore
    fn page_backup(&mut self, ui: &mut egui::Ui) {
        kit::page_title(ui, "Backup & restore", "Automatic copies of the whole database");
        let mut ch = false;
        kit::card(ui, "Automatic backup", |ui| {
            let b = &mut self.data.s.backup;
            ch |= ui.checkbox(&mut b.enabled, tr("Make a backup every day")).changed();
            ui.horizontal(|ui| {
                ui.label(tr("Time (HH:MM)"));
                ch |= ui.add(egui::TextEdit::singleline(&mut b.time).desired_width(60.0)).changed();
                ui.add(egui::DragValue::new(&mut b.keep).range(1..=365).prefix(tr("Keep the last ")).suffix(tr(" backups")));
            });
            ui.horizontal(|ui| {
                ui.label(tr("Folder"));
                ch |= ui.add(egui::TextEdit::singleline(&mut b.dir).desired_width(380.0)).changed();
                if ui.button(tr("...")).clicked() {
                    if let Some(d) = rfd::FileDialog::new().pick_folder() {
                        b.dir = d.to_string_lossy().to_string();
                        ch = true;
                    }
                }
            });
            ui.label(RichText::new(tr("Also a backup is made before the database is upgraded to a new program version.")).color(kit::GRAY).small());
        });
        if ch {
            self.save_settings();
        }
        let dir = self.backup_dir();
        kit::card(ui, "Manual backup and restore", |ui| {
            ui.horizontal(|ui| {
                if kit::primary_button(ui, "Back up now").clicked() {
                    let _ = std::fs::create_dir_all(&dir);
                    let name = format!("radiomoon-{}.rmbak", Local::now().format("%Y%m%d-%H%M%S"));
                    match backup::export(&self.db, &dir.join(&name), VERSION) {
                        Ok(()) => self.cp.msg = format!("{} {}", tr("Backup saved:"), dir.join(&name).display()),
                        Err(e) => self.cp.msg = format!("{e:#}"),
                    }
                }
                if ui.button(tr("Restore from file...")).clicked() {
                    if let Some(f) = rfd::FileDialog::new().add_filter("RadioMoon backup", &["rmbak"]).set_directory(&dir).pick_file() {
                        self.cp.restore_target = Some(f);
                    }
                }
            });
        });
        let files = backup::list(&dir);
        kit::card(ui, "Backups in the folder", |ui| {
            ui.label(RichText::new(dir.display().to_string()).color(kit::GRAY).small());
            if files.is_empty() {
                ui.label(tr("No backups yet"));
            }
            egui::ScrollArea::vertical().id_salt("bklist").max_height(260.0).show(ui, |ui| {
                for (name, size, when) in &files {
                    ui.horizontal(|ui| {
                        ui.label(format!("{when}   {name}   ({} KB)", size / 1024));
                        if ui.small_button(tr("Restore")).clicked() {
                            self.cp.restore_target = Some(dir.join(name));
                        }
                        if ui.small_button(tr("Delete")).clicked() {
                            let _ = std::fs::remove_file(dir.join(name));
                        }
                    });
                }
            });
        });
    }

    fn win_mass(&mut self, ctx: &egui::Context) {
        let Some(mut m) = self.cp.mass.clone() else { return };
        let (mut apply, mut cancel) = (false, false);
        let c1 = self.data.cat_names("song1");
        let c2 = self.data.cat_names("song2");
        let c3 = self.data.cat_names("song3");
        let cv = self.data.cat_names("vocal");
        let cp = self.data.cat_names("props");
        let n = self.cp.lib_sel.len();
        dialog(ctx, format!("Mass change ({n} items)")).show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
            let opt = |ui: &mut egui::Ui, id: &str, label: &str, v: &mut Option<String>, opts: &[String]| {
                ui.horizontal(|ui| {
                    let mut on = v.is_some();
                    if ui.checkbox(&mut on, label).changed() {
                        *v = if on { Some(String::new()) } else { None };
                    }
                    if let Some(x) = v {
                        egui::ComboBox::from_id_salt(id).selected_text(if x.is_empty() { "(empty)".to_string() } else { x.clone() }).show_ui(ui, |ui| {
                            ui.selectable_value(x, String::new(), tr("(empty)"));
                            for o in opts {
                                ui.selectable_value(x, o.clone(), o);
                            }
                        });
                    }
                });
            };
            opt(ui, "m1", "Category 1", &mut m.cat1, &c1);
            opt(ui, "m2", "Category 2", &mut m.cat2, &c2);
            opt(ui, "m3", "Category 3", &mut m.cat3, &c3);
            opt(ui, "mv", "Vocal", &mut m.vocal, &cv);
            opt(ui, "mp", "Properties", &mut m.props, &cp);
            ui.horizontal(|ui| {
                let mut on = m.enabled.is_some();
                if ui.checkbox(&mut on, tr("Enabled / disabled")).changed() {
                    m.enabled = if on { Some(true) } else { None };
                }
                if let Some(e) = &mut m.enabled {
                    ui.checkbox(e, tr("enabled"));
                }
            });
            ui.horizontal(|ui| {
                let mut on = m.bookmark.is_some();
                if ui.checkbox(&mut on, tr("Bookmark")).changed() {
                    m.bookmark = if on { Some(true) } else { None };
                }
                if let Some(e) = &mut m.bookmark {
                    ui.checkbox(e, tr("bookmarked"));
                }
            });
            ui.horizontal(|ui| {
                let mut on = m.sep.is_some();
                if ui.checkbox(&mut on, tr("Song separation (min)")).changed() {
                    m.sep = if on { Some(0) } else { None };
                }
                if let Some(s) = &mut m.sep {
                    ui.add(egui::DragValue::new(s).range(0..=10080));
                }
            });
            ui.checkbox(&mut m.normalize, tr("Normalize volume to the default level (analyses each file)"));
            ui.horizontal(|ui| {
                if ui.button(tr("Apply")).clicked() {
                    apply = true;
                }
                if ui.button(tr("Cancel")).clicked() {
                    cancel = true;
                }
            });
                    });
        });
        if apply {
            let target = self.data.s.default_gain_db;
            for id in self.cp.lib_sel.clone() {
                if let Some(mut t) = self.data.track(id).cloned() {
                    if let Some(v) = &m.cat1 {
                        t.cat1 = v.clone();
                    }
                    if let Some(v) = &m.cat2 {
                        t.cat2 = v.clone();
                    }
                    if let Some(v) = &m.cat3 {
                        t.cat3 = v.clone();
                    }
                    if let Some(v) = &m.vocal {
                        t.vocal = v.clone();
                    }
                    if let Some(v) = &m.props {
                        t.props = v.clone();
                    }
                    if let Some(v) = m.enabled {
                        t.enabled = v;
                    }
                    if let Some(v) = m.bookmark {
                        t.bookmarked = v;
                    }
                    if let Some(v) = m.sep {
                        t.sep_min = v;
                    }
                    if m.normalize {
                        if let Some(peak) = crate::waveform::peak_db(&self.local_audio(&t.path)) {
                            t.gain_db = target - peak;
                        }
                    }
                    let _ = self.db.update_track(&t);
                }
            }
            self.reload_tracks();
            self.cp.mass = None;
            self.cp.msg = tr("Mass change applied");
        } else if cancel {
            self.cp.mass = None;
        } else {
            self.cp.mass = Some(m);
        }
    }

    fn win_campaign(&mut self, ctx: &egui::Context) {
        let Some(mut c) = self.cp.camp.clone() else { return };
        let (mut save, mut cancel) = (false, false);
        let cats = self.data.cat_names("spot");
        let prios: Vec<(i32, String)> = self.data.s.prios.iter().map(|p| (p.num, p.name.clone())).collect();
        let spots: Vec<(u64, String)> = self.data.tracks.iter().filter(|t| t.kind == "spot").map(|t| (t.id, t.label())).collect();
        let times: Vec<String> = {
            let mut v: Vec<String> = self.data.breaks.iter().map(|b| b.time.clone()).collect();
            v.sort();
            v
        };
        dialog(ctx, format!("Campaign: {}", c.name)).show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
            text_row(ui, "Name", &mut c.name);
            combo(ui, "ccat", "Category", &mut c.category, &cats);
            ui.horizontal(|ui| {
                ui.label(tr("Starts"));
                ui.add(egui::TextEdit::singleline(&mut c.start).desired_width(90.0));
                ui.checkbox(&mut c.never, tr("never expires"));
                if !c.never {
                    ui.label(tr("Expires"));
                    ui.add(egui::TextEdit::singleline(&mut c.end).desired_width(90.0));
                }
                ui.checkbox(&mut c.enabled, tr("enabled"));
            });
            ui.horizontal(|ui| {
                ui.label(tr("Programming mode"));
                ui.radio_value(&mut c.mode, "weekly".to_string(), tr("Weekly"));
                ui.radio_value(&mut c.mode, "detailed".to_string(), tr("Detailed (dates)"));
                ui.radio_value(&mut c.mode, "playlister".to_string(), tr("Playlister"));
            });
            ui.horizontal(|ui| {
                ui.label(tr("Programming system"));
                ui.radio_value(&mut c.system, "order".to_string(), tr("In order"));
                ui.radio_value(&mut c.system, "random".to_string(), tr("At random"));
                ui.radio_value(&mut c.system, "specific".to_string(), tr("Specific order"));
            });
            ui.label(RichText::new(tr("Spots of this campaign")).strong());
            let mut rem = None;
            for (i, id) in c.spots.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(spots.iter().find(|s| s.0 == *id).map(|s| s.1.clone()).unwrap_or("(missing)".into()));
                    if ui.small_button(tr("remove")).clicked() {
                        rem = Some(i);
                    }
                });
            }
            if let Some(i) = rem {
                c.spots.remove(i);
            }
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("addspot").selected_text(tr("Add spot...")).show_ui(ui, |ui| {
                    for (id, l) in &spots {
                        if ui.selectable_label(false, l).clicked() && !c.spots.contains(id) {
                            c.spots.push(*id);
                        }
                    }
                });
                if ui.button(tr("Add selected library spot")).clicked() {
                    if let Some(id) = self.cp.lib_sel.first().copied() {
                        if spots.iter().any(|s| s.0 == id) && !c.spots.contains(&id) {
                            c.spots.push(id);
                        }
                    }
                }
            });
            if c.mode != "playlister" {
                ui.separator();
                ui.label(RichText::new(tr("Programming")).strong());
                ui.horizontal(|ui| {
                    ui.label(tr("Priority:"));
                    egui::ComboBox::from_id_salt("cprio").selected_text(prios.iter().find(|p| p.0 == self.cp.camp_prio).map(|p| format!("{} {}", p.0, p.1)).unwrap_or_default()).show_ui(ui, |ui| {
                        for (n, name) in &prios {
                            ui.selectable_value(&mut self.cp.camp_prio, *n, format!("{n} {name}"));
                        }
                    });
                });
                if c.mode == "weekly" {
                    ui.horizontal(|ui| {
                        for d in 0..7 {
                            ui.checkbox(&mut self.cp.camp_days[d], tr(DAYS[d]));
                        }
                    });
                } else {
                    ui.horizontal(|ui| {
                        ui.label(tr("Date:"));
                        ui.add(egui::TextEdit::singleline(&mut self.cp.pl_date).desired_width(90.0));
                    });
                }
                ui.label(tr("Break times (click to mark), then press 'Program selected':"));
                ui.horizontal_wrapped(|ui| {
                    for t in &times {
                        let mut on = self.cp.camp_times.contains(t);
                        if ui.toggle_value(&mut on, t).changed() {
                            if on {
                                self.cp.camp_times.push(t.clone());
                            } else {
                                self.cp.camp_times.retain(|x| x != t);
                            }
                        }
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button(tr("Program selected")).clicked() {
                        for t in self.cp.camp_times.clone() {
                            if c.mode == "weekly" {
                                for d in 0..7 {
                                    if self.cp.camp_days[d] && !c.slots.iter().any(|x| x.weekday == d as i8 && x.time == t && x.priority == self.cp.camp_prio) {
                                        c.slots.push(Slot { weekday: d as i8, time: t.clone(), priority: self.cp.camp_prio, ..Default::default() });
                                    }
                                }
                            } else {
                                c.slots.push(Slot { date: self.cp.pl_date.clone(), time: t.clone(), priority: self.cp.camp_prio, ..Default::default() });
                            }
                        }
                    }
                    if ui.button(tr("Clear programming")).clicked() {
                        c.slots.clear();
                    }
                });
                ui.label(format!("{} programmed slots", c.slots.len()));
            }
            ui.horizontal(|ui| {
                if ui.button(tr("OK")).clicked() {
                    save = true;
                }
                if ui.button(tr("Cancel")).clicked() {
                    cancel = true;
                }
            });
                    });
        });
        if save {
            self.save_doc("campaign", &mut c);
            self.cp.camp = None;
        } else if cancel {
            self.cp.camp = None;
        } else {
            self.cp.camp = Some(c);
        }
    }

    fn win_clock(&mut self, ctx: &egui::Context) {
        let Some(mut c) = self.cp.clock.clone() else { return };
        let (mut save, mut cancel) = (false, false);
        let c1 = self.data.cat_names("song1");
        let c2 = self.data.cat_names("song2");
        let c3 = self.data.cat_names("song3");
        let cv = self.data.cat_names("vocal");
        let cp = self.data.cat_names("props");
        let names = self.data.s.cat_names.clone();
        let tracks: Vec<(u64, String)> = self.data.tracks.iter().map(|t| (t.id, format!("[{}] {}", t.kind, t.label()))).collect();
        let elements = [("song", "Song"), ("jingle", "Jingle"), ("sweeper", "Sweeper (over next song)"), ("voice", "Voice track (over next song)"), ("asset", "Asset"), ("track", "Specific item"), ("stitch_hooks", "Stitcher: next songs hooks"), ("stitch_time", "Stitcher: time announcement"), ("stitch_news", "Stitcher: news")];
        dialog(ctx, format!("Clock: {}", c.name)).show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
            text_row(ui, "Name", &mut c.name);
            text_row(ui, "Comments", &mut c.comment);
            ui.horizontal(|ui| {
                ui.label(tr("Color"));
                ui.color_edit_button_srgb(&mut c.color);
            });
            combo(ui, "bk", "Backup category for compensation tracks", &mut c.backup_cat, &c1);
            ui.separator();
            let mut rem = None;
            let mut swap = None;
            for (i, r) in c.rows.iter().enumerate() {
                ui.horizontal(|ui| {
                    let mut f = format!("{:>2}. {}", i + 1, r.element);
                    for x in [&r.cat1, &r.cat2, &r.cat3, &r.vocal, &r.props] {
                        if !x.is_empty() {
                            f.push_str(&format!(" | {x}"));
                        }
                    }
                    if r.track_id != 0 {
                        f.push_str(&format!(" | {}", tracks.iter().find(|t| t.0 == r.track_id).map(|t| t.1.clone()).unwrap_or_default()));
                    }
                    if !r.overlay_pos.is_empty() {
                        f.push_str(&format!(" @ {}", r.overlay_pos));
                    }
                    ui.label(f);
                    if ui.small_button(tr("up")).clicked() && i > 0 {
                        swap = Some((i, i - 1));
                    }
                    if ui.small_button(tr("down")).clicked() && i + 1 < c.rows.len() {
                        swap = Some((i, i + 1));
                    }
                    if ui.small_button(tr("remove")).clicked() {
                        rem = Some(i);
                    }
                });
            }
            if let Some((a, b)) = swap {
                c.rows.swap(a, b);
            }
            if let Some(i) = rem {
                c.rows.remove(i);
            }
            ui.separator();
            ui.label(RichText::new(tr("Add a row")).strong());
            let nr = &mut self.cp.new_row;
            ui.horizontal(|ui| {
                ui.label(tr("Element"));
                egui::ComboBox::from_id_salt("elem").selected_text(elements.iter().find(|e| e.0 == nr.element).map(|e| e.1).unwrap_or("Song")).show_ui(ui, |ui| {
                    for (k, l) in elements {
                        ui.selectable_value(&mut nr.element, k.to_string(), tr(l));
                    }
                });
            });
            let el = if nr.element.is_empty() { "song".to_string() } else { nr.element.clone() };
            if el == "song" {
                combo(ui, "n1", &names[0], &mut nr.cat1, &c1);
                combo(ui, "n2", &names[1], &mut nr.cat2, &c2);
                combo(ui, "n3", &names[2], &mut nr.cat3, &c3);
                combo(ui, "nv", "Vocal", &mut nr.vocal, &cv);
                combo(ui, "np", "Properties", &mut nr.props, &cp);
            } else if ["jingle", "sweeper", "voice", "asset"].contains(&el.as_str()) {
                let cats = self.data.cat_names(&el);
                combo(ui, "ncat", "Category", &mut nr.cat1, &cats);
            } else if el == "track" {
                egui::ComboBox::from_id_salt("ntrack").selected_text(tracks.iter().find(|t| t.0 == nr.track_id).map(|t| t.1.clone()).unwrap_or("(choose)".into())).show_ui(ui, |ui| {
                    for (id, l) in tracks.iter().take(3000) {
                        ui.selectable_value(&mut nr.track_id, *id, l);
                    }
                });
            }
            if el == "sweeper" || el == "voice" {
                ui.horizontal(|ui| {
                    ui.label(tr("Position"));
                    egui::ComboBox::from_id_salt("npos").selected_text(POSITIONS.iter().find(|p| p.0 == nr.overlay_pos).map(|p| p.1).unwrap_or("Start of song")).show_ui(ui, |ui| {
                        for (k, l) in POSITIONS.iter().filter(|p| p.0 != "custom") {
                            ui.selectable_value(&mut nr.overlay_pos, k.to_string(), tr(l));
                        }
                    });
                });
            }
            if ui.button(tr("Add row")).clicked() {
                let mut row = nr.clone();
                row.element = el;
                c.rows.push(row);
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(tr("OK")).clicked() {
                    save = true;
                }
                if ui.button(tr("Cancel")).clicked() {
                    cancel = true;
                }
            });
                    });
        });
        if save {
            self.save_doc("clock", &mut c);
            self.gen = None;
            self.cp.clock = None;
        } else if cancel {
            self.cp.clock = None;
        } else {
            self.cp.clock = Some(c);
        }
    }

    fn win_playlist(&mut self, ctx: &egui::Context) {
        let Some(mut p) = self.cp.pl.clone() else { return };
        let (mut save, mut cancel) = (false, false);
        let breaks: Vec<String> = {
            let mut v: Vec<String> = self.data.breaks.iter().map(|b| b.time.clone()).collect();
            v.sort();
            v
        };
        dialog(ctx, format!("Playlist: {}", p.name)).show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
            text_row(ui, "Name", &mut p.name);
            ui.horizontal(|ui| {
                ui.checkbox(&mut p.random, tr("random order when loaded"));
                ui.checkbox(&mut p.override_prev, tr("overrides previous playlist"));
            });
            // ---- pick songs from the database
            ui.label(RichText::new(tr("Songs in the database (double-click or + to add; songs already in the playlist are hidden)")).strong());
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.cp.pl_q).desired_width(220.0).hint_text(tr("search artist, title, album...")));
                if self.cp.pl_kind.is_empty() {
                    self.cp.pl_kind = "music".into();
                }
                egui::ComboBox::from_id_salt("plkind").selected_text(self.cp.pl_kind.clone()).show_ui(ui, |ui| {
                    for k in ["music", "jingle", "sweeper", "voice", "spot"] {
                        ui.selectable_value(&mut self.cp.pl_kind, k.to_string(), k);
                    }
                });
            });
            let q = self.cp.pl_q.to_lowercase();
            let hits: Vec<(u64, String, u64)> = self
                .data
                .tracks
                .iter()
                .filter(|t| t.enabled && t.kind == self.cp.pl_kind && !p.items.iter().any(|it| it.track_id == t.id) && (q.is_empty() || t.label().to_lowercase().contains(&q) || t.album.to_lowercase().contains(&q)))
                .take(300)
                .map(|t| (t.id, t.label(), t.duration_ms))
                .collect();
            let mut add_ids: Vec<u64> = Vec::new();
            egui::ScrollArea::vertical().id_salt("plpick").max_height(190.0).auto_shrink([false, false]).show(ui, |ui| {
                for (id, label, dur) in &hits {
                    ui.horizontal(|ui| {
                        if ui.small_button("+").clicked() {
                            add_ids.push(*id);
                        }
                        let r = ui.add(egui::Label::new(format!("{label}   {}", fmt_ms(*dur))).sense(Sense::click()));
                        if r.double_clicked() {
                            add_ids.push(*id);
                        }
                    });
                }
                if hits.is_empty() {
                    ui.label(RichText::new(tr("No songs found")).color(kit::TEXT_DIM));
                }
            });
            ui.horizontal(|ui| {
                if ui.button(tr("Add all shown")).clicked() {
                    add_ids.extend(hits.iter().map(|h| h.0));
                }
                ui.label(RichText::new(format!("{} {}", hits.len(), tr("shown"))).small().color(kit::TEXT_DIM));
            });
            for id in add_ids {
                if p.items.iter().any(|it| it.track_id == id) {
                    continue; // a song is only once in a playlist
                }
                let kind = self.data.track(id).map(|t| t.kind.clone()).unwrap_or_default();
                p.items.push(PItem { track_id: id, overlay_pos: if kind == "sweeper" || kind == "voice" { "start".into() } else { String::new() }, ..Default::default() });
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(tr("Add selected library item")).clicked() {
                    if let Some(id) = self.cp.lib_sel.first().copied().or(self.lib_sel) {
                        if self.data.track(id).is_some() && !p.items.iter().any(|it| it.track_id == id) {
                            let kind = self.data.track(id).map(|t| t.kind.clone()).unwrap_or_default();
                            p.items.push(PItem { track_id: id, overlay_pos: if kind == "sweeper" || kind == "voice" { "start".into() } else { String::new() }, ..Default::default() });
                        }
                    }
                }
                egui::ComboBox::from_id_salt("brkref").selected_text(tr("Add break reference...")).show_ui(ui, |ui| {
                    for b in &breaks {
                        if ui.selectable_label(false, b).clicked() {
                            p.items.push(PItem { break_ref: b.clone(), ..Default::default() });
                        }
                    }
                });
                if ui.button(tr("Clear")).clicked() {
                    p.items.clear();
                }
            });
            let mut rem = None;
            let mut swap = None;
            egui::ScrollArea::vertical().id_salt("plitems").max_height(220.0).show(ui, |ui| {
                for (i, it) in p.items.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        let label = if !it.break_ref.is_empty() {
                            format!("Break {}", it.break_ref)
                        } else {
                            self.data.track(it.track_id).map(|t| format!("[{}] {}", t.kind, t.label())).unwrap_or("(missing)".into())
                        };
                        ui.label(format!("{:>3} {label}", i + 1));
                        if !it.overlay_pos.is_empty() {
                            egui::ComboBox::from_id_salt(("ovp", i)).selected_text(it.overlay_pos.clone()).show_ui(ui, |ui| {
                                for (k, l) in POSITIONS {
                                    ui.selectable_value(&mut it.overlay_pos, k.to_string(), l);
                                }
                            });
                            if it.overlay_pos == "custom" {
                                let mut s = it.overlay_custom_ms as f64 / 1000.0;
                                if ui.add(egui::DragValue::new(&mut s).speed(0.1).suffix(tr(" s"))).changed() {
                                    it.overlay_custom_ms = (s * 1000.0) as u64;
                                }
                            }
                        }
                        if ui.small_button(tr("up")).clicked() && i > 0 {
                            swap = Some((i, i - 1));
                        }
                        if ui.small_button(tr("down")).clicked() {
                            swap = Some((i, i + 1));
                        }
                        if ui.small_button(tr("remove")).clicked() {
                            rem = Some(i);
                        }
                    });
                }
            });
            if let Some((a, b)) = swap {
                if b < p.items.len() {
                    p.items.swap(a, b);
                }
            }
            if let Some(i) = rem {
                p.items.remove(i);
            }
            ui.separator();
            ui.label(RichText::new(tr("Schedule")).strong());
            let mut srem = None;
            for (i, s) in p.sched.iter().enumerate() {
                ui.horizontal(|ui| {
                    let when = if s.date.is_empty() { (0..7).filter(|d| (s.weekdays >> d) & 1 == 1).map(|d| DAYS[d]).collect::<Vec<_>>().join(",") } else { s.date.clone() };
                    ui.label(format!("{when} {}{}", s.time, if s.immediate { " (immediately)" } else { "" }));
                    if ui.small_button(tr("remove")).clicked() {
                        srem = Some(i);
                    }
                });
            }
            if let Some(i) = srem {
                p.sched.remove(i);
            }
            ui.horizontal_wrapped(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.cp.pl_time).desired_width(50.0).hint_text(tr("HH:MM")));
                ui.add(egui::TextEdit::singleline(&mut self.cp.pl_date).desired_width(90.0).hint_text(tr("date")));
                for d in 0..7 {
                    ui.checkbox(&mut self.cp.pl_days[d], tr(DAYS[d]));
                }
                let imm = ui.memory_mut(|m| m.data.get_temp_mut_or_default::<bool>(egui::Id::new("pl_imm")).clone());
                let mut imm2 = imm;
                if ui.checkbox(&mut imm2, tr("start immediately")).changed() {
                    ui.memory_mut(|m| m.data.insert_temp(egui::Id::new("pl_imm"), imm2));
                }
                if ui.button(tr("Add once (date)")).clicked() && engine::parse_hm(&self.cp.pl_time).is_some() {
                    p.sched.push(PSched { date: self.cp.pl_date.clone(), weekdays: 0, time: self.cp.pl_time.clone(), immediate: imm2 });
                }
                if ui.button(tr("Add repeated (weekdays)")).clicked() && engine::parse_hm(&self.cp.pl_time).is_some() {
                    let mask = (0..7).fold(0u8, |m, d| if self.cp.pl_days[d] { m | (1 << d) } else { m });
                    p.sched.push(PSched { date: String::new(), weekdays: mask, time: self.cp.pl_time.clone(), immediate: imm2 });
                }
            });
            ui.horizontal(|ui| {
                if ui.button(tr("Save & exit")).clicked() {
                    save = true;
                }
                if ui.button(tr("Cancel")).clicked() {
                    cancel = true;
                }
            });
                    });
        });
        if save {
            self.save_doc("playlist", &mut p);
            self.gen = None;
            self.cp.pl = None;
        } else if cancel {
            self.cp.pl = None;
        } else {
            self.cp.pl = Some(p);
        }
    }

    fn win_force(&mut self, ctx: &egui::Context) {
        let Some(mut f) = self.cp.fc.clone() else { return };
        let (mut save, mut cancel) = (false, false);
        let clocks: Vec<(u64, String)> = self.data.clocks.iter().map(|c| (c.id, c.name.clone())).collect();
        dialog(ctx, format!("Force clock: {}", f.title)).show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
            text_row(ui, "Title", &mut f.title);
            ui.horizontal(|ui| {
                ui.label(tr("Clock"));
                egui::ComboBox::from_id_salt("fcclock").selected_text(clocks.iter().find(|c| c.0 == f.clock_id).map(|c| c.1.clone()).unwrap_or("(choose)".into())).show_ui(ui, |ui| {
                    for (id, n) in &clocks {
                        ui.selectable_value(&mut f.clock_id, *id, n);
                    }
                });
            });
            ui.horizontal(|ui| {
                ui.label(tr("From"));
                ui.add(egui::TextEdit::singleline(&mut f.start_date).desired_width(90.0));
                ui.label(tr("to"));
                ui.add(egui::TextEdit::singleline(&mut f.end_date).desired_width(90.0));
            });
            ui.label(format!("Times: {}", f.times.join(", ")));
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.cp.fc_time).desired_width(50.0).hint_text(tr("HH:MM")));
                if ui.button(tr("Add time")).clicked() && engine::parse_hm(&self.cp.fc_time).is_some() {
                    f.times.push(self.cp.fc_time.trim().to_string());
                }
                if ui.button(tr("Clear times")).clicked() {
                    f.times.clear();
                }
            });
            ui.horizontal(|ui| {
                if ui.button(tr("OK")).clicked() {
                    save = true;
                }
                if ui.button(tr("Cancel")).clicked() {
                    cancel = true;
                }
            });
                    });
        });
        if save {
            self.save_doc("force", &mut f);
            self.gen = None;
            self.cp.fc = None;
        } else if cancel {
            self.cp.fc = None;
        } else {
            self.cp.fc = Some(f);
        }
    }

    fn win_memo(&mut self, ctx: &egui::Context) {
        let Some(mut m) = self.cp.memo.clone() else { return };
        let (mut save, mut cancel) = (false, false);
        let cats = self.data.cat_names("memo");
        dialog(ctx, tr("Memo")).show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
            combo(ui, "mcat", "Category", &mut m.category, &cats);
            text_row(ui, "Title", &mut m.title);
            ui.text_edit_multiline(&mut m.body);
            ui.horizontal(|ui| {
                if ui.button(tr("OK")).clicked() {
                    save = true;
                }
                if ui.button(tr("Cancel")).clicked() {
                    cancel = true;
                }
            });
                    });
        });
        if save {
            self.save_doc("memo", &mut m);
            self.cp.memo = None;
        } else if cancel {
            self.cp.memo = None;
        } else {
            self.cp.memo = Some(m);
        }
    }

    fn win_pallet(&mut self, ctx: &egui::Context) {
        let Some(mut p) = self.cp.pallet.clone() else { return };
        let (mut save, mut cancel) = (false, false);
        dialog(ctx, tr("Pallet")).show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
            text_row(ui, "Name", &mut p.name);
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut p.pages).range(1..=5).prefix(tr("pages: ")));
                ui.add(egui::DragValue::new(&mut p.rows).range(1..=8).prefix(tr("rows: ")));
                ui.add(egui::DragValue::new(&mut p.cols).range(1..=8).prefix(tr("columns: ")));
            });
            ui.horizontal(|ui| {
                if ui.button(tr("OK")).clicked() {
                    save = true;
                }
                if ui.button(tr("Cancel")).clicked() {
                    cancel = true;
                }
            });
                    });
        });
        if save {
            let size = p.pages.max(1) * p.rows.max(1) * p.cols.max(1);
            p.buttons.resize(size, PBtn::default());
            self.save_doc("pallet", &mut p);
            self.cp.pallet = None;
        } else if cancel {
            self.cp.pallet = None;
        } else {
            self.cp.pallet = Some(p);
        }
    }

    fn win_user(&mut self, ctx: &egui::Context) {
        let Some(mut u) = self.cp.user.clone() else { return };
        let (mut save, mut cancel) = (false, false);
        dialog(ctx, tr("User")).show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
            text_row(ui, "User name", &mut u.name);
            ui.horizontal(|ui| {
                ui.label(tr("Password (leave empty to keep)"));
                ui.add(egui::TextEdit::singleline(&mut self.cp.user_pw).password(true));
            });
            ui.add(egui::DragValue::new(&mut u.timeout_min).range(0..=600).prefix(tr("Login timeout (min, 0 = off): ")));
            ui.checkbox(&mut u.admin, tr("Administrator (all permissions)"));
            if !u.admin {
                for (k, l) in PERMS {
                    let mut on = u.perms.iter().any(|p| p == k);
                    if ui.checkbox(&mut on, tr(l)).changed() {
                        if on {
                            u.perms.push(k.to_string());
                        } else {
                            u.perms.retain(|p| p != k);
                        }
                    }
                }
            }
            ui.horizontal(|ui| {
                if ui.button(tr("OK")).clicked() {
                    save = true;
                }
                if ui.button(tr("Cancel")).clicked() {
                    cancel = true;
                }
            });
                    });
        });
        if save {
            if !self.cp.user_pw.is_empty() || u.hash.is_empty() {
                u.salt = format!("{:x}", Local::now().timestamp_nanos_opt().unwrap_or(0));
                u.hash = hash_pw(&u.salt, &self.cp.user_pw);
            }
            self.save_doc("user", &mut u);
            self.cp.user = None;
            self.cp.user_pw.clear();
        } else if cancel {
            self.cp.user = None;
        } else {
            self.cp.user = Some(u);
        }
        let _ = Datelike::year(&Local::now());
    }
}
