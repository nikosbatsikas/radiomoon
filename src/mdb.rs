//! Minimal reader for Microsoft Access 2000/2002/2003 (.mdb, Jet 4) files. Standard library only.
//! Used by the importer for databases of other radio automation programs.
#![allow(dead_code)]
use std::collections::HashMap;
use std::path::Path;

const PS: usize = 4096;

#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
    /// Date/time as days since 1899-12-30.
    Date(f64),
    Bin(Vec<u8>),
}

impl Val {
    pub fn text(&self) -> String {
        match self {
            Val::Text(s) => s.trim().to_string(),
            Val::Int(i) => i.to_string(),
            Val::Float(f) => f.to_string(),
            _ => String::new(),
        }
    }
    pub fn int(&self) -> i64 {
        match self {
            Val::Int(i) => *i,
            Val::Float(f) => *f as i64,
            Val::Bool(b) => *b as i64,
            Val::Text(s) => s.trim().parse().unwrap_or(0),
            _ => 0,
        }
    }
    pub fn float(&self) -> f64 {
        match self {
            Val::Int(i) => *i as f64,
            Val::Float(f) => *f,
            Val::Date(f) => *f,
            Val::Text(s) => s.trim().replace(',', ".").parse().unwrap_or(0.0),
            _ => 0.0,
        }
    }
    pub fn bool(&self) -> bool {
        match self {
            Val::Bool(b) => *b,
            Val::Int(i) => *i != 0,
            _ => false,
        }
    }
    pub fn is_null(&self) -> bool {
        matches!(self, Val::Null)
    }
}

#[derive(Clone, Debug)]
pub struct Col {
    pub name: String,
    typ: u8,
    num: usize,
    var: usize,
    fixed: bool,
    fixoff: usize,
    len: usize,
}

#[derive(Clone, Debug)]
pub struct Table {
    pub name: String,
    pg: u32,
    pub cols: Vec<Col>,
}

pub type Row = HashMap<String, Val>;

pub struct Mdb {
    data: Vec<u8>,
    npages: usize,
    /// table definition page -> its data pages
    owned: HashMap<u32, Vec<u32>>,
}

fn u16_at(b: &[u8], o: usize) -> usize {
    if o + 2 > b.len() {
        return 0;
    }
    u16::from_le_bytes([b[o], b[o + 1]]) as usize
}
fn u32_at(b: &[u8], o: usize) -> u32 {
    if o + 4 > b.len() {
        return 0;
    }
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

impl Mdb {
    pub fn open(path: &Path) -> Result<Mdb, String> {
        let data = std::fs::read(path).map_err(|e| e.to_string())?;
        if data.len() < PS * 3 || &data[4..8] != b"Stan" {
            return Err("This is not an Access database (.mdb)".into());
        }
        if data[0x14] != 1 {
            return Err("Only Access 2000-2003 files (.mdb, Jet 4) are supported. Open the file in Access and save it as an Access 2000 database.".into());
        }
        let npages = data.len() / PS;
        let mut owned: HashMap<u32, Vec<u32>> = HashMap::new();
        for pg in 1..npages {
            let p = &data[pg * PS..(pg + 1) * PS];
            if p[0] == 1 {
                owned.entry(u32_at(p, 4)).or_default().push(pg as u32);
            }
        }
        Ok(Mdb { data, npages, owned })
    }

    fn page(&self, n: usize) -> &[u8] {
        if n >= self.npages {
            return &[];
        }
        &self.data[n * PS..(n + 1) * PS]
    }

    fn tdef(&self, name: &str, pg: u32) -> Option<Table> {
        let mut buf = self.page(pg as usize).to_vec();
        if buf.is_empty() || buf[0] != 2 {
            return None;
        }
        let mut next = u32_at(&buf, 4);
        let mut guard = 0;
        while next != 0 && guard < 64 {
            let p = self.page(next as usize);
            if p.len() < 8 {
                break;
            }
            buf.extend_from_slice(&p[8..]);
            next = u32_at(p, 4);
            guard += 1;
        }
        let ncols = u16_at(&buf, 45);
        let nridx = u32_at(&buf, 51) as usize;
        let mut pos = 63 + nridx * 12;
        let mut cols = Vec::with_capacity(ncols);
        for _ in 0..ncols {
            if pos + 25 > buf.len() {
                return None;
            }
            let c = &buf[pos..pos + 25];
            cols.push(Col { name: String::new(), typ: c[0], num: u16_at(c, 5), var: u16_at(c, 7), fixed: c[15] & 1 != 0, fixoff: u16_at(c, 21), len: u16_at(c, 23) });
            pos += 25;
        }
        for c in cols.iter_mut() {
            let n = u16_at(&buf, pos);
            if pos + 2 + n > buf.len() {
                return None;
            }
            let units: Vec<u16> = buf[pos + 2..pos + 2 + n].chunks_exact(2).map(|x| u16::from_le_bytes([x[0], x[1]])).collect();
            c.name = String::from_utf16_lossy(&units);
            pos += 2 + n;
        }
        cols.sort_by_key(|c| c.num);
        Some(Table { name: name.to_string(), pg, cols })
    }

    /// Names of all user tables.
    pub fn table_names(&self) -> Vec<String> {
        let mut v: Vec<String> = self.catalog().into_iter().map(|x| x.0).collect();
        v.sort();
        v
    }

    fn catalog(&self) -> Vec<(String, u32)> {
        let Some(cat) = self.tdef("MSysObjects", 2) else { return Vec::new() };
        let mut out = Vec::new();
        for r in self.rows(&cat) {
            let typ = r.get("Type").map(|v| v.int()).unwrap_or(0);
            let name = r.get("Name").map(|v| v.text()).unwrap_or_default();
            let flags = r.get("Flags").map(|v| v.int()).unwrap_or(0) as u32;
            let id = r.get("Id").map(|v| v.int()).unwrap_or(0) as u32;
            if typ == 1 && !name.is_empty() && !name.starts_with("MSys") && flags & 0x8000_0002 == 0 {
                out.push((name, id & 0x00FF_FFFF));
            }
        }
        // Some databases (MegaMix) have table definitions whose catalog row is not readable: they are found by their first column.
        for pg in 3..self.npages as u32 {
            if out.iter().any(|x| x.1 == pg) || self.page(pg as usize).first() != Some(&2) {
                continue;
            }
            if let Some(t) = self.tdef("?", pg) {
                let name = match t.cols.first().map(|c| c.name.as_str()) {
                    Some("MediaID") if t.cols.len() > 20 => "Media",
                    Some("PlayListItemID") => "PlayListItems",
                    _ => continue,
                };
                if !out.iter().any(|x| x.0 == name) {
                    out.push((name.to_string(), pg));
                }
            }
        }
        out
    }

    pub fn table(&self, name: &str) -> Option<Table> {
        let (n, pg) = self.catalog().into_iter().find(|x| x.0.eq_ignore_ascii_case(name))?;
        self.tdef(&n, pg)
    }

    fn row_at(&self, ptr: u32) -> Option<&[u8]> {
        let (pg, ri) = ((ptr >> 8) as usize, (ptr & 0xFF) as usize);
        let p = self.page(pg);
        if p.len() < PS || ri >= u16_at(p, 12) {
            return None;
        }
        let o = u16_at(p, 14 + ri * 2);
        if o & 0x8000 != 0 {
            return None;
        }
        let start = o & 0x1FFF;
        let end = if ri == 0 { PS } else { u16_at(p, 14 + (ri - 1) * 2) & 0x1FFF };
        if start >= end || end > PS {
            return None;
        }
        Some(&p[start..end])
    }

    fn lval(&self, d: &[u8]) -> Vec<u8> {
        if d.len() < 12 {
            return Vec::new();
        }
        let raw = u32_at(d, 0);
        let (flag, ln) = ((raw >> 24) as u8, (raw & 0x00FF_FFFF) as usize);
        if flag & 0x80 != 0 {
            return d[12..(12 + ln).min(d.len())].to_vec();
        }
        let mut ptr = u32_at(d, 4);
        if flag & 0x40 != 0 {
            return self.row_at(ptr).map(|r| r[..ln.min(r.len())].to_vec()).unwrap_or_default();
        }
        let mut out = Vec::new();
        let mut guard = 0;
        while ptr != 0 && guard < 100_000 {
            let Some(r) = self.row_at(ptr) else { break };
            ptr = u32_at(r, 0);
            out.extend_from_slice(&r[4..]);
            guard += 1;
        }
        out.truncate(ln);
        out
    }

    fn decode_text(b: &[u8]) -> String {
        if b.len() >= 2 && b[0] == 0xFF && b[1] == 0xFE {
            let mut out: Vec<u16> = Vec::new();
            let mut comp = true;
            let mut i = 2;
            while i < b.len() {
                if comp {
                    if b[i] == 0 {
                        comp = false;
                    } else {
                        out.push(b[i] as u16);
                    }
                    i += 1;
                } else if b[i] == 0 {
                    comp = true;
                    i += 1;
                } else if i + 1 < b.len() {
                    out.push(b[i] as u16 | ((b[i + 1] as u16) << 8));
                    i += 2;
                } else {
                    break;
                }
            }
            return String::from_utf16_lossy(&out);
        }
        let units: Vec<u16> = b.chunks_exact(2).map(|x| u16::from_le_bytes([x[0], x[1]])).collect();
        String::from_utf16_lossy(&units)
    }

    fn parse_row(&self, t: &Table, r: &[u8]) -> Option<Row> {
        if r.len() < 4 {
            return None;
        }
        let ncols = u16_at(r, 0);
        let nullsz = (ncols + 7) / 8;
        if r.len() < nullsz + 4 {
            return None;
        }
        let nullmask = &r[r.len() - nullsz..];
        let vend = r.len() - nullsz - 2;
        let nvar = u16_at(r, vend);
        let mut offs = Vec::with_capacity(nvar + 1);
        for i in 0..=nvar {
            let p = vend.checked_sub(2 + 2 * i)?;
            offs.push(u16_at(r, p));
        }
        let mut res = Row::with_capacity(t.cols.len());
        for c in &t.cols {
            let n = c.num;
            let v = if n >= ncols {
                Val::Null
            } else {
                let notnull = nullmask[n / 8] & (1 << (n % 8)) != 0;
                if c.typ == 1 {
                    Val::Bool(notnull)
                } else if !notnull {
                    Val::Null
                } else if c.fixed {
                    let o = 2 + c.fixoff;
                    let need = match c.typ {
                        2 => 1,
                        3 => 2,
                        4 | 6 => 4,
                        5 | 7 | 8 => 8,
                        _ => c.len.max(1),
                    };
                    if o + need > r.len() {
                        Val::Null
                    } else {
                        match c.typ {
                            2 => Val::Int(r[o] as i64),
                            3 => Val::Int(i16::from_le_bytes([r[o], r[o + 1]]) as i64),
                            4 => Val::Int(i32::from_le_bytes([r[o], r[o + 1], r[o + 2], r[o + 3]]) as i64),
                            5 => Val::Float(i64::from_le_bytes(r[o..o + 8].try_into().unwrap()) as f64 / 10000.0),
                            6 => Val::Float(f32::from_le_bytes([r[o], r[o + 1], r[o + 2], r[o + 3]]) as f64),
                            7 => Val::Float(f64::from_le_bytes(r[o..o + 8].try_into().unwrap())),
                            8 => Val::Date(f64::from_le_bytes(r[o..o + 8].try_into().unwrap())),
                            _ => Val::Bin(r[o..o + need].to_vec()),
                        }
                    }
                } else if c.var >= nvar {
                    Val::Null
                } else {
                    let (a, b) = (offs[c.var], offs[c.var + 1]);
                    if a > b || b > r.len() {
                        Val::Null
                    } else {
                        let d = &r[a..b];
                        match c.typ {
                            10 => Val::Text(Self::decode_text(d)),
                            12 => Val::Text(Self::decode_text(&self.lval(d))),
                            11 => Val::Bin(self.lval(d)),
                            _ => Val::Bin(d.to_vec()),
                        }
                    }
                }
            };
            res.insert(c.name.clone(), v);
        }
        Some(res)
    }

    /// All rows of a table.
    pub fn rows(&self, t: &Table) -> Vec<Row> {
        let mut out = Vec::new();
        let Some(pages) = self.owned.get(&t.pg) else { return out };
        for &pg in pages {
            let p = self.page(pg as usize);
            let n = u16_at(p, 12);
            for ri in 0..n {
                let o = u16_at(p, 14 + ri * 2);
                if o & 0x8000 != 0 {
                    continue;
                }
                let row: Option<&[u8]> = if o & 0x4000 != 0 {
                    self.row_at(u32_at(p, o & 0x1FFF))
                } else {
                    let start = o & 0x1FFF;
                    let end = if ri == 0 { PS } else { u16_at(p, 14 + (ri - 1) * 2) & 0x1FFF };
                    if start < end && end <= PS { Some(&p[start..end]) } else { None }
                };
                if let Some(r) = row {
                    if let Some(x) = self.parse_row(t, r) {
                        out.push(x);
                    }
                }
            }
        }
        out
    }
}

/// Date value (days since 1899-12-30) to "YYYY-MM-DD HH:MM:SS"; empty for the Access "no date" values.
pub fn date_text(days: f64) -> String {
    if days < 2.0 {
        return String::new();
    }
    let secs = ((days - days.floor()) * 86400.0).round() as i64;
    // civil date from the day number (days since 1899-12-30 = 1970-01-01 minus 25569)
    let z = days.floor() as i64 - 25569 + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", y, m, d, secs / 3600, (secs / 60) % 60, secs % 60)
}
