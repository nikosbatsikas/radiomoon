//! Two-step login (MFA): time based one-time codes (TOTP, RFC 6238: SHA-1, 6 digits, 30 s) for the authenticator apps
//! (Google Authenticator, Microsoft Authenticator, Authy, 2FAS, ...). No external crate: SHA-1, HMAC and Base32 are here.

const B32: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

pub fn base32_encode(data: &[u8]) -> String {
    let (mut out, mut buf, mut bits) = (String::new(), 0u32, 0u32);
    for &b in data {
        buf = (buf << 8) | b as u32;
        bits += 8;
        while bits >= 5 {
            out.push(B32[((buf >> (bits - 5)) & 31) as usize] as char);
            bits -= 5;
        }
    }
    if bits > 0 {
        out.push(B32[((buf << (5 - bits)) & 31) as usize] as char);
    }
    out
}

pub fn base32_decode(s: &str) -> Option<Vec<u8>> {
    let (mut out, mut buf, mut bits) = (Vec::new(), 0u32, 0u32);
    for c in s.chars() {
        if c == ' ' || c == '-' || c == '=' {
            continue;
        }
        let v = B32.iter().position(|&x| x as char == c.to_ascii_uppercase())? as u32;
        buf = (buf << 5) | v;
        bits += 5;
        if bits >= 8 {
            out.push(((buf >> (bits - 8)) & 255) as u8);
            bits -= 8;
        }
    }
    Some(out)
}

fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&((data.len() as u64) * 8).to_be_bytes());
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([chunk[i * 4], chunk[i * 4 + 1], chunk[i * 4 + 2], chunk[i * 4 + 3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6u32),
            };
            let t = a.rotate_left(5).wrapping_add(f).wrapping_add(e).wrapping_add(k).wrapping_add(*wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = t;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }
    let mut out = [0u8; 20];
    for i in 0..5 {
        out[i * 4..i * 4 + 4].copy_from_slice(&h[i].to_be_bytes());
    }
    out
}

fn hmac_sha1(key: &[u8], msg: &[u8]) -> [u8; 20] {
    let mut k = if key.len() > 64 { sha1(key).to_vec() } else { key.to_vec() };
    k.resize(64, 0);
    let mut inner: Vec<u8> = k.iter().map(|b| b ^ 0x36).collect();
    inner.extend_from_slice(msg);
    let ih = sha1(&inner);
    let mut outer: Vec<u8> = k.iter().map(|b| b ^ 0x5c).collect();
    outer.extend_from_slice(&ih);
    sha1(&outer)
}

/// The 6 digit code of the time step `step` (unix time / 30).
pub fn code_at(secret: &[u8], step: u64) -> u32 {
    let h = hmac_sha1(secret, &step.to_be_bytes());
    let o = (h[19] & 15) as usize;
    let n = u32::from_be_bytes([h[o] & 0x7f, h[o + 1], h[o + 2], h[o + 3]]);
    n % 1_000_000
}

/// Is `code` (6 digits) right for the Base32 `secret` at the unix time `now`? The step before and after are accepted too
/// (the clock of the phone is never exactly the same).
pub fn verify(secret_b32: &str, code: &str, now: u64) -> bool {
    let code = code.trim();
    if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let Some(secret) = base32_decode(secret_b32) else { return false };
    if secret.is_empty() {
        return false;
    }
    let want: u32 = code.parse().unwrap_or(u32::MAX);
    let step = now / 30;
    (step.saturating_sub(1)..=step + 1).any(|s| code_at(&secret, s) == want)
}

#[allow(dead_code)] // the server checks the sign-in
/// Splits a typed password at the end into (password, code): the code is the last 6 digits.
pub fn split_code(typed: &str) -> Option<(&str, &str)> {
    if typed.len() > 6 && typed.is_char_boundary(typed.len() - 6) {
        let (p, c) = typed.split_at(typed.len() - 6);
        if c.bytes().all(|b| b.is_ascii_digit()) {
            return Some((p, c));
        }
    }
    None
}

/// A new random secret (160 bits, Base32).
pub fn new_secret() -> String {
    use rand::Rng;
    let mut b = [0u8; 20];
    rand::thread_rng().fill(&mut b);
    base32_encode(&b)
}

/// The address an authenticator app reads (as a QR code or typed in).
pub fn otpauth_uri(user: &str, issuer: &str, secret: &str) -> String {
    let enc = |s: &str| -> String {
        s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
    };
    format!("otpauth://totp/{}:{}?secret={}&issuer={}&algorithm=SHA1&digits=6&period=30", enc(issuer), enc(user), secret, enc(issuer))
}

pub fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rfc6238_vectors() {
        let s = b"12345678901234567890";
        assert_eq!(code_at(s, 59 / 30), 287082);
        assert_eq!(code_at(s, 1111111109 / 30), 81804);
        assert_eq!(code_at(s, 2000000000 / 30), 279037);
    }
    #[test]
    fn base32_roundtrip_and_verify() {
        let s = base32_encode(b"12345678901234567890");
        assert_eq!(s, "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ");
        assert_eq!(base32_decode(&s).unwrap(), b"12345678901234567890");
        assert!(verify(&s, "287082", 59));
        assert!(verify(&s, "287082", 59 + 30));
        assert!(!verify(&s, "287082", 59 + 120));
        assert!(!verify(&s, "12345", 59));
        assert_eq!(split_code("secret287082"), Some(("secret", "287082")));
        assert_eq!(split_code("287082"), None);
    }
}
