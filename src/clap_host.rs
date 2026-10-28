//! Hosting of CLAP plugins (cleveraudio.org): effects that process the on-air mix before it reaches the sound card and the internet
//! streams. The plugin interface (a C ABI) is declared here; plugins are loaded from the usual CLAP folders or from any file.
//! Effects are edited with their own parameters (generic sliders); the plugin windows are not hosted.
use crate::app::App;
use libloading::Library;
use rodio::Source;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::ffi::{c_char, c_void, CStr, CString};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const BLOCK: usize = 512;
const RATE: f64 = 44100.0;

// ------------------------------------------------------------------ the CLAP C interface (the parts that are used)
#[repr(C)]
#[derive(Clone, Copy)]
struct ClapVersion {
    major: u32,
    minor: u32,
    revision: u32,
}
const VERSION: ClapVersion = ClapVersion { major: 1, minor: 2, revision: 0 };

#[repr(C)]
struct PluginEntry {
    clap_version: ClapVersion,
    init: unsafe extern "C" fn(*const c_char) -> bool,
    deinit: unsafe extern "C" fn(),
    get_factory: unsafe extern "C" fn(*const c_char) -> *const c_void,
}

#[repr(C)]
struct PluginDescriptor {
    clap_version: ClapVersion,
    id: *const c_char,
    name: *const c_char,
    vendor: *const c_char,
    url: *const c_char,
    manual_url: *const c_char,
    support_url: *const c_char,
    version: *const c_char,
    description: *const c_char,
    features: *const *const c_char,
}

#[repr(C)]
struct PluginFactory {
    get_plugin_count: unsafe extern "C" fn(*const PluginFactory) -> u32,
    get_plugin_descriptor: unsafe extern "C" fn(*const PluginFactory, u32) -> *const PluginDescriptor,
    create_plugin: unsafe extern "C" fn(*const PluginFactory, *const ClapHost, *const c_char) -> *const ClapPlugin,
}

#[repr(C)]
struct ClapHost {
    clap_version: ClapVersion,
    host_data: *mut c_void,
    name: *const c_char,
    vendor: *const c_char,
    url: *const c_char,
    version: *const c_char,
    get_extension: unsafe extern "C" fn(*const ClapHost, *const c_char) -> *const c_void,
    request_restart: unsafe extern "C" fn(*const ClapHost),
    request_process: unsafe extern "C" fn(*const ClapHost),
    request_callback: unsafe extern "C" fn(*const ClapHost),
}

#[repr(C)]
struct ClapPlugin {
    desc: *const PluginDescriptor,
    plugin_data: *mut c_void,
    init: unsafe extern "C" fn(*const ClapPlugin) -> bool,
    destroy: unsafe extern "C" fn(*const ClapPlugin),
    activate: unsafe extern "C" fn(*const ClapPlugin, f64, u32, u32) -> bool,
    deactivate: unsafe extern "C" fn(*const ClapPlugin),
    start_processing: unsafe extern "C" fn(*const ClapPlugin) -> bool,
    stop_processing: unsafe extern "C" fn(*const ClapPlugin),
    reset: unsafe extern "C" fn(*const ClapPlugin),
    process: unsafe extern "C" fn(*const ClapPlugin, *const Process) -> i32,
    get_extension: unsafe extern "C" fn(*const ClapPlugin, *const c_char) -> *const c_void,
    on_main_thread: unsafe extern "C" fn(*const ClapPlugin),
}

/// clap.gui (a floating window the plugin opens itself)
#[repr(C)]
struct PluginGui {
    is_api_supported: Option<unsafe extern "C" fn(*const ClapPlugin, *const c_char, bool) -> bool>,
    get_preferred_api: Option<unsafe extern "C" fn(*const ClapPlugin, *mut *const c_char, *mut bool) -> bool>,
    create: Option<unsafe extern "C" fn(*const ClapPlugin, *const c_char, bool) -> bool>,
    destroy: Option<unsafe extern "C" fn(*const ClapPlugin)>,
    set_scale: Option<unsafe extern "C" fn(*const ClapPlugin, f64) -> bool>,
    get_size: Option<unsafe extern "C" fn(*const ClapPlugin, *mut u32, *mut u32) -> bool>,
    can_resize: Option<unsafe extern "C" fn(*const ClapPlugin) -> bool>,
    get_resize_hints: Option<unsafe extern "C" fn(*const ClapPlugin, *mut c_void) -> bool>,
    adjust_size: Option<unsafe extern "C" fn(*const ClapPlugin, *mut u32, *mut u32) -> bool>,
    set_size: Option<unsafe extern "C" fn(*const ClapPlugin, u32, u32) -> bool>,
    set_parent: Option<unsafe extern "C" fn(*const ClapPlugin, *const c_void) -> bool>,
    set_transient: Option<unsafe extern "C" fn(*const ClapPlugin, *const c_void) -> bool>,
    suggest_title: Option<unsafe extern "C" fn(*const ClapPlugin, *const c_char)>,
    show: Option<unsafe extern "C" fn(*const ClapPlugin) -> bool>,
    hide: Option<unsafe extern "C" fn(*const ClapPlugin) -> bool>,
}

#[repr(C)]
struct AudioBuffer {
    data32: *mut *mut f32,
    data64: *mut *mut f64,
    channel_count: u32,
    latency: u32,
    constant_mask: u64,
}

#[repr(C)]
struct EventHeader {
    size: u32,
    time: u32,
    space_id: u16,
    type_: u16,
    flags: u32,
}

#[repr(C)]
struct EventParamValue {
    header: EventHeader,
    param_id: u32,
    cookie: *mut c_void,
    note_id: i32,
    port_index: i16,
    channel: i16,
    key: i16,
    value: f64,
}

#[repr(C)]
struct InputEvents {
    ctx: *mut c_void,
    size: unsafe extern "C" fn(*const InputEvents) -> u32,
    get: unsafe extern "C" fn(*const InputEvents, u32) -> *const EventHeader,
}

#[repr(C)]
struct OutputEvents {
    ctx: *mut c_void,
    try_push: unsafe extern "C" fn(*const OutputEvents, *const EventHeader) -> bool,
}

#[repr(C)]
struct Process {
    steady_time: i64,
    frames_count: u32,
    transport: *const c_void,
    audio_inputs: *const AudioBuffer,
    audio_outputs: *mut AudioBuffer,
    audio_inputs_count: u32,
    audio_outputs_count: u32,
    in_events: *const InputEvents,
    out_events: *const OutputEvents,
}

#[repr(C)]
struct PluginAudioPorts {
    count: Option<unsafe extern "C" fn(*const ClapPlugin, bool) -> u32>,
    get: Option<unsafe extern "C" fn(*const ClapPlugin, u32, bool, *mut AudioPortInfo) -> bool>,
}

#[repr(C)]
struct AudioPortInfo {
    id: u32,
    name: [c_char; 256],
    flags: u32,
    channel_count: u32,
    port_type: *const c_char,
    in_place_pair: u32,
}

#[repr(C)]
struct PluginParams {
    count: Option<unsafe extern "C" fn(*const ClapPlugin) -> u32>,
    get_info: Option<unsafe extern "C" fn(*const ClapPlugin, u32, *mut ParamInfo) -> bool>,
    get_value: Option<unsafe extern "C" fn(*const ClapPlugin, u32, *mut f64) -> bool>,
    value_to_text: *const c_void,
    text_to_value: *const c_void,
    flush: *const c_void,
}

#[repr(C)]
struct ParamInfo {
    id: u32,
    flags: u32,
    cookie: *mut c_void,
    name: [c_char; 256],
    module: [c_char; 1024],
    min_value: f64,
    max_value: f64,
    default_value: f64,
}

const PROCESS_ERROR: i32 = 0;
const EVENT_PARAM_VALUE: u16 = 5;
const PARAM_STEPPED: u32 = 1;
const PARAM_HIDDEN: u32 = 1 << 2;
const PARAM_READONLY: u32 = 1 << 3;
const PARAM_BYPASS: u32 = 1 << 4;

unsafe extern "C" fn host_ext(_: *const ClapHost, _: *const c_char) -> *const c_void {
    std::ptr::null()
}
unsafe extern "C" fn host_nop(_: *const ClapHost) {}
unsafe extern "C" fn ev_size(l: *const InputEvents) -> u32 {
    let c = &*((*l).ctx as *const (*const EventParamValue, usize));
    c.1 as u32
}
unsafe extern "C" fn ev_get(l: *const InputEvents, i: u32) -> *const EventHeader {
    let c = &*((*l).ctx as *const (*const EventParamValue, usize));
    if (i as usize) < c.1 {
        c.0.add(i as usize) as *const EventHeader
    } else {
        std::ptr::null()
    }
}
unsafe extern "C" fn ev_push(_: *const OutputEvents, _: *const EventHeader) -> bool {
    true
}

fn cstr(p: *const c_char) -> String {
    if p.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(p).to_string_lossy().to_string() }
    }
}

fn arr_str(a: &[c_char]) -> String {
    let b: Vec<u8> = a.iter().take_while(|c| **c != 0).map(|c| *c as u8).collect();
    String::from_utf8_lossy(&b).to_string()
}

// ------------------------------------------------------------------ finding plugins
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PluginInfo {
    pub path: String,
    pub id: String,
    pub name: String,
    pub vendor: String,
}

static PLUGIN_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);

/// The folder of the plugins that were added from RadioMoon (uploaded to the server, or copied here).
pub fn set_plugin_dir(base: &Path) -> PathBuf {
    let d = base.join("plugins").join("clap");
    if let Ok(mut g) = PLUGIN_DIR.lock() {
        *g = Some(d.clone());
    }
    d
}

fn search_dirs() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = Vec::new();
    if let Ok(g) = PLUGIN_DIR.lock() {
        if let Some(d) = g.as_ref() {
            v.push(d.clone());
        }
    }
    if let Some(p) = std::env::var_os("CLAP_PATH") {
        v.extend(std::env::split_paths(&p));
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if cfg!(target_os = "linux") {
        if let Some(h) = &home {
            v.push(h.join(".clap"));
        }
        v.push("/usr/lib/clap".into());
        v.push("/usr/lib64/clap".into());
        v.push("/usr/local/lib/clap".into());
    } else if cfg!(target_os = "macos") {
        v.push("/Library/Audio/Plug-Ins/CLAP".into());
        if let Some(h) = &home {
            v.push(h.join("Library/Audio/Plug-Ins/CLAP"));
        }
    } else if cfg!(windows) {
        if let Some(c) = std::env::var_os("COMMONPROGRAMFILES") {
            v.push(PathBuf::from(c).join("CLAP"));
        }
        if let Some(l) = std::env::var_os("LOCALAPPDATA") {
            v.push(PathBuf::from(l).join("Programs").join("Common").join("CLAP"));
        }
    }
    v
}

/// The file to open for a plugin path (a macOS .clap is a folder with the program inside).
fn binary_of(p: &Path) -> PathBuf {
    if p.is_dir() {
        let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let inner = p.join("Contents").join("MacOS").join(&stem);
        if inner.exists() {
            return inner;
        }
    }
    p.to_path_buf()
}

static SCAN: Mutex<Option<Vec<PluginInfo>>> = Mutex::new(None);
static SCANNING: AtomicBool = AtomicBool::new(false);

/// Looks at every plugin file of the CLAP folders (each file is opened once to read its plugins).
pub fn scan() -> Vec<PluginInfo> {
    if let Ok(mut g) = SCAN_ERRS.lock() {
        g.clear();
    }
    let mut found: Vec<PluginInfo> = Vec::new();
    for d in search_dirs() {
        for e in walkdir::WalkDir::new(&d).max_depth(3).into_iter().filter_map(|e| e.ok()) {
            let p = e.path();
            if p.extension().map(|x| x.to_string_lossy().to_lowercase() == "clap").unwrap_or(false) {
                found.extend(list_file(p));
            }
        }
    }
    found.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    if let Ok(mut g) = SCAN.lock() {
        *g = Some(found.clone());
    }
    found
}

pub fn scanned() -> Option<Vec<PluginInfo>> {
    SCAN.lock().ok().and_then(|g| g.clone())
}

static SCAN_ERRS: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn scan_err(path: &Path, m: &str) {
    let n = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    if let Ok(mut g) = SCAN_ERRS.lock() {
        g.push(format!("{n}: {m}"));
    }
}

/// The plugins of one file.
pub fn list_file(path: &Path) -> Vec<PluginInfo> {
    let mut out = Vec::new();
    let file = binary_of(path);
    unsafe {
        let lib = match Library::new(&file) {
            Ok(l) => l,
            Err(e) => {
                scan_err(path, &format!("cannot be opened on this computer ({e}). Is it built for this system (Linux / Windows / macOS, 64-bit, same C library)?"));
                return out;
            }
        };
        let Ok(sym) = lib.get::<*const PluginEntry>(b"clap_entry\0") else {
            scan_err(path, "is not a CLAP plugin (no clap_entry)");
            return out;
        };
        let entry: *const PluginEntry = *sym;
        if entry.is_null() {
            scan_err(path, "is not a CLAP plugin");
            return out;
        }
        let Ok(cpath) = CString::new(path.to_string_lossy().as_bytes()) else { return out };
        if !((*entry).init)(cpath.as_ptr()) {
            scan_err(path, "the plugin did not start");
            return out;
        }
        let f = ((*entry).get_factory)(b"clap.plugin-factory\0".as_ptr() as *const c_char) as *const PluginFactory;
        if !f.is_null() {
            let n = ((*f).get_plugin_count)(f);
            for i in 0..n {
                let d = ((*f).get_plugin_descriptor)(f, i);
                if !d.is_null() {
                    out.push(PluginInfo { path: path.to_string_lossy().to_string(), id: cstr((*d).id), name: cstr((*d).name), vendor: cstr((*d).vendor) });
                }
            }
        }
        ((*entry).deinit)();
    }
    out
}

// ------------------------------------------------------------------ a loaded plugin
struct Inner {
    plugin: *const ClapPlugin,
    entry: *const PluginEntry,
    #[allow(dead_code)]
    host: Box<ClapHost>,
    activated: bool,
    processing: bool,
    in_ports: Vec<u32>,
    out_ports: Vec<u32>,
    params: *const PluginParams,
    gui: *const PluginGui,
    gui_made: AtomicBool,
    _lib: Library, // last: the library is closed after the plugin was destroyed
}

unsafe impl Send for Inner {}
unsafe impl Sync for Inner {}

impl Drop for Inner {
    fn drop(&mut self) {
        unsafe {
            let p = &*self.plugin;
            if self.gui_made.load(Ordering::Relaxed) && !self.gui.is_null() {
                if let Some(d) = (*self.gui).destroy {
                    d(self.plugin);
                }
            }
            if self.processing {
                (p.stop_processing)(self.plugin);
            }
            if self.activated {
                (p.deactivate)(self.plugin);
            }
            (p.destroy)(self.plugin);
            ((*self.entry).deinit)();
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ParamDesc {
    pub id: u32,
    pub name: String,
    pub module: String,
    pub min: f64,
    pub max: f64,
    pub default: f64,
    pub stepped: bool,
}

/// The side of a plugin that the program uses to show and change it (main thread).
pub struct SlotCtl {
    pub path: String,
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub enabled: Arc<AtomicBool>,
    pending: Arc<Mutex<Vec<(u32, f64)>>>,
    pub params: Vec<ParamDesc>,
    pub values: Vec<f64>,
    /// the plugin has a window of its own (floating) that can be opened
    pub gui: bool,
    _inner: Arc<Inner>,
}

impl SlotCtl {
    /// Opens the plugin's own window (it is a separate floating window, shown by the plugin).
    pub fn open_window(&self) -> Result<(), String> {
        let i = &*self._inner;
        if !self.gui || i.gui.is_null() {
            return Err("this plugin has no window".into());
        }
        unsafe {
            let g = &*i.gui;
            if !i.gui_made.load(Ordering::Relaxed) {
                let mut api: *const c_char = std::ptr::null();
                let mut fl = false;
                if let Some(f) = g.get_preferred_api {
                    f(i.plugin, &mut api, &mut fl);
                }
                let created = match g.create {
                    Some(c) => c(i.plugin, api, true),
                    None => false,
                };
                if !created {
                    return Err("the plugin could not create its window".into());
                }
                i.gui_made.store(true, Ordering::Relaxed);
                if let Some(t) = g.suggest_title {
                    let n = std::ffi::CString::new(self.name.as_bytes()).unwrap_or_default();
                    t(i.plugin, n.as_ptr());
                }
            }
            match g.show {
                Some(sh) if sh(i.plugin) => Ok(()),
                _ => Err("the plugin window could not be opened (is its window program installed?)".into()),
            }
        }
    }
}

/// The side that processes audio (audio thread).
pub struct Runner {
    inner: Arc<Inner>,
    enabled: Arc<AtomicBool>,
    pending: Arc<Mutex<Vec<(u32, f64)>>>,
    steady: i64,
    ev: Vec<EventParamValue>,
    in_store: Vec<Vec<Vec<f32>>>,
    out_store: Vec<Vec<Vec<f32>>>,
}

unsafe impl Send for Runner {}

/// Loads one plugin of a file, ready to process stereo audio at 44.1 kHz.
pub fn load(path: &str, id: &str) -> Result<(SlotCtl, Runner), String> {
    unsafe {
        let file = binary_of(Path::new(path));
        let lib = Library::new(&file).map_err(|e| format!("cannot open {path}: {e}"))?;
        let entry: *const PluginEntry = *lib.get::<*const PluginEntry>(b"clap_entry\0").map_err(|_| "this file is not a CLAP plugin".to_string())?;
        if entry.is_null() {
            return Err("this file is not a CLAP plugin".into());
        }
        let cpath = CString::new(path.as_bytes()).map_err(|e| e.to_string())?;
        if !((*entry).init)(cpath.as_ptr()) {
            return Err("the plugin did not start".into());
        }
        let fail = |m: &str| -> Result<(SlotCtl, Runner), String> {
            ((*entry).deinit)();
            Err(m.to_string())
        };
        let f = ((*entry).get_factory)(b"clap.plugin-factory\0".as_ptr() as *const c_char) as *const PluginFactory;
        if f.is_null() {
            return fail("the file has no plugin factory");
        }
        let host = Box::new(ClapHost {
            clap_version: VERSION,
            host_data: std::ptr::null_mut(),
            name: b"RadioMoon\0".as_ptr() as *const c_char,
            vendor: b"RadioMoon\0".as_ptr() as *const c_char,
            url: b"https://github.com/nikosbatsikas/radiomoon\0".as_ptr() as *const c_char,
            version: b"1\0".as_ptr() as *const c_char,
            get_extension: host_ext,
            request_restart: host_nop,
            request_process: host_nop,
            request_callback: host_nop,
        });
        let cid = CString::new(id.as_bytes()).map_err(|e| e.to_string())?;
        let plugin = ((*f).create_plugin)(f, &*host as *const ClapHost, cid.as_ptr());
        if plugin.is_null() {
            return fail("the plugin could not be created");
        }
        let mut inner = Inner { plugin, entry, host, activated: false, processing: false, in_ports: Vec::new(), out_ports: Vec::new(), params: std::ptr::null(), gui: std::ptr::null(), gui_made: AtomicBool::new(false), _lib: lib };
        let p = &*plugin;
        if !(p.init)(plugin) {
            return Err("the plugin could not be initialised".into()); // Inner::drop destroys it
        }
        // audio ports: an effect has at least one input and one output
        let ap = (p.get_extension)(plugin, b"clap.audio-ports\0".as_ptr() as *const c_char) as *const PluginAudioPorts;
        if !ap.is_null() {
            if let (Some(count), Some(get)) = ((*ap).count, (*ap).get) {
                for is_input in [true, false] {
                    for i in 0..count(plugin, is_input) {
                        let mut info: AudioPortInfo = std::mem::zeroed();
                        if get(plugin, i, is_input, &mut info) {
                            if is_input { inner.in_ports.push(info.channel_count.max(1)) } else { inner.out_ports.push(info.channel_count.max(1)) }
                        }
                    }
                }
            }
        } else {
            inner.in_ports = vec![2];
            inner.out_ports = vec![2];
        }
        if inner.in_ports.is_empty() || inner.out_ports.is_empty() {
            return Err("this plugin is not an effect (it has no audio input)".into());
        }
        inner.params = (p.get_extension)(plugin, b"clap.params\0".as_ptr() as *const c_char) as *const PluginParams;
        inner.gui = (p.get_extension)(plugin, b"clap.gui\0".as_ptr() as *const c_char) as *const PluginGui;
        let gui_ok = !inner.gui.is_null() && {
            let g = &*inner.gui;
            let mut api: *const c_char = std::ptr::null();
            let mut fl = false;
            match (g.get_preferred_api, g.is_api_supported, g.create, g.show) {
                (Some(pref), Some(sup), Some(_), Some(_)) => pref(plugin, &mut api, &mut fl) && !api.is_null() && sup(plugin, api, true),
                _ => false,
            }
        };
        if !(p.activate)(plugin, RATE, 1, BLOCK as u32) {
            return Err("the plugin could not be activated".into());
        }
        inner.activated = true;
        if !(p.start_processing)(plugin) {
            return Err("the plugin could not start processing".into());
        }
        inner.processing = true;

        // description and parameters
        let d = p.desc;
        let (name, vendor) = if d.is_null() { (id.to_string(), String::new()) } else { (cstr((*d).name), cstr((*d).vendor)) };
        let mut params = Vec::new();
        let mut values = Vec::new();
        if !inner.params.is_null() {
            if let (Some(count), Some(get_info), Some(get_value)) = ((*inner.params).count, (*inner.params).get_info, (*inner.params).get_value) {
                for i in 0..count(plugin) {
                    let mut pi: ParamInfo = std::mem::zeroed();
                    if get_info(plugin, i, &mut pi) && pi.flags & (PARAM_HIDDEN | PARAM_READONLY | PARAM_BYPASS) == 0 {
                        let mut v = pi.default_value;
                        get_value(plugin, pi.id, &mut v);
                        params.push(ParamDesc { id: pi.id, name: arr_str(&pi.name), module: arr_str(&pi.module), min: pi.min_value, max: pi.max_value, default: pi.default_value, stepped: pi.flags & PARAM_STEPPED != 0 });
                        values.push(v);
                    }
                }
            }
        }
        let inner = Arc::new(inner);
        let enabled = Arc::new(AtomicBool::new(true));
        let pending = Arc::new(Mutex::new(Vec::new()));
        let mk = |ports: &Vec<u32>| -> Vec<Vec<Vec<f32>>> { ports.iter().map(|c| (0..*c).map(|_| vec![0.0f32; BLOCK]).collect()).collect() };
        let runner = Runner { inner: inner.clone(), enabled: enabled.clone(), pending: pending.clone(), steady: 0, ev: Vec::new(), in_store: mk(&inner.in_ports), out_store: mk(&inner.out_ports) };
        let slot = SlotCtl { path: path.to_string(), id: id.to_string(), name, vendor, enabled, pending, params, values, gui: gui_ok, _inner: inner };
        Ok((slot, runner))
    }
}

impl Runner {
    /// Processes a block of stereo audio in place (at most BLOCK frames).
    fn run(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len()).min(BLOCK);
        if n == 0 {
            return;
        }
        self.ev.clear();
        if let Ok(mut q) = self.pending.try_lock() {
            for (id, v) in q.drain(..) {
                self.ev.push(EventParamValue {
                    header: EventHeader { size: std::mem::size_of::<EventParamValue>() as u32, time: 0, space_id: 0, type_: EVENT_PARAM_VALUE, flags: 0 },
                    param_id: id,
                    cookie: std::ptr::null_mut(),
                    note_id: -1,
                    port_index: -1,
                    channel: -1,
                    key: -1,
                    value: v,
                });
            }
        }
        // the signal goes to the first input port; the other inputs are silent
        for (pi, port) in self.in_store.iter_mut().enumerate() {
            for (ci, ch) in port.iter_mut().enumerate() {
                if pi == 0 && ci == 0 {
                    if port_channels(&self.inner.in_ports, 0) == 1 {
                        for i in 0..n {
                            ch[i] = (l[i] + r[i]) * 0.5;
                        }
                    } else {
                        ch[..n].copy_from_slice(&l[..n]);
                    }
                } else if pi == 0 && ci == 1 {
                    ch[..n].copy_from_slice(&r[..n]);
                } else {
                    for x in ch[..n].iter_mut() {
                        *x = 0.0;
                    }
                }
            }
        }
        let mut in_ptrs: Vec<Vec<*mut f32>> = self.in_store.iter_mut().map(|p| p.iter_mut().map(|c| c.as_mut_ptr()).collect()).collect();
        let mut out_ptrs: Vec<Vec<*mut f32>> = self.out_store.iter_mut().map(|p| p.iter_mut().map(|c| c.as_mut_ptr()).collect()).collect();
        let in_bufs: Vec<AudioBuffer> = in_ptrs.iter_mut().map(|p| AudioBuffer { data32: p.as_mut_ptr(), data64: std::ptr::null_mut(), channel_count: p.len() as u32, latency: 0, constant_mask: 0 }).collect();
        let mut out_bufs: Vec<AudioBuffer> = out_ptrs.iter_mut().map(|p| AudioBuffer { data32: p.as_mut_ptr(), data64: std::ptr::null_mut(), channel_count: p.len() as u32, latency: 0, constant_mask: 0 }).collect();
        let list: (*const EventParamValue, usize) = (self.ev.as_ptr(), self.ev.len());
        let inev = InputEvents { ctx: &list as *const (*const EventParamValue, usize) as *mut c_void, size: ev_size, get: ev_get };
        let outev = OutputEvents { ctx: std::ptr::null_mut(), try_push: ev_push };
        let pr = Process {
            steady_time: self.steady,
            frames_count: n as u32,
            transport: std::ptr::null(),
            audio_inputs: in_bufs.as_ptr(),
            audio_outputs: out_bufs.as_mut_ptr(),
            audio_inputs_count: in_bufs.len() as u32,
            audio_outputs_count: out_bufs.len() as u32,
            in_events: &inev,
            out_events: &outev,
        };
        self.steady += n as i64;
        let status = unsafe { ((*self.inner.plugin).process)(self.inner.plugin, &pr) };
        if status == PROCESS_ERROR {
            return; // the signal goes through unchanged
        }
        let o = &self.out_store[0];
        if o.len() >= 2 {
            l[..n].copy_from_slice(&o[0][..n]);
            r[..n].copy_from_slice(&o[1][..n]);
        } else {
            l[..n].copy_from_slice(&o[0][..n]);
            r[..n].copy_from_slice(&o[0][..n]);
        }
    }
}

fn port_channels(ports: &[u32], i: usize) -> u32 {
    ports.get(i).copied().unwrap_or(2)
}

// ------------------------------------------------------------------ the chain in the audio path
pub enum ChainCmd {
    Add(Runner),
    Remove(usize),
    Swap(usize, usize),
}

/// The controls of the effect chain (main thread).
pub struct ChainCtl {
    tx: Sender<ChainCmd>,
    pub slots: Vec<SlotCtl>,
    /// what the chain was built from (to know when the settings changed)
    pub sig: String,
}

impl ChainCtl {
    pub fn add(&mut self, slot: SlotCtl, runner: Runner) {
        let _ = self.tx.send(ChainCmd::Add(runner));
        self.slots.push(slot);
    }
    pub fn remove(&mut self, i: usize) {
        if i < self.slots.len() {
            let _ = self.tx.send(ChainCmd::Remove(i));
            self.slots.remove(i);
        }
    }
    pub fn swap(&mut self, a: usize, b: usize) {
        if a < self.slots.len() && b < self.slots.len() {
            let _ = self.tx.send(ChainCmd::Swap(a, b));
            self.slots.swap(a, b);
        }
    }
    pub fn clear(&mut self) {
        while !self.slots.is_empty() {
            self.remove(0);
        }
    }
    pub fn set_param(&mut self, slot: usize, id: u32, v: f64) {
        if let Some(s) = self.slots.get_mut(slot) {
            if let Ok(mut q) = s.pending.lock() {
                q.push((id, v));
            }
            if let Some(i) = s.params.iter().position(|p| p.id == id) {
                s.values[i] = v;
            }
        }
    }
    pub fn set_enabled(&mut self, slot: usize, on: bool) {
        if let Some(s) = self.slots.get(slot) {
            s.enabled.store(on, Ordering::Relaxed);
        }
    }
}

pub fn chain() -> (ChainCtl, Receiver<ChainCmd>) {
    let (tx, rx) = channel();
    (ChainCtl { tx, slots: Vec::new(), sig: String::new() }, rx)
}

/// Wraps the on-air mix: the audio goes through the plugins of the chain (nothing changes while the chain is empty).
pub struct ClapSource<S: Source<Item = f32>> {
    inner: S,
    rx: Receiver<ChainCmd>,
    runners: Vec<Runner>,
    out: Vec<f32>,
    pos: usize,
    l: Vec<f32>,
    r: Vec<f32>,
}

impl<S: Source<Item = f32>> ClapSource<S> {
    pub fn new(inner: S, rx: Receiver<ChainCmd>) -> Self {
        ClapSource { inner, rx, runners: Vec::new(), out: Vec::with_capacity(BLOCK * 2), pos: 0, l: Vec::with_capacity(BLOCK), r: Vec::with_capacity(BLOCK) }
    }

    fn refill(&mut self) -> bool {
        while let Ok(c) = self.rx.try_recv() {
            match c {
                ChainCmd::Add(r) => self.runners.push(r),
                ChainCmd::Remove(i) => {
                    if i < self.runners.len() {
                        self.runners.remove(i);
                    }
                }
                ChainCmd::Swap(a, b) => {
                    if a < self.runners.len() && b < self.runners.len() {
                        self.runners.swap(a, b);
                    }
                }
            }
        }
        self.out.clear();
        self.pos = 0;
        if self.runners.is_empty() {
            for _ in 0..1024 {
                match self.inner.next() {
                    Some(s) => self.out.push(s),
                    None => break,
                }
            }
            return !self.out.is_empty();
        }
        self.l.clear();
        self.r.clear();
        for _ in 0..BLOCK {
            let a = self.inner.next();
            let b = self.inner.next();
            match (a, b) {
                (Some(a), Some(b)) => {
                    self.l.push(a);
                    self.r.push(b);
                }
                (Some(a), None) => {
                    self.l.push(a);
                    self.r.push(a);
                    break;
                }
                _ => break,
            }
        }
        if self.l.is_empty() {
            return false;
        }
        for run in self.runners.iter_mut() {
            if run.enabled.load(Ordering::Relaxed) {
                run.run(&mut self.l, &mut self.r);
            }
        }
        for i in 0..self.l.len() {
            self.out.push(self.l[i]);
            self.out.push(self.r[i]);
        }
        true
    }
}

impl<S: Source<Item = f32>> Iterator for ClapSource<S> {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.pos >= self.out.len() && !self.refill() {
            return None;
        }
        let v = self.out[self.pos];
        self.pos += 1;
        Some(v)
    }
}

impl<S: Source<Item = f32>> Source for ClapSource<S> {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        2
    }
    fn sample_rate(&self) -> u32 {
        44100
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

// ------------------------------------------------------------------ settings and the control panel
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ClapSlotCfg {
    pub path: String,
    pub id: String,
    pub enabled: bool,
    pub params: Vec<(u32, f64)>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ParamView {
    pub id: u32,
    pub name: String,
    pub module: String,
    pub min: f64,
    pub max: f64,
    pub default: f64,
    pub value: f64,
    pub stepped: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SlotView {
    pub name: String,
    pub vendor: String,
    pub id: String,
    pub path: String,
    pub enabled: bool,
    pub params: Vec<ParamView>,
    #[serde(default)]
    pub gui: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ClapView {
    pub plugins: Vec<PluginInfo>,
    pub scanned: bool,
    pub scanning: bool,
    pub slots: Vec<SlotView>,
    pub error: String,
}

pub enum ClapOp {
    Scan,
    Add(String, String),
    Remove(usize),
    Enable(usize, bool),
    Param(usize, u32, f64, bool),
    Move(usize, usize),
    /// open the plugin's own window (on the computer where the plugin runs)
    Window(usize),
}

impl App {
    /// Builds the chain from the settings when they are different from what runs.
    pub fn clap_apply_settings(&mut self) {
        set_plugin_dir(&self.base);
        if self.remote.is_some() {
            return;
        }
        let want = serde_json::to_string(&self.data.s.clap_chain).unwrap_or_default();
        let Some(p) = self.player.as_mut() else { return };
        if p.clap.sig == want {
            return;
        }
        p.clap.clear();
        let mut errs = Vec::new();
        for c in self.data.s.clap_chain.clone() {
            match load(&c.path, &c.id) {
                Ok((slot, runner)) => {
                    slot.enabled.store(c.enabled, Ordering::Relaxed);
                    let idx = p.clap.slots.len();
                    p.clap.add(slot, runner);
                    for (id, v) in &c.params {
                        p.clap.set_param(idx, *id, *v);
                    }
                }
                Err(e) => errs.push(format!("{}: {e}", c.id)),
            }
        }
        p.clap.sig = want;
        if !errs.is_empty() {
            self.status = format!("CLAP: {}", errs.join("; "));
        }
    }

    /// What the control panel shows: the plugins that were found and the chain (of this program, or of the server).
    pub fn clap_view(&mut self) -> ClapView {
        if let Some(r) = &self.remote {
            return match crate::remote::get_json(&r.link, "/api/clap") {
                Ok(v) => serde_json::from_value(v).unwrap_or_default(),
                Err(e) => ClapView { error: e, ..Default::default() },
            };
        }
        let mut v = ClapView::default();
        v.scanning = SCANNING.load(Ordering::Relaxed);
        if let Ok(g) = SCAN_ERRS.lock() {
            v.error = g.join("\n");
        }
        if let Some(l) = scanned() {
            v.plugins = l;
            v.scanned = true;
        }
        match self.player.as_ref() {
            Some(p) => {
                for s in &p.clap.slots {
                    v.slots.push(SlotView {
                        name: s.name.clone(),
                        vendor: s.vendor.clone(),
                        id: s.id.clone(),
                        path: s.path.clone(),
                        enabled: s.enabled.load(Ordering::Relaxed),
                        gui: s.gui,
                        params: s.params.iter().zip(s.values.iter()).map(|(d, val)| ParamView { id: d.id, name: d.name.clone(), module: d.module.clone(), min: d.min, max: d.max, default: d.default, value: *val, stepped: d.stepped }).collect(),
                    });
                }
            }
            None => v.error = "No audio output: effects need the audio engine".into(),
        }
        v
    }

    /// Adds plugin files: copied to the plugin folder of this program, or uploaded to the server.
    pub fn clap_install(&mut self, files: Vec<PathBuf>) {
        let files: Vec<PathBuf> = files.into_iter().filter(|f| f.is_file()).collect();
        if files.is_empty() {
            self.status = "Choose .clap plugin files (a folder-type plugin of macOS must be added to the CLAP folder by hand)".into();
            return;
        }
        if let Some(r) = &self.remote {
            let (base, auth) = (r.link.base.clone(), r.link.auth.clone());
            let (st, done) = (self.scan_status.clone(), self.scan_done.clone());
            std::thread::spawn(move || {
                let (mut ok, mut last) = (0, String::new());
                for (i, f) in files.iter().enumerate() {
                    if let Ok(mut s) = st.lock() {
                        *s = format!("Uploading plugin {} / {}", i + 1, files.len());
                    }
                    match crate::remote::upload_file(&base, &auth, f, "clap", None, None) {
                        Ok(()) => ok += 1,
                        Err(e) => last = e,
                    }
                }
                if let Ok(mut s) = st.lock() {
                    *s = if last.is_empty() { format!("{ok} plugin files are on the server") } else { format!("Plugin upload: {ok} done, error: {last}") };
                }
                done.store(true, Ordering::SeqCst);
            });
            return;
        }
        let dir = set_plugin_dir(&self.base);
        let _ = std::fs::create_dir_all(&dir);
        let mut ok = 0;
        for f in &files {
            if let Some(n) = f.file_name() {
                if std::fs::copy(f, dir.join(n)).is_ok() {
                    ok += 1;
                }
            }
        }
        self.status = format!("{ok} plugin files added");
        let _ = self.clap_local(ClapOp::Scan);
    }

    /// A change of the chain (in this program, or on the server).
    pub fn clap_do(&mut self, op: ClapOp) -> Result<(), String> {
        if self.remote.is_some() {
            if matches!(op, ClapOp::Window(_)) {
                return Err("The plugin window opens on the computer where the plugin runs; the server has no screen".into());
            }
            let a = match &op {
                ClapOp::Scan => json!({ "do": "clap_scan" }),
                ClapOp::Add(p, i) => json!({ "do": "clap_add", "path": p, "id": i }),
                ClapOp::Remove(i) => json!({ "do": "clap_remove", "slot": i }),
                ClapOp::Enable(i, on) => json!({ "do": "clap_enable", "slot": i, "on": on }),
                ClapOp::Param(i, id, v, save) => json!({ "do": "clap_param", "slot": i, "id": id, "value": v, "save": save }),
                ClapOp::Move(a, b) => json!({ "do": "clap_move", "from": a, "to": b }),
                ClapOp::Window(_) => return Ok(()),
            };
            let r = self.remote.as_ref().unwrap();
            return crate::remote::post_json(&r.link, &a).map(|_| ());
        }
        self.clap_local(op)
    }

    /// The same change made in this program (the server runs it for the clients).
    pub fn clap_local(&mut self, op: ClapOp) -> Result<(), String> {
        match op {
            ClapOp::Scan => {
                // opening every plugin file takes a while: in the background
                if !SCANNING.swap(true, Ordering::SeqCst) {
                    std::thread::spawn(|| {
                        scan();
                        SCANNING.store(false, Ordering::SeqCst);
                    });
                }
                Ok(())
            }
            ClapOp::Add(path, id) => {
                let (slot, runner) = load(&path, &id)?;
                let Some(p) = self.player.as_mut() else { return Err("No audio output".into()) };
                p.clap.add(slot, runner);
                self.data.s.clap_chain.push(ClapSlotCfg { path, id, enabled: true, params: Vec::new() });
                self.clap_saved();
                Ok(())
            }
            ClapOp::Remove(i) => {
                if let Some(p) = self.player.as_mut() {
                    p.clap.remove(i);
                }
                if i < self.data.s.clap_chain.len() {
                    self.data.s.clap_chain.remove(i);
                }
                self.clap_saved();
                Ok(())
            }
            ClapOp::Enable(i, on) => {
                if let Some(p) = self.player.as_mut() {
                    p.clap.set_enabled(i, on);
                }
                if let Some(c) = self.data.s.clap_chain.get_mut(i) {
                    c.enabled = on;
                }
                self.clap_saved();
                Ok(())
            }
            ClapOp::Param(i, id, v, save) => {
                if let Some(p) = self.player.as_mut() {
                    p.clap.set_param(i, id, v);
                }
                if let Some(c) = self.data.s.clap_chain.get_mut(i) {
                    match c.params.iter_mut().find(|x| x.0 == id) {
                        Some(x) => x.1 = v,
                        None => c.params.push((id, v)),
                    }
                }
                if save {
                    self.clap_saved();
                }
                Ok(())
            }
            ClapOp::Window(i) => match self.player.as_ref().and_then(|p| p.clap.slots.get(i)) {
                Some(s) => s.open_window(),
                None => Err("No such plugin".into()),
            },
            ClapOp::Move(a, b) => {
                if let Some(p) = self.player.as_mut() {
                    p.clap.swap(a, b);
                }
                if a < self.data.s.clap_chain.len() && b < self.data.s.clap_chain.len() {
                    self.data.s.clap_chain.swap(a, b);
                }
                self.clap_saved();
                Ok(())
            }
        }
    }

    /// The chain is stored in the settings; the signature says that what runs matches them.
    fn clap_saved(&mut self) {
        self.save_settings();
        let sig = serde_json::to_string(&self.data.s.clap_chain).unwrap_or_default();
        if let Some(p) = self.player.as_mut() {
            p.clap.sig = sig;
        }
    }
}

#[allow(dead_code)] // used by the server only
pub fn view_json(v: &ClapView) -> Value {
    serde_json::to_value(v).unwrap_or(Value::Null)
}
