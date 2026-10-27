//! The studio screen (what the operator sees on air) and its actions.
use crate::app::{fmt_count, fmt_dur, fmt_ms, App, Mode, BUILD, VERSION};
use crate::engine;
use crate::i18n::tr;
use crate::kit::{self, Icon};
use crate::model::*;
use chrono::{DateTime, Local, Timelike};
use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Rect, Sense, Stroke};
use std::f32::consts::PI;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

const WHITE: Color32 = Color32::WHITE;

pub enum Action {
    PlayIndex(usize),
    Next,
    Restart,
    ToggleLoop,
    Pause,
    StopNext,
    Stop,
    Volume(f32),
    Jingle(u64),
    StopJingles,
    Select(usize),
    MoveSel(i32),
    SelTop,
    SelBottom,
    MoveItemUp,
    MoveItemDown,
    RemoveLog(usize),
    PreviewTrack(String, String, u64),
    PreviewSel,
    StopPreview,
    LibSelect(u64),
    AddSel,
    InsertSel,
    DeleteSel,
    ClearLog,
    Tab(&'static str),
    SetMode(Mode),
    ToggleLayout,
    ToggleLive,
    StartLive(usize),
    OpenControl,
    PalletStep(i32),
    Page(usize),
    AssignBtn(usize),
    ClearBtn(usize),
    BtnColor(usize, [u8; 3]),
    RegenAuto,
}

fn upcoming_breaks(log: &[Track], starts: &[DateTime<Local>], from: usize) -> Vec<(String, Vec<String>, u64)> {
    let mut out = Vec::new();
    let mut i = from;
    while i < log.len() {
        let is_b = |t: &Track| t.kind == "break" || t.kind == "spot";
        if is_b(&log[i]) {
            let st = starts[i].format("%H:%M:%S").to_string();
            let mut items = Vec::new();
            let mut tot = 0u64;
            while i < log.len() && is_b(&log[i]) {
                items.push(log[i].title.clone());
                tot += log[i].duration_ms;
                i += 1;
            }
            out.push((st, items, tot));
            if out.len() >= 4 {
                break;
            }
        } else {
            i += 1;
        }
    }
    out
}

impl App {
    pub fn handle(&mut self, a: Action) {
        if self.remote.is_some() && self.remote_send(&a) {
            return;
        }
        match a {
            Action::PlayIndex(i) => self.play_index(i, 1200),
            Action::Next => self.advance(1200),
            Action::Restart => {
                if let Some(c) = self.current {
                    self.play_index(c, 0);
                }
            }
            Action::ToggleLoop => self.loop_current = !self.loop_current,
            Action::Pause => {
                if let Some(p) = self.player.as_mut() {
                    p.toggle_pause();
                }
            }
            Action::StopNext => self.stop_next = !self.stop_next,
            Action::Stop => self.stop(),
            Action::Volume(v) => {
                if let Some(p) = self.player.as_mut() {
                    p.set_volume(v);
                }
            }
            Action::Jingle(id) => {
                if let Some(t) = self.data.track(id).cloned() {
                    let path = self.map_path(&t.path);
                    if let Some(p) = self.player.as_mut() {
                        match p.play_jingle(&path, t.gain_db) {
                            Ok(()) => self.jingle_now = Some((t.id, t.title.clone(), Instant::now(), t.duration_ms)),
                            Err(e) => self.status = format!("Jingle error: {e}"),
                        }
                    }
                }
            }
            Action::StopJingles => {
                if let Some(p) = self.player.as_mut() {
                    p.stop_jingles();
                }
                self.jingle_now = None;
            }
            Action::Select(i) => self.sel = Some(i),
            Action::MoveSel(d) => {
                let n = self.log.len() as i32;
                if n > 0 {
                    let s = self.sel.map(|s| s as i32 + d).unwrap_or(0).clamp(0, n - 1) as usize;
                    self.sel = Some(s);
                    self.scroll_to = Some(s);
                }
            }
            Action::SelTop => {
                if !self.log.is_empty() {
                    self.sel = Some(0);
                    self.scroll_to = Some(0);
                }
            }
            Action::SelBottom => {
                if !self.log.is_empty() {
                    let l = self.log.len() - 1;
                    self.sel = Some(l);
                    self.scroll_to = Some(l);
                }
            }
            Action::MoveItemUp => {
                if let Some(s) = self.sel {
                    if s > 0 && s < self.log.len() {
                        self.log.swap(s, s - 1);
                        if self.current == Some(s) {
                            self.current = Some(s - 1);
                        } else if self.current == Some(s - 1) {
                            self.current = Some(s);
                        }
                        self.sel = Some(s - 1);
                        self.save_log();
                    }
                }
            }
            Action::MoveItemDown => {
                if let Some(s) = self.sel {
                    if s + 1 < self.log.len() {
                        self.log.swap(s, s + 1);
                        if self.current == Some(s) {
                            self.current = Some(s + 1);
                        } else if self.current == Some(s + 1) {
                            self.current = Some(s);
                        }
                        self.sel = Some(s + 1);
                        self.save_log();
                    }
                }
            }
            Action::RemoveLog(i) => {
                if !self.can("studio_delete") {
                    self.status = tr("You are not allowed to delete tracks");
                } else if i < self.log.len() {
                    if self.current == Some(i) {
                        self.status = tr("Cannot remove the item that is on air");
                    } else {
                        self.log.remove(i);
                        if let Some(c) = self.current {
                            if c > i {
                                self.current = Some(c - 1);
                            }
                        }
                        self.sel = None;
                        self.save_log();
                    }
                }
            }
            Action::PreviewTrack(path, name, dur) => {
                let path = self.local_audio(&path);
                if let Some(p) = self.player.as_mut() {
                    match p.play_preview(&path, 0, 0) {
                        Ok(()) => {
                            self.preview_name = name;
                            self.preview_dur = dur;
                        }
                        Err(e) => self.status = format!("Preview error: {e}"),
                    }
                }
            }
            Action::PreviewSel => {
                if let Some(t) = self.lib_sel.and_then(|id| self.data.track(id).cloned()) {
                    self.handle(Action::PreviewTrack(t.path.clone(), t.label(), t.duration_ms));
                }
            }
            Action::StopPreview => {
                if let Some(p) = self.player.as_mut() {
                    p.stop_preview();
                }
            }
            Action::LibSelect(id) => self.lib_sel = Some(id),
            Action::AddSel | Action::InsertSel => {
                if !self.can("studio_add") {
                    self.status = tr("You are not allowed to add items");
                } else if let Some(mut t) = self.lib_sel.and_then(|id| self.data.track(id).cloned()) {
                    t.src = "manual".into();
                    t.reason = "Added manually in the studio".into();
                    if self.lib_kind == "sweeper" || self.lib_kind == "voice" {
                        t.overlay_pos = "start".into();
                    }
                    if matches!(a, Action::AddSel) && self.mode == Mode::Auto {
                        // in AUTO the schedule has already filled the next hours: a manual item is queued right after the
                        // current song (and after the manual items added before it), so it is really played
                        let from = self.current.map(|c| c + 1).unwrap_or(0).min(self.log.len());
                        let mut pos = from;
                        while pos < self.log.len() && self.log[pos].src == "manual" {
                            pos += 1;
                        }
                        self.log.insert(pos, t);
                        self.sel = Some(pos);
                    } else if matches!(a, Action::AddSel) {
                        self.log.push(t);
                    } else {
                        let pos = self.current.map(|c| c + 1).unwrap_or(0).min(self.log.len());
                        self.log.insert(pos, t);
                        self.sel = Some(pos);
                    }
                    self.save_log();
                }
            }
            Action::DeleteSel => {
                // removes the selected item from the playlist (the song stays in the library)
                match self.sel {
                    Some(i) if i < self.log.len() => self.handle(Action::RemoveLog(i)),
                    _ => self.status = tr("Select an item in the playlist first"),
                }
            }
            Action::ClearLog => {
                self.stop();
                self.log.clear();
                self.sel = None;
                self.gen = None;
                self.save_log();
            }
            Action::Tab(k) => {
                self.lib_kind = k;
                self.lib_sel = None;
            }
            Action::ToggleLive => {
                if self.player.as_ref().map(|p| p.live_active()).unwrap_or(false) {
                    self.stop_live_source();
                } else {
                    let (i, m) = (self.live_sel.min(self.data.s.live_sources.len().saturating_sub(1)), self.data.s.live_mute_automation);
                    self.start_live_source(i, m, None);
                }
            }
            Action::StartLive(i) => {
                let m = self.data.s.live_mute_automation;
                self.start_live_source(i, m, None);
            }
            Action::SetMode(m) => {
                if !self.can("studio_modes") {
                    self.status = tr("You are not allowed to change the mode");
                } else {
                    self.mode = m;
                    self.gen = None;
                    if m == Mode::Offline {
                        self.stop();
                    }
                    if m == Mode::Auto {
                        self.autoplay_pending = true;
                        self.auto_fill();
                        self.expand_pending();
                        self.auto_start();
                    }
                }
            }
            Action::ToggleLayout => {
                self.data.s.layout = match self.data.s.layout.as_str() {
                    "classic" => "simple".into(),
                    "simple" => "wide".into(),
                    _ => "classic".into(),
                };
                self.save_settings();
            }
            Action::OpenControl => self.cp_open = true,
            Action::PalletStep(d) => {
                let n = self.data.pallets.len() as i32;
                if n > 0 {
                    self.pallet_idx = (self.pallet_idx as i32 + d).rem_euclid(n) as usize;
                    self.page = 0;
                }
            }
            Action::Page(p) => self.page = p,
            Action::AssignBtn(gi) | Action::ClearBtn(gi) => {
                if !self.can("studio_instants") {
                    self.status = tr("You are not allowed to edit instant jingles");
                } else if let Some(mut p) = self.data.pallets.get(self.pallet_idx).cloned() {
                    let size = p.pages.max(1) * p.rows.max(1) * p.cols.max(1);
                    if p.buttons.len() < size {
                        p.buttons.resize(size, PBtn::default());
                    }
                    if gi < p.buttons.len() {
                        if matches!(a, Action::AssignBtn(_)) {
                            if let Some(id) = self.lib_sel.filter(|id| self.data.track(*id).is_some()) {
                                p.buttons[gi].track_id = id;
                            }
                        } else {
                            p.buttons[gi] = PBtn::default();
                        }
                        self.save_doc("pallet", &mut p);
                    }
                }
            }
            Action::BtnColor(gi, c) => {
                if self.can("studio_instants") {
                    if let Some(mut p) = self.data.pallets.get(self.pallet_idx).cloned() {
                        if gi < p.buttons.len() {
                            p.buttons[gi].color = c;
                            self.save_doc("pallet", &mut p);
                        }
                    }
                }
            }
            Action::RegenAuto => {
                let keep = self.current.map(|c| c + 1).unwrap_or(0);
                self.log.truncate(keep.min(self.log.len()));
                self.gen = None;
                self.save_log();
            }
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        if ctx.input(|i| i.key_pressed(egui::Key::F10)) {
            toggle_fullscreen(ctx);
        }
        if ctx.wants_keyboard_input() {
            return;
        }
        for (act, key) in self.data.s.shortcuts.clone() {
            if let Some(k) = egui::Key::from_name(&key) {
                if ctx.input(|i| i.key_pressed(k)) {
                    match act.as_str() {
                        "next" => actions.push(Action::Next),
                        "pause" => actions.push(Action::Pause),
                        "stop" => actions.push(Action::Stop),
                        "restart" => actions.push(Action::Restart),
                        "loop" => actions.push(Action::ToggleLoop),
                        "stopnext" => actions.push(Action::StopNext),
                        "stopjingles" => actions.push(Action::StopJingles),
                        _ => {}
                    }
                }
            }
        }
    }

    fn login_screen(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(120.0);
            ui.vertical_centered(|ui| {
                ui.heading(tr("RadioMoon"));
                ui.add_space(12.0);
                ui.add(egui::TextEdit::singleline(&mut self.login_name).hint_text(tr("User name")).desired_width(220.0));
                ui.add(egui::TextEdit::singleline(&mut self.login_pass).password(true).hint_text(tr("Password")).desired_width(220.0));
                ui.add_space(8.0);
                let go = ui.button(tr("Log in")).clicked();
                if !self.login_err.is_empty() {
                    ui.colored_label(kit::BAD, &self.login_err);
                }
                if go {
                    self.login();
                }
            });
        });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // the studio is designed for 1250 x 700: on a smaller window or screen everything is scaled down to fit (nothing is cut off)
        {
            let z = ctx.zoom_factor();
            let sz = ctx.screen_rect().size() * z;
            let target = (sz.x / 1250.0).min(sz.y / 700.0).clamp(0.5, 1.0);
            let target = if target > 0.97 { 1.0 } else { target };
            if (target - z).abs() > 0.01 {
                ctx.set_zoom_factor(target);
            }
        }
        let busy = self.px_elapsed() > 0 || self.player.as_ref().map(|p| p.preview_active()).unwrap_or(false);
        ctx.request_repaint_after(Duration::from_millis(if busy { 66 } else { 250 }));
        if ctx.input(|i| !i.events.is_empty()) {
            self.last_activity = Instant::now();
        }
        if self.needs_login() {
            self.login_screen(ctx);
            return;
        }
        if let Some(u) = &self.user {
            if u.timeout_min > 0 && self.last_activity.elapsed() > Duration::from_secs(u.timeout_min as u64 * 60) {
                self.logout();
                return;
            }
        }
        if self.remote.is_some() {
            self.remote_tick();
        } else {
            self.playout_tick();
        }

        // ---- values computed once per frame ----
        let now = Local::now();
        let elapsed = self.px_elapsed();
        let pos = self.px_pos();
        let paused = self.px_paused();
        let (pv_active, pv_el) = self.player.as_ref().map(|p| (p.preview_active(), p.preview_elapsed_ms())).unwrap_or((false, 0));
        let live_label: Option<String> = if self.px_live() {
            Some(self.data.s.live_sources.get(self.live_sel).map(|x| x.name.clone()).unwrap_or_else(|| "LIVE".into()))
        } else {
            None
        };
        let starts_naive = self.starts();
        let starts: Vec<DateTime<Local>> = starts_naive.iter().map(|n| n.and_local_timezone(Local).single().unwrap_or(now)).collect();
        let from = self.current.map(|c| c + 1).unwrap_or(0);
        let breaks = upcoming_breaks(&self.log, &starts, from);
        // file-system checks are cached for 3 s (they used to run on every frame)
        thread_local! { static PROB: std::cell::RefCell<(Option<Instant>, usize, Vec<String>)> = const { std::cell::RefCell::new((None, usize::MAX, Vec::new())) }; }
        let problems: Vec<String> = PROB.with(|c| {
            let mut c = c.borrow_mut();
            let stale = c.0.map(|t| t.elapsed().as_secs() >= 3).unwrap_or(true) || c.1 != from;
            if stale {
                c.2 = self
                    .log
                    .iter()
                    .skip(from)
                    .take(12)
                    .filter(|t| t.id != 0 && !self.file_ok(&t.path))
                    .map(|t| format!("Missing file: {}", t.title))
                    .collect();
                c.0 = Some(Instant::now());
                c.1 = from;
            }
            c.2.clone()
        });
        let ct: Option<Track> = self.current.and_then(|i| self.log.get(i)).cloned();
        let peaks: Arc<Vec<f32>> = match self.wave.lock() {
            Ok(g) if Some(g.0) == ct.as_ref().map(|t| t.id) => g.1.clone(),
            _ => Arc::new(Vec::new()),
        };
        let jn = self.jingle_now.as_ref().map(|(id, title, t0, d)| (*id, title.clone(), d.saturating_sub(t0.elapsed().as_millis() as u64)));
        let scroll_to = self.scroll_to.take();
        let trigger = self.trigger_ms();
        let layout = self.data.s.layout.clone();
        let station = self.data.s.station.clone();
        let mut actions: Vec<Action> = Vec::new();
        self.shortcuts(ctx, &mut actions);

        self.ensure_logo(ctx);
        // ---- header ----
        egui::TopBottomPanel::top("header").exact_height(30.0).frame(egui::Frame::none().fill(Color32::BLACK)).show(ctx, |ui| {
            let r = ui.max_rect();
            let p = ui.painter().clone();
            let cy = r.center().y;
            p.text(pos2(r.left() + 12.0, cy), Align2::LEFT_CENTER, tr("RadioMoon"), FontId::proportional(20.0), WHITE);
            p.text(pos2(r.left() + 114.0, cy + 3.0), Align2::LEFT_CENTER, format!("v{VERSION}"), FontId::proportional(11.0), kit::GRAY);
            let _ = BUILD;
            let vr = Rect::from_min_size(pos2(r.left() + 170.0, r.top() + 4.0), vec2(120.0, 22.0));
            let mut vol = self.px_volume();
            if ui.put(vr, egui::Slider::new(&mut vol, 0.0..=1.0).show_value(false)).changed() {
                actions.push(Action::Volume(vol));
            }
            // listeners that are connected to the internet broadcasts
            let ls = self.px_listeners();
            if !ls.is_empty() {
                let total: i64 = ls.iter().filter(|x| x.1 >= 0).map(|x| x.1).sum();
                let known = ls.iter().any(|x| x.1 >= 0);
                let txt = if known { format!("{total} {}", tr("listeners")) } else { format!("- {}", tr("listeners")) };
                let lr = Rect::from_min_size(pos2(r.left() + 304.0, r.top()), vec2(150.0, 30.0));
                p.circle_filled(pos2(lr.left() + 8.0, cy), 4.0, if known { Color32::from_rgb(60, 220, 100) } else { kit::GRAY });
                p.text(pos2(lr.left() + 20.0, cy), Align2::LEFT_CENTER, txt, FontId::proportional(14.0), WHITE);
            }
            let h = r.height();
            let mut x = r.right();
            let mut slot = |w: f32| -> Rect {
                x -= w;
                Rect::from_min_size(pos2(x, r.top()), vec2(w, h))
            };
            let cp_r = slot(kit::label_w(ui, "Control Panel", 110.0));
            if kit::button_at(ui, cp_r, "h_cp", None, "Control Panel", Color32::from_rgb(40, 60, 110), WHITE).clicked() {
                actions.push(Action::OpenControl);
            }
            let ab_r = slot(kit::label_w(ui, "About", 64.0));
            if kit::button_at(ui, ab_r, "h_about", None, "About", kit::DARK, WHITE).clicked() {
                self.about_open = true;
            }
            let live_on = self.px_live();
            let live_r = slot(kit::label_w(ui, "LIVE", 70.0));
            let live_resp = kit::button_at(ui, live_r, "h_live", None, "LIVE", if live_on { Color32::from_rgb(220, 120, 0) } else { kit::DARK }, WHITE);
            if live_resp.clicked() {
                actions.push(Action::ToggleLive);
            }
            let live_names: Vec<String> = self.data.s.live_sources.iter().map(|x| x.name.clone()).collect();
            live_resp.context_menu(|ui| {
                for (i, n) in live_names.iter().enumerate() {
                    if ui.button(n).clicked() {
                        actions.push(Action::StartLive(i));
                        ui.close_menu();
                    }
                }
                if live_names.is_empty() {
                    ui.label(tr("No live source defined"));
                }
            });
            let lay_label = format!("Layout: {layout}");
            let lay_r = slot(kit::label_w(ui, &lay_label, 90.0));
            if kit::button_at(ui, lay_r, "h_lay", None, &lay_label, kit::DARK, WHITE).clicked() {
                actions.push(Action::ToggleLayout);
            }
            for (m, label, col) in [(Mode::Offline, "OFFLINE", Color32::from_rgb(90, 90, 100)), (Mode::Manual, "MANUAL", kit::RED), (Mode::Auto, "AUTO", kit::GREEN)] {
                let rr = slot(kit::label_w(ui, label, 84.0));
                let fill = if self.mode == m { col } else { kit::DARK };
                if kit::button_at(ui, rr, label, None, label, fill, WHITE).clicked() {
                    actions.push(Action::SetMode(m));
                }
            }
            p.text(pos2(x - 14.0, cy), Align2::RIGHT_CENTER, &station, FontId::proportional(15.0), kit::GRAY);
        });

        // ---- deck (now playing) ----
        egui::TopBottomPanel::top("deck").exact_height(150.0).frame(egui::Frame::none().fill(Color32::from_rgb(10, 10, 12))).show(ctx, |ui| {
            let r = ui.max_rect();
            let p = ui.painter().clone();
            if kit::button_at(ui, Rect::from_min_size(r.min, vec2(58.0, r.height())), "d_next", Some(Icon::Eject), "Next", kit::DARK, WHITE).clicked() {
                actions.push(Action::Next);
            }
            let x0 = r.left() + 68.0;
            let rx = (r.right() - 540.0).max(x0 + 360.0);
            let dur = ct.as_ref().map(|t| t.duration_ms).unwrap_or(0);
            let frac = if dur > 0 { (pos as f32 / dur as f32).min(1.0) } else { 0.0 };

            if let Some(name) = &live_label {
                let on = (now.timestamp_millis() / 500) % 2 == 0;
                p.circle_filled(pos2(x0 + 14.0, r.top() + 34.0), 11.0, if on { kit::BAD } else { kit::BAD.gamma_multiply(0.35) });
                p.text(pos2(x0 + 36.0, r.top() + 34.0), Align2::LEFT_CENTER, "LIVE", FontId::proportional(40.0), WHITE);
                p.text(pos2(x0, r.top() + 68.0), Align2::LEFT_CENTER, name, FontId::proportional(20.0), kit::GRAY);
                ctx.request_repaint_after(std::time::Duration::from_millis(250));
            } else if let Some(t) = &ct {
                p.text(pos2(x0, r.top() + 20.0), Align2::LEFT_CENTER, t.artist.to_uppercase(), FontId::proportional(20.0), WHITE);
                p.text(pos2(x0, r.top() + 46.0), Align2::LEFT_CENTER, &t.title, FontId::proportional(28.0), WHITE);
                let meta = format!("{} | {} | {} | {}", t.kind, t.cat1, t.genre, fmt_ms(t.duration_ms));
                p.text(pos2(x0, r.top() + 68.0), Align2::LEFT_CENTER, meta, FontId::proportional(12.0), kit::GRAY);
                let rem = trigger.map(|tr| tr.saturating_sub(pos)).unwrap_or_else(|| 20_000u64.saturating_sub(elapsed));
                let col = if rem < 10_000 { Color32::from_rgb(255, 80, 60) } else { Color32::from_rgb(0, 220, 40) };
                p.text(pos2(rx - 50.0, r.top() + 42.0), Align2::RIGHT_CENTER, fmt_count(rem), FontId::proportional(52.0), col);
            } else {
                p.text(pos2(x0, r.top() + 40.0), Align2::LEFT_CENTER, tr("- off air -"), FontId::proportional(28.0), kit::GRAY);
            }

            let wr = Rect::from_min_max(pos2(x0, r.top() + 84.0), pos2(rx - 34.0, r.bottom() - 8.0));
            p.rect_filled(wr, 0.0, kit::INSET);
            if !peaks.is_empty() {
                let cols = (wr.width() / 2.0) as usize;
                let mid = wr.center().y;
                for c in 0..cols {
                    let idx = c * peaks.len() / cols.max(1);
                    let a = peaks[idx.min(peaks.len() - 1)].powf(0.7);
                    let h = a * wr.height() * 0.5;
                    let x = wr.left() + c as f32 * 2.0 + 1.0;
                    let col = if (c as f32 / cols as f32) <= frac { Color32::from_rgb(210, 40, 40) } else { Color32::from_rgb(110, 110, 120) };
                    p.line_segment([pos2(x, mid - h), pos2(x, mid + h)], Stroke::new(1.5_f32, col));
                }
                if let Some(t) = &ct {
                    if dur > 0 {
                        for (m, col) in [(t.mix_in_ms, kit::GOOD), (t.mix_out_ms, kit::WARN), (t.intro_ms, Color32::from_rgb(60, 160, 230))] {
                            if m > 0 {
                                let mx = wr.left() + (m as f32 / dur as f32).min(1.0) * wr.width();
                                p.line_segment([pos2(mx, wr.top()), pos2(mx, wr.bottom())], Stroke::new(1.0_f32, col));
                            }
                        }
                    }
                }
                let px = wr.left() + frac * wr.width();
                p.line_segment([pos2(px, wr.top()), pos2(px, wr.bottom())], Stroke::new(1.5_f32, WHITE));
            } else if ct.is_some() {
                p.text(wr.center(), Align2::CENTER_CENTER, tr("analysing waveform..."), FontId::proportional(12.0), kit::GRAY);
            }

            let level = if paused || peaks.is_empty() {
                0.0
            } else {
                let i = ((frac * peaks.len() as f32) as usize).min(peaks.len() - 1);
                let j = (i + 3).min(peaks.len());
                peaks[i..j].iter().cloned().fold(0.0f32, f32::max).powf(0.6)
            };
            let vu_top = r.top() + 10.0;
            let vu_h = r.height() - 20.0;
            let segs = 24usize;
            for side in 0..2 {
                let x = rx - 26.0 + side as f32 * 12.0;
                for k in 0..segs {
                    let y = vu_top + vu_h - (k as f32 + 1.0) * (vu_h / segs as f32);
                    let on = (k as f32 / segs as f32) < level;
                    let col = if !on {
                        Color32::from_rgb(28, 30, 28)
                    } else if k > 20 {
                        Color32::from_rgb(230, 50, 40)
                    } else if k > 15 {
                        Color32::from_rgb(230, 200, 40)
                    } else {
                        Color32::from_rgb(60, 220, 60)
                    };
                    p.rect_filled(Rect::from_min_size(pos2(x, y), vec2(9.0, vu_h / segs as f32 - 1.5)), 0.0, col);
                }
            }

            // clock: dotted ring for the seconds, round neon arc for the minutes
            let cc = pos2(rx + 75.0, r.top() + 75.0);
            let secf = now.second() as f32 + now.timestamp_subsec_millis() as f32 / 1000.0;
            let frac = (now.minute() as f32 + secf / 60.0) / 60.0;
            let arc = |rad: f32, f: f32| -> Vec<egui::Pos2> {
                let n = ((f * 120.0) as usize).max(1);
                (0..=n)
                    .map(|i| {
                        let a = -PI / 2.0 + 2.0 * PI * f * (i as f32 / n as f32);
                        pos2(cc.x + rad * a.cos(), cc.y + rad * a.sin())
                    })
                    .collect()
            };
            {
                // round neon minutes outside, thin green seconds ring inside
                let (rm, rs) = (66.0_f32, 53.0_f32);
                p.circle_stroke(cc, rm, egui::Stroke::new(7.0_f32, Color32::from_rgb(30, 39, 51)));
                let pts = arc(rm, frac);
                let (first, last) = (pts[0], pts[pts.len() - 1]);
                p.add(egui::Shape::line(pts.clone(), egui::Stroke::new(13.0_f32, Color32::from_rgba_unmultiplied(56, 182, 255, 38))));
                p.add(egui::Shape::line(pts, egui::Stroke::new(7.0_f32, kit::ACCENT)));
                p.circle_filled(first, 3.5, kit::ACCENT);
                p.circle_filled(last, 3.5, kit::ACCENT);
                p.circle_filled(last, 10.0, Color32::from_rgba_unmultiplied(56, 182, 255, 45));
                p.circle_filled(last, 3.6, WHITE);
                p.circle_stroke(cc, rs, egui::Stroke::new(2.0_f32, Color32::from_rgb(30, 39, 51)));
                let sf = secf / 60.0;
                if sf > 0.002 {
                    let ps = arc(rs, sf);
                    p.add(egui::Shape::line(ps.clone(), egui::Stroke::new(5.0_f32, Color32::from_rgba_unmultiplied(52, 211, 134, 40))));
                    p.add(egui::Shape::line(ps, egui::Stroke::new(2.0_f32, Color32::from_rgb(52, 211, 134))));
                }
            }
            p.text(pos2(cc.x, cc.y - 21.0), Align2::CENTER_CENTER, now.format("%A").to_string(), FontId::proportional(11.0), kit::GRAY);
            p.text(pos2(cc.x, cc.y), Align2::CENTER_CENTER, now.format("%H:%M:%S").to_string(), FontId::proportional(21.0), WHITE);
            p.text(pos2(cc.x, cc.y + 19.0), Align2::CENTER_CENTER, now.format("%d %B %Y").to_string(), FontId::proportional(10.5), kit::GRAY);

            let lr = Rect::from_min_size(pos2(rx + 150.0, r.top() + 8.0), vec2(130.0, r.height() - 16.0));
            // the program icon instead of the old blue tile
            let side = (lr.width() - 6.0).min(lr.height() - 30.0).max(40.0);
            let ir = Rect::from_min_size(pos2(lr.center().x - side / 2.0, lr.top() + 2.0), vec2(side, side));
            match &self.about_tex {
                Some(t) => {
                    p.image(t.id(), ir, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                }
                None => {
                    p.rect_filled(lr, 4.0, Color32::from_rgb(18, 50, 130));
                    kit::draw_moon(&p, pos2(lr.center().x, lr.top() + 48.0), 30.0, WHITE, Color32::from_rgb(18, 50, 130));
                }
            }
            p.text(pos2(lr.center().x, lr.bottom() - 14.0), Align2::CENTER_CENTER, tr("RadioMoon"), FontId::proportional(18.0), WHITE);

            let bx = rx + 290.0;
            let bh = r.height();
            let mk = |n: f32| Rect::from_min_size(pos2(bx + n * 62.0, r.top()), vec2(61.0, bh));
            if kit::button_at(ui, mk(0.0), "d_restart", Some(Icon::Restart), "Restart", kit::DARK, WHITE).clicked() {
                actions.push(Action::Restart);
            }
            let loop_fill = if self.loop_current { Color32::from_rgb(40, 110, 60) } else { kit::DARK };
            if kit::button_at(ui, mk(1.0), "d_loop", Some(Icon::Loop), "Loop", loop_fill, WHITE).clicked() {
                actions.push(Action::ToggleLoop);
            }
            let (pi, pl) = if paused { (Icon::Play, "Resume") } else { (Icon::Pause, "Pause") };
            if kit::button_at(ui, mk(2.0), "d_pause", Some(pi), pl, kit::DARK, WHITE).clicked() {
                actions.push(Action::Pause);
            }
            let sn_fill = if self.stop_next { Color32::from_rgb(150, 90, 20) } else { kit::DARK };
            if kit::button_at(ui, mk(3.0), "d_stopnext", Some(Icon::Next), "StopNext", sn_fill, WHITE).clicked() {
                actions.push(Action::StopNext);
            }
        });

        // ---- status bar ----
        egui::TopBottomPanel::bottom("status").exact_height(22.0).frame(egui::Frame::none().fill(kit::INSET)).show(ctx, |ui| {
            let scan = self.scan_status.lock().map(|s| s.clone()).unwrap_or_default();
            let who = self.user.as_ref().map(|u| format!("   [{}]", u.name)).unwrap_or_default();
            let sv = self.px_streams();
            let live = if sv.is_empty() { String::new() } else { format!("   [LIVE: {}]", sv.iter().map(|(_, t)| t.clone()).collect::<Vec<_>>().join(" | ")) };
            let bw = self.remote.as_ref().map(|r| format!("   [server: \u{2193} {}  \u{2191} {}]", crate::remote::fmt_rate(r.rx_rate), crate::remote::fmt_rate(r.tx_rate))).unwrap_or_default();
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(format!("{}    {}{}{}{}", self.status, scan, who, live, bw)).color(kit::GRAY).size(12.0));
                if self.remote.is_some() {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let mut m = crate::remote::bw_mode();
                        let old = m;
                        egui::ComboBox::from_id_salt("bw_mode").width(170.0).selected_text(tr(crate::remote::bw_name(m))).show_ui(ui, |ui| {
                            for k in 0..3u8 {
                                ui.selectable_value(&mut m, k, tr(crate::remote::bw_name(k)));
                            }
                        });
                        if m != old {
                            crate::remote::LOW_BW.store(m, Ordering::Relaxed);
                            crate::remote::save_bw(&self.base);
                        }
                    });
                }
            });
        });

        // ---- playlist (left) ----
        egui::SidePanel::left("playlist").exact_width(540.0).resizable(false).frame(egui::Frame::none().fill(Color32::from_rgb(12, 12, 16))).show(ctx, |ui| {
            let row_h = 66.0;
            let list_h = (ui.available_height() - 84.0).max(100.0);
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            let mut sa = egui::ScrollArea::vertical().auto_shrink([false, false]).max_height(list_h);
            if let Some(i) = scroll_to {
                sa = sa.vertical_scroll_offset((i as f32 * row_h - row_h).max(0.0));
            }
            sa.show_rows(ui, row_h, self.log.len(), |ui, range| {
                for i in range {
                    let t = &self.log[i];
                    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), row_h), Sense::click());
                    let is_cur = self.current == Some(i);
                    let played = self.current.map(|c| i < c).unwrap_or(false);
                    // the same colours as the library tabs, darkened
                    let dark = |c: Color32| Color32::from_rgb((c.r() as f32 * 0.34) as u8, (c.g() as f32 * 0.34) as u8, (c.b() as f32 * 0.34) as u8);
                    let base = match t.kind.as_str() {
                        "jingle" => dark(Color32::from_rgb(230, 200, 60)),
                        "spot" | "break" => dark(Color32::from_rgb(60, 200, 90)),
                        "sweeper" => dark(Color32::from_rgb(180, 90, 220)),
                        "voice" => dark(Color32::from_rgb(210, 60, 60)),
                        "stitch_hooks" | "stitch_time" | "stitch_news" => Color32::from_rgb(20, 70, 70),
                        "asset" => dark(Color32::from_rgb(150, 150, 150)),
                        _ => dark(Color32::from_rgb(60, 160, 230)),
                    };
                    let bg = if is_cur {
                        kit::lighten(base, 28)
                    } else if played {
                        base.gamma_multiply(0.55)
                    } else {
                        base
                    };
                    let p = ui.painter_at(rect);
                    p.rect_filled(rect.shrink2(vec2(0.0, 1.0)), 0.0, bg);
                    if is_cur {
                        p.rect_filled(Rect::from_min_size(rect.min, vec2(4.0, row_h)), 0.0, Color32::from_rgb(0, 220, 40));
                    }
                    if self.sel == Some(i) {
                        p.rect_stroke(rect.shrink(1.0), 0.0, Stroke::new(1.5_f32, Color32::from_rgb(240, 200, 60)));
                    }
                    let play_rect = Rect::from_min_size(rect.min + vec2(4.0, 0.0), vec2(46.0, row_h));
                    let hp_rect = Rect::from_min_size(pos2(rect.right() - 112.0, rect.top()), vec2(54.0, row_h));
                    let tr_rect = Rect::from_min_size(pos2(rect.right() - 56.0, rect.top()), vec2(54.0, row_h));
                    let r_play = ui.interact(play_rect, ui.id().with(("pl_play", i)), Sense::click());
                    let r_hp = ui.interact(hp_rect, ui.id().with(("pl_hp", i)), Sense::click());
                    let r_tr = ui.interact(tr_rect, ui.id().with(("pl_tr", i)), Sense::click());
                    let ic = |h: bool| if h { WHITE } else { Color32::from_gray(190) };
                    kit::draw_icon(&p, Icon::Play, play_rect.center(), 13.0, ic(r_play.hovered()));
                    kit::draw_icon(&p, Icon::Headphones, hp_rect.center(), 12.0, ic(r_hp.hovered()));
                    kit::draw_icon(&p, Icon::Trash, tr_rect.center(), 11.0, ic(r_tr.hovered()));

                    let tp = ui.painter_at(Rect::from_min_max(pos2(rect.left() + 56.0, rect.top()), pos2(rect.right() - 114.0, rect.bottom())));
                    let tx = rect.left() + 60.0;
                    let r1 = tp.text(pos2(tx, rect.top() + 14.0), Align2::LEFT_CENTER, format!("AT {}", starts[i].format("%H:%M:%S")), FontId::proportional(11.0), Color32::from_gray(190));
                    let mut nx = r1.right() + 10.0;
                    if !t.is_overlay() {
                        let r2 = tp.text(pos2(nx, rect.top() + 14.0), Align2::LEFT_CENTER, format!("Dur: {}", fmt_dur(t.duration_ms)), FontId::proportional(11.0), kit::GOOD);
                        nx = r2.right() + 10.0;
                    }
                    if !t.fixed_start.is_empty() {
                        tp.text(pos2(nx, rect.top() + 14.0), Align2::LEFT_CENTER, tr("EXACT TIME"), FontId::proportional(11.0), kit::WARN);
                    }
                    let mut title = if t.kind == "music" && !t.artist.is_empty() { format!("{} - {}", t.artist, t.title) } else { t.title.clone() };
                    if t.is_overlay() {
                        title = format!("~ {title}");
                    }
                    tp.text(pos2(tx, rect.top() + 35.0), Align2::LEFT_CENTER, title, FontId::proportional(19.0), WHITE);
                    let mut meta = if t.genre.is_empty() { t.kind.clone() } else { format!("{} | {}", t.kind, t.genre) };
                    if t.is_overlay() {
                        meta = format!("{} over next song: {}", t.kind, t.overlay_pos);
                    }
                    tp.text(pos2(tx, rect.top() + 55.0), Align2::LEFT_CENTER, meta, FontId::proportional(11.0), Color32::from_gray(150));

                    if r_play.clicked() {
                        actions.push(Action::PlayIndex(i));
                    } else if r_hp.clicked() {
                        actions.push(Action::PreviewTrack(t.path.clone(), t.label(), t.duration_ms));
                    } else if r_tr.clicked() {
                        actions.push(Action::RemoveLog(i));
                    } else if resp.double_clicked() {
                        actions.push(Action::PlayIndex(i));
                    } else if resp.clicked() {
                        actions.push(Action::Select(i));
                    }
                }
            });

            let (crect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 84.0), Sense::hover());
            let bw = 96.0;
            let at = |n: f32| Rect::from_min_size(pos2(crect.left() + n * bw, crect.top()), vec2(bw - 2.0, 84.0));
            if kit::button_at(ui, at(0.0), "c_up", Some(Icon::Up), "Up", kit::DARK, WHITE).clicked() {
                actions.push(Action::MoveSel(-1));
            }
            if kit::button_at(ui, at(1.0), "c_down", Some(Icon::Down), "Down", kit::DARK, WHITE).clicked() {
                actions.push(Action::MoveSel(1));
            }
            if kit::button_at(ui, at(2.0), "c_top", Some(Icon::Top), "Top", kit::DARK, WHITE).clicked() {
                actions.push(Action::SelTop);
            }
            if kit::button_at(ui, at(3.0), "c_bottom", Some(Icon::Bottom), "Bottom", kit::DARK, WHITE).clicked() {
                actions.push(Action::SelBottom);
            }
            let mx = crect.left() + 4.0 * bw;
            let mw = crect.right() - mx;
            let mu = Rect::from_min_size(pos2(mx, crect.top()), vec2(mw, 41.0));
            let md = Rect::from_min_size(pos2(mx, crect.top() + 43.0), vec2(mw, 41.0));
            if kit::button_at(ui, mu, "c_mup", Some(Icon::MoveUp), "Move Up", kit::DARK, WHITE).clicked() {
                actions.push(Action::MoveItemUp);
            }
            if kit::button_at(ui, md, "c_mdown", Some(Icon::MoveDown), "Move Down", kit::DARK, WHITE).clicked() {
                actions.push(Action::MoveItemDown);
            }
        });

        // ---- central info (right-most) ----
        if layout == "classic" {
            egui::SidePanel::right("central").exact_width(250.0).resizable(false).frame(egui::Frame::none().fill(Color32::from_rgb(14, 14, 16)).inner_margin(6.0)).show(ctx, |ui| {
                ui.label(egui::RichText::new(tr("History")).strong().size(15.0).color(kit::GRAY));
                egui::ScrollArea::vertical().id_salt("hist").max_height(ui.available_height() * 0.3).auto_shrink([false, false]).show(ui, |ui| {
                    for (tm, label) in self.history.iter() {
                        ui.label(egui::RichText::new(format!("{tm}  {label}")).size(12.0).color(Color32::from_gray(210)));
                    }
                });
                ui.separator();
                ui.label(egui::RichText::new(tr("Upcoming events")).strong().size(15.0).color(kit::GRAY));
                egui::ScrollArea::vertical().id_salt("events").max_height(ui.available_height() * 0.5).auto_shrink([false, false]).show(ui, |ui| {
                    if breaks.is_empty() {
                        ui.label(egui::RichText::new(tr("No ad breaks ahead")).size(12.0).color(kit::GRAY));
                    }
                    for (tm, items, tot) in breaks.iter() {
                        ui.label(egui::RichText::new(format!("{tm}  Ad break")).strong().size(13.0).color(kit::WARN));
                        for it in items {
                            ui.label(egui::RichText::new(format!("  - {it}")).size(12.0));
                        }
                        ui.label(egui::RichText::new(format!("  Total ~{}", fmt_ms(*tot))).size(11.0).color(kit::GRAY));
                        ui.add_space(4.0);
                    }
                });
                ui.separator();
                ui.label(egui::RichText::new(tr("Problems")).strong().size(15.0).color(kit::GRAY));
                if problems.is_empty() {
                    ui.label(egui::RichText::new(tr("None")).size(12.0).color(kit::GOOD));
                }
                for pr in problems.iter() {
                    ui.label(egui::RichText::new(pr).size(12.0).color(kit::BAD));
                }
                ui.separator();
                ui.label(egui::RichText::new(tr("Memos")).strong().size(15.0).color(kit::GRAY));
                egui::ScrollArea::vertical().id_salt("memos").auto_shrink([false, false]).show(ui, |ui| {
                    for m in self.data.memos.iter().take(8) {
                        ui.label(egui::RichText::new(&m.title).strong().size(12.0));
                        ui.label(egui::RichText::new(&m.body).size(11.0).color(Color32::from_gray(190)));
                    }
                });
            });
        }

        // ---- instant jingles (pallets) ----
        if layout != "wide" {
            let pallet = self.data.pallets.get(self.pallet_idx).cloned();
            let page = self.page;
            egui::SidePanel::right("jingles").exact_width(380.0).resizable(false).frame(egui::Frame::none().fill(Color32::from_rgb(10, 10, 14)).inner_margin(4.0)).show(ctx, |ui| {
                let p = ui.painter().clone();
                let w = ui.available_width();
                let (hr, _) = ui.allocate_exact_size(vec2(w, 54.0), Sense::hover());
                let pname = pallet.as_ref().map(|p| p.name.clone()).unwrap_or_else(|| "(no pallet)".into());
                if kit::button_at(ui, Rect::from_min_size(hr.min, vec2(24.0, 20.0)), "pal_prev", None, "<", kit::DARK, WHITE).clicked() {
                    actions.push(Action::PalletStep(-1));
                }
                if kit::button_at(ui, Rect::from_min_size(hr.min + vec2(26.0, 0.0), vec2(24.0, 20.0)), "pal_next", None, ">", kit::DARK, WHITE).clicked() {
                    actions.push(Action::PalletStep(1));
                }
                p.text(pos2(hr.left() + 58.0, hr.top() + 10.0), Align2::LEFT_CENTER, format!("Instant Jingles: {pname}"), FontId::proportional(13.0), kit::GRAY);
                let (name, rem) = match &jn {
                    Some((_, title, rem)) => (title.clone(), *rem),
                    None => ("-".to_string(), 0),
                };
                ui.painter_at(Rect::from_min_max(hr.min + vec2(0.0, 22.0), pos2(hr.right() - 90.0, hr.bottom()))).text(pos2(hr.left() + 2.0, hr.top() + 38.0), Align2::LEFT_CENTER, name, FontId::proportional(22.0), WHITE);
                p.text(pos2(hr.right() - 2.0, hr.top() + 36.0), Align2::RIGHT_CENTER, fmt_dur(rem), FontId::proportional(34.0), Color32::from_rgb(0, 220, 40));

                let ctrl_h = 64.0;
                let grid_h = (ui.available_height() - ctrl_h - 6.0).max(100.0);
                let (grect, _) = ui.allocate_exact_size(vec2(w, grid_h), Sense::hover());
                if let Some(pal) = &pallet {
                    let (rows, cols) = (pal.rows.max(1), pal.cols.max(1));
                    let gap = 3.0;
                    let cw = (grect.width() - (cols as f32 - 1.0) * gap) / cols as f32;
                    let ch = (grect.height() - (rows as f32 - 1.0) * gap) / rows as f32;
                    for k in 0..rows * cols {
                        let (row, col) = (k / cols, k % cols);
                        let rect = Rect::from_min_size(pos2(grect.left() + col as f32 * (cw + gap), grect.top() + row as f32 * (ch + gap)), vec2(cw, ch));
                        let gi = page * rows * cols + k;
                        let btn = pal.buttons.get(gi).cloned().unwrap_or_default();
                        let cp = ui.painter_at(rect);
                        let resp = ui.interact(rect, ui.id().with(("jbtn", k)), Sense::click());
                        let track = if btn.track_id != 0 { self.data.track(btn.track_id) } else { None };
                        if let Some(t) = track {
                            let base = if btn.color == [0, 0, 0] { kit::BLUE } else { Color32::from_rgb(btn.color[0], btn.color[1], btn.color[2]) };
                            let playing = matches!(&jn, Some((id, _, _)) if *id == t.id);
                            let fill = if playing {
                                kit::lighten(base, 60)
                            } else if resp.is_pointer_button_down_on() {
                                base.gamma_multiply(0.7)
                            } else if resp.hovered() {
                                kit::lighten(base, 22)
                            } else {
                                base
                            };
                            cp.rect_filled(rect, 0.0, fill);
                            let label = if btn.label.is_empty() { t.title.to_uppercase() } else { btn.label.to_uppercase() };
                            let galley = cp.layout(label, FontId::proportional(14.0), WHITE, rect.width() - 12.0);
                            let gpos = pos2(rect.center().x - galley.size().x / 2.0, rect.center().y - galley.size().y / 2.0 - 6.0);
                            cp.galley(gpos, galley, WHITE);
                            let dl = match &jn {
                                Some((id, _, rem)) if *id == t.id => fmt_dur(*rem),
                                _ => fmt_dur(t.duration_ms),
                            };
                            cp.text(pos2(rect.left() + 6.0, rect.bottom() - 9.0), Align2::LEFT_CENTER, dl, FontId::proportional(11.0), Color32::from_gray(225));
                            if resp.clicked() {
                                actions.push(Action::Jingle(t.id));
                            }
                        } else {
                            cp.rect_filled(rect, 0.0, if resp.hovered() { Color32::from_rgb(30, 30, 36) } else { Color32::from_rgb(20, 20, 24) });
                        }
                        resp.context_menu(|ui| {
                            if ui.button(tr("Assign selected library item")).clicked() {
                                actions.push(Action::AssignBtn(gi));
                                ui.close_menu();
                            }
                            if ui.button(tr("Clear button")).clicked() {
                                actions.push(Action::ClearBtn(gi));
                                ui.close_menu();
                            }
                            let mut c = if btn.color == [0, 0, 0] { [33u8, 102, 240] } else { btn.color };
                            if ui.color_edit_button_srgb(&mut c).changed() {
                                actions.push(Action::BtnColor(gi, c));
                            }
                        });
                    }
                }

                ui.add_space(6.0);
                let (crect, _) = ui.allocate_exact_size(vec2(w, ctrl_h), Sense::hover());
                let pages = pallet.as_ref().map(|p| p.pages.max(1)).unwrap_or(1).min(5);
                let bw = (crect.width() - 5.0 * 3.0) / 6.0;
                for b in 0..pages {
                    let rr = Rect::from_min_size(pos2(crect.left() + b as f32 * (bw + 3.0), crect.top()), vec2(bw, ctrl_h));
                    let fill = if page == b { Color32::from_rgb(52, 56, 70) } else { kit::DARK };
                    if kit::button_at(ui, rr, &format!("page{b}"), None, &format!("{}", b + 1), fill, WHITE).clicked() {
                        actions.push(Action::Page(b));
                    }
                }
                let rr = Rect::from_min_size(pos2(crect.left() + 5.0 * (bw + 3.0), crect.top()), vec2(bw, ctrl_h));
                if kit::button_at(ui, rr, "stopall", Some(Icon::Stop), "Stop All", Color32::from_rgb(120, 30, 30), WHITE).clicked() {
                    actions.push(Action::StopJingles);
                }
            });
        }

        // ---- database (centre) ----
        egui::CentralPanel::default().frame(egui::Frame::none().fill(kit::PANEL).inner_margin(4.0)).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(tr("Database")).size(14.0).color(kit::GRAY));
                if self.mode == Mode::Auto && ui.small_button(tr("Rebuild upcoming auto items")).clicked() {
                    actions.push(Action::RegenAuto);
                }
            });
            let p = ui.painter().clone();
            let tabs: [(&'static str, &str, Color32); 7] = [
                ("music", "Songs", Color32::from_rgb(60, 160, 230)),
                ("spot", "Spots", Color32::from_rgb(60, 200, 90)),
                ("jingle", "Jingles", Color32::from_rgb(230, 200, 60)),
                ("asset", "Assets", Color32::from_rgb(150, 150, 150)),
                ("sweeper", "Sweepers", Color32::from_rgb(180, 90, 220)),
                ("voice", "Voice", Color32::from_rgb(210, 60, 60)),
                ("memo", "Memos", Color32::from_rgb(230, 140, 40)),
            ];
            let (trect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 46.0), Sense::hover());
            let tw = trect.width() / 7.0;
            for (i, (k, lbl, col)) in tabs.iter().enumerate() {
                let rr = Rect::from_min_size(pos2(trect.left() + i as f32 * tw, trect.top()), vec2(tw - 2.0, 46.0));
                let fill = if self.lib_kind == *k { kit::CONTROL_ACTIVE } else { kit::CONTROL };
                if kit::button_at(ui, rr, &format!("tab{i}"), None, lbl, fill, WHITE).clicked() {
                    actions.push(Action::Tab(*k));
                }
                p.rect_filled(Rect::from_min_size(pos2(rr.left(), rr.bottom() - 4.0), vec2(rr.width(), 4.0)), 0.0, *col);
            }
            ui.add_space(4.0);

            let body_h = (ui.available_height() - 112.0).max(120.0);
            let is_memo = self.lib_kind == "memo";
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(vec2(84.0, body_h), egui::Layout::top_down(egui::Align::LEFT), |ui| {
                    let btn = |ui: &mut egui::Ui, id: &str, icon: Icon, label: &str, fill: Color32| -> bool {
                        let (rr, _) = ui.allocate_exact_size(vec2(80.0, 66.0), Sense::hover());
                        let r = kit::button_at(ui, rr, id, Some(icon), label, fill, WHITE).clicked();
                        ui.add_space(3.0);
                        r
                    };
                    if btn(ui, "l_add", Icon::Plus, "Add", Color32::from_rgb(30, 110, 50)) {
                        actions.push(Action::AddSel);
                    }
                    if btn(ui, "l_ins", Icon::Insert, "Insert", kit::DARK) {
                        actions.push(Action::InsertSel);
                    }
                    if btn(ui, "l_del", Icon::Trash, "Delete", Color32::from_rgb(110, 30, 30)) {
                        actions.push(Action::DeleteSel);
                    }
                    if btn(ui, "l_clear", Icon::Clear, "Clear log", kit::DARK) {
                        actions.push(Action::ClearLog);
                    }
                });
                ui.allocate_ui_with_layout(vec2(ui.available_width(), body_h), egui::Layout::top_down(egui::Align::LEFT), |ui| {
                    let w = ui.available_width();
                    let cols = [0.0, 0.30 * w, 0.68 * w, 0.78 * w, w];
                    let (hr, _) = ui.allocate_exact_size(vec2(w, 22.0), Sense::hover());
                    ui.painter_at(hr).rect_filled(hr, 0.0, kit::CONTROL_HOVER);
                    let heads = if is_memo { ["Category", "Title", "", ""] } else { ["Artist", "Title", "Dur", "Category"] };
                    for (k, name) in heads.iter().enumerate() {
                        kit::cell_text(ui, hr, hr.left() + cols[k], hr.left() + cols[k + 1], name, 12.5, kit::GRAY);
                    }
                    let f = self.filter.to_lowercase();
                    if is_memo {
                        let items: Vec<&Memo> = self.data.memos.iter().filter(|m| f.is_empty() || m.title.to_lowercase().contains(&f)).collect();
                        egui::ScrollArea::vertical().id_salt("memotable").auto_shrink([false, false]).max_height(body_h - 26.0).show(ui, |ui| {
                            for (r, m) in items.iter().enumerate() {
                                let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::click());
                                let bg = if self.lib_sel == Some(m.id) { Color32::from_rgb(90, 45, 110) } else if r % 2 == 0 { Color32::from_rgb(44, 42, 20) } else { Color32::from_rgb(38, 36, 16) };
                                ui.painter_at(rect).rect_filled(rect, 0.0, bg);
                                kit::cell_text(ui, rect, rect.left() + cols[0], rect.left() + cols[1], &m.category, 13.0, WHITE);
                                kit::cell_text(ui, rect, rect.left() + cols[1], rect.right(), &m.title, 13.0, WHITE);
                                if resp.clicked() {
                                    actions.push(Action::LibSelect(m.id));
                                }
                            }
                        });
                    } else {
                        let idx: Vec<usize> = self
                            .data
                            .tracks
                            .iter()
                            .enumerate()
                            .filter(|(_, t)| t.kind == self.lib_kind)
                            .filter(|(_, t)| {
                                f.is_empty()
                                    || t.title.to_lowercase().contains(&f)
                                    || (self.super_search && (t.artist.to_lowercase().contains(&f) || t.genre.to_lowercase().contains(&f) || t.cat1.to_lowercase().contains(&f)))
                            })
                            .map(|(i, _)| i)
                            .collect();
                        egui::ScrollArea::vertical().id_salt("libtable").auto_shrink([false, false]).max_height(body_h - 26.0).show_rows(ui, 24.0, idx.len(), |ui, range| {
                            for r in range {
                                let t = &self.data.tracks[idx[r]];
                                let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::click());
                                let selected = self.lib_sel == Some(t.id);
                                let bg = if selected {
                                    Color32::from_rgb(90, 45, 110)
                                } else if !t.enabled {
                                    Color32::from_rgb(50, 20, 20)
                                } else if r % 2 == 0 {
                                    Color32::from_rgb(44, 42, 20)
                                } else {
                                    Color32::from_rgb(38, 36, 16)
                                };
                                ui.painter_at(rect).rect_filled(rect, 0.0, bg);
                                kit::cell_text(ui, rect, rect.left() + cols[0], rect.left() + cols[1], &t.artist, 13.0, WHITE);
                                kit::cell_text(ui, rect, rect.left() + cols[1], rect.left() + cols[2], &t.title, 13.0, WHITE);
                                kit::cell_text(ui, rect, rect.left() + cols[2], rect.left() + cols[3], &fmt_ms(t.duration_ms), 13.0, WHITE);
                                kit::cell_text(ui, rect, rect.left() + cols[3], rect.left() + cols[4], &t.cat1, 13.0, Color32::from_gray(190));
                                if resp.double_clicked() {
                                    actions.push(Action::LibSelect(t.id));
                                    actions.push(Action::AddSel);
                                } else if resp.clicked() {
                                    actions.push(Action::LibSelect(t.id));
                                }
                            }
                        });
                    }
                });
            });

            ui.add_space(4.0);
            if is_memo {
                if let Some(m) = self.lib_sel.and_then(|id| self.data.memos.iter().find(|m| m.id == id)) {
                    ui.label(egui::RichText::new(&m.body).size(12.0));
                }
            }
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.filter).hint_text(tr("Search...")).desired_width((ui.available_width() - 190.0).max(60.0)));
                ui.checkbox(&mut self.super_search, tr("Artist + genre too"));
            });
            ui.add_space(4.0);

            let (prect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 44.0), Sense::hover());
            let pp = ui.painter_at(prect);
            pp.rect_filled(prect, 0.0, Color32::from_rgb(26, 26, 30));
            if kit::button_at(ui, Rect::from_min_size(prect.min, vec2(54.0, 44.0)), "pv_play", Some(Icon::Play), "", kit::GREEN, WHITE).clicked() {
                actions.push(Action::PreviewSel);
            }
            if kit::button_at(ui, Rect::from_min_size(prect.min + vec2(56.0, 0.0), vec2(54.0, 44.0)), "pv_stop", Some(Icon::Stop), "", kit::DARK, WHITE).clicked() {
                actions.push(Action::StopPreview);
            }
            let bar = Rect::from_min_max(pos2(prect.left() + 120.0, prect.top() + 8.0), pos2(prect.right() - 8.0, prect.bottom() - 8.0));
            pp.rect_filled(bar, 2.0, Color32::from_rgb(60, 20, 20));
            if pv_active && self.preview_dur > 0 {
                let fr = (pv_el as f32 / self.preview_dur as f32).min(1.0);
                pp.rect_filled(Rect::from_min_size(bar.min, vec2(bar.width() * fr, bar.height())), 2.0, Color32::from_rgb(200, 40, 40));
            }
            if pv_active {
                pp.text(bar.center(), Align2::CENTER_CENTER, format!("{}   {} / {}", self.preview_name, fmt_ms(pv_el), fmt_ms(self.preview_dur)), FontId::proportional(12.0), WHITE);
            } else {
                pp.text(bar.center(), Align2::CENTER_CENTER, tr("Preview (select an item, press play)"), FontId::proportional(12.0), kit::GRAY);
            }
        });

        self.about_window(ctx);
        ctx.style_mut(|st| st.visuals.override_text_color = if self.cp_open { Some(Color32::WHITE) } else { None });
        kit::cp_text_sizes(ctx, self.cp_open);
        if self.cp_open {
            self.control_panel(ctx);
        }

        for a in actions {
            self.handle(a);
        }
        let _ = engine::dstr(now.date_naive());
    }
}

impl App {
    fn ensure_logo(&mut self, ctx: &egui::Context) {
        if self.about_tex.is_none() {
            if let Ok(img) = image::load_from_memory(include_bytes!("../packaging/icon.png")) {
                let rgba = img.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];
                self.about_tex = Some(ctx.load_texture("about_logo", egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()), egui::TextureOptions::LINEAR));
            }
        }
    }

    /// About window (same layout as the one of FM Analyzer).
    fn about_window(&mut self, ctx: &egui::Context) {
        if !self.about_open {
            return;
        }
        self.ensure_logo(ctx);
        let (mut open, mut close) = (true, false);
        let dim = kit::TEXT_DIM;
        egui::Window::new(egui::RichText::new(tr("About")).strong())
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .fixed_size(vec2(440.0, 0.0))
            .frame(egui::Frame::window(&ctx.style()).fill(kit::CARD).stroke(Stroke::new(1.0_f32, kit::ACCENT.gamma_multiply(0.5))).inner_margin(egui::Margin::same(22.0)))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    if let Some(t) = &self.about_tex {
                        ui.add(egui::Image::new((t.id(), vec2(88.0, 88.0))));
                    }
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("RADIOMOON").size(28.0).strong().color(kit::TEXT));
                    ui.label(egui::RichText::new(format!("Version {VERSION}")).monospace().color(kit::GOOD));
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(tr("Radio automation for Windows, Linux and macOS")).color(dim));
                });
                ui.add_space(14.0);
                ui.separator();
                ui.add_space(10.0);
                ui.label(egui::RichText::new(tr("CREATED BY")).small().strong().color(kit::ACCENT));
                ui.add_space(4.0);
                egui::Grid::new("about_credits").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
                    ui.label(egui::RichText::new("Nikos Batsikas").size(15.0).strong().color(kit::TEXT));
                    ui.label(egui::RichText::new(tr("Author - idea, design, testing")).color(dim));
                    ui.end_row();
                    ui.label(egui::RichText::new("Claude").size(15.0).strong().color(kit::TEXT));
                    ui.label(egui::RichText::new(tr("AI pair programmer by Anthropic")).color(dim));
                    ui.end_row();
                });
                ui.add_space(12.0);
                ui.label(egui::RichText::new(tr("FEATURES")).small().strong().color(kit::ACCENT));
                ui.add_space(2.0);
                for line in [
                    "Auto schedule with clocks, playlists, breaks and force clocks",
                    "Crossfade player, instant jingles, live rebroadcast",
                    "Multi-server streaming with song titles",
                    "Jazler RadioStar 2 database import",
                ] {
                    ui.label(egui::RichText::new(format!("\u{2022}  {}", tr(line))).color(kit::TEXT));
                }
                ui.add_space(12.0);
                ui.label(egui::RichText::new(tr("PROJECT")).small().strong().color(kit::ACCENT));
                ui.add_space(2.0);
                ui.hyperlink_to("github.com/nikosbatsikas/radiomoon", "https://github.com/nikosbatsikas/radiomoon");
                ui.label(egui::RichText::new("Written in Rust - egui / eframe - rodio - MySQL / MariaDB").size(11.5).color(dim));
                ui.label(egui::RichText::new("\u{A9} 2026 Nikos Batsikas").size(11.5).color(dim));
                ui.add_space(14.0);
                egui::Frame::none().fill(kit::INSET).stroke(Stroke::new(1.0_f32, kit::BORDER)).rounding(6.0).inner_margin(egui::Margin::same(12.0)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(tr("Like RadioMoon?")).strong().color(kit::TEXT));
                            ui.label(egui::RichText::new(tr("Send a gift to the author.")).size(11.5).color(dim));
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let btn = egui::Button::new(egui::RichText::new(tr("Gift with PayPal")).strong().size(13.5).color(Color32::from_rgb(0x00, 0x30, 0x87)))
                                .fill(Color32::from_rgb(0xFF, 0xC4, 0x39))
                                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(0x00, 0x70, 0xBA)))
                                .rounding(14.0)
                                .min_size(vec2(170.0, 30.0));
                            if ui.add(btn).on_hover_text("paypal.me/nikosbatsikas").clicked() {
                                ui.ctx().open_url(egui::OpenUrl::new_tab("https://paypal.me/nikosbatsikas"));
                            }
                        });
                    });
                });
                ui.add_space(14.0);
                ui.vertical_centered(|ui| {
                    if ui.add(egui::Button::new(egui::RichText::new(tr("CLOSE")).strong().color(Color32::WHITE)).fill(kit::BLUE).min_size(vec2(110.0, 28.0))).clicked() {
                        close = true;
                    }
                });
            });
        if !open || close {
            self.about_open = false;
        }
    }
}

/// F10: full screen on / off for the window that has the keyboard focus.
fn toggle_fullscreen(ctx: &egui::Context) {
    let fs = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
    ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fs));
}

impl App {
    /// Everything that keeps the broadcast going and does not draw anything: used by the window and by the headless server.
    pub fn playout_tick(&mut self) {
        let t = Instant::now();
        if let Some(p) = self.player.as_mut() {
            p.tick();
        }
        if t.elapsed() > Duration::from_millis(800) {
            eprintln!("slow step: player tick {} ms", t.elapsed().as_millis());
        }
        if self.scan_done.swap(false, Ordering::SeqCst) {
            self.reload_tracks();
        }
        if let Some((_, _, t0, d)) = &self.jingle_now {
            if t0.elapsed().as_millis() as u64 >= *d {
                self.jingle_now = None;
            }
        }

        // ---- playout control ----
        if self.current.is_some() && self.mode != Mode::Offline {
            let (pos, empty, paused) = self
                .player
                .as_ref()
                .map(|p| (p.pos_ms(), p.newest_empty(), p.is_paused()))
                .unwrap_or((0, true, false));
            let due = !paused && (empty || self.trigger_ms().map(|t| pos >= t).unwrap_or(false));
            let t_due = Instant::now();
            if due {
                if self.loop_current {
                    let c = self.current.unwrap_or(0);
                    self.play_index(c, 0);
                } else if self.stop_next {
                    self.stop();
                    self.stop_next = false;
                    self.status = tr("Stopped after track");
                } else if self.mode == Mode::Auto {
                    self.advance(0);
                } else {
                    self.stop();
                    self.status = tr("Manual mode: stopped at end of track");
                }
                if t_due.elapsed() > Duration::from_millis(800) {
                    eprintln!("slow step: start of the next item {} ms", t_due.elapsed().as_millis());
                }
            }
        }
        if self.last_fill.elapsed() > Duration::from_secs(2) {
            self.last_fill = Instant::now();
            let t = Instant::now();
            self.auto_fill();
            let a = t.elapsed();
            if self.mode == Mode::Auto {
                self.expand_pending();
            }
            let b = t.elapsed();
            self.check_immediate();
            self.auto_start();
            if t.elapsed() > Duration::from_millis(800) {
                eprintln!("slow step: auto_fill {} ms, expand_pending {} ms, check_immediate+auto_start {} ms (log {} items, library {} items)", a.as_millis(), (b - a).as_millis(), (t.elapsed() - b).as_millis(), self.log.len(), self.data.tracks.len());
            }
        }
        let t = Instant::now();
        self.auto_backup_tick();
        self.live_tick();
        if t.elapsed() > Duration::from_millis(800) {
            eprintln!("slow step: backup/live {} ms", t.elapsed().as_millis());
        }
    }
}
