//! Text and byte functions computed exactly on literals (`models::pure` calls them): `bytes`
//! methods and constructors, `str(b, encoding)`, `string.capwords`, `shlex`, `html`,
//! `urllib.parse` quoting, `base64` / `binascii`, `fnmatch`. Each follows CPython; anything
//! that would raise, or whose exact result is not certain here, is declined (None), so the
//! ordinary (unknown) model answers.

use crate::intern::{self, Key, KeyId};
use crate::models::pyvalues::{self, PyResult};

/// The module families handled here (the part of the qualified name before the function).
pub const FAMILIES: &[&str] = &["string", "shlex", "html", "urllib.parse", "base64", "binascii", "fnmatch", "builtins.bytes"];

/// Functions whose list arguments are sequences of literals (passed as tuple keys).
pub fn seq_arg(family: &str, name: &str) -> bool {
    matches!((family, name), ("shlex", "join") | ("fnmatch", "filter") | ("builtins", "bytes"))
}

fn s_of(k: KeyId) -> Option<String> {
    intern::key_as_str(k).map(|s| s.to_string())
}

fn b_of(k: KeyId) -> Option<Vec<u8>> {
    match intern::key_get(k) {
        Key::Bytes(b) => Some(b),
        _ => None,
    }
}

fn skey(s: &str) -> KeyId {
    intern::key_str_lit(s)
}

fn bkey(b: Vec<u8>) -> KeyId {
    intern::key(Key::Bytes(b))
}

fn is_none(k: KeyId) -> bool {
    matches!(intern::key_get(k), Key::None)
}

fn bool_of(k: KeyId) -> Option<bool> {
    match intern::key_get(k) {
        Key::Bool(b) => Some(b),
        Key::Int(i) => Some(i != 0),
        Key::None => Some(false),
        _ => None,
    }
}

fn tuple_of(k: KeyId) -> Option<Vec<KeyId>> {
    match intern::key_get(k) {
        Key::Tuple(xs) => Some(xs),
        _ => None,
    }
}

/// Positional and keyword arguments bound to `names` (None when an unknown keyword is given
/// or there are too many arguments).
fn bind(args: &[KeyId], kwargs: &[(&str, KeyId)], names: &[&str]) -> Option<Vec<Option<KeyId>>> {
    if args.len() > names.len() {
        return None;
    }
    let mut out: Vec<Option<KeyId>> = names.iter().map(|_| None).collect();
    for (i, a) in args.iter().enumerate() {
        out[i] = Some(*a);
    }
    for (k, v) in kwargs {
        let i = names.iter().position(|n| n == k)?;
        if out[i].is_some() {
            return None;
        }
        out[i] = Some(*v);
    }
    Some(out)
}

/// The codec name as CPython normalises it, for the codecs handled here.
fn codec(enc: Option<KeyId>) -> Option<&'static str> {
    let Some(e) = enc else { return Some("utf-8") };
    let e = s_of(e)?.to_ascii_lowercase().replace('_', "-");
    match e.as_str() {
        "utf-8" | "utf8" | "u8" | "utf" | "cp65001" => Some("utf-8"),
        "ascii" | "us-ascii" | "646" => Some("ascii"),
        "latin-1" | "latin1" | "iso-8859-1" | "iso8859-1" | "8859" | "cp819" | "l1" | "latin" => Some("latin-1"),
        _ => None,
    }
}

/// `b.decode(encoding, errors)`: strict decoding only (an error would raise).
pub fn decode(b: &[u8], enc: Option<KeyId>, errors: Option<KeyId>) -> Option<String> {
    if let Some(e) = errors {
        if s_of(e)? != "strict" {
            return None;
        }
    }
    match codec(enc)? {
        "utf-8" => String::from_utf8(b.to_vec()).ok(),
        "ascii" => b.iter().all(|x| *x < 128).then(|| b.iter().map(|x| *x as char).collect()),
        _ => Some(b.iter().map(|x| *x as char).collect()),
    }
}

/// `s.encode(encoding)` for the codecs handled here (strict).
fn encode(s: &str, enc: Option<KeyId>) -> Option<Vec<u8>> {
    match codec(enc)? {
        "utf-8" => Some(s.as_bytes().to_vec()),
        "ascii" => s.chars().all(|c| (c as u32) < 128).then(|| s.bytes().collect()),
        _ => s.chars().map(|c| u8::try_from(c as u32).ok()).collect(),
    }
}

const ASCII_WS: &[u8] = b" \t\n\r\x0b\x0c";

fn bytes_split_ws(b: &[u8], maxsplit: i64) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut i = 0;
    let n = b.len();
    let mut splits = 0;
    while i < n {
        while i < n && ASCII_WS.contains(&b[i]) {
            i += 1;
        }
        if i >= n {
            break;
        }
        if maxsplit >= 0 && splits >= maxsplit {
            let mut end = n;
            while end > i && ASCII_WS.contains(&b[end - 1]) {
                end -= 1;
            }
            out.push(b[i..end].to_vec());
            return out;
        }
        let start = i;
        while i < n && !ASCII_WS.contains(&b[i]) {
            i += 1;
        }
        out.push(b[start..i].to_vec());
        splits += 1;
    }
    out
}

fn bytes_find(h: &[u8], n: &[u8], from: usize) -> Option<usize> {
    if n.is_empty() {
        return (from <= h.len()).then_some(from);
    }
    (from..h.len().saturating_sub(n.len() - 1)).find(|&i| h[i..].starts_with(n))
}

fn bytes_split_sep(b: &[u8], sep: &[u8], maxsplit: i64) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut splits = 0;
    while maxsplit < 0 || splits < maxsplit {
        match bytes_find(b, sep, start) {
            Some(i) => {
                out.push(b[start..i].to_vec());
                start = i + sep.len();
                splits += 1;
            }
            None => break,
        }
    }
    out.push(b[start..].to_vec());
    out
}

fn bytes_count(h: &[u8], n: &[u8]) -> usize {
    if n.is_empty() {
        return h.len() + 1;
    }
    let mut c = 0;
    let mut i = 0;
    while let Some(j) = bytes_find(h, n, i) {
        c += 1;
        i = j + n.len();
    }
    c
}

fn bytes_replace(h: &[u8], old: &[u8], new: &[u8], count: i64) -> Option<Vec<u8>> {
    if old.is_empty() {
        return None;
    }
    let mut out = Vec::new();
    let mut i = 0;
    let mut done = 0;
    while count < 0 || done < count {
        match bytes_find(h, old, i) {
            Some(j) => {
                out.extend_from_slice(&h[i..j]);
                out.extend_from_slice(new);
                i = j + old.len();
                done += 1;
            }
            None => break,
        }
    }
    out.extend_from_slice(&h[i..]);
    Some(out)
}

fn hex_of(b: &[u8], upper: bool) -> String {
    b.iter().map(|x| if upper { format!("{:02X}", x) } else { format!("{:02x}", x) }).collect()
}

/// `bytes.fromhex(s)`: pairs of hex digits, ASCII whitespace between pairs allowed.
fn from_hex(s: &str) -> Option<Vec<u8>> {
    let cs: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < cs.len() {
        if cs[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        let hi = cs[i].to_digit(16)?;
        let lo = cs.get(i + 1)?.to_digit(16)?;
        out.push((hi * 16 + lo) as u8);
        i += 2;
    }
    Some(out)
}

fn affix_arg(k: KeyId) -> Option<Vec<Vec<u8>>> {
    match intern::key_get(k) {
        Key::Bytes(b) => Some(vec![b]),
        Key::Tuple(xs) => xs.into_iter().map(b_of).collect(),
        _ => None,
    }
}

/// A `bytes` method on a literal receiver.
pub fn bytes_method(recv: &[u8], name: &str, args: &[KeyId], kwargs: &[(&str, KeyId)]) -> Option<PyResult> {
    let key = |b: Vec<u8>| Some(PyResult::Key(bkey(b)));
    let flag = |b: bool| Some(PyResult::Key(intern::key_bool(b)));
    let list = |xs: Vec<Vec<u8>>| Some(PyResult::List(xs.into_iter().map(bkey).collect()));
    match name {
        "decode" => {
            let a = bind(args, kwargs, &["encoding", "errors"])?;
            Some(PyResult::Key(skey(&decode(recv, a[0], a[1])?)))
        }
        "hex" if args.is_empty() && kwargs.is_empty() => Some(PyResult::Key(skey(&hex_of(recv, false)))),
        "upper" if args.is_empty() => key(recv.to_ascii_uppercase()),
        "lower" if args.is_empty() => key(recv.to_ascii_lowercase()),
        "strip" | "lstrip" | "rstrip" if kwargs.is_empty() && args.len() <= 1 => {
            let set: Vec<u8> = match args.first() {
                None => ASCII_WS.to_vec(),
                Some(k) if is_none(*k) => ASCII_WS.to_vec(),
                Some(k) => b_of(*k)?,
            };
            let mut lo = 0;
            let mut hi = recv.len();
            if name != "rstrip" {
                while lo < hi && set.contains(&recv[lo]) {
                    lo += 1;
                }
            }
            if name != "lstrip" {
                while hi > lo && set.contains(&recv[hi - 1]) {
                    hi -= 1;
                }
            }
            key(recv[lo..hi].to_vec())
        }
        "startswith" | "endswith" if args.len() == 1 && kwargs.is_empty() => {
            let xs = affix_arg(args[0])?;
            flag(xs.iter().any(|x| if name == "startswith" { recv.starts_with(x) } else { recv.ends_with(x) }))
        }
        "split" => {
            let a = bind(args, kwargs, &["sep", "maxsplit"])?;
            let maxsplit = match a[1] {
                Some(k) => intern::key_as_int(k)?,
                None => -1,
            };
            match a[0] {
                None => list(bytes_split_ws(recv, maxsplit)),
                Some(k) if is_none(k) => list(bytes_split_ws(recv, maxsplit)),
                Some(k) => {
                    let sep = b_of(k)?;
                    if sep.is_empty() {
                        return None;
                    }
                    list(bytes_split_sep(recv, &sep, maxsplit))
                }
            }
        }
        "replace" if kwargs.is_empty() && (2..=3).contains(&args.len()) => {
            let count = match args.get(2) {
                Some(k) => intern::key_as_int(*k)?,
                None => -1,
            };
            key(bytes_replace(recv, &b_of(args[0])?, &b_of(args[1])?, count)?)
        }
        "count" if args.len() == 1 && kwargs.is_empty() => Some(PyResult::Key(intern::key_int(bytes_count(recv, &b_of(args[0])?) as i64))),
        "find" if args.len() == 1 && kwargs.is_empty() => Some(PyResult::Key(intern::key_int(bytes_find(recv, &b_of(args[0])?, 0).map(|i| i as i64).unwrap_or(-1)))),
        "removeprefix" if args.len() == 1 => {
            let p = b_of(args[0])?;
            key(recv.strip_prefix(p.as_slice()).unwrap_or(recv).to_vec())
        }
        "removesuffix" if args.len() == 1 => {
            let p = b_of(args[0])?;
            key(recv.strip_suffix(p.as_slice()).unwrap_or(recv).to_vec())
        }
        "join" if args.len() == 1 && kwargs.is_empty() => {
            let parts: Vec<Vec<u8>> = tuple_of(args[0])?.into_iter().map(b_of).collect::<Option<_>>()?;
            key(parts.join(recv))
        }
        "isdigit" | "isalpha" | "isalnum" | "isspace" | "isupper" | "islower" if args.is_empty() => {
            let all = |f: fn(&u8) -> bool| !recv.is_empty() && recv.iter().all(f);
            flag(match name {
                "isdigit" => all(|c| c.is_ascii_digit()),
                "isalpha" => all(|c| c.is_ascii_alphabetic()),
                "isalnum" => all(|c| c.is_ascii_alphanumeric()),
                "isspace" => all(|c| ASCII_WS.contains(c)),
                "isupper" => recv.iter().any(|c| c.is_ascii_uppercase()) && !recv.iter().any(|c| c.is_ascii_lowercase()),
                _ => recv.iter().any(|c| c.is_ascii_lowercase()) && !recv.iter().any(|c| c.is_ascii_uppercase()),
            })
        }
        _ => None,
    }
}

/// `bytes(...)`: from a str and an encoding, a length, or a sequence of ints.
pub fn bytes_new(args: &[KeyId], kwargs: &[(&str, KeyId)]) -> Option<KeyId> {
    let a = bind(args, kwargs, &["source", "encoding", "errors"])?;
    let Some(src) = a[0] else { return Some(bkey(Vec::new())) };
    match intern::key_get(src) {
        Key::Str(s) => {
            a[1]?;
            if let Some(e) = a[2] {
                if s_of(e)? != "strict" {
                    return None;
                }
            }
            Some(bkey(encode(&s, a[1])?))
        }
        _ if a[1].is_some() || a[2].is_some() => None,
        Key::Int(n) if (0..=4096).contains(&n) => Some(bkey(vec![0; n as usize])),
        Key::Bytes(b) => Some(bkey(b)),
        Key::Tuple(xs) => xs.iter().map(|x| intern::key_as_int(*x).and_then(|i| u8::try_from(i).ok())).collect::<Option<Vec<u8>>>().map(bkey),
        _ => None,
    }
}

/// `str(b, encoding[, errors])`.
pub fn str_decode(args: &[KeyId], kwargs: &[(&str, KeyId)]) -> Option<KeyId> {
    let a = bind(args, kwargs, &["object", "encoding", "errors"])?;
    let b = b_of(a[0]?)?;
    if a[1].is_none() && a[2].is_none() {
        return None;
    }
    Some(skey(&decode(&b, a[1], a[2])?))
}

// ------------------------------------------------------------------------------- shlex
/// `shlex.split(s)` (POSIX mode, no comments); None where shlex raises.
pub fn shlex_split(s: &str) -> Option<Vec<String>> {
    let cs: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut tok = String::new();
    let mut in_tok = false;
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        match c {
            ' ' | '\t' | '\n' | '\r' => {
                if in_tok {
                    out.push(std::mem::take(&mut tok));
                    in_tok = false;
                }
            }
            '\\' => {
                // outside, shlex keeps the character)
                i += 1;
                tok.push(*cs.get(i)?);
                in_tok = true;
            }
            '\'' => {
                in_tok = true;
                i += 1;
                loop {
                    let x = *cs.get(i)?;
                    if x == '\'' {
                        break;
                    }
                    tok.push(x);
                    i += 1;
                }
            }
            '"' => {
                in_tok = true;
                i += 1;
                loop {
                    let x = *cs.get(i)?;
                    if x == '"' {
                        break;
                    }
                    if x == '\\' {
                        let y = *cs.get(i + 1)?;
                        if y != '"' && y != '\\' {
                            tok.push('\\');
                        }
                        tok.push(y);
                        i += 2;
                        continue;
                    }
                    tok.push(x);
                    i += 1;
                }
            }
            _ => {
                tok.push(c);
                in_tok = true;
            }
        }
        i += 1;
    }
    if in_tok {
        out.push(tok);
    }
    Some(out)
}

/// `shlex.quote(s)`.
pub fn shlex_quote(s: &str) -> String {
    if s.is_empty() {
        return "''".into();
    }
    if s.chars().all(|c| c.is_ascii_alphanumeric() || "_@%+=:,./-".contains(c)) {
        return s.into();
    }
    format!("'{}'", s.replace('\'', "'\"'\"'"))
}

// -------------------------------------------------------------------------------- html
pub fn html_escape(s: &str, quote: bool) -> String {
    let mut t = s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    if quote {
        t = t.replace('"', "&quot;").replace('\'', "&#x27;");
    }
    t
}

/// `html.unescape(s)` for numeric references and the common named ones (each with its `;`);
/// None for anything else after a `&` that html.unescape might replace.
pub fn html_unescape(s: &str) -> Option<String> {
    const NAMED: &[(&str, &str)] = &[("amp", "&"), ("lt", "<"), ("gt", ">"), ("quot", "\""), ("apos", "'"), ("nbsp", "\u{a0}"), ("copy", "©"), ("reg", "®"), ("hellip", "…"), ("mdash", "—"), ("ndash", "–"), ("euro", "€"), ("laquo", "«"), ("raquo", "»")];
    let cs: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < cs.len() {
        if cs[i] != '&' {
            out.push(cs[i]);
            i += 1;
            continue;
        }
        let rest: String = cs[i + 1..].iter().collect();
        if let Some(num) = rest.strip_prefix('#') {
            let (hex, digits) = match num.strip_prefix(['x', 'X']) {
                Some(h) => (true, h),
                None => (false, num),
            };
            let n: String = digits.chars().take_while(|c| if hex { c.is_ascii_hexdigit() } else { c.is_ascii_digit() }).collect();
            if n.is_empty() {
                // `&#` not followed by digits stays as it is
                out.push('&');
                i += 1;
                continue;
            }
            let after = digits[n.len()..].chars().next();
            if after != Some(';') {
                return None;
            }
            let v = u32::from_str_radix(&n, if hex { 16 } else { 10 }).ok()?;
            // (C1 replacements, surrogates and out-of-range numbers follow special rules)
            if (0x80..=0x9f).contains(&v) || (0xd800..=0xdfff).contains(&v) || v == 0 || v > 0x10ffff {
                return None;
            }
            out.push(char::from_u32(v)?);
            i += 1 + 1 + usize::from(hex) + n.chars().count() + 1;
            continue;
        }
        let name: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
        if name.is_empty() {
            out.push('&');
            i += 1;
            continue;
        }
        match (NAMED.iter().find(|(n, _)| *n == name), rest[name.len()..].starts_with(';')) {
            (Some((_, v)), true) => {
                out.push_str(v);
                i += 1 + name.chars().count() + 1;
            }
            _ => return None,
        }
    }
    Some(out)
}

// ------------------------------------------------------------------------ urllib.parse
fn always_safe(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b"_.-~".contains(&b)
}

/// `quote(string, safe)` (UTF-8, strict).
pub fn quote(s: &[u8], safe: &str) -> Option<String> {
    if !safe.is_ascii() {
        return None;
    }
    let safe = safe.as_bytes();
    Some(s.iter().map(|&b| if always_safe(b) || safe.contains(&b) { (b as char).to_string() } else { format!("%{:02X}", b) }).collect())
}

pub fn quote_plus(s: &str, safe: &str) -> Option<String> {
    if s.contains(' ') {
        Some(quote(s.as_bytes(), &format!("{} ", safe))?.replace(' ', "+"))
    } else {
        quote(s.as_bytes(), safe)
    }
}

/// `unquote(string)` (UTF-8, errors="replace"): each run of %XX escapes decoded on its own.
pub fn unquote(s: &str) -> String {
    let cs: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut run: Vec<u8> = Vec::new();
    let mut i = 0;
    while i < cs.len() {
        if cs[i] == '%' && i + 2 < cs.len() && cs[i + 1].is_ascii_hexdigit() && cs[i + 2].is_ascii_hexdigit() {
            run.push((cs[i + 1].to_digit(16).unwrap() * 16 + cs[i + 2].to_digit(16).unwrap()) as u8);
            i += 3;
            continue;
        }
        if !run.is_empty() {
            out.push_str(&String::from_utf8_lossy(&run));
            run.clear();
        }
        out.push(cs[i]);
        i += 1;
    }
    if !run.is_empty() {
        out.push_str(&String::from_utf8_lossy(&run));
    }
    out
}

// ------------------------------------------------------------------------------ base64
const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn b64encode(b: &[u8], alt: Option<(u8, u8)>) -> Vec<u8> {
    let mut out = Vec::new();
    for ch in b.chunks(3) {
        let n = (ch[0] as u32) << 16 | (*ch.get(1).unwrap_or(&0) as u32) << 8 | *ch.get(2).unwrap_or(&0) as u32;
        let idx = [(n >> 18) & 63, (n >> 12) & 63, (n >> 6) & 63, n & 63];
        for (j, x) in idx.iter().enumerate() {
            if j <= ch.len() {
                let mut c = B64[*x as usize];
                if let Some((p, q)) = alt {
                    if c == b'+' {
                        c = p;
                    } else if c == b'/' {
                        c = q;
                    }
                }
                out.push(c);
            } else {
                out.push(b'=');
            }
        }
    }
    out
}

/// `b64decode(s, altchars, validate=False)`: characters outside the alphabet are discarded;
/// None where CPython raises (bad padding).
pub fn b64decode(s: &[u8], alt: Option<(u8, u8)>) -> Option<Vec<u8>> {
    let mut vals: Vec<u8> = Vec::new();
    let mut pad = 0;
    for &c0 in s {
        let c = match alt {
            Some((p, _)) if c0 == p => b'+',
            Some((_, q)) if c0 == q => b'/',
            _ => c0,
        };
        if c == b'=' {
            pad += 1;
            continue;
        }
        if let Some(v) = B64.iter().position(|x| *x == c) {
            if pad > 0 {
                // data after padding: binascii's rules are subtle
                return None;
            }
            vals.push(v as u8);
        }
    }
    if vals.len() % 4 == 1 {
        return None;
    }
    let need = (4 - vals.len() % 4) % 4;
    if pad < need {
        return None;
    }
    let mut out = Vec::new();
    for ch in vals.chunks(4) {
        let n = ch.iter().enumerate().fold(0u32, |a, (j, v)| a | (*v as u32) << (18 - 6 * j));
        let bytes = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
        out.extend_from_slice(&bytes[..ch.len() - 1]);
    }
    Some(out)
}

fn bytes_like(k: KeyId) -> Option<Vec<u8>> {
    match intern::key_get(k) {
        Key::Bytes(b) => Some(b),
        // (base64 decoders accept ASCII str)
        Key::Str(s) if s.is_ascii() => Some(s.as_bytes().to_vec()),
        _ => None,
    }
}

// ------------------------------------------------------------------------------ fnmatch
enum Pat {
    Star,
    Any,
    Lit(char),
    Class(bool, Vec<(char, char)>),
}

/// The pattern as `fnmatch.translate` reads it; None for a character class whose `-` use is
/// one of translate's subtle cases.
fn glob_parse(p: &str) -> Option<Vec<Pat>> {
    let cs: Vec<char> = p.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        i += 1;
        match c {
            '*' => {
                if !matches!(out.last(), Some(Pat::Star)) {
                    out.push(Pat::Star);
                }
            }
            '?' => out.push(Pat::Any),
            '[' => {
                let mut j = i;
                if j < cs.len() && cs[j] == '!' {
                    j += 1;
                }
                if j < cs.len() && cs[j] == ']' {
                    j += 1;
                }
                while j < cs.len() && cs[j] != ']' {
                    j += 1;
                }
                if j >= cs.len() {
                    out.push(Pat::Lit('['));
                    continue;
                }
                let mut stuff: &[char] = &cs[i..j];
                i = j + 1;
                let neg = stuff.first() == Some(&'!');
                if neg {
                    stuff = &stuff[1..];
                }
                let mut items = Vec::new();
                let mut k = 0;
                while k < stuff.len() {
                    let a = stuff[k];
                    if k + 2 < stuff.len() && stuff[k + 1] == '-' {
                        let b = stuff[k + 2];
                        if a == '-' || b == '-' || a > b {
                            return None;
                        }
                        items.push((a, b));
                        k += 3;
                    } else {
                        // a `-` only first or last (literal)
                        if a == '-' && k != 0 && k + 1 != stuff.len() {
                            return None;
                        }
                        items.push((a, a));
                        k += 1;
                    }
                }
                if items.is_empty() {
                    return None;
                }
                out.push(Pat::Class(neg, items));
            }
            _ => out.push(Pat::Lit(c)),
        }
    }
    Some(out)
}

fn glob_match(p: &[Pat], s: &[char]) -> bool {
    match p.first() {
        None => s.is_empty(),
        Some(Pat::Star) => (0..=s.len()).any(|i| glob_match(&p[1..], &s[i..])),
        Some(Pat::Any) => !s.is_empty() && glob_match(&p[1..], &s[1..]),
        Some(Pat::Lit(c)) => s.first() == Some(c) && glob_match(&p[1..], &s[1..]),
        Some(Pat::Class(neg, items)) => match s.first() {
            Some(x) => (items.iter().any(|(a, b)| a <= x && x <= b) != *neg) && glob_match(&p[1..], &s[1..]),
            None => false,
        },
    }
}

pub fn fnmatch(name: &str, pat: &str) -> Option<bool> {
    let p = glob_parse(pat)?;
    let cs: Vec<char> = name.chars().collect();
    if cs.len() > 512 {
        return None;
    }
    Some(glob_match(&p, &cs))
}

// ------------------------------------------------------------------------- dispatcher
/// A module function of one of `FAMILIES` on literal arguments.
pub fn module_call(family: &str, name: &str, args: &[KeyId], kwargs: &[(&str, KeyId)]) -> Option<PyResult> {
    let k = |x: KeyId| Some(PyResult::Key(x));
    let st = |x: String| Some(PyResult::Key(skey(&x)));
    match (family, name) {
        ("string", "capwords") => {
            let a = bind(args, kwargs, &["s", "sep"])?;
            let s = s_of(a[0]?)?;
            let sep = a[1].filter(|x| !is_none(*x));
            let parts = match pyvalues::str_method(&s, "split", &sep.into_iter().collect::<Vec<_>>(), &[])? {
                PyResult::List(ps) => ps,
                _ => return None,
            };
            let mut words = Vec::new();
            for p in parts {
                match pyvalues::str_method(&s_of(p)?, "capitalize", &[], &[])? {
                    PyResult::Key(w) => words.push(s_of(w)?),
                    _ => return None,
                }
            }
            let joiner = match sep {
                Some(x) => s_of(x)?,
                None => " ".into(),
            };
            st(words.join(&joiner))
        }
        ("shlex", "split") => {
            let a = bind(args, kwargs, &["s", "comments", "posix"])?;
            if a[1].map(|x| bool_of(x) != Some(false)).unwrap_or(false) || a[2].map(|x| bool_of(x) != Some(true)).unwrap_or(false) {
                return None;
            }
            Some(PyResult::List(shlex_split(&s_of(a[0]?)?)?.iter().map(|x| skey(x)).collect()))
        }
        ("shlex", "quote") if args.len() == 1 && kwargs.is_empty() => st(shlex_quote(&s_of(args[0])?)),
        ("shlex", "join") if args.len() == 1 && kwargs.is_empty() => {
            let parts: Vec<String> = tuple_of(args[0])?.into_iter().map(|x| s_of(x).map(|s| shlex_quote(&s))).collect::<Option<_>>()?;
            st(parts.join(" "))
        }
        ("html", "escape") => {
            let a = bind(args, kwargs, &["s", "quote"])?;
            st(html_escape(&s_of(a[0]?)?, a[1].map(bool_of).unwrap_or(Some(true))?))
        }
        ("html", "unescape") if args.len() == 1 && kwargs.is_empty() => st(html_unescape(&s_of(args[0])?)?),
        ("urllib.parse", "quote" | "quote_plus") => {
            let a = bind(args, kwargs, &["string", "safe", "encoding", "errors"])?;
            if a[2].map(|e| codec(Some(e)) != Some("utf-8")).unwrap_or(false) || a[3].map(|e| s_of(e).as_deref() != Some("strict")).unwrap_or(false) {
                return None;
            }
            let safe = match a[1] {
                Some(x) => s_of(x)?,
                None if name == "quote" => "/".into(),
                None => String::new(),
            };
            let s = a[0]?;
            match (name, intern::key_get(s)) {
                ("quote", Key::Str(t)) => st(quote(t.as_bytes(), &safe)?),
                ("quote", Key::Bytes(b)) if a[2].is_none() => st(quote(&b, &safe)?),
                ("quote_plus", Key::Str(t)) => st(quote_plus(&t, &safe)?),
                _ => None,
            }
        }
        ("urllib.parse", "unquote" | "unquote_plus") => {
            let a = bind(args, kwargs, &["string", "encoding", "errors"])?;
            if a[1].map(|e| codec(Some(e)) != Some("utf-8")).unwrap_or(false) || a[2].map(|e| s_of(e).as_deref() != Some("replace")).unwrap_or(false) {
                return None;
            }
            let s = s_of(a[0]?)?;
            st(unquote(&if name == "unquote_plus" { s.replace('+', " ") } else { s }))
        }
        ("base64", "b64encode" | "standard_b64encode" | "urlsafe_b64encode") => {
            let a = bind(args, kwargs, &["s", "altchars"])?;
            let b = b_of(a[0]?)?;
            let alt = match (name, a.get(1).copied().flatten()) {
                ("urlsafe_b64encode", None) => Some((b'-', b'_')),
                (_, None) => None,
                (_, Some(x)) if is_none(x) => None,
                ("b64encode", Some(x)) => match b_of(x)?.as_slice() {
                    [p, q] => Some((*p, *q)),
                    _ => return None,
                },
                _ => return None,
            };
            k(bkey(b64encode(&b, alt)))
        }
        ("base64", "b64decode" | "standard_b64decode" | "urlsafe_b64decode") => {
            let a = bind(args, kwargs, &["s", "altchars", "validate"])?;
            if a[2].map(|x| bool_of(x) != Some(false)).unwrap_or(false) {
                return None;
            }
            let b = bytes_like(a[0]?)?;
            let alt = match (name, a[1]) {
                ("urlsafe_b64decode", None) => Some((b'-', b'_')),
                (_, None) => None,
                (_, Some(x)) if is_none(x) => None,
                ("b64decode", Some(x)) => match bytes_like(x)?.as_slice() {
                    [p, q] => Some((*p, *q)),
                    _ => return None,
                },
                _ => return None,
            };
            k(bkey(b64decode(&b, alt)?))
        }
        ("base64", "b16encode") if args.len() == 1 && kwargs.is_empty() => k(bkey(hex_of(&b_of(args[0])?, true).into_bytes())),
        ("base64", "b16decode") if args.len() == 1 && kwargs.is_empty() => {
            let b = bytes_like(args[0])?;
            if b.iter().any(|c| c.is_ascii_lowercase()) || b.len() % 2 == 1 {
                return None;
            }
            k(bkey(from_hex(std::str::from_utf8(&b).ok()?)?))
        }
        ("binascii", "hexlify" | "b2a_hex") if args.len() == 1 && kwargs.is_empty() => k(bkey(hex_of(&b_of(args[0])?, false).into_bytes())),
        ("binascii", "unhexlify" | "a2b_hex") if args.len() == 1 && kwargs.is_empty() => {
            let b = bytes_like(args[0])?;
            if b.len() % 2 == 1 || b.iter().any(|c| !c.is_ascii_hexdigit()) {
                return None;
            }
            k(bkey(from_hex(std::str::from_utf8(&b).ok()?)?))
        }
        ("fnmatch", "fnmatch" | "fnmatchcase") if args.len() == 2 && kwargs.is_empty() => k(intern::key_bool(fnmatch(&s_of(args[0])?, &s_of(args[1])?)?)),
        ("fnmatch", "filter") if args.len() == 2 && kwargs.is_empty() => {
            let pat = s_of(args[1])?;
            let mut out = Vec::new();
            for n in tuple_of(args[0])? {
                if fnmatch(&s_of(n)?, &pat)? {
                    out.push(n);
                }
            }
            Some(PyResult::List(out))
        }
        ("builtins.bytes", "fromhex") if args.len() == 1 && kwargs.is_empty() => k(bkey(from_hex(&s_of(args[0])?)?)),
        _ => None,
    }
}

// ---------------------------------------------------------------- json errors

/// Where CPython's `json.loads(s)` fails on a text: the exception's `msg` and `pos` (a
/// character index), following `json.decoder` / `json.scanner` and the C scanner CPython
/// uses (its messages for bad escapes / control characters); None when the text parses (NaN / Infinity included).
pub fn py_json_error(s: &str) -> Option<(String, usize)> {
    let c: Vec<char> = s.chars().collect();
    if c.first() == Some(&'\u{feff}') {
        return Some(("Unexpected UTF-8 BOM (decode using utf-8-sig)".to_string(), 0));
    }
    let ws = |c: &[char], mut i: usize| {
        while i < c.len() && matches!(c[i], ' ' | '\t' | '\n' | '\r') {
            i += 1;
        }
        i
    };
    let idx = ws(&c, 0);
    let end = match json_scan(&c, idx) {
        Ok(e) => e,
        Err(JErr::Stop(i)) => return Some(("Expecting value".to_string(), i)),
        Err(JErr::At(m, i)) => return Some((m, i)),
    };
    let end = ws(&c, end);
    if end != c.len() {
        return Some(("Extra data".to_string(), end));
    }
    None
}

enum JErr {
    /// (no value starts here: the caller reports "Expecting value")
    Stop(usize),
    At(String, usize),
}

fn is_ws(c: &[char], i: usize) -> bool {
    i < c.len() && matches!(c[i], ' ' | '\t' | '\n' | '\r')
}

fn skip_ws(c: &[char], mut i: usize) -> usize {
    while is_ws(c, i) {
        i += 1;
    }
    i
}

fn starts(c: &[char], i: usize, w: &str) -> bool {
    let w: Vec<char> = w.chars().collect();
    i + w.len() <= c.len() && c[i..i + w.len()] == w[..]
}

fn json_scan(c: &[char], idx: usize) -> Result<usize, JErr> {
    let Some(&ch) = c.get(idx) else { return Err(JErr::Stop(idx)) };
    match ch {
        '"' => return json_string(c, idx + 1),
        '{' => return json_object(c, idx + 1),
        '[' => return json_array(c, idx + 1),
        'n' if starts(c, idx, "null") => return Ok(idx + 4),
        't' if starts(c, idx, "true") => return Ok(idx + 4),
        'f' if starts(c, idx, "false") => return Ok(idx + 5),
        _ => {}
    }
    // -?(0|[1-9][0-9]*)(\.[0-9]+)?([eE][-+]?[0-9]+)?
    let mut i = idx;
    if c.get(i) == Some(&'-') {
        i += 1;
    }
    let digit = |i: usize| c.get(i).is_some_and(|d| d.is_ascii_digit());
    if digit(i) {
        if c[i] == '0' {
            i += 1;
        } else {
            while digit(i) {
                i += 1;
            }
        }
        if c.get(i) == Some(&'.') && digit(i + 1) {
            i += 1;
            while digit(i) {
                i += 1;
            }
        }
        if matches!(c.get(i), Some('e' | 'E')) {
            let mut j = i + 1;
            if matches!(c.get(j), Some('+' | '-')) {
                j += 1;
            }
            if digit(j) {
                while digit(j) {
                    j += 1;
                }
                i = j;
            }
        }
        return Ok(i);
    }
    if ch == 'N' && starts(c, idx, "NaN") {
        return Ok(idx + 3);
    }
    if ch == 'I' && starts(c, idx, "Infinity") {
        return Ok(idx + 8);
    }
    if ch == '-' && starts(c, idx, "-Infinity") {
        return Ok(idx + 9);
    }
    Err(JErr::Stop(idx))
}

/// `end` is just after the opening quote.
fn json_string(c: &[char], mut end: usize) -> Result<usize, JErr> {
    let begin = end - 1;
    let unterminated = || JErr::At("Unterminated string starting at".to_string(), begin);
    loop {
        let Some(&ch) = c.get(end) else { return Err(unterminated()) };
        end += 1;
        if ch == '"' {
            return Ok(end);
        }
        if (ch as u32) < 0x20 {
            return Err(JErr::At("Invalid control character at".to_string(), end - 1));
        }
        if ch != '\\' {
            continue;
        }
        let Some(&esc) = c.get(end) else { return Err(unterminated()) };
        if esc != 'u' {
            if !matches!(esc, '"' | '\\' | '/' | 'b' | 'f' | 'n' | 'r' | 't') {
                // (the C scanner's message, at the backslash)
                return Err(JErr::At("Invalid \\escape".to_string(), end - 1));
            }
            end += 1;
        } else {
            let hex4 = |p: usize| -> Option<u32> {
                let h: String = c.get(p + 1..p + 5)?.iter().collect();
                if h.len() == 4 && h.chars().all(|x| x.is_ascii_hexdigit()) { u32::from_str_radix(&h, 16).ok() } else { None }
            };
            let Some(uni) = hex4(end) else { return Err(JErr::At("Invalid \\uXXXX escape".to_string(), end)) };
            end += 5;
            if (0xd800..=0xdbff).contains(&uni) && starts(c, end, "\\u") {
                let Some(uni2) = hex4(end + 1) else { return Err(JErr::At("Invalid \\uXXXX escape".to_string(), end + 1)) };
                if (0xdc00..=0xdfff).contains(&uni2) {
                    end += 6;
                }
            }
        }
    }
}

/// `end` is just after `{`.
fn json_object(c: &[char], mut end: usize) -> Result<usize, JErr> {
    let mut next = c.get(end).copied();
    if next != Some('"') {
        if is_ws(c, end) {
            end = skip_ws(c, end);
            next = c.get(end).copied();
        }
        if next == Some('}') {
            return Ok(end + 1);
        } else if next != Some('"') {
            return Err(JErr::At("Expecting property name enclosed in double quotes".to_string(), end));
        }
    }
    end += 1;
    loop {
        end = json_string(c, end)?;
        if c.get(end) != Some(&':') {
            end = skip_ws(c, end);
            if c.get(end) != Some(&':') {
                return Err(JErr::At("Expecting ':' delimiter".to_string(), end));
            }
        }
        end += 1;
        end = skip_ws(c, end);
        end = match json_scan(c, end) {
            Ok(e) => e,
            Err(JErr::Stop(i)) => return Err(JErr::At("Expecting value".to_string(), i)),
            Err(e) => return Err(e),
        };
        end = skip_ws(c, end);
        let next = c.get(end).copied();
        end += 1;
        if next == Some('}') {
            return Ok(end);
        } else if next != Some(',') {
            return Err(JErr::At("Expecting ',' delimiter".to_string(), end - 1));
        }
        let comma = end - 1;
        end = skip_ws(c, end);
        let next = c.get(end).copied();
        end += 1;
        if next != Some('"') {
            if next == Some('}') {
                return Err(JErr::At("Illegal trailing comma before end of object".to_string(), comma));
            }
            return Err(JErr::At("Expecting property name enclosed in double quotes".to_string(), end - 1));
        }
    }
}

/// `end` is just after `[`.
fn json_array(c: &[char], mut end: usize) -> Result<usize, JErr> {
    end = skip_ws(c, end);
    if c.get(end) == Some(&']') {
        return Ok(end + 1);
    }
    loop {
        end = match json_scan(c, end) {
            Ok(e) => e,
            Err(JErr::Stop(i)) => return Err(JErr::At("Expecting value".to_string(), i)),
            Err(e) => return Err(e),
        };
        end = skip_ws(c, end);
        let next = c.get(end).copied();
        end += 1;
        if next == Some(']') {
            return Ok(end);
        } else if next != Some(',') {
            return Err(JErr::At("Expecting ',' delimiter".to_string(), end - 1));
        }
        let comma = end - 1;
        end = skip_ws(c, end);
        if c.get(end) == Some(&']') {
            return Err(JErr::At("Illegal trailing comma before end of array".to_string(), comma));
        }
    }
}

/// `JSONDecodeError(msg, doc, pos)`: (lineno, colno, str(e)).
pub fn py_json_error_parts(doc: &str, msg: &str, pos: usize) -> (i64, i64, String) {
    let before: Vec<char> = doc.chars().take(pos).collect();
    let lineno = before.iter().filter(|c| **c == '\n').count() as i64 + 1;
    let colno = match before.iter().rposition(|c| *c == '\n') {
        Some(i) => (pos - i) as i64,
        None => pos as i64 + 1,
    };
    (lineno, colno, format!("{}: line {} column {} (char {})", msg, lineno, colno, pos))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_errors_as_cpython() {
        // (expected: CPython 3.14's json.loads, JSONDecodeError msg / pos)
        let cases: Vec<(&str, Option<(String, usize)>)> = vec![
        ("not json", Some(("Expecting value".to_string(), 0))),
        ("", Some(("Expecting value".to_string(), 0))),
        ("   ", Some(("Expecting value".to_string(), 3))),
        ("{", Some(("Expecting property name enclosed in double quotes".to_string(), 1))),
        ("}", Some(("Expecting value".to_string(), 0))),
        ("[", Some(("Expecting value".to_string(), 1))),
        ("]", Some(("Expecting value".to_string(), 0))),
        ("[1,]", Some(("Illegal trailing comma before end of array".to_string(), 2))),
        ("{\"a\":1,}", Some(("Illegal trailing comma before end of object".to_string(), 6))),
        ("{\"a\" 1}", Some(("Expecting ':' delimiter".to_string(), 5))),
        ("{\"a\":}", Some(("Expecting value".to_string(), 5))),
        ("{a:1}", Some(("Expecting property name enclosed in double quotes".to_string(), 1))),
        ("{\"a\":1 \"b\":2}", Some(("Expecting ',' delimiter".to_string(), 7))),
        ("[1 2]", Some(("Expecting ',' delimiter".to_string(), 3))),
        ("\"abc", Some(("Unterminated string starting at".to_string(), 0))),
        ("\"a\\x\"", Some(("Invalid \\escape".to_string(), 2))),
        ("\"a\\u12\"", Some(("Invalid \\uXXXX escape".to_string(), 3))),
        ("\"a\tb\"", Some(("Invalid control character at".to_string(), 2))),
        ("[1, 2] x", Some(("Extra data".to_string(), 7))),
        ("NaN", None),
        ("-Infinity", None),
        ("Infinity", None),
        ("[NaN, 1e5, -0.5E-3]", None),
        ("01", Some(("Extra data".to_string(), 1))),
        ("-", Some(("Expecting value".to_string(), 0))),
        ("1.", Some(("Extra data".to_string(), 1))),
        ("1e", Some(("Extra data".to_string(), 1))),
        ("tru", Some(("Expecting value".to_string(), 0))),
        ("nul", Some(("Expecting value".to_string(), 0))),
        ("{\"a\": [1, {\"b\": }]}", Some(("Expecting value".to_string(), 16))),
        ("\n\n  {\"x\": 1,\n \"y\" }", Some(("Expecting ':' delimiter".to_string(), 18))),
        ("﻿{}", Some(("Unexpected UTF-8 BOM (decode using utf-8-sig)".to_string(), 0))),
        ("{\"a\":1}\n", None),
        ("[\"\\ud800\"]", None),
        ("[\"\\ud800\\u12\"]", Some(("Invalid \\uXXXX escape".to_string(), 9))),
        ("{\"k\": \"v\"} {\"k\": 2}", Some(("Extra data".to_string(), 11))),
        ("{ }", None),
        ("[ ]", None),
        ("[,1]", Some(("Expecting value".to_string(), 1))),
        ("{,}", Some(("Expecting property name enclosed in double quotes".to_string(), 1))),
        ("{\"a\":1,,}", Some(("Expecting property name enclosed in double quotes".to_string(), 7))),
        ("[1,,2]", Some(("Expecting value".to_string(), 3))),
        ("\"\\", Some(("Unterminated string starting at".to_string(), 0))),
        ("{\"a\"", Some(("Expecting ':' delimiter".to_string(), 4))),
        ("{\"a\":", Some(("Expecting value".to_string(), 5))),
        ("{\"a\":1", Some(("Expecting ',' delimiter".to_string(), 6))),
        ("[1", Some(("Expecting ',' delimiter".to_string(), 2))),
        ("[1,", Some(("Expecting value".to_string(), 3))),
        ("-a", Some(("Expecting value".to_string(), 0))),
        ("+1", Some(("Expecting value".to_string(), 0))),
        (".5", Some(("Expecting value".to_string(), 0))),
        ("[true, false, null]", None),
        ("\"é\\u00e9\" x", Some(("Extra data".to_string(), 10))),
        ("{\"é\": 1, x}", Some(("Expecting property name enclosed in double quotes".to_string(), 9))),
        ];
        let bad: Vec<String> = cases.iter().filter(|(s, want)| py_json_error(s) != *want).map(|(s, want)| format!("{:?}: got {:?} want {:?}", s, py_json_error(s), want)).collect();
        assert!(bad.is_empty(), "{}", bad.join("\n"));
        assert_eq!(py_json_error_parts("a\nbc", "Expecting value", 3).2, "Expecting value: line 2 column 2 (char 3)");
    }

    #[test]
    fn text_functions() {
        assert_eq!(shlex_split("cmd --x 'a b' \"c\\\"d\" e\\ f ''"), Some(vec!["cmd".into(), "--x".into(), "a b".into(), "c\"d".into(), "e f".into(), "".into()]));
        assert_eq!(shlex_split("'open"), None);
        assert_eq!(shlex_quote("a b'c"), "'a b'\"'\"'c'");
        assert_eq!(html_escape("<a href=\"x\">'&'</a>", true), "&lt;a href=&quot;x&quot;&gt;&#x27;&amp;&#x27;&lt;/a&gt;");
        assert_eq!(html_unescape("&lt;p&gt; &#65;&#x42; &amp;"), Some("<p> AB &".into()));
        assert_eq!(html_unescape("&amp"), None);
        assert_eq!(quote("a b/ü".as_bytes(), "/"), Some("a%20b/%C3%BC".into()));
        assert_eq!(quote_plus("a b&c", ""), Some("a+b%26c".into()));
        assert_eq!(unquote("a%20b%C3%BC%zz%E2"), "a büzz".replace("zz", "%zz") + "\u{fffd}");
        assert_eq!(b64encode(b"hi", None), b"aGk=".to_vec());
        assert_eq!(b64encode(b"\xfb\xff", Some((b'-', b'_'))), b"-_8=".to_vec());
        assert_eq!(b64decode(b"aGk=", None), Some(b"hi".to_vec()));
        assert_eq!(b64decode(b"aGk", None), None);
        assert_eq!(fnmatch("a.py", "*.py"), Some(true));
        assert_eq!(fnmatch("b.txt", "[a-c]?[!x]xt"), Some(true));
        assert_eq!(fnmatch("[x", "[x"), Some(true));
        assert_eq!(from_hex("68 69"), Some(b"hi".to_vec()));
    }
}
