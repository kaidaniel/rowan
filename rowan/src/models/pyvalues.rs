//! Python operations on known literal values, evaluated exactly as CPython 3.14 does.
//!
//! Every entry point returns `None` whenever the result cannot be computed exactly: the
//! operation is unsupported here, it would raise in Python, the result would not fit the key
//! domain (ints beyond i64, strings beyond `MAX_LEN` characters), or the exact behaviour depends
//! on Unicode tables this module does not carry.  Nothing here panics on malformed input.
//!
//! Floats are formatted from their exact decimal expansion with round-half-even (what David
//! Gay's dtoa does for CPython), shortest reprs come from Rust's shortest round-trip digits,
//! and the layout code (`float_short`, `group_digits`, padding) ports CPython's
//! `format_float_short`, `_PyUnicode_InsertThousandsGrouping` and `calc_number_widths`.

use std::cmp::Ordering;
use std::sync::Arc;

use crate::intern::{key, key_bool, key_float, key_get, key_int, key_str_lit, key_tuple, Key, KeyId};

/// Longest string / tuple / bytes result built here (characters or elements).
const MAX_LEN: usize = 1 << 20;

/// Result of a `str` method: a single key, or a list (split, rsplit, splitlines).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PyResult {
    Key(KeyId),
    List(Vec<KeyId>),
}

/// Right operand of printf-style `%` formatting.
#[derive(Clone, Copy, Debug)]
pub enum PercentArgs<'a> {
    Positional(&'a [KeyId]),
    Mapping(&'a [(String, KeyId)]),
}

// ---------------------------------------------------------------------------
// small value helpers
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
enum Num {
    I(i64),
    F(f64),
}

fn num(k: KeyId) -> Option<Num> {
    match key_get(k) {
        Key::Int(i) => Some(Num::I(i)),
        Key::Bool(b) => Some(Num::I(b as i64)),
        Key::Float(bits) => Some(Num::F(f64::from_bits(bits))),
        _ => None,
    }
}

fn nkey(n: Num) -> KeyId {
    match n {
        Num::I(i) => key_int(i),
        Num::F(f) => key_float(f),
    }
}

fn fl(n: Num) -> f64 {
    match n {
        Num::I(i) => i as f64,
        Num::F(f) => f,
    }
}

fn is_bool(k: KeyId) -> bool {
    matches!(key_get(k), Key::Bool(_))
}

fn is_float(k: KeyId) -> bool {
    matches!(key_get(k), Key::Float(_))
}

/// An int-like key (int or bool).
fn int_of(k: KeyId) -> Option<i64> {
    match key_get(k) {
        Key::Int(i) => Some(i),
        Key::Bool(b) => Some(b as i64),
        _ => None,
    }
}

fn str_of(k: KeyId) -> Option<Arc<str>> {
    match key_get(k) {
        Key::Str(s) => Some(s),
        _ => None,
    }
}

fn skey(s: &str) -> Option<KeyId> {
    if s.chars().count() > MAX_LEN {
        return None;
    }
    Some(key_str_lit(s))
}

/// Python truth value.
fn truthy(k: KeyId) -> Option<bool> {
    Some(match key_get(k) {
        Key::Int(i) => i != 0,
        Key::Bool(b) => b,
        Key::Float(bits) => f64::from_bits(bits) != 0.0,
        Key::None => false,
        Key::Str(s) => !s.is_empty(),
        Key::Bytes(b) => !b.is_empty(),
        Key::Tuple(t) | Key::Frozen(t) => !t.is_empty(),
        Key::Ellipsis => true,
        Key::Inv(..) | Key::Member(..) => return None,
    })
}

/// Python's whitespace (`str.isspace`): Unicode White_Space plus U+001C..U+001F.
fn py_isspace(c: char) -> bool {
    c.is_whitespace() || ('\x1c'..='\x1f').contains(&c)
}

/// Printability of a character as `str.isprintable` / `repr` see it; None when unsure.
fn printable(c: char) -> Option<bool> {
    let u = c as u32;
    if u < 0x80 {
        return Some((0x20..0x7f).contains(&u));
    }
    if u < 0xa0 || u == 0xad || c.is_whitespace() {
        return Some(false);
    }
    if (0x200b..=0x200f).contains(&u) || (0x2060..=0x2064).contains(&u) || u == 0xfeff {
        return Some(false);
    }
    if c.is_alphanumeric() {
        return Some(true);
    }
    let known = (0xa1..=0xff).contains(&u)
        || (0x2010..=0x2027).contains(&u)
        || (0x2030..=0x205e).contains(&u)
        || (0x20a0..=0x20c0).contains(&u)
        || (0x2190..=0x23ff).contains(&u)
        || (0x2500..=0x27bf).contains(&u)
        || (0x1f300..=0x1f64f).contains(&u);
    if known {
        Some(true)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// str() / repr() / ascii()
// ---------------------------------------------------------------------------

fn repr_str(s: &str) -> Option<String> {
    let quote = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    let mut out = String::with_capacity(s.len() + 2);
    out.push(quote);
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c => {
                if printable(c)? {
                    out.push(c)
                } else {
                    push_escape(&mut out, c)
                }
            }
        }
    }
    out.push(quote);
    Some(out)
}

fn push_escape(out: &mut String, c: char) {
    let u = c as u32;
    if u <= 0xff {
        out.push_str(&format!("\\x{:02x}", u));
    } else if u <= 0xffff {
        out.push_str(&format!("\\u{:04x}", u));
    } else {
        out.push_str(&format!("\\U{:08x}", u));
    }
}

/// `ascii(s)`: the repr with every non-ASCII character escaped.
fn ascii_str(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    let mut out = String::with_capacity(s.len() + 2);
    out.push(quote);
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c if (' '..='~').contains(&c) => out.push(c),
            c => push_escape(&mut out, c),
        }
    }
    out.push(quote);
    out
}

fn repr_bytes(b: &[u8]) -> String {
    let quote = if b.contains(&b'\'') && !b.contains(&b'"') { '"' } else { '\'' };
    let mut out = String::from("b");
    out.push(quote);
    for &c in b {
        match c {
            b'\\' => out.push_str("\\\\"),
            b'\n' => out.push_str("\\n"),
            b'\t' => out.push_str("\\t"),
            b'\r' => out.push_str("\\r"),
            c if c as char == quote => {
                out.push('\\');
                out.push(c as char);
            }
            0x20..=0x7e => out.push(c as char),
            _ => out.push_str(&format!("\\x{:02x}", c)),
        }
    }
    out.push(quote);
    out
}

fn py_repr_k(k: &Key, ascii: bool) -> Option<String> {
    Some(match k {
        Key::Str(s) => {
            if ascii {
                ascii_str(s)
            } else {
                repr_str(s)?
            }
        }
        Key::Int(i) => i.to_string(),
        Key::Float(bits) => float_repr(f64::from_bits(*bits)),
        Key::Bool(true) => "True".into(),
        Key::Bool(false) => "False".into(),
        Key::None => "None".into(),
        Key::Bytes(b) => repr_bytes(b),
        Key::Ellipsis => "Ellipsis".into(),
        // (several items print in hash order)
        Key::Frozen(items) if items.is_empty() => "frozenset()".into(),
        Key::Frozen(items) if items.len() == 1 => format!("frozenset({{{}}})", py_repr_k(&key_get(items[0]), ascii)?),
        Key::Frozen(_) => return None,
        Key::Tuple(items) => {
            let mut parts = Vec::with_capacity(items.len());
            for it in items {
                parts.push(py_repr_k(&key_get(*it), ascii)?);
            }
            if parts.len() == 1 {
                format!("({},)", parts[0])
            } else {
                format!("({})", parts.join(", "))
            }
        }
        Key::Inv(..) | Key::Member(..) => return None,
    })
}

/// Python's `repr(k)`.
pub fn py_repr(k: KeyId) -> Option<String> {
    py_repr_k(&key_get(k), false)
}

/// Python's `ascii(k)`.
pub fn py_ascii(k: KeyId) -> Option<String> {
    py_repr_k(&key_get(k), true)
}

/// Python's `str(k)`.
pub fn py_str(k: KeyId) -> Option<String> {
    match key_get(k) {
        Key::Str(s) => Some(s.to_string()),
        other => py_repr_k(&other, false),
    }
}

// ---------------------------------------------------------------------------
// float digits
// ---------------------------------------------------------------------------

/// Shortest round-trip digits of |x| (finite): (digits, decpt) with |x| = 0.DIGITS × 10^decpt;
/// zero is ([0], 1).
fn shortest_digits(x: f64) -> (Vec<u8>, i32) {
    let s = format!("{:e}", x.abs());
    let (mant, exp) = s.split_once('e').unwrap_or((&s, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let mut d: Vec<u8> = mant.bytes().filter(|b| b.is_ascii_digit()).map(|b| b - b'0').collect();
    while d.len() > 1 && *d.last().unwrap() == 0 {
        d.pop();
    }
    if d.is_empty() || d == [0] {
        return (vec![0], 1);
    }
    (d, exp + 1)
}

/// The exact decimal expansion of |x| (finite), same convention as `shortest_digits`.
fn exact_digits(x: f64) -> (Vec<u8>, i32) {
    let s = format!("{:.1100}", x.abs());
    let (ip, fp) = s.split_once('.').unwrap_or((&s, ""));
    let mut d: Vec<u8> = ip.bytes().chain(fp.bytes()).map(|b| b.wrapping_sub(b'0')).collect();
    let mut decpt = ip.len() as i32;
    let lz = d.iter().take_while(|&&c| c == 0).count();
    d.drain(..lz);
    decpt -= lz as i32;
    while d.last() == Some(&0) {
        d.pop();
    }
    if d.is_empty() {
        return (vec![0], 1);
    }
    (d, decpt)
}

/// Keep the first `keep` digits, rounding half-even on the exact remainder.
fn round_digits(mut d: Vec<u8>, decpt: i32, keep: i64) -> (Vec<u8>, i32) {
    if keep >= d.len() as i64 {
        return (d, decpt);
    }
    if keep < 0 {
        return (vec![0], 1);
    }
    let k = keep as usize;
    let first = d[k];
    let up = match first.cmp(&5) {
        Ordering::Greater => true,
        Ordering::Less => false,
        Ordering::Equal => d[k + 1..].iter().any(|&c| c != 0) || (k > 0 && d[k - 1] % 2 == 1),
    };
    d.truncate(k);
    let mut decpt = decpt;
    if up {
        let mut i = k;
        loop {
            if i == 0 {
                d.insert(0, 1);
                decpt += 1;
                break;
            }
            i -= 1;
            if d[i] == 9 {
                d[i] = 0;
            } else {
                d[i] += 1;
                break;
            }
        }
    }
    while d.last() == Some(&0) {
        d.pop();
    }
    if d.is_empty() {
        return (vec![0], 1);
    }
    (d, decpt)
}

/// Port of CPython's `format_float_short` for a finite non-negative `x` (no sign).
/// `code`: 'e', 'f', 'g' or 'r' (shortest).
fn float_short(x: f64, code: char, precision: i32, add_dot_0: bool, alt: bool) -> String {
    let x = x.abs();
    let (digits, mut decpt, precision) = match code {
        'e' => {
            let (d, p) = exact_digits(x);
            let (d, p) = if x == 0.0 { (d, p) } else { round_digits(d, p, precision as i64 + 1) };
            (d, p, precision + 1)
        }
        'f' => {
            let (d, p) = exact_digits(x);
            let (d, p) = if x == 0.0 { (d, p) } else { round_digits(d, p, p as i64 + precision as i64) };
            (d, p, precision)
        }
        'g' => {
            let pr = if precision == 0 { 1 } else { precision };
            let (d, p) = exact_digits(x);
            let (d, p) = if x == 0.0 { (d, p) } else { round_digits(d, p, pr as i64) };
            (d, p, pr)
        }
        _ => {
            let (d, p) = shortest_digits(x);
            (d, p, 0)
        }
    };
    let digits_len = digits.len() as i32;
    let mut vend = digits_len;
    let mut use_exp = false;
    match code {
        'e' => {
            use_exp = true;
            vend = precision;
        }
        'f' => vend = decpt + precision,
        'g' => {
            if decpt <= -4 || decpt > (if add_dot_0 { precision - 1 } else { precision }) {
                use_exp = true;
            }
            if alt {
                vend = precision;
            }
        }
        _ => {
            if decpt <= -4 || decpt > 16 {
                use_exp = true;
            }
        }
    }
    let mut exp = 0;
    if use_exp {
        exp = decpt - 1;
        decpt = 1;
    }
    let vstart = if decpt <= 0 { decpt - 1 } else { 0 };
    if !use_exp && add_dot_0 {
        vend = vend.max(decpt + 1);
    } else {
        vend = vend.max(decpt);
    }
    let zeros = |s: &mut String, n: i32| {
        for _ in 0..n.max(0) {
            s.push('0');
        }
    };
    let dstr: String = digits.iter().map(|&c| (b'0' + c) as char).collect();
    let mut s = String::new();
    if decpt <= 0 {
        zeros(&mut s, decpt - vstart);
        s.push('.');
        zeros(&mut s, -decpt);
    } else {
        zeros(&mut s, -vstart);
    }
    if 0 < decpt && decpt <= digits_len {
        s.push_str(&dstr[..decpt as usize]);
        s.push('.');
        s.push_str(&dstr[decpt as usize..]);
    } else {
        s.push_str(&dstr);
    }
    if digits_len < decpt {
        zeros(&mut s, decpt - digits_len);
        s.push('.');
        zeros(&mut s, vend - decpt);
    } else {
        zeros(&mut s, vend - digits_len);
    }
    if s.ends_with('.') && !alt {
        s.pop();
    }
    if use_exp {
        s.push('e');
        s.push(if exp < 0 { '-' } else { '+' });
        s.push_str(&format!("{:02}", exp.abs()));
    }
    s
}

/// Python's `repr(float)`.
pub fn float_repr(x: f64) -> String {
    if x.is_nan() {
        return "nan".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "inf".into() } else { "-inf".into() };
    }
    let body = float_short(x, 'r', 0, true, false);
    if x.is_sign_negative() {
        format!("-{}", body)
    } else {
        body
    }
}

/// Body (no sign) of a float under a dtoa-style code; handles inf / nan.
fn float_body(x: f64, code: char, precision: i32, add_dot_0: bool, alt: bool, upper: bool) -> String {
    let s = if x.is_nan() {
        "nan".to_string()
    } else if x.is_infinite() {
        "inf".to_string()
    } else {
        float_short(x, code, precision, add_dot_0, alt)
    };
    if upper {
        s.to_ascii_uppercase()
    } else {
        s
    }
}

// ---------------------------------------------------------------------------
// exact rational helpers
// ---------------------------------------------------------------------------

fn bitlen(x: u128) -> i32 {
    128 - x.leading_zeros() as i32
}

/// x × 2^k exactly; None unless the result is a normal finite float.
fn ldexp_exact(x: f64, k: i32) -> Option<f64> {
    if x == 0.0 {
        return Some(x);
    }
    if !x.is_normal() {
        return None;
    }
    let bits = x.to_bits();
    let e = ((bits >> 52) & 0x7ff) as i64 + k as i64;
    if !(1..=2046).contains(&e) {
        return None;
    }
    Some(f64::from_bits((bits & !(0x7ffu64 << 52)) | ((e as u64) << 52)))
}

/// Correctly rounded float of ±a / b × 2^exp2.
fn frac_to_f64(neg: bool, a: u128, b: u128, exp2: i32) -> Option<f64> {
    if b == 0 {
        return None;
    }
    if a == 0 {
        return Some(0.0);
    }
    let s = 60 - (bitlen(a) - bitlen(b));
    let (mut q, r) = if s >= 0 {
        if bitlen(a) + s > 127 {
            return None;
        }
        let n = a << s;
        (n / b, n % b)
    } else {
        if bitlen(b) - s > 127 {
            return None;
        }
        let d = b << (-s);
        (a / d, a % d)
    };
    if r != 0 {
        q |= 1;
    }
    let f = ldexp_exact(q as f64, exp2 - s)?;
    Some(if neg { -f } else { f })
}

fn isqrt_u128(n: u128) -> u128 {
    if n < 2 {
        return n;
    }
    let mut x = (n as f64).sqrt() as u128;
    while x.checked_mul(x).is_none_or(|sq| sq > n) {
        x -= 1;
    }
    while (x + 1).checked_mul(x + 1).is_some_and(|sq| sq <= n) {
        x += 1;
    }
    x
}

/// Correctly rounded sqrt(a / b) (CPython's `_float_sqrt_of_frac`).
fn sqrt_frac(a: u128, b: u128) -> Option<f64> {
    if b == 0 {
        return None;
    }
    if a == 0 {
        return Some(0.0);
    }
    let s = (112 - (bitlen(a) - bitlen(b))).div_euclid(2);
    let (x, mut exact) = if s >= 0 {
        if bitlen(a) + 2 * s > 127 {
            return None;
        }
        let n = a << (2 * s);
        (n / b, n.is_multiple_of(b))
    } else {
        if bitlen(b) - 2 * s > 127 {
            return None;
        }
        let d = b << (-2 * s);
        (a / d, a.is_multiple_of(d))
    };
    let mut r = isqrt_u128(x);
    if r * r != x {
        exact = false;
    }
    if !exact {
        r |= 1;
    }
    ldexp_exact(r as f64, -s)
}

fn gcd_u128(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

/// A finite number as (numerator, exponent of two): value = n × 2^e.
fn exact_parts(n: Num) -> Option<(i128, i32)> {
    match n {
        Num::I(i) => Some((i as i128, 0)),
        Num::F(f) => {
            if !f.is_finite() {
                return None;
            }
            if f == 0.0 {
                return Some((0, 0));
            }
            let bits = f.to_bits();
            let e = ((bits >> 52) & 0x7ff) as i32;
            let m = bits & ((1u64 << 52) - 1);
            let (mut m, mut e) = if e == 0 { (m, -1074) } else { (m | (1u64 << 52), e - 1075) };
            let tz = m.trailing_zeros();
            m >>= tz;
            e += tz as i32;
            let v = m as i128;
            Some((if f < 0.0 { -v } else { v }, e))
        }
    }
}

// ---------------------------------------------------------------------------
// format-spec mini-language
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct Spec {
    fill: char,
    align: char,
    sign: Option<char>,
    z: bool,
    alt: bool,
    width: usize,
    grouping: Option<char>,
    precision: Option<usize>,
    ty: Option<char>,
}

fn parse_spec(spec: &str, default_align: char, default_type: Option<char>) -> Option<Spec> {
    let c: Vec<char> = spec.chars().collect();
    let n = c.len();
    let mut p = 0;
    let is_align = |ch: char| matches!(ch, '<' | '>' | '^' | '=');
    let mut fill = ' ';
    let mut align = default_align;
    let mut fill_given = false;
    let mut align_given = false;
    if n >= 2 && is_align(c[1]) {
        fill = c[0];
        align = c[1];
        fill_given = true;
        align_given = true;
        p = 2;
    } else if n >= 1 && is_align(c[0]) {
        align = c[0];
        align_given = true;
        p = 1;
    }
    let mut sign = None;
    if p < n && matches!(c[p], '+' | '-' | ' ') {
        sign = Some(c[p]);
        p += 1;
    }
    let mut z = false;
    if p < n && c[p] == 'z' {
        z = true;
        p += 1;
    }
    let mut alt = false;
    if p < n && c[p] == '#' {
        alt = true;
        p += 1;
    }
    if !fill_given && p < n && c[p] == '0' {
        fill = '0';
        if !align_given && default_align == '>' {
            align = '=';
        }
        p += 1;
    }
    let read_int = |p: &mut usize| -> Option<Option<usize>> {
        let start = *p;
        let mut v: usize = 0;
        while *p < n && c[*p].is_ascii_digit() {
            v = v.checked_mul(10)?.checked_add(c[*p] as usize - '0' as usize)?;
            *p += 1;
        }
        if v > MAX_LEN {
            return None;
        }
        Some(if *p > start { Some(v) } else { None })
    };
    let width = read_int(&mut p)?.unwrap_or(0);
    let mut grouping = None;
    if p < n && (c[p] == ',' || c[p] == '_') {
        grouping = Some(c[p]);
        p += 1;
        if p < n && (c[p] == ',' || c[p] == '_') {
            return None;
        }
    }
    let mut precision = None;
    if p < n && c[p] == '.' {
        p += 1;
        precision = Some(read_int(&mut p)??);
        if p < n && (c[p] == ',' || c[p] == '_') {
            return None; // grouping of the fractional part (3.14): not modelled
        }
    }
    if n - p > 1 {
        return None;
    }
    let ty = if n - p == 1 { Some(c[p]) } else { default_type };
    if let Some(g) = grouping {
        let ok = match ty {
            None => true,
            Some('d' | 'e' | 'f' | 'g' | 'E' | 'G' | '%' | 'F') => true,
            Some('b' | 'o' | 'x' | 'X') => g == '_',
            _ => false,
        };
        if !ok {
            return None;
        }
    }
    Some(Spec { fill, align, sign, z, alt, width, grouping, precision, ty })
}

/// Port of `_PyUnicode_InsertThousandsGrouping` (min_width zero padding included).
fn group_digits(digits: &str, min_width: i64, sep: Option<char>, g: i64) -> String {
    let d: Vec<char> = digits.chars().collect();
    let Some(sep) = sep else {
        let pad = (min_width - d.len() as i64).max(0) as usize;
        return "0".repeat(pad) + digits;
    };
    let mut pieces: Vec<String> = Vec::new();
    let mut remaining = d.len() as i64;
    let mut mw = min_width;
    let mut end = d.len();
    let mut use_sep = false;
    loop {
        let l = g.min(remaining.max(mw).max(1));
        let n_zeros = (l - remaining).max(0);
        let n_chars = remaining.min(l).max(0);
        let mut chunk = "0".repeat(n_zeros as usize);
        chunk.extend(&d[end - n_chars as usize..end]);
        end -= n_chars as usize;
        if use_sep {
            chunk.push(sep);
        }
        pieces.push(chunk);
        use_sep = true;
        remaining -= n_chars;
        mw -= l;
        if remaining <= 0 && mw <= 0 {
            break;
        }
        mw -= 1;
    }
    pieces.reverse();
    pieces.concat()
}

/// Lay out a number: `digits` get grouped / zero-padded, `rest` (decimal point, fraction,
/// exponent, suffix) is copied; port of `calc_number_widths` + `fill_number`.
fn layout_number(spec: &Spec, neg: bool, prefix: &str, digits: &str, rest: &str, group: i64) -> Option<String> {
    let sign = match spec.sign {
        Some('+') => Some(if neg { '-' } else { '+' }),
        Some(' ') => Some(if neg { '-' } else { ' ' }),
        _ => {
            if neg {
                Some('-')
            } else {
                None
            }
        }
    };
    let nondigit = sign.is_some() as i64 + prefix.chars().count() as i64 + rest.chars().count() as i64;
    let width = spec.width as i64;
    let min_width = if spec.fill == '0' && spec.align == '=' { width - nondigit } else { 0 };
    let grouped = if digits.is_empty() { String::new() } else { group_digits(digits, min_width, spec.grouping, group) };
    let n_padding = width - (nondigit + grouped.chars().count() as i64);
    let (mut l, mut s, mut r) = (0, 0, 0);
    if n_padding > 0 {
        match spec.align {
            '<' => r = n_padding,
            '^' => {
                l = n_padding / 2;
                r = n_padding - l;
            }
            '=' => s = n_padding,
            _ => l = n_padding,
        }
    }
    let fill = |n: i64| spec.fill.to_string().repeat(n as usize);
    let mut out = fill(l);
    if let Some(c) = sign {
        out.push(c);
    }
    out.push_str(prefix);
    out.push_str(&fill(s));
    out.push_str(&grouped);
    out.push_str(rest);
    out.push_str(&fill(r));
    if out.chars().count() > MAX_LEN {
        return None;
    }
    Some(out)
}

fn pad_text(text: &str, spec: &Spec) -> String {
    let len = text.chars().count();
    if spec.width <= len {
        return text.to_string();
    }
    let pad = spec.width - len;
    let fill = |n: usize| spec.fill.to_string().repeat(n);
    match spec.align {
        '>' => fill(pad) + text,
        '^' => fill(pad / 2) + text + &fill(pad - pad / 2),
        _ => text.to_string() + &fill(pad),
    }
}

fn format_str(s: &str, spec: &str) -> Option<String> {
    let sp = parse_spec(spec, '<', Some('s'))?;
    if sp.ty != Some('s') || sp.sign.is_some() || sp.alt || sp.z || sp.align == '=' || sp.grouping.is_some() {
        return None;
    }
    let text: String = match sp.precision {
        Some(p) => s.chars().take(p).collect(),
        None => s.to_string(),
    };
    Some(pad_text(&text, &sp))
}

fn to_base(mut m: u128, base: u32, upper: bool) -> String {
    if m == 0 {
        return "0".into();
    }
    let mut v = Vec::new();
    while m > 0 {
        let d = (m % base as u128) as u32;
        let c = std::char::from_digit(d, base).unwrap_or('0');
        v.push(if upper { c.to_ascii_uppercase() } else { c });
        m /= base as u128;
    }
    v.iter().rev().collect()
}

fn format_int(i: i64, spec: &str) -> Option<String> {
    let sp = parse_spec(spec, '>', Some('d'))?;
    match sp.ty {
        Some('e' | 'E' | 'f' | 'F' | 'g' | 'G' | '%') => return format_float_spec(i as f64, sp),
        Some('d' | 'n' | 'b' | 'o' | 'x' | 'X' | 'c') => {}
        _ => return None,
    }
    if sp.precision.is_some() || sp.z {
        return None;
    }
    let ty = sp.ty.unwrap_or('d');
    if ty == 'c' {
        if sp.sign.is_some() || sp.alt {
            return None;
        }
        let ch = char::from_u32(u32::try_from(i).ok()?)?;
        return layout_number(&sp, false, "", "", &ch.to_string(), 3);
    }
    let neg = i < 0;
    let m = (i as i128).unsigned_abs();
    let (base, prefix, group) = match ty {
        'b' => (2, "0b", 4),
        'o' => (8, "0o", 4),
        'x' => (16, "0x", 4),
        'X' => (16, "0X", 4),
        _ => (10, "", 3),
    };
    let digits = to_base(m, base, ty == 'X');
    let prefix = if sp.alt { prefix } else { "" };
    layout_number(&sp, neg, prefix, &digits, "", group)
}

fn format_float_spec(x: f64, sp: Spec) -> Option<String> {
    if sp.precision.is_some_and(|p| p > 10_000) {
        return None;
    }
    let mut x = x;
    let mut suffix = "";
    let (code, prec, add_dot_0, upper) = match sp.ty {
        None => match sp.precision {
            None => ('r', 0, true, false),
            Some(p) => ('g', p as i32, true, false),
        },
        Some('e') => ('e', sp.precision.unwrap_or(6) as i32, false, false),
        Some('E') => ('e', sp.precision.unwrap_or(6) as i32, false, true),
        Some('f') => ('f', sp.precision.unwrap_or(6) as i32, false, false),
        Some('F') => ('f', sp.precision.unwrap_or(6) as i32, false, true),
        Some('g' | 'n') => ('g', sp.precision.unwrap_or(6) as i32, false, false),
        Some('G') => ('g', sp.precision.unwrap_or(6) as i32, false, true),
        Some('%') => {
            x *= 100.0;
            suffix = "%";
            ('f', sp.precision.unwrap_or(6) as i32, false, false)
        }
        _ => return None,
    };
    if sp.ty == Some('n') && sp.grouping.is_some() {
        return None;
    }
    let mut neg = x.is_sign_negative() && !x.is_nan();
    let body = float_body(x, code, prec, add_dot_0, sp.alt, upper);
    if sp.z && neg && body.bytes().all(|b| !(b'1'..=b'9').contains(&b)) && x.is_finite() {
        neg = false;
    }
    let nd = body.bytes().take_while(|b| b.is_ascii_digit()).count();
    let rest = format!("{}{}", &body[nd..], suffix);
    layout_number(&sp, neg, "", &body[..nd], &rest, 3)
}

/// `format(value, spec)`: the full format-spec mini-language for str, int, float and bool.
pub fn format_value(value: KeyId, spec: &str) -> Option<String> {
    match key_get(value) {
        Key::Str(s) => format_str(&s, spec),
        Key::Int(i) => {
            if spec.is_empty() {
                Some(i.to_string())
            } else {
                format_int(i, spec)
            }
        }
        Key::Bool(b) => {
            if spec.is_empty() {
                Some(if b { "True" } else { "False" }.into())
            } else {
                format_int(b as i64, spec)
            }
        }
        Key::Float(bits) => {
            let x = f64::from_bits(bits);
            if spec.is_empty() {
                return Some(float_repr(x));
            }
            let sp = parse_spec(spec, '>', None)?;
            format_float_spec(x, sp)
        }
        Key::Inv(..) | Key::Member(..) => None,
        _ => {
            if spec.is_empty() {
                py_str(value)
            } else {
                None
            }
        }
    }
}

// ---------------------------------------------------------------------------
// str.format
// ---------------------------------------------------------------------------

struct AutoNum {
    next: usize,
    auto: bool,
    manual: bool,
}

/// `str.format(*args, **kwargs)` with literal arguments.
pub fn str_format(fmt: &str, args: &[KeyId], kwargs: &[(String, KeyId)]) -> Option<String> {
    let mut st = AutoNum { next: 0, auto: false, manual: false };
    render_format(fmt, args, kwargs, &mut st, 2)
}

fn render_format(fmt: &str, args: &[KeyId], kwargs: &[(String, KeyId)], st: &mut AutoNum, depth: u32) -> Option<String> {
    if depth == 0 {
        return None;
    }
    let c: Vec<char> = fmt.chars().collect();
    let n = c.len();
    let mut out = String::new();
    let mut i = 0;
    while i < n {
        let ch = c[i];
        if ch == '}' {
            if i + 1 < n && c[i + 1] == '}' {
                out.push('}');
                i += 2;
                continue;
            }
            return None;
        }
        if ch != '{' {
            out.push(ch);
            i += 1;
            continue;
        }
        if i + 1 < n && c[i + 1] == '{' {
            out.push('{');
            i += 2;
            continue;
        }
        // a replacement field: find its matching '}'
        let start = i + 1;
        let mut j = start;
        let mut level = 1;
        while j < n {
            match c[j] {
                '[' => return None,
                '{' => level += 1,
                '}' => {
                    level -= 1;
                    if level == 0 {
                        break;
                    }
                }
                _ => {}
            }
            j += 1;
        }
        if j >= n {
            return None;
        }
        let field: Vec<char> = c[start..j].to_vec();
        i = j + 1;
        // split name / conversion / spec
        let stop = field.iter().position(|&ch| ch == ':' || ch == '!').unwrap_or(field.len());
        let name: String = field[..stop].iter().collect();
        let mut conv = None;
        let mut spec = String::new();
        if stop < field.len() {
            if field[stop] == '!' {
                let cv = *field.get(stop + 1)?;
                conv = Some(cv);
                if stop + 2 < field.len() {
                    if field[stop + 2] != ':' {
                        return None;
                    }
                    spec = field[stop + 3..].iter().collect();
                }
            } else {
                spec = field[stop + 1..].iter().collect();
            }
        }
        if name.contains(['.', '[', '{', '}']) {
            return None;
        }
        let value = if name.is_empty() {
            if st.manual {
                return None;
            }
            st.auto = true;
            let v = *args.get(st.next)?;
            st.next += 1;
            v
        } else if name.chars().all(|ch| ch.is_ascii_digit()) {
            if st.auto {
                return None;
            }
            st.manual = true;
            let idx: usize = name.parse().ok()?;
            *args.get(idx)?
        } else {
            kwargs.iter().find(|(k, _)| *k == name)?.1
        };
        let value = match conv {
            None => value,
            Some('s') => key_str_lit(&py_str(value)?),
            Some('r') => key_str_lit(&py_repr(value)?),
            Some('a') => key_str_lit(&py_ascii(value)?),
            Some(_) => return None,
        };
        let spec = if spec.contains('{') { render_format(&spec, args, kwargs, st, depth - 1)? } else { spec };
        out.push_str(&format_value(value, &spec)?);
        if out.len() > 4 * MAX_LEN {
            return None;
        }
    }
    Some(out)
}

// ---------------------------------------------------------------------------
// printf-style formatting
// ---------------------------------------------------------------------------

/// printf-style `fmt % args`.
pub fn percent_format(fmt: &str, args: PercentArgs) -> Option<String> {
    let c: Vec<char> = fmt.chars().collect();
    let n = c.len();
    let mut out = String::new();
    let mut i = 0;
    let mut ai = 0;
    let positional: &[KeyId] = match args {
        PercentArgs::Positional(a) => a,
        PercentArgs::Mapping(_) => &[],
    };
    let next_arg = |ai: &mut usize| -> Option<KeyId> {
        let v = *positional.get(*ai)?;
        *ai += 1;
        Some(v)
    };
    while i < n {
        if c[i] != '%' {
            out.push(c[i]);
            i += 1;
            continue;
        }
        i += 1;
        if i >= n {
            return None;
        }
        if c[i] == '%' {
            out.push('%');
            i += 1;
            continue;
        }
        let mut value = None;
        if c[i] == '(' {
            let PercentArgs::Mapping(m) = args else { return None };
            let mut level = 1;
            let start = i + 1;
            i += 1;
            while i < n && level > 0 {
                if c[i] == '(' {
                    level += 1;
                } else if c[i] == ')' {
                    level -= 1;
                }
                i += 1;
            }
            if level > 0 {
                return None;
            }
            let name: String = c[start..i - 1].iter().collect();
            value = Some(m.iter().find(|(k, _)| *k == name)?.1);
        } else if matches!(args, PercentArgs::Mapping(_)) {
            return None;
        }
        let (mut left, mut plus, mut space, mut alt, mut zero) = (false, false, false, false, false);
        while i < n {
            match c[i] {
                '-' => left = true,
                '+' => plus = true,
                ' ' => space = true,
                '#' => alt = true,
                '0' => zero = true,
                _ => break,
            }
            i += 1;
        }
        let mut width: usize = 0;
        if i < n && c[i] == '*' {
            if value.is_some() {
                return None;
            }
            let w = int_of(next_arg(&mut ai)?)?;
            if w < 0 {
                left = true;
            }
            width = w.unsigned_abs() as usize;
            i += 1;
        } else {
            while i < n && c[i].is_ascii_digit() {
                width = width.checked_mul(10)?.checked_add(c[i] as usize - '0' as usize)?;
                i += 1;
            }
        }
        let mut prec: Option<usize> = None;
        if i < n && c[i] == '.' {
            i += 1;
            if i < n && c[i] == '*' {
                if value.is_some() {
                    return None;
                }
                let p = int_of(next_arg(&mut ai)?)?;
                prec = Some(p.max(0) as usize);
                i += 1;
            } else {
                let mut p: usize = 0;
                while i < n && c[i].is_ascii_digit() {
                    p = p.checked_mul(10)?.checked_add(c[i] as usize - '0' as usize)?;
                    i += 1;
                }
                prec = Some(p);
            }
        }
        if width > MAX_LEN || prec.is_some_and(|p| p > 10_000) {
            return None;
        }
        while i < n && matches!(c[i], 'h' | 'l' | 'L') {
            i += 1;
        }
        if i >= n {
            return None;
        }
        let conv = c[i];
        i += 1;
        if conv == '%' {
            return None;
        }
        let v = match value {
            Some(v) => v,
            None => next_arg(&mut ai)?,
        };
        let neg_sign = |neg: bool| -> &'static str {
            if neg {
                "-"
            } else if plus {
                "+"
            } else if space {
                " "
            } else {
                ""
            }
        };
        let (text, numeric_parts): (String, Option<(String, String, String)>) = match conv {
            's' | 'r' | 'a' => {
                let s = match conv {
                    's' => py_str(v)?,
                    'r' => py_repr(v)?,
                    _ => py_ascii(v)?,
                };
                let s = match prec {
                    Some(p) => s.chars().take(p).collect(),
                    None => s,
                };
                (s, None)
            }
            'c' => {
                let ch = match key_get(v) {
                    Key::Str(s) if s.chars().count() == 1 => s.chars().next()?,
                    Key::Int(_) | Key::Bool(_) => char::from_u32(u32::try_from(int_of(v)?).ok()?)?,
                    _ => return None,
                };
                (ch.to_string(), None)
            }
            'd' | 'i' | 'u' | 'o' | 'x' | 'X' => {
                let iv: i64 = match key_get(v) {
                    Key::Int(x) => x,
                    Key::Bool(b) => b as i64,
                    Key::Float(bits) if matches!(conv, 'd' | 'i' | 'u') => {
                        let f = f64::from_bits(bits).trunc();
                        if !f.is_finite() || f.abs() >= 9.2e18 {
                            return None;
                        }
                        f as i64
                    }
                    _ => return None,
                };
                let m = (iv as i128).unsigned_abs();
                let (base, pre) = match conv {
                    'o' => (8, "0o"),
                    'x' => (16, "0x"),
                    'X' => (16, "0X"),
                    _ => (10, ""),
                };
                let mut digits = to_base(m, base, conv == 'X');
                if let Some(p) = prec {
                    if p > digits.len() {
                        digits = "0".repeat(p - digits.len()) + &digits;
                    }
                }
                let pre = if alt { pre } else { "" };
                (String::new(), Some((neg_sign(iv < 0).to_string(), pre.to_string(), digits)))
            }
            'e' | 'E' | 'f' | 'F' | 'g' | 'G' => {
                let x = fl(num(v)?);
                let p = prec.unwrap_or(6) as i32;
                let code = conv.to_ascii_lowercase();
                let body = float_body(x, code, p, false, alt, conv.is_ascii_uppercase());
                let neg = x.is_sign_negative() && !x.is_nan();
                (String::new(), Some((neg_sign(neg).to_string(), String::new(), body)))
            }
            _ => return None,
        };
        let piece = match numeric_parts {
            None => {
                let len = text.chars().count();
                if width > len {
                    if left {
                        text + &" ".repeat(width - len)
                    } else {
                        " ".repeat(width - len) + &text
                    }
                } else {
                    text
                }
            }
            Some((sign, pre, body)) => {
                let len = sign.len() + pre.len() + body.chars().count();
                if width > len {
                    if left {
                        format!("{}{}{}{}", sign, pre, body, " ".repeat(width - len))
                    } else if zero {
                        format!("{}{}{}{}", sign, pre, "0".repeat(width - len), body)
                    } else {
                        format!("{}{}{}{}", " ".repeat(width - len), sign, pre, body)
                    }
                } else {
                    format!("{}{}{}", sign, pre, body)
                }
            }
        };
        out.push_str(&piece);
        if out.len() > 4 * MAX_LEN {
            return None;
        }
    }
    if let PercentArgs::Positional(a) = args {
        if ai < a.len() {
            return None;
        }
    }
    Some(out)
}

// ---------------------------------------------------------------------------
// comparisons
// ---------------------------------------------------------------------------

/// Exact comparison of an int with a float (None for NaN).
fn cmp_int_float(i: i64, f: f64) -> Option<Ordering> {
    if f.is_nan() {
        return None;
    }
    if f >= 9223372036854775808.0 {
        return Some(Ordering::Less);
    }
    if f < -9223372036854775808.0 {
        return Some(Ordering::Greater);
    }
    let t = f.trunc();
    let ti = t as i64;
    match i.cmp(&ti) {
        Ordering::Equal => {
            let frac = f - t;
            Some(if frac > 0.0 {
                Ordering::Less
            } else if frac < 0.0 {
                Ordering::Greater
            } else {
                Ordering::Equal
            })
        }
        o => Some(o),
    }
}

fn num_cmp(a: Num, b: Num) -> Option<Ordering> {
    match (a, b) {
        (Num::I(x), Num::I(y)) => Some(x.cmp(&y)),
        (Num::F(x), Num::F(y)) => x.partial_cmp(&y),
        (Num::I(x), Num::F(y)) => cmp_int_float(x, y),
        (Num::F(x), Num::I(y)) => cmp_int_float(y, x).map(Ordering::reverse),
    }
}

/// Python `a == b`; None when unknowable (NaN inside a tuple: identity matters).
fn py_eq(a: KeyId, b: KeyId) -> Option<bool> {
    if let (Some(x), Some(y)) = (num(a), num(b)) {
        return Some(num_cmp(x, y) == Some(Ordering::Equal));
    }
    match (key_get(a), key_get(b)) {
        (Key::Inv(..) | Key::Member(..), _) | (_, Key::Inv(..) | Key::Member(..)) => None,
        (Key::Tuple(x), Key::Tuple(y)) => {
            if x.len() != y.len() {
                return Some(false);
            }
            for (p, q) in x.iter().zip(y.iter()) {
                if num(*p).is_some_and(|n| matches!(n, Num::F(f) if f.is_nan())) {
                    return None;
                }
                if !py_eq(*p, *q)? {
                    return Some(false);
                }
            }
            Some(true)
        }
        (x, y) => Some(x == y),
    }
}

/// Python ordering `a op b` for op in lt le gt ge; None when it raises.
fn py_order(op: &str, a: KeyId, b: KeyId) -> Option<bool> {
    let test = |o: Option<Ordering>| -> bool {
        match o {
            None => false,
            Some(o) => match op {
                "lt" => o == Ordering::Less,
                "le" => o != Ordering::Greater,
                "gt" => o == Ordering::Greater,
                _ => o != Ordering::Less,
            },
        }
    };
    if let (Some(x), Some(y)) = (num(a), num(b)) {
        return Some(test(num_cmp(x, y)));
    }
    match (key_get(a), key_get(b)) {
        (Key::Str(x), Key::Str(y)) => Some(test(Some(x.cmp(&y)))),
        (Key::Bytes(x), Key::Bytes(y)) => Some(test(Some(x.cmp(&y)))),
        (Key::Tuple(x), Key::Tuple(y)) => {
            for (p, q) in x.iter().zip(y.iter()) {
                if !py_eq(*p, *q)? {
                    return py_order(op, *p, *q);
                }
            }
            Some(test(Some(x.len().cmp(&y.len()))))
        }
        _ => None,
    }
}

fn py_lt(a: KeyId, b: KeyId) -> Option<bool> {
    py_order("lt", a, b)
}

// ---------------------------------------------------------------------------
// arithmetic
// ---------------------------------------------------------------------------

fn int_floordiv(a: i64, b: i64) -> Option<i64> {
    if b == 0 {
        return None;
    }
    let (a, b) = (a as i128, b as i128);
    let mut q = a / b;
    if a % b != 0 && ((a < 0) != (b < 0)) {
        q -= 1;
    }
    i64::try_from(q).ok()
}

fn int_mod(a: i64, b: i64) -> Option<i64> {
    if b == 0 {
        return None;
    }
    let (a, b) = (a as i128, b as i128);
    let mut r = a % b;
    if r != 0 && ((r < 0) != (b < 0)) {
        r += b;
    }
    i64::try_from(r).ok()
}

/// CPython's `_float_div_mod`.
fn float_divmod(vx: f64, wx: f64) -> Option<(f64, f64)> {
    if wx == 0.0 {
        return None;
    }
    let mut m = vx % wx;
    let mut div = (vx - m) / wx;
    if m != 0.0 {
        if (wx < 0.0) != (m < 0.0) {
            m += wx;
            div -= 1.0;
        }
    } else {
        m = 0.0f64.copysign(wx);
    }
    let floordiv = if div != 0.0 {
        let mut fd = div.floor();
        if div - fd > 0.5 {
            fd += 1.0;
        }
        fd
    } else {
        0.0f64.copysign(vx / wx)
    };
    Some((floordiv, m))
}

fn is_odd_integer(x: f64) -> bool {
    x.is_finite() && x == x.trunc() && (x / 2.0) != (x / 2.0).trunc()
}

/// CPython's `float_pow` (None where it raises or returns a complex).
fn float_pow(iv: f64, iw: f64) -> Option<f64> {
    if iw == 0.0 {
        return Some(1.0);
    }
    if iv.is_nan() {
        return Some(iv);
    }
    if iw.is_nan() {
        return Some(if iv == 1.0 { 1.0 } else { iw });
    }
    if iw.is_infinite() {
        let a = iv.abs();
        return Some(if a == 1.0 {
            1.0
        } else if (iw > 0.0) == (a > 1.0) {
            iw.abs()
        } else {
            0.0
        });
    }
    if iv.is_infinite() {
        let odd = is_odd_integer(iw);
        return Some(if iw > 0.0 {
            if odd {
                iv
            } else {
                iv.abs()
            }
        } else if odd {
            0.0f64.copysign(iv)
        } else {
            0.0
        });
    }
    if iv == 0.0 {
        if iw < 0.0 {
            return None;
        }
        return Some(if is_odd_integer(iw) { iv } else { 0.0 });
    }
    let mut iv = iv;
    let mut negate = false;
    if iv < 0.0 {
        if iw != iw.floor() {
            return None;
        }
        iv = -iv;
        negate = is_odd_integer(iw);
    }
    if iv == 1.0 {
        return Some(if negate { -1.0 } else { 1.0 });
    }
    let r = iv.powf(iw);
    if r.is_infinite() {
        return None;
    }
    Some(if negate { -r } else { r })
}

fn int_truediv(a: i64, b: i64) -> Option<f64> {
    if b == 0 {
        return None;
    }
    let neg = (a < 0) != (b < 0);
    if a == 0 {
        return Some(if b < 0 { -0.0 } else { 0.0 });
    }
    frac_to_f64(neg, (a as i128).unsigned_abs(), (b as i128).unsigned_abs(), 0)
}

fn num_binop(op: &str, a: Num, b: Num) -> Option<Num> {
    use Num::*;
    match op {
        "add" | "sub" | "mul" => match (a, b) {
            (I(x), I(y)) => Some(I(match op {
                "add" => x.checked_add(y)?,
                "sub" => x.checked_sub(y)?,
                _ => x.checked_mul(y)?,
            })),
            _ => {
                let (x, y) = (fl(a), fl(b));
                Some(F(match op {
                    "add" => x + y,
                    "sub" => x - y,
                    _ => x * y,
                }))
            }
        },
        "truediv" => match (a, b) {
            (I(x), I(y)) => Some(F(int_truediv(x, y)?)),
            _ => {
                let y = fl(b);
                if y == 0.0 {
                    return None;
                }
                Some(F(fl(a) / y))
            }
        },
        "floordiv" => match (a, b) {
            (I(x), I(y)) => Some(I(int_floordiv(x, y)?)),
            _ => Some(F(float_divmod(fl(a), fl(b))?.0)),
        },
        "mod" => match (a, b) {
            (I(x), I(y)) => Some(I(int_mod(x, y)?)),
            _ => Some(F(float_divmod(fl(a), fl(b))?.1)),
        },
        "pow" => match (a, b) {
            (I(x), I(y)) => {
                if y >= 0 {
                    if let Ok(e) = u32::try_from(y) {
                        return Some(I(x.checked_pow(e)?));
                    }
                    return match x {
                        0 | 1 => Some(I(x)),
                        -1 => Some(I(if y % 2 == 0 { 1 } else { -1 })),
                        _ => None,
                    };
                }
                Some(F(float_pow(x as f64, y as f64)?))
            }
            _ => Some(F(float_pow(fl(a), fl(b))?)),
        },
        _ => None,
    }
}

fn repeat_seq(k: &Key, n: i64) -> Option<KeyId> {
    let n = n.max(0) as usize;
    match k {
        Key::Str(s) => {
            if s.chars().count().checked_mul(n)? > MAX_LEN {
                return None;
            }
            Some(key_str_lit(&s.repeat(n)))
        }
        Key::Bytes(b) => {
            if b.len().checked_mul(n)? > MAX_LEN {
                return None;
            }
            Some(key(Key::Bytes(b.repeat(n))))
        }
        Key::Tuple(t) => {
            if t.len().checked_mul(n)? > MAX_LEN {
                return None;
            }
            Some(key_tuple(t.repeat(n)))
        }
        _ => None,
    }
}

fn concat_seq(a: &Key, b: &Key) -> Option<KeyId> {
    match (a, b) {
        (Key::Str(x), Key::Str(y)) => skey(&format!("{}{}", x, y)),
        (Key::Bytes(x), Key::Bytes(y)) => {
            if x.len() + y.len() > MAX_LEN {
                return None;
            }
            Some(key(Key::Bytes([x.as_slice(), y.as_slice()].concat())))
        }
        (Key::Tuple(x), Key::Tuple(y)) => {
            if x.len() + y.len() > MAX_LEN {
                return None;
            }
            Some(key_tuple([x.as_slice(), y.as_slice()].concat()))
        }
        _ => None,
    }
}

fn contains(container: KeyId, item: KeyId) -> Option<bool> {
    match key_get(container) {
        Key::Str(s) => Some(s.contains(&*str_of(item)?)),
        Key::Bytes(b) => match key_get(item) {
            Key::Bytes(x) => Some(x.is_empty() || b.windows(x.len()).any(|w| w == x.as_slice())),
            Key::Int(_) | Key::Bool(_) => {
                let v = int_of(item)?;
                if !(0..=255).contains(&v) {
                    return None;
                }
                Some(b.contains(&(v as u8)))
            }
            _ => None,
        },
        Key::Tuple(t) => {
            for e in t {
                if e == item || py_eq(e, item)? {
                    return Some(true);
                }
            }
            Some(false)
        }
        _ => None,
    }
}

fn binop(op: &str, a: KeyId, b: KeyId) -> Option<KeyId> {
    match op {
        "eq" => return Some(key_bool(py_eq(a, b)?)),
        "ne" => return Some(key_bool(!py_eq(a, b)?)),
        "lt" | "le" | "gt" | "ge" => return Some(key_bool(py_order(op, a, b)?)),
        "contains" => return Some(key_bool(contains(a, b)?)),
        "concat" => return concat_seq(&key_get(a), &key_get(b)),
        "is_" | "is_not" => return None,
        _ => {}
    }
    if let (Some(x), Some(y)) = (num(a), num(b)) {
        match op {
            "and_" | "or_" | "xor" | "lshift" | "rshift" => {
                let (Num::I(x), Num::I(y)) = (x, y) else { return None };
                let both_bool = is_bool(a) && is_bool(b);
                let r = match op {
                    "and_" => x & y,
                    "or_" => x | y,
                    "xor" => x ^ y,
                    "lshift" => {
                        if y < 0 {
                            return None;
                        }
                        if x == 0 {
                            0
                        } else {
                            if y >= 64 {
                                return None;
                            }
                            let r = (x as i128) << y;
                            i64::try_from(r).ok()?
                        }
                    }
                    _ => {
                        if y < 0 {
                            return None;
                        }
                        x >> y.min(63)
                    }
                };
                if both_bool && matches!(op, "and_" | "or_" | "xor") {
                    return Some(key_bool(r != 0));
                }
                return Some(key_int(r));
            }
            _ => return num_binop(op, x, y).map(nkey),
        }
    }
    let (ka, kb) = (key_get(a), key_get(b));
    match op {
        "add" => concat_seq(&ka, &kb),
        "mul" => match (&ka, &kb) {
            (Key::Str(_) | Key::Bytes(_) | Key::Tuple(_), Key::Int(_) | Key::Bool(_)) => repeat_seq(&ka, int_of(b)?),
            (Key::Int(_) | Key::Bool(_), Key::Str(_) | Key::Bytes(_) | Key::Tuple(_)) => repeat_seq(&kb, int_of(a)?),
            _ => None,
        },
        "mod" => match ka {
            Key::Str(s) => {
                let r = match &kb {
                    Key::Tuple(t) => percent_format(&s, PercentArgs::Positional(t))?,
                    _ => percent_format(&s, PercentArgs::Positional(&[b]))?,
                };
                skey(&r)
            }
            _ => None,
        },
        _ => None,
    }
}

fn unop(op: &str, a: KeyId) -> Option<KeyId> {
    match op {
        "not_" => return Some(key_bool(!truthy(a)?)),
        "truth" => return Some(key_bool(truthy(a)?)),
        _ => {}
    }
    let n = num(a)?;
    match (op, n) {
        ("neg", Num::I(i)) => Some(key_int(i.checked_neg()?)),
        ("neg", Num::F(f)) => Some(key_float(-f)),
        ("pos", _) => Some(nkey(n)),
        ("abs", Num::I(i)) => Some(key_int(i.checked_abs()?)),
        ("abs", Num::F(f)) => Some(key_float(f.abs())),
        ("invert" | "inv", Num::I(i)) => Some(key_int(!i)),
        ("index", Num::I(i)) => Some(key_int(i)),
        _ => None,
    }
}

/// `operator.<name>(…)` and the operators by name (also dunder names and symbols).
pub fn operator_call(name: &str, args: &[KeyId]) -> Option<KeyId> {
    let base = name.strip_prefix("__").and_then(|n| n.strip_suffix("__")).unwrap_or(name);
    let op = match base {
        "+" => "add",
        "-" => {
            if args.len() == 1 {
                "neg"
            } else {
                "sub"
            }
        }
        "*" => "mul",
        "/" | "div" => "truediv",
        "//" => "floordiv",
        "%" => "mod",
        "**" => "pow",
        "==" => "eq",
        "!=" => "ne",
        "<" => "lt",
        "<=" => "le",
        ">" => "gt",
        ">=" => "ge",
        "&" | "and" => "and_",
        "|" | "or" => "or_",
        "^" => "xor",
        "<<" => "lshift",
        ">>" => "rshift",
        "~" => "invert",
        "not" => "not_",
        "in" => "contains",
        "bool" => "truth",
        other => other,
    };
    match args {
        [a] => unop(op, *a),
        [a, b] => binop(op, *a, *b),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// int() / float() parsing
// ---------------------------------------------------------------------------

fn parse_int_str(s: &str, base: u32) -> Option<i64> {
    if !s.is_ascii() || !(base == 0 || (2..=36).contains(&base)) {
        return None;
    }
    let t = s.trim_matches(py_isspace);
    let b = t.as_bytes();
    let mut p = 0;
    let mut neg = false;
    if p < b.len() && (b[p] == b'+' || b[p] == b'-') {
        neg = b[p] == b'-';
        p += 1;
    }
    let mut base = base;
    let mut prefixed = false;
    if b.len() >= p + 2 && b[p] == b'0' {
        let pc = b[p + 1].to_ascii_lowercase();
        let pb = match pc {
            b'x' => 16,
            b'o' => 8,
            b'b' => 2,
            _ => 0,
        };
        if pb != 0 && (base == 0 || base == pb) {
            base = pb;
            prefixed = true;
            p += 2;
        }
    }
    let leading_zero_rule = base == 0;
    if base == 0 {
        base = 10;
    }
    let digits = &b[p..];
    let digits = if prefixed && digits.first() == Some(&b'_') { &digits[1..] } else { digits };
    if digits.is_empty() || digits[0] == b'_' || *digits.last()? == b'_' {
        return None;
    }
    let mut v: i128 = 0;
    let mut prev_us = false;
    let mut nonzero = false;
    for &ch in digits {
        if ch == b'_' {
            if prev_us {
                return None;
            }
            prev_us = true;
            continue;
        }
        prev_us = false;
        let d = (ch as char).to_digit(base)?;
        if d != 0 {
            nonzero = true;
        }
        v = v * base as i128 + d as i128;
        if v > (1i128 << 64) {
            return None;
        }
    }
    if leading_zero_rule && !prefixed && digits[0] == b'0' && nonzero {
        return None;
    }
    i64::try_from(if neg { -v } else { v }).ok()
}

fn parse_float_str(s: &str) -> Option<f64> {
    if !s.is_ascii() {
        return None;
    }
    let t = s.trim_matches(py_isspace);
    let (neg, body) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    let lower = body.to_ascii_lowercase();
    let special = match lower.as_str() {
        "inf" | "infinity" => Some(f64::INFINITY),
        "nan" => Some(f64::NAN),
        _ => None,
    };
    if let Some(v) = special {
        return Some(if neg { -v } else { v });
    }
    // digits with single underscores between digits
    fn digit_run(s: &[u8], p: &mut usize, out: &mut String) -> bool {
        let start = *p;
        while *p < s.len() {
            let c = s[*p];
            if c.is_ascii_digit() {
                out.push(c as char);
                *p += 1;
            } else if c == b'_' && *p > start && *p + 1 < s.len() && s[*p - 1].is_ascii_digit() && s[*p + 1].is_ascii_digit() {
                *p += 1;
            } else {
                break;
            }
        }
        *p > start
    }
    let b = body.as_bytes();
    let mut p = 0;
    let mut ip = String::new();
    let mut fp = String::new();
    let has_int = digit_run(b, &mut p, &mut ip);
    let mut has_frac = false;
    if p < b.len() && b[p] == b'.' {
        p += 1;
        has_frac = digit_run(b, &mut p, &mut fp);
    }
    if !has_int && !has_frac {
        return None;
    }
    let mut exp = String::new();
    if p < b.len() && (b[p] == b'e' || b[p] == b'E') {
        p += 1;
        if p < b.len() && (b[p] == b'+' || b[p] == b'-') {
            exp.push(b[p] as char);
            p += 1;
        }
        let mut ed = String::new();
        if !digit_run(b, &mut p, &mut ed) {
            return None;
        }
        exp.push_str(&ed);
    }
    if p != b.len() {
        return None;
    }
    let canon = format!(
        "{}{}.{}e{}",
        if neg { "-" } else { "" },
        if ip.is_empty() { "0" } else { &ip },
        if fp.is_empty() { "0" } else { &fp },
        if exp.is_empty() { "0" } else { &exp }
    );
    // exponents beyond i32 overflow Rust's parser: clamp them (same value)
    canon.parse::<f64>().ok().or_else(|| {
        let e: f64 = exp.parse().ok()?;
        let mant_zero = ip.chars().chain(fp.chars()).all(|c| c == '0');
        Some(if mant_zero || e < 0.0 {
            if neg {
                -0.0
            } else {
                0.0
            }
        } else if neg {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        })
    })
}

// ---------------------------------------------------------------------------
// builtins
// ---------------------------------------------------------------------------

fn round_float_ndigits(x: f64, nd: i64) -> Option<f64> {
    if !x.is_finite() || x == 0.0 {
        return Some(x);
    }
    if nd > 323 {
        return Some(x);
    }
    if nd < -308 {
        return Some(0.0f64.copysign(x));
    }
    let (d, p) = exact_digits(x);
    let (d, p) = round_digits(d, p, p as i64 + nd);
    let ds: String = d.iter().map(|&c| (b'0' + c) as char).collect();
    let v: f64 = format!("0.{}e{}", ds, p).parse().ok()?;
    if v.is_infinite() {
        return None;
    }
    Some(v.copysign(x))
}

fn round_int_ndigits(x: i64, nd: i64) -> Option<i64> {
    if nd >= 0 {
        return Some(x);
    }
    let e = nd.unsigned_abs();
    if e > 38 {
        return Some(0);
    }
    let p = 10i128.pow(e as u32);
    let xv = x as i128;
    let mut q = xv.div_euclid(p);
    let r = xv.rem_euclid(p);
    if 2 * r > p || (2 * r == p && q % 2 != 0) {
        q += 1;
    }
    i64::try_from(q.checked_mul(p)?).ok()
}

fn float_to_int(f: f64) -> Option<i64> {
    if !(-9223372036854775808.0..9223372036854775808.0).contains(&f) {
        return None;
    }
    Some(f as i64)
}

fn mod_pow(base: i64, exp: i64, m: i64) -> Option<i64> {
    if m == 0 {
        return None;
    }
    let mm = (m as i128).unsigned_abs();
    let mut b = (base as i128).rem_euclid(mm as i128) as u128;
    let mut e = exp;
    if e < 0 {
        // modular inverse via extended Euclid
        let (mut old_r, mut r) = (b as i128, mm as i128);
        let (mut old_s, mut s) = (1i128, 0i128);
        while r != 0 {
            let q = old_r / r;
            (old_r, r) = (r, old_r - q * r);
            (old_s, s) = (s, old_s - q * s);
        }
        if old_r != 1 && mm != 1 {
            return None;
        }
        b = old_s.rem_euclid(mm as i128) as u128;
        e = e.checked_neg()?;
    }
    let mut result: u128 = 1 % mm;
    let mut e = e as u64;
    while e > 0 {
        if e & 1 == 1 {
            result = result * b % mm;
        }
        b = b * b % mm;
        e >>= 1;
    }
    let mut r = result as i128;
    if m < 0 && r != 0 {
        r -= mm as i128;
    }
    i64::try_from(r).ok()
}

/// A builtin called with literal arguments.
pub fn builtin(name: &str, args: &[KeyId], kwargs: &[(&str, KeyId)]) -> Option<KeyId> {
    let kw = |k: &str| kwargs.iter().find(|(n, _)| *n == k).map(|(_, v)| *v);
    let allowed: &[&str] = match name {
        "int" => &["base"],
        "round" => &["ndigits"],
        "pow" => &["base", "exp", "mod"],
        _ => &[],
    };
    if kwargs.iter().any(|(n, _)| !allowed.contains(n)) {
        return None;
    }
    match name {
        "abs" => match args {
            [a] => unop("abs", *a),
            _ => None,
        },
        "round" => {
            let (x, nd) = match args {
                [x] => (*x, kw("ndigits")),
                [x, n] if kw("ndigits").is_none() => (*x, Some(*n)),
                _ => return None,
            };
            let nd = match nd {
                None => None,
                Some(n) if key_get(n) == Key::None => None,
                Some(n) => Some(int_of(n)?),
            };
            match (num(x)?, nd) {
                (Num::I(i), None) => Some(key_int(i)),
                (Num::I(i), Some(n)) => Some(key_int(round_int_ndigits(i, n)?)),
                (Num::F(f), None) => Some(key_int(float_to_int(f.round_ties_even())?)),
                (Num::F(f), Some(n)) => Some(key_float(round_float_ndigits(f, n)?)),
            }
        }
        "pow" => {
            let mut v: Vec<Option<KeyId>> = args.iter().map(|a| Some(*a)).collect();
            if v.len() > 3 {
                return None;
            }
            v.resize(3, None);
            for (i, n) in ["base", "exp", "mod"].iter().enumerate() {
                if let Some(k) = kw(n) {
                    if v[i].is_some() {
                        return None;
                    }
                    v[i] = Some(k);
                }
            }
            let (b, e) = (v[0]?, v[1]?);
            match v[2] {
                None => binop("pow", b, e),
                Some(m) if key_get(m) == Key::None => binop("pow", b, e),
                Some(m) => Some(key_int(mod_pow(int_of(b)?, int_of(e)?, int_of(m)?)?)),
            }
        }
        "divmod" => match args {
            [a, b] => match (num(*a)?, num(*b)?) {
                (Num::I(x), Num::I(y)) => Some(key_tuple(vec![key_int(int_floordiv(x, y)?), key_int(int_mod(x, y)?)])),
                (x, y) => {
                    let (q, r) = float_divmod(fl(x), fl(y))?;
                    Some(key_tuple(vec![key_float(q), key_float(r)]))
                }
            },
            _ => None,
        },
        "min" | "max" => {
            let items: Vec<KeyId> = match args {
                [] => return None,
                [one] => match key_get(*one) {
                    Key::Tuple(t) => t,
                    Key::Str(s) => s.chars().map(|c| key_str_lit(&c.to_string())).collect(),
                    _ => return None,
                },
                many => many.to_vec(),
            };
            let mut best = *items.first()?;
            for &it in &items[1..] {
                let better = if name == "min" { py_lt(it, best)? } else { py_lt(best, it)? };
                if better {
                    best = it;
                }
            }
            Some(best)
        }
        "int" => {
            let base = match (args.get(1), kw("base")) {
                (Some(_), Some(_)) => return None,
                (Some(b), None) => Some(int_of(*b)?),
                (None, Some(b)) => Some(int_of(b)?),
                (None, None) => None,
            };
            if args.len() > 2 {
                return None;
            }
            match (args.first(), base) {
                (None, None) => Some(key_int(0)),
                (None, Some(_)) => None,
                (Some(x), None) => match key_get(*x) {
                    Key::Int(i) => Some(key_int(i)),
                    Key::Bool(b) => Some(key_int(b as i64)),
                    Key::Float(bits) => Some(key_int(float_to_int(f64::from_bits(bits).trunc())?)),
                    Key::Str(s) => Some(key_int(parse_int_str(&s, 10)?)),
                    Key::Bytes(b) => Some(key_int(parse_int_str(std::str::from_utf8(&b).ok()?, 10)?)),
                    _ => None,
                },
                (Some(x), Some(base)) => {
                    let base = u32::try_from(base).ok()?;
                    match key_get(*x) {
                        Key::Str(s) => Some(key_int(parse_int_str(&s, base)?)),
                        Key::Bytes(b) => Some(key_int(parse_int_str(std::str::from_utf8(&b).ok()?, base)?)),
                        _ => None,
                    }
                }
            }
        }
        "float" => match args {
            [] => Some(key_float(0.0)),
            [x] => match key_get(*x) {
                Key::Int(i) => Some(key_float(i as f64)),
                Key::Bool(b) => Some(key_float(b as i64 as f64)),
                Key::Float(_) => Some(*x),
                Key::Str(s) => Some(key_float(parse_float_str(&s)?)),
                Key::Bytes(b) => Some(key_float(parse_float_str(std::str::from_utf8(&b).ok()?)?)),
                _ => None,
            },
            _ => None,
        },
        "str" => match args {
            [] => Some(key_str_lit("")),
            [x] => skey(&py_str(*x)?),
            _ => None,
        },
        "repr" => match args {
            [x] => skey(&py_repr(*x)?),
            _ => None,
        },
        "ascii" => match args {
            [x] => skey(&py_ascii(*x)?),
            _ => None,
        },
        "bool" => match args {
            [] => Some(key_bool(false)),
            [x] => Some(key_bool(truthy(*x)?)),
            _ => None,
        },
        "chr" => match args {
            [x] => {
                let i = int_of(*x)?;
                let c = char::from_u32(u32::try_from(i).ok()?)?;
                Some(key_str_lit(&c.to_string()))
            }
            _ => None,
        },
        "ord" => match args {
            [x] => match key_get(*x) {
                Key::Str(s) => {
                    let mut it = s.chars();
                    let c = it.next()?;
                    if it.next().is_some() {
                        return None;
                    }
                    Some(key_int(c as i64))
                }
                Key::Bytes(b) if b.len() == 1 => Some(key_int(b[0] as i64)),
                _ => None,
            },
            _ => None,
        },
        "hex" | "oct" | "bin" => match args {
            [x] => {
                let i = int_of(*x)?;
                let (base, pre) = match name {
                    "hex" => (16, "0x"),
                    "oct" => (8, "0o"),
                    _ => (2, "0b"),
                };
                let s = format!("{}{}{}", if i < 0 { "-" } else { "" }, pre, to_base((i as i128).unsigned_abs(), base, false));
                Some(key_str_lit(&s))
            }
            _ => None,
        },
        "len" => match args {
            [x] => match key_get(*x) {
                Key::Str(s) => Some(key_int(s.chars().count() as i64)),
                Key::Bytes(b) => Some(key_int(b.len() as i64)),
                Key::Tuple(t) => Some(key_int(t.len() as i64)),
                _ => None,
            },
            _ => None,
        },
        "format" => match args {
            [v] => skey(&format_value(*v, "")?),
            [v, spec] => skey(&format_value(*v, &str_of(*spec)?)?),
            _ => None,
        },
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// str methods
// ---------------------------------------------------------------------------

/// Bind positional / keyword arguments to `names`; the first `posonly` are positional-only and
/// the first `required` must be given.
fn bind(args: &[KeyId], kwargs: &[(&str, KeyId)], names: &[&str], posonly: usize, required: usize) -> Option<Vec<Option<KeyId>>> {
    if args.len() > names.len() {
        return None;
    }
    let mut out: Vec<Option<KeyId>> = vec![None; names.len()];
    for (i, a) in args.iter().enumerate() {
        out[i] = Some(*a);
    }
    for (k, v) in kwargs {
        let idx = names.iter().position(|n| n == k)?;
        if idx < posonly || out[idx].is_some() {
            return None;
        }
        out[idx] = Some(*v);
    }
    if out[..required].iter().any(|o| o.is_none()) {
        return None;
    }
    Some(out)
}

/// A slice index argument: None (absent or `None`) or an int.
fn opt_index(k: Option<KeyId>) -> Option<Option<i64>> {
    match k {
        None => Some(None),
        Some(k) if key_get(k) == Key::None => Some(None),
        Some(k) => Some(Some(int_of(k)?)),
    }
}

/// CPython's ADJUST_INDICES.
fn adjust(start: Option<i64>, end: Option<i64>, len: i64) -> (i64, i64) {
    let mut s = start.unwrap_or(0);
    let mut e = end.unwrap_or(i64::MAX);
    if e > len {
        e = len;
    } else if e < 0 {
        e += len;
        if e < 0 {
            e = 0;
        }
    }
    if s < 0 {
        s += len;
        if s < 0 {
            s = 0;
        }
    }
    (s, e)
}

fn find_in(h: &[char], n: &[char], from: usize) -> Option<usize> {
    if n.is_empty() {
        return if from <= h.len() { Some(from) } else { None };
    }
    if n.len() > h.len() {
        return None;
    }
    (from..=h.len() - n.len()).find(|&i| h[i..i + n.len()] == *n)
}

fn rfind_in(h: &[char], n: &[char]) -> Option<usize> {
    if n.len() > h.len() {
        return None;
    }
    (0..=h.len() - n.len()).rev().find(|&i| h[i..i + n.len()] == *n)
}

fn s_of(v: &[char]) -> String {
    v.iter().collect()
}

fn ks(v: &[char]) -> KeyId {
    key_str_lit(&s_of(v))
}

fn ascii_cased(c: char) -> bool {
    c.is_ascii_alphabetic()
}

fn title_ascii(s: &[char]) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_cased = false;
    for &c in s {
        if prev_cased {
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c.to_ascii_uppercase());
        }
        prev_cased = ascii_cased(c);
    }
    out
}

fn split_ws(s: &[char], maxsplit: i64) -> Vec<KeyId> {
    let mut out = Vec::new();
    let mut i = 0;
    let n = s.len();
    let mut splits = 0;
    loop {
        while i < n && py_isspace(s[i]) {
            i += 1;
        }
        if i >= n {
            break;
        }
        if maxsplit >= 0 && splits >= maxsplit {
            out.push(ks(&s[i..]));
            break;
        }
        let start = i;
        while i < n && !py_isspace(s[i]) {
            i += 1;
        }
        out.push(ks(&s[start..i]));
        splits += 1;
    }
    out
}

fn rsplit_ws(s: &[char], maxsplit: i64) -> Vec<KeyId> {
    let mut out = Vec::new();
    let mut i = s.len();
    let mut splits = 0;
    loop {
        while i > 0 && py_isspace(s[i - 1]) {
            i -= 1;
        }
        if i == 0 {
            break;
        }
        if maxsplit >= 0 && splits >= maxsplit {
            out.push(ks(&s[..i]));
            break;
        }
        let end = i;
        while i > 0 && !py_isspace(s[i - 1]) {
            i -= 1;
        }
        out.push(ks(&s[i..end]));
        splits += 1;
    }
    out.reverse();
    out
}

fn split_sep(s: &[char], sep: &[char], maxsplit: i64) -> Vec<KeyId> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut splits = 0;
    while maxsplit < 0 || splits < maxsplit {
        match find_in(s, sep, start) {
            Some(p) => {
                out.push(ks(&s[start..p]));
                start = p + sep.len();
                splits += 1;
            }
            None => break,
        }
    }
    out.push(ks(&s[start..]));
    out
}

fn rsplit_sep(s: &[char], sep: &[char], maxsplit: i64) -> Vec<KeyId> {
    let mut out = Vec::new();
    let mut end = s.len();
    let mut splits = 0;
    while maxsplit < 0 || splits < maxsplit {
        match rfind_in(&s[..end], sep) {
            Some(p) => {
                out.push(ks(&s[p + sep.len()..end]));
                end = p;
                splits += 1;
            }
            None => break,
        }
    }
    out.push(ks(&s[..end]));
    out.reverse();
    out
}

fn is_line_break(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\x0b' | '\x0c' | '\x1c' | '\x1d' | '\x1e' | '\u{85}' | '\u{2028}' | '\u{2029}')
}

fn strip_set(chars_arg: Option<KeyId>) -> Option<Option<Vec<char>>> {
    match chars_arg {
        None => Some(None),
        Some(k) if key_get(k) == Key::None => Some(None),
        Some(k) => Some(Some(str_of(k)?.chars().collect())),
    }
}

fn width_arg(k: KeyId) -> Option<usize> {
    let w = int_of(k)?;
    let w = w.max(0) as usize;
    if w > MAX_LEN {
        return None;
    }
    Some(w)
}

fn fill_arg(k: Option<KeyId>) -> Option<char> {
    match k {
        None => Some(' '),
        Some(k) => {
            let s = str_of(k)?;
            let mut it = s.chars();
            let c = it.next()?;
            if it.next().is_some() {
                return None;
            }
            Some(c)
        }
    }
}

/// Lower / upper case through Rust's Unicode tables for scripts where they agree with
/// CPython's (Latin, Greek, Cyrillic, Armenian); None beyond.
fn case_safe(s: &str) -> bool {
    s.chars().all(|c| (c as u32) < 0x590)
}

/// A `str` method on a literal receiver.
pub fn str_method(recv: &str, name: &str, args: &[KeyId], kwargs: &[(&str, KeyId)]) -> Option<PyResult> {
    let s: Vec<char> = recv.chars().collect();
    let len = s.len() as i64;
    let ascii = recv.is_ascii();
    let k = |v: KeyId| Some(PyResult::Key(v));
    let kstr = |v: &str| -> Option<PyResult> { Some(PyResult::Key(skey(v)?)) };
    let kb = |b: bool| Some(PyResult::Key(key_bool(b)));
    let noargs = args.is_empty() && kwargs.is_empty();
    match name {
        "lower" | "upper" | "casefold" | "capitalize" | "swapcase" | "title" | "isalnum" | "isalpha" | "isascii" | "isdecimal"
        | "isdigit" | "isidentifier" | "islower" | "isnumeric" | "isprintable" | "isspace" | "istitle" | "isupper"
            if !noargs =>
        {
            None
        }
        "lower" => {
            if !case_safe(recv) {
                return None;
            }
            kstr(&recv.to_lowercase())
        }
        "upper" => {
            if !case_safe(recv) {
                return None;
            }
            kstr(&recv.to_uppercase())
        }
        "casefold" => {
            if !ascii {
                return None;
            }
            kstr(&recv.to_ascii_lowercase())
        }
        "capitalize" => {
            if !ascii {
                return None;
            }
            let mut out = String::new();
            for (i, c) in s.iter().enumerate() {
                out.push(if i == 0 { c.to_ascii_uppercase() } else { c.to_ascii_lowercase() });
            }
            kstr(&out)
        }
        "swapcase" => {
            if !ascii {
                return None;
            }
            let out: String = s
                .iter()
                .map(|c| if c.is_ascii_uppercase() { c.to_ascii_lowercase() } else { c.to_ascii_uppercase() })
                .collect();
            kstr(&out)
        }
        "title" => {
            if !ascii {
                return None;
            }
            kstr(&title_ascii(&s))
        }
        "isascii" => kb(ascii),
        "isspace" => kb(!s.is_empty() && s.iter().all(|&c| py_isspace(c))),
        "isprintable" => {
            for &c in &s {
                if !printable(c)? {
                    return kb(false);
                }
            }
            kb(true)
        }
        "isalnum" | "isalpha" | "isdecimal" | "isdigit" | "isnumeric" | "islower" | "isupper" | "istitle" | "isidentifier" => {
            if !ascii {
                return None;
            }
            let r = match name {
                "isalnum" => !s.is_empty() && s.iter().all(|c| c.is_ascii_alphanumeric()),
                "isalpha" => !s.is_empty() && s.iter().all(|c| c.is_ascii_alphabetic()),
                "isdecimal" | "isdigit" | "isnumeric" => !s.is_empty() && s.iter().all(|c| c.is_ascii_digit()),
                "islower" => s.iter().any(|c| c.is_ascii_lowercase()) && !s.iter().any(|c| c.is_ascii_uppercase()),
                "isupper" => s.iter().any(|c| c.is_ascii_uppercase()) && !s.iter().any(|c| c.is_ascii_lowercase()),
                "istitle" => {
                    let mut prev_cased = false;
                    let mut cased = false;
                    let mut ok = true;
                    for &c in &s {
                        if c.is_ascii_uppercase() {
                            if prev_cased {
                                ok = false;
                                break;
                            }
                            prev_cased = true;
                            cased = true;
                        } else if c.is_ascii_lowercase() {
                            if !prev_cased {
                                ok = false;
                                break;
                            }
                            prev_cased = true;
                            cased = true;
                        } else {
                            prev_cased = false;
                        }
                    }
                    ok && cased
                }
                _ => {
                    !s.is_empty()
                        && (s[0].is_ascii_alphabetic() || s[0] == '_')
                        && s.iter().all(|c| c.is_ascii_alphanumeric() || *c == '_')
                }
            };
            kb(r)
        }
        "center" | "ljust" | "rjust" => {
            let a = bind(args, kwargs, &["width", "fillchar"], 2, 1)?;
            let width = width_arg(a[0]?)?;
            let fill = fill_arg(a[1])?;
            if s.len() >= width {
                return kstr(recv);
            }
            let marg = width - s.len();
            let left = match name {
                "ljust" => 0,
                "rjust" => marg,
                _ => marg / 2 + (marg & width & 1),
            };
            let f = |n: usize| fill.to_string().repeat(n);
            kstr(&format!("{}{}{}", f(left), recv, f(marg - left)))
        }
        "zfill" => {
            let a = bind(args, kwargs, &["width"], 1, 1)?;
            let width = width_arg(a[0]?)?;
            if s.len() >= width {
                return kstr(recv);
            }
            let fill = width - s.len();
            let mut out: Vec<char> = std::iter::repeat_n('0', fill).chain(s.iter().copied()).collect();
            if !s.is_empty() && (s[0] == '+' || s[0] == '-') {
                out[0] = s[0];
                out[fill] = '0';
            }
            kstr(&s_of(&out))
        }
        "count" | "find" | "rfind" | "index" | "rindex" => {
            let a = bind(args, kwargs, &["sub", "start", "end"], 3, 1)?;
            let sub: Vec<char> = str_of(a[0]?)?.chars().collect();
            let (st, en) = adjust(opt_index(a[1])?, opt_index(a[2])?, len);
            let sl = sub.len() as i64;
            if name == "count" {
                if en - st < sl {
                    return k(key_int(0));
                }
                if sub.is_empty() {
                    return k(key_int(en - st + 1));
                }
                let hay = &s[st as usize..en as usize];
                let mut cnt = 0;
                let mut p = 0;
                while let Some(q) = find_in(hay, &sub, p) {
                    cnt += 1;
                    p = q + sub.len();
                }
                return k(key_int(cnt));
            }
            let found = if en - st < sl {
                None
            } else {
                let hay = &s[st as usize..en as usize];
                if name == "find" || name == "index" {
                    find_in(hay, &sub, 0)
                } else {
                    rfind_in(hay, &sub)
                }
                .map(|p| p as i64 + st)
            };
            match (found, name) {
                (Some(p), _) => k(key_int(p)),
                (None, "find" | "rfind") => k(key_int(-1)),
                _ => None,
            }
        }
        "startswith" | "endswith" => {
            let a = bind(args, kwargs, &["prefix", "start", "end"], 3, 1)?;
            let cands: Vec<Arc<str>> = match key_get(a[0]?) {
                Key::Str(x) => vec![x],
                Key::Tuple(t) => t.iter().map(|e| str_of(*e)).collect::<Option<Vec<_>>>()?,
                _ => return None,
            };
            let (st, en) = adjust(opt_index(a[1])?, opt_index(a[2])?, len);
            for c in cands {
                let sub: Vec<char> = c.chars().collect();
                let sl = sub.len() as i64;
                if en - sl < st {
                    continue;
                }
                if sub.is_empty() {
                    return kb(true);
                }
                let hit = if name == "startswith" {
                    s[st as usize..st as usize + sub.len()] == *sub
                } else {
                    s[(en - sl) as usize..en as usize] == *sub
                };
                if hit {
                    return kb(true);
                }
            }
            kb(false)
        }
        "encode" => {
            let a = bind(args, kwargs, &["encoding", "errors"], 0, 0)?;
            let enc = match a[0] {
                None => "utf-8".to_string(),
                Some(e) => str_of(e)?.to_ascii_lowercase().replace('_', "-"),
            };
            if let Some(e) = a[1] {
                if &*str_of(e)? != "strict" {
                    return None;
                }
            }
            let bytes: Vec<u8> = match enc.as_str() {
                "utf-8" | "utf8" => recv.as_bytes().to_vec(),
                "ascii" | "us-ascii" => {
                    if !ascii {
                        return None;
                    }
                    recv.as_bytes().to_vec()
                }
                "latin-1" | "latin1" | "iso-8859-1" | "iso8859-1" | "l1" => {
                    s.iter().map(|&c| u8::try_from(c as u32).ok()).collect::<Option<Vec<u8>>>()?
                }
                _ => return None,
            };
            k(key(Key::Bytes(bytes)))
        }
        "expandtabs" => {
            let a = bind(args, kwargs, &["tabsize"], 0, 0)?;
            let ts = match a[0] {
                None => 8,
                Some(t) => int_of(t)?,
            };
            if ts > MAX_LEN as i64 {
                return None;
            }
            let mut out = String::new();
            let mut col: i64 = 0;
            for &c in &s {
                if c == '\t' {
                    if ts > 0 {
                        let incr = ts - col % ts;
                        col += incr;
                        out.push_str(&" ".repeat(incr as usize));
                    }
                } else {
                    out.push(c);
                    col += 1;
                    if c == '\n' || c == '\r' {
                        col = 0;
                    }
                }
                if out.len() > MAX_LEN {
                    return None;
                }
            }
            kstr(&out)
        }
        "format" => {
            let kw: Vec<(String, KeyId)> = kwargs.iter().map(|(n, v)| (n.to_string(), *v)).collect();
            kstr(&str_format(recv, args, &kw)?)
        }
        "join" => {
            if !kwargs.is_empty() || args.len() != 1 {
                return None;
            }
            let parts: Vec<String> = match key_get(args[0]) {
                Key::Tuple(t) => t.iter().map(|e| str_of(*e).map(|x| x.to_string())).collect::<Option<Vec<_>>>()?,
                Key::Str(x) => x.chars().map(|c| c.to_string()).collect(),
                _ => return None,
            };
            kstr(&parts.join(recv))
        }
        "partition" | "rpartition" => {
            let a = bind(args, kwargs, &["sep"], 1, 1)?;
            let sep: Vec<char> = str_of(a[0]?)?.chars().collect();
            if sep.is_empty() {
                return None;
            }
            let pos = if name == "partition" { find_in(&s, &sep, 0) } else { rfind_in(&s, &sep) };
            let parts = match pos {
                Some(p) => vec![ks(&s[..p]), ks(&sep), ks(&s[p + sep.len()..])],
                None => {
                    let e = key_str_lit("");
                    if name == "partition" {
                        vec![ks(&s), e, e]
                    } else {
                        vec![e, e, ks(&s)]
                    }
                }
            };
            k(key_tuple(parts))
        }
        "removeprefix" | "removesuffix" => {
            let a = bind(args, kwargs, &["fix"], 1, 1)?;
            let fix = str_of(a[0]?)?;
            let r = if name == "removeprefix" { recv.strip_prefix(&*fix) } else { recv.strip_suffix(&*fix) };
            kstr(r.unwrap_or(recv))
        }
        "replace" => {
            let a = bind(args, kwargs, &["old", "new", "count"], 2, 2)?;
            let old: Vec<char> = str_of(a[0]?)?.chars().collect();
            let new = str_of(a[1]?)?;
            let count = match a[2] {
                None => -1,
                Some(c) => int_of(c)?,
            };
            let mut out = String::new();
            let mut done: i64 = 0;
            let limit = |d: i64| count < 0 || d < count;
            if old.is_empty() {
                for (i, c) in s.iter().enumerate() {
                    if limit(done) {
                        out.push_str(&new);
                        done += 1;
                    }
                    out.push(*c);
                    let _ = i;
                    if out.len() > 4 * MAX_LEN {
                        return None;
                    }
                }
                if limit(done) {
                    out.push_str(&new);
                }
                return kstr(&out);
            }
            let mut i = 0;
            while i < s.len() {
                if limit(done) && s[i..].starts_with(&old) {
                    out.push_str(&new);
                    i += old.len();
                    done += 1;
                } else {
                    out.push(s[i]);
                    i += 1;
                }
                if out.len() > 4 * MAX_LEN {
                    return None;
                }
            }
            kstr(&out)
        }
        "split" | "rsplit" => {
            let a = bind(args, kwargs, &["sep", "maxsplit"], 0, 0)?;
            let maxsplit = match a[1] {
                None => -1,
                Some(m) => int_of(m)?,
            };
            let sep = match a[0] {
                None => None,
                Some(x) if key_get(x) == Key::None => None,
                Some(x) => Some(str_of(x)?.chars().collect::<Vec<char>>()),
            };
            let parts = match sep {
                None => {
                    if name == "split" {
                        split_ws(&s, maxsplit)
                    } else {
                        rsplit_ws(&s, maxsplit)
                    }
                }
                Some(sep) => {
                    if sep.is_empty() {
                        return None;
                    }
                    if name == "split" {
                        split_sep(&s, &sep, maxsplit)
                    } else {
                        rsplit_sep(&s, &sep, maxsplit)
                    }
                }
            };
            Some(PyResult::List(parts))
        }
        "splitlines" => {
            let a = bind(args, kwargs, &["keepends"], 0, 0)?;
            let keep = match a[0] {
                None => false,
                Some(x) => truthy(x)?,
            };
            let mut out = Vec::new();
            let mut i = 0;
            let n = s.len();
            while i < n {
                let start = i;
                while i < n && !is_line_break(s[i]) {
                    i += 1;
                }
                let eol = i;
                if i < n {
                    if s[i] == '\r' && i + 1 < n && s[i + 1] == '\n' {
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                out.push(ks(&s[start..if keep { i } else { eol }]));
            }
            Some(PyResult::List(out))
        }
        "strip" | "lstrip" | "rstrip" => {
            let a = bind(args, kwargs, &["chars"], 1, 0)?;
            let set = strip_set(a[0])?;
            let strip = |c: char| match &set {
                None => py_isspace(c),
                Some(v) => v.contains(&c),
            };
            let mut lo = 0;
            let mut hi = s.len();
            if name != "rstrip" {
                while lo < hi && strip(s[lo]) {
                    lo += 1;
                }
            }
            if name != "lstrip" {
                while hi > lo && strip(s[hi - 1]) {
                    hi -= 1;
                }
            }
            k(ks(&s[lo..hi]))
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// math
// ---------------------------------------------------------------------------

fn fsum(values: &[KeyId]) -> Option<f64> {
    let mut p: Vec<f64> = Vec::new();
    let mut special_sum = 0.0f64;
    let mut inf_sum = 0.0f64;
    for &v in values {
        let mut x = fl(num(v)?);
        let xsave = x;
        let mut i = 0;
        for j in 0..p.len() {
            let mut y = p[j];
            if x.abs() < y.abs() {
                std::mem::swap(&mut x, &mut y);
            }
            let hi = x + y;
            let yr = hi - x;
            let lo = y - yr;
            if lo != 0.0 {
                p[i] = lo;
                i += 1;
            }
            x = hi;
        }
        p.truncate(i);
        if x != 0.0 {
            if !x.is_finite() {
                if xsave.is_finite() {
                    return None;
                }
                if xsave.is_infinite() {
                    inf_sum += xsave;
                }
                special_sum += xsave;
                p.clear();
            } else {
                p.push(x);
            }
        }
    }
    if special_sum != 0.0 {
        if inf_sum.is_nan() {
            return None;
        }
        return Some(special_sum);
    }
    let mut hi = 0.0f64;
    let mut n = p.len();
    if n > 0 {
        n -= 1;
        hi = p[n];
        let mut lo = 0.0f64;
        while n > 0 {
            let x = hi;
            n -= 1;
            let y = p[n];
            hi = x + y;
            let yr = hi - x;
            lo = y - yr;
            if lo != 0.0 {
                break;
            }
        }
        if n > 0 && ((lo < 0.0 && p[n - 1] < 0.0) || (lo > 0.0 && p[n - 1] > 0.0)) {
            let y = lo * 2.0;
            let x = hi + y;
            let yr = x - hi;
            if y == yr {
                hi = x;
            }
        }
    }
    Some(hi)
}

/// CPython's `math_1`: errors when a finite input gives nan / inf.
fn math1(x: f64, f: fn(f64) -> f64) -> Option<f64> {
    let r = f(x);
    if r.is_nan() && !x.is_nan() {
        return None;
    }
    if r.is_infinite() && x.is_finite() {
        return None;
    }
    Some(r)
}

/// CPython's `m_log`-style helpers for log / log2 / log10.
fn mlog(x: f64, f: fn(f64) -> f64) -> Option<f64> {
    if x.is_nan() {
        return Some(x);
    }
    if x.is_infinite() {
        return if x > 0.0 { Some(x) } else { None };
    }
    if x <= 0.0 {
        return None;
    }
    Some(f(x))
}

fn floor_like(n: Num, f: fn(f64) -> f64) -> Option<KeyId> {
    match n {
        Num::I(i) => Some(key_int(i)),
        Num::F(x) => Some(key_int(float_to_int(f(x))?)),
    }
}

fn nonneg_int(k: KeyId) -> Option<u64> {
    if is_float(k) {
        return None;
    }
    u64::try_from(int_of(k)?).ok()
}

fn comb_perm(n: u64, k: u64, comb: bool) -> Option<i64> {
    if k > n {
        return Some(0);
    }
    let kk = if comb { k.min(n - k) } else { k };
    let mut r: u128 = 1;
    for i in 0..kk {
        r = r.checked_mul((n - i) as u128)?;
        if comb {
            r /= (i + 1) as u128;
        }
        if r > i64::MAX as u128 * (kk as u128 + 1) {
            return None;
        }
    }
    i64::try_from(r).ok()
}

/// `math.<name>(…)` on literal numbers.
pub fn math_call(name: &str, args: &[KeyId]) -> Option<KeyId> {
    let one = || -> Option<Num> {
        match args {
            [a] => num(*a),
            _ => None,
        }
    };
    let two = || -> Option<(Num, Num)> {
        match args {
            [a, b] => Some((num(*a)?, num(*b)?)),
            _ => None,
        }
    };
    let fkey = |f: f64| Some(key_float(f));
    match name {
        "ceil" => floor_like(one()?, f64::ceil),
        "floor" => floor_like(one()?, f64::floor),
        "trunc" => floor_like(one()?, f64::trunc),
        "sqrt" => fkey(math1(fl(one()?), f64::sqrt)?),
        "exp" => fkey(math1(fl(one()?), f64::exp)?),
        "sin" => fkey(math1(fl(one()?), f64::sin)?),
        "cos" => fkey(math1(fl(one()?), f64::cos)?),
        "tan" => fkey(math1(fl(one()?), f64::tan)?),
        "fabs" => fkey(fl(one()?).abs()),
        "degrees" => fkey(fl(one()?) * (180.0 / std::f64::consts::PI)),
        "radians" => fkey(fl(one()?) * (std::f64::consts::PI / 180.0)),
        "log2" => fkey(mlog(fl(one()?), f64::log2)?),
        "log10" => fkey(mlog(fl(one()?), f64::log10)?),
        "log" => match args {
            [x] => fkey(mlog(fl(num(*x)?), f64::ln)?),
            [x, b] => {
                let n = mlog(fl(num(*x)?), f64::ln)?;
                let d = mlog(fl(num(*b)?), f64::ln)?;
                if d == 0.0 {
                    return None;
                }
                fkey(n / d)
            }
            _ => None,
        },
        "pow" => {
            let (a, b) = two()?;
            fkey(float_pow(fl(a), fl(b))?)
        }
        "copysign" => {
            let (a, b) = two()?;
            fkey(fl(a).copysign(fl(b)))
        }
        "fmod" => {
            let (a, b) = two()?;
            let (x, y) = (fl(a), fl(b));
            let r = x % y;
            if r.is_nan() && !x.is_nan() && !y.is_nan() {
                return None;
            }
            fkey(r)
        }
        "isfinite" => Some(key_bool(fl(one()?).is_finite())),
        "isinf" => Some(key_bool(fl(one()?).is_infinite())),
        "isnan" => Some(key_bool(fl(one()?).is_nan())),
        "isclose" => {
            let (a, b) = two()?;
            let (a, b) = (fl(a), fl(b));
            if a == b {
                return Some(key_bool(true));
            }
            if a.is_infinite() || b.is_infinite() {
                return Some(key_bool(false));
            }
            let rel = 1e-09;
            let diff = (b - a).abs();
            Some(key_bool(diff <= (rel * b).abs() || diff <= (rel * a).abs() || diff <= 0.0))
        }
        "isqrt" => {
            let n = nonneg_int(*args.first()?)?;
            if args.len() != 1 {
                return None;
            }
            Some(key_int(isqrt_u128(n as u128) as i64))
        }
        "factorial" => {
            let n = nonneg_int(*args.first()?)?;
            if args.len() != 1 {
                return None;
            }
            let mut r: i64 = 1;
            for i in 2..=n {
                r = r.checked_mul(i as i64)?;
            }
            Some(key_int(r))
        }
        "gcd" | "lcm" => {
            let mut acc: u128 = if name == "gcd" { 0 } else { 1 };
            for &a in args {
                if is_float(a) {
                    return None;
                }
                let v = (int_of(a)? as i128).unsigned_abs();
                if name == "gcd" {
                    acc = gcd_u128(acc, v);
                } else if acc == 0 || v == 0 {
                    acc = 0;
                } else {
                    acc = (acc / gcd_u128(acc, v)).checked_mul(v)?;
                    if acc > i64::MAX as u128 {
                        return None;
                    }
                }
            }
            Some(key_int(i64::try_from(acc).ok()?))
        }
        "comb" => match args {
            [n, k] => Some(key_int(comb_perm(nonneg_int(*n)?, nonneg_int(*k)?, true)?)),
            _ => None,
        },
        "perm" => match args {
            [n] => {
                let n = nonneg_int(*n)?;
                Some(key_int(comb_perm(n, n, false)?))
            }
            [n, k] if key_get(*k) == Key::None => {
                let n = nonneg_int(*n)?;
                Some(key_int(comb_perm(n, n, false)?))
            }
            [n, k] => Some(key_int(comb_perm(nonneg_int(*n)?, nonneg_int(*k)?, false)?)),
            _ => None,
        },
        "hypot" => {
            let xs: Vec<f64> = args.iter().map(|a| num(*a).map(fl)).collect::<Option<Vec<_>>>()?;
            if xs.iter().any(|x| x.is_infinite()) {
                return fkey(f64::INFINITY);
            }
            if xs.iter().any(|x| x.is_nan()) {
                return fkey(f64::NAN);
            }
            match xs.len() {
                0 => fkey(0.0),
                1 => fkey(xs[0].abs()),
                _ => {
                    // exact only: integral coordinates whose sum of squares is a float
                    let mut sum: u128 = 0;
                    for x in &xs {
                        if *x != x.trunc() || x.abs() > 67108864.0 {
                            return None;
                        }
                        let v = x.abs() as u128;
                        sum += v * v;
                    }
                    if sum > (1u128 << 53) {
                        return None;
                    }
                    fkey((sum as f64).sqrt())
                }
            }
        }
        "prod" => {
            // `start` is keyword-only in Python: a second positional argument raises
            let (items, start) = match args {
                [it] => (*it, key_int(1)),
                _ => return None,
            };
            let Key::Tuple(t) = key_get(items) else { return None };
            num(start)?;
            let mut acc = start;
            for e in t {
                num(e)?;
                acc = binop("mul", acc, e)?;
            }
            Some(acc)
        }
        "fsum" => match args {
            [it] => {
                let Key::Tuple(t) = key_get(*it) else { return None };
                fkey(fsum(&t)?)
            }
            _ => None,
        },
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// statistics
// ---------------------------------------------------------------------------

/// Exact numerators over a common power of two: (N_i, E, any_float).
fn aligned(values: &[KeyId]) -> Option<(Vec<i128>, i32, bool)> {
    let mut parts = Vec::with_capacity(values.len());
    let mut any_float = false;
    for &v in values {
        let n = num(v)?;
        if matches!(n, Num::F(_)) {
            any_float = true;
        }
        parts.push(exact_parts(n)?);
    }
    let e_min = parts.iter().filter(|(m, _)| *m != 0).map(|(_, e)| *e).min().unwrap_or(0);
    let mut out = Vec::with_capacity(parts.len());
    for (m, e) in parts {
        if m == 0 {
            out.push(0);
            continue;
        }
        let sh = e - e_min;
        if sh as u32 + bitlen(m.unsigned_abs()) as u32 > 100 {
            return None;
        }
        out.push(m << sh);
    }
    Some((out, e_min, any_float))
}

/// CPython's `_convert` of the exact value num / den × 2^exp2 to int (when exact and all
/// inputs were ints) or float.
fn convert(num_: i128, den: u128, exp2: i32, int_type: bool) -> Option<KeyId> {
    if int_type && exp2 == 0 && num_.unsigned_abs().is_multiple_of(den) {
        let q = num_ / den as i128;
        return Some(key_int(i64::try_from(q).ok()?));
    }
    Some(key_float(frac_to_f64(num_ < 0, num_.unsigned_abs(), den, exp2)?))
}

fn sorted_values(values: &[KeyId]) -> Option<Vec<KeyId>> {
    for &v in values {
        match key_get(v) {
            Key::Float(b) if f64::from_bits(b).is_nan() => return None,
            Key::Int(_) | Key::Float(_) | Key::Bool(_) | Key::Str(_) => {}
            _ => return None,
        }
    }
    let mut v = values.to_vec();
    let mut err = false;
    v.sort_by(|a, b| match py_lt(*a, *b) {
        Some(true) => Ordering::Less,
        Some(false) => match py_lt(*b, *a) {
            Some(true) => Ordering::Greater,
            Some(false) => Ordering::Equal,
            None => {
                err = true;
                Ordering::Equal
            }
        },
        None => {
            err = true;
            Ordering::Equal
        }
    });
    if err {
        return None;
    }
    Some(v)
}

/// `statistics.<name>(values)` when the result is exactly what CPython computes.
pub fn statistics_call(name: &str, values: &[KeyId]) -> Option<KeyId> {
    let n = values.len();
    match name {
        "mean" => {
            if n == 0 {
                return None;
            }
            let (ns, e, any_float) = aligned(values)?;
            let mut s: i128 = 0;
            for x in ns {
                s = s.checked_add(x)?;
            }
            convert(s, n as u128, e, !any_float)
        }
        "fmean" => {
            if n == 0 {
                return None;
            }
            Some(key_float(fsum(values)? / n as f64))
        }
        "median" | "median_low" | "median_high" => {
            if n == 0 {
                return None;
            }
            let v = sorted_values(values)?;
            match name {
                "median_low" => Some(v[if n % 2 == 1 { n / 2 } else { n / 2 - 1 }]),
                "median_high" => Some(v[n / 2]),
                _ => {
                    if n % 2 == 1 {
                        Some(v[n / 2])
                    } else {
                        let s = binop("add", v[n / 2 - 1], v[n / 2])?;
                        num(s)?;
                        binop("truediv", s, key_int(2))
                    }
                }
            }
        }
        "mode" => {
            let mut groups: Vec<(KeyId, usize)> = Vec::new();
            for &v in values {
                if let Some(Num::F(f)) = num(v) {
                    if f.is_nan() {
                        return None;
                    }
                }
                let mut found = false;
                for g in groups.iter_mut() {
                    if py_eq(g.0, v)? {
                        g.1 += 1;
                        found = true;
                        break;
                    }
                }
                if !found {
                    groups.push((v, 1));
                }
            }
            let mut best: Option<(KeyId, usize)> = None;
            for g in groups {
                if best.is_none_or(|b| g.1 > b.1) {
                    best = Some(g);
                }
            }
            best.map(|b| b.0)
        }
        "pvariance" | "variance" | "pstdev" | "stdev" => {
            let sample = matches!(name, "variance" | "stdev");
            if n < if sample { 2 } else { 1 } {
                return None;
            }
            let (ns, e, any_float) = aligned(values)?;
            let mut s: i128 = 0;
            let mut q: i128 = 0;
            for x in ns {
                s = s.checked_add(x)?;
                q = q.checked_add(x.checked_mul(x)?)?;
            }
            let nn = n as i128;
            let num_ = nn.checked_mul(q)?.checked_sub(s.checked_mul(s)?)?;
            if num_ < 0 {
                return None;
            }
            let den = (nn.checked_mul(if sample { nn - 1 } else { nn })?) as u128;
            let num_u = num_ as u128;
            let g = gcd_u128(num_u, den).max(1);
            let (a, b) = (num_u / g, den / g);
            if name.ends_with("variance") {
                convert(a as i128, b, 2 * e, !any_float)
            } else {
                Some(key_float(ldexp_exact(sqrt_frac(a, b)?, e)?))
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    //! Expectations below were produced by running each Python expression (the third
    //! argument of every assertion) under CPython 3.14; `<err>` means Python raises, so the
    //! model must answer `None`.
    use super::*;
    use crate::intern::{key_inv, key_none};

    fn i(v: i64) -> KeyId {
        key_int(v)
    }
    fn f(v: f64) -> KeyId {
        key_float(v)
    }
    fn s(v: &str) -> KeyId {
        key_str_lit(v)
    }
    fn bo(v: bool) -> KeyId {
        key_bool(v)
    }
    fn nn() -> KeyId {
        key_none()
    }
    fn by(v: &[u8]) -> KeyId {
        key(Key::Bytes(v.to_vec()))
    }
    fn t(v: &[KeyId]) -> KeyId {
        key_tuple(v.to_vec())
    }
    fn rk(k: Option<KeyId>) -> String {
        match k {
            Some(k) => py_repr(k).unwrap_or_else(|| "<norepr>".into()),
            None => "<err>".into(),
        }
    }
    fn rs(v: Option<String>) -> String {
        match v {
            Some(v) => repr_str(&v).unwrap_or_else(|| "<norepr>".into()),
            None => "<err>".into(),
        }
    }
    fn rr(v: Option<PyResult>) -> String {
        match v {
            Some(PyResult::Key(k)) => rk(Some(k)),
            Some(PyResult::List(l)) => format!("[{}]", l.iter().map(|k| rk(Some(*k))).collect::<Vec<_>>().join(", ")),
            None => "<err>".into(),
        }
    }
    fn b(name: &str, args: &[KeyId]) -> String {
        rk(builtin(name, args, &[]))
    }
    fn bk(name: &str, args: &[KeyId], kw: &[(&str, KeyId)]) -> String {
        rk(builtin(name, args, kw))
    }
    fn fv(v: KeyId, spec: &str) -> String {
        rs(format_value(v, spec))
    }
    fn sf(fmt: &str, args: &[KeyId], kw: &[(&str, KeyId)]) -> String {
        let kw: Vec<(String, KeyId)> = kw.iter().map(|(n, v)| (n.to_string(), *v)).collect();
        rs(str_format(fmt, args, &kw))
    }
    fn pf(fmt: &str, args: &[KeyId]) -> String {
        rs(percent_format(fmt, PercentArgs::Positional(args)))
    }
    fn pm(fmt: &str, kw: &[(&str, KeyId)]) -> String {
        let kw: Vec<(String, KeyId)> = kw.iter().map(|(n, v)| (n.to_string(), *v)).collect();
        rs(percent_format(fmt, PercentArgs::Mapping(&kw)))
    }
    fn sm(recv: &str, name: &str, args: &[KeyId]) -> String {
        rr(str_method(recv, name, args, &[]))
    }
    fn smk(recv: &str, name: &str, args: &[KeyId], kw: &[(&str, KeyId)]) -> String {
        rr(str_method(recv, name, args, kw))
    }
    fn m(name: &str, args: &[KeyId]) -> String {
        rk(math_call(name, args))
    }
    fn op(name: &str, args: &[KeyId]) -> String {
        rk(operator_call(name, args))
    }
    fn st(name: &str, values: &[KeyId]) -> String {
        rk(statistics_call(name, values))
    }

    /// Cases this module declines on purpose although Python has an answer.
    #[test]
    fn declined() {
        assert_eq!(sm("éa", "title", &[]), "<err>");
        assert_eq!(sm("ǅ", "isalpha", &[]), "<err>");
        assert_eq!(sm("abc", "translate", &[]), "<err>");
        assert_eq!(sm("{a}", "format_map", &[]), "<err>");
        assert_eq!(sf("{0.real}", &[i(1)], &[]), "<err>");
        assert_eq!(sf("{0[0]}", &[t(&[i(1)])], &[]), "<err>");
        assert_eq!(st("multimode", &[i(1), i(1)]), "<err>");
        assert_eq!(m("hypot", &[f(0.1), f(0.2)]), "<err>");
        assert_eq!(fv(f(1.5), ".3_f"), "<err>");
        assert_eq!(b("str", &[key_inv("f", i(1))]), "<err>");
    }

    #[test]
    fn float_repr_shortest() {
        for (x, want) in [(0.1, "0.1"), (1e16, "1e+16"), (1e15, "1000000000000000.0"), (1e-5, "1e-05"), (123.456, "123.456"), (5e-324, "5e-324")] {
            assert_eq!(float_repr(x), want);
        }
    }

    fn chk(bad: &mut Vec<String>, got: String, want: &str, py: &str) {
        if got != want {
            bad.push(format!("{py}\n    python: {want}\n    rowan:  {got}"));
        }
    }

    #[test]
    fn cpython_cases() {
        let mut bad = Vec::new();
        cases(&mut bad);
        assert!(bad.is_empty(), "{} mismatches:\n{}", bad.len(), bad.join("\n"));
    }

    #[rustfmt::skip]
    #[allow(clippy::approx_constant)]
    fn cases(bad: &mut Vec<String>) {
        chk(bad, b("abs", &[i(-5)]), r#"5"#, r#"abs(-5)"#);
        chk(bad, b("abs", &[f(-2.5)]), r#"2.5"#, r#"abs(-2.5)"#);
        chk(bad, b("abs", &[bo(true)]), r#"1"#, r#"abs(True)"#);
        chk(bad, b("round", &[f(2.5)]), r#"2"#, r#"round(2.5)"#);
        chk(bad, b("round", &[f(3.5)]), r#"4"#, r#"round(3.5)"#);
        chk(bad, b("round", &[f(-2.5)]), r#"-2"#, r#"round(-2.5)"#);
        chk(bad, b("round", &[f(2.675), i(2)]), r#"2.67"#, r#"round(2.675, 2)"#);
        chk(bad, b("round", &[f(0.125), i(2)]), r#"0.12"#, r#"round(0.125, 2)"#);
        chk(bad, b("round", &[f(0.375), i(2)]), r#"0.38"#, r#"round(0.375, 2)"#);
        chk(bad, b("round", &[f(-0.4), i(0)]), r#"-0.0"#, r#"round(-0.4, 0)"#);
        chk(bad, b("round", &[f(1234.5678), i(-2)]), r#"1200.0"#, r#"round(1234.5678, -2)"#);
        chk(bad, b("round", &[i(1250), i(-2)]), r#"1200"#, r#"round(1250, -2)"#);
        chk(bad, b("round", &[i(1350), i(-2)]), r#"1400"#, r#"round(1350, -2)"#);
        chk(bad, b("round", &[i(-1250), i(-2)]), r#"-1200"#, r#"round(-1250, -2)"#);
        chk(bad, b("round", &[i(7), i(3)]), r#"7"#, r#"round(7, 3)"#);
        chk(bad, b("round", &[f(1e300)]), r#"<err>"#, r#"round(1e300) [declined; Python gives 1000000000000000052504760255204420248704]"#);
        chk(bad, b("round", &[f(f64::INFINITY)]), r#"<err>"#, r#"round(float('inf'))"#);
        chk(bad, b("round", &[f(f64::INFINITY), i(2)]), r#"inf"#, r#"round(float('inf'), 2)"#);
        chk(bad, b("round", &[f(5e-324), i(400)]), r#"5e-324"#, r#"round(5e-324, 400)"#);
        chk(bad, b("round", &[f(123.456), nn()]), r#"123"#, r#"round(123.456, None)"#);
        chk(bad, b("round", &[f(0.5)]), r#"0"#, r#"round(0.5)"#);
        chk(bad, b("round", &[f(1.5)]), r#"2"#, r#"round(1.5)"#);
        chk(bad, b("round", &[f(2.345), i(2)]), r#"2.35"#, r#"round(2.345, 2)"#);
        chk(bad, b("round", &[f(1.0000000000000002), i(15)]), r#"1.0"#, r#"round(1.0000000000000002, 15)"#);
        chk(bad, b("pow", &[i(2), i(10)]), r#"1024"#, r#"pow(2, 10)"#);
        chk(bad, b("pow", &[i(2), i(-2)]), r#"0.25"#, r#"pow(2, -2)"#);
        chk(bad, b("pow", &[i(2), i(63)]), r#"<err>"#, r#"pow(2, 63) [declined; Python gives 9223372036854775808]"#);
        chk(bad, b("pow", &[i(-2), i(63)]), r#"-9223372036854775808"#, r#"pow(-2, 63)"#);
        chk(bad, b("pow", &[i(3), i(4), i(5)]), r#"1"#, r#"pow(3, 4, 5)"#);
        chk(bad, b("pow", &[i(3), i(2), i(-5)]), r#"-1"#, r#"pow(3, 2, -5)"#);
        chk(bad, b("pow", &[i(3), i(-1), i(7)]), r#"5"#, r#"pow(3, -1, 7)"#);
        chk(bad, b("pow", &[i(2), i(-1), i(4)]), r#"<err>"#, r#"pow(2, -1, 4)"#);
        chk(bad, b("pow", &[i(0), i(-1)]), r#"<err>"#, r#"pow(0, -1)"#);
        chk(bad, b("pow", &[f(-8.0), f(1.0/3.0)]), r#"<err>"#, r#"pow(-8.0, 1/3) [declined; Python gives (1.0000000000000002+1.7320508075688772j)]"#);
        chk(bad, b("pow", &[f(2.0), f(0.5)]), r#"1.4142135623730951"#, r#"pow(2.0, 0.5)"#);
        chk(bad, b("pow", &[f(10.0), i(400)]), r#"<err>"#, r#"pow(10.0, 400)"#);
        chk(bad, b("pow", &[f(-2.0), i(3)]), r#"-8.0"#, r#"pow(-2.0, 3)"#);
        chk(bad, b("divmod", &[i(7), i(-2)]), r#"(-4, -1)"#, r#"divmod(7, -2)"#);
        chk(bad, b("divmod", &[i(-7), i(2)]), r#"(-4, 1)"#, r#"divmod(-7, 2)"#);
        chk(bad, b("divmod", &[f(7.5), i(2)]), r#"(3.0, 1.5)"#, r#"divmod(7.5, 2)"#);
        chk(bad, b("divmod", &[f(-7.5), i(2)]), r#"(-4.0, 0.5)"#, r#"divmod(-7.5, 2)"#);
        chk(bad, b("divmod", &[i(1), i(0)]), r#"<err>"#, r#"divmod(1, 0)"#);
        chk(bad, b("divmod", &[f(0.0), f(-3.0)]), r#"(-0.0, -0.0)"#, r#"divmod(0.0, -3.0)"#);
        chk(bad, b("min", &[i(3), f(1.0), i(1), i(2)]), r#"1.0"#, r#"min(3, 1.0, 1, 2)"#);
        chk(bad, b("max", &[i(1), bo(true), i(0)]), r#"1"#, r#"max(1, True, 0)"#);
        chk(bad, b("max", &[s("apple"), s("pear"), s("fig")]), r#"'pear'"#, r#"max('apple', 'pear', 'fig')"#);
        chk(bad, b("min", &[t(&[i(4), i(2), i(9)])]), r#"2"#, r#"min((4, 2, 9))"#);
        chk(bad, b("max", &[s("hello")]), r#"'o'"#, r#"max('hello')"#);
        chk(bad, b("min", &[i(1), s("a")]), r#"<err>"#, r#"min(1, 'a')"#);
        chk(bad, b("max", &[t(&[i(1), i(2)]), t(&[i(1), i(3)])]), r#"(1, 3)"#, r#"max((1, 2), (1, 3))"#);
        chk(bad, b("int", &[s("  42 ")]), r#"42"#, r#"int('  42 ')"#);
        chk(bad, b("int", &[s("-1_000")]), r#"-1000"#, r#"int('-1_000')"#);
        chk(bad, b("int", &[s("1__0")]), r#"<err>"#, r#"int('1__0')"#);
        chk(bad, b("int", &[s("ff"), i(16)]), r#"255"#, r#"int('ff', 16)"#);
        chk(bad, b("int", &[s("0x_ff"), i(16)]), r#"255"#, r#"int('0x_ff', 16)"#);
        chk(bad, b("int", &[s("0b101"), i(0)]), r#"5"#, r#"int('0b101', 0)"#);
        chk(bad, b("int", &[s("010"), i(0)]), r#"<err>"#, r#"int('010', 0)"#);
        chk(bad, b("int", &[s("000"), i(0)]), r#"0"#, r#"int('000', 0)"#);
        chk(bad, b("int", &[s("z"), i(36)]), r#"35"#, r#"int('z', 36)"#);
        chk(bad, b("int", &[s("12.5")]), r#"<err>"#, r#"int('12.5')"#);
        chk(bad, b("int", &[f(-3.9)]), r#"-3"#, r#"int(-3.9)"#);
        chk(bad, b("int", &[bo(true)]), r#"1"#, r#"int(True)"#);
        chk(bad, b("int", &[s("9223372036854775807")]), r#"9223372036854775807"#, r#"int('9223372036854775807')"#);
        chk(bad, b("int", &[s("-9223372036854775808")]), r#"-9223372036854775808"#, r#"int('-9223372036854775808')"#);
        chk(bad, b("float", &[s(" 1e3 ")]), r#"1000.0"#, r#"float(' 1e3 ')"#);
        chk(bad, b("float", &[s("-inf")]), r#"-inf"#, r#"float('-inf')"#);
        chk(bad, b("float", &[s("Infinity")]), r#"inf"#, r#"float('Infinity')"#);
        chk(bad, b("float", &[s("1_000.5")]), r#"1000.5"#, r#"float('1_000.5')"#);
        chk(bad, b("float", &[s(".5")]), r#"0.5"#, r#"float('.5')"#);
        chk(bad, b("float", &[s("5.")]), r#"5.0"#, r#"float('5.')"#);
        chk(bad, b("float", &[s("1e400")]), r#"inf"#, r#"float('1e400')"#);
        chk(bad, b("float", &[s("abc")]), r#"<err>"#, r#"float('abc')"#);
        chk(bad, b("float", &[s("0.1")]), r#"0.1"#, r#"float('0.1')"#);
        chk(bad, b("float", &[i(7)]), r#"7.0"#, r#"float(7)"#);
        chk(bad, b("str", &[f(1e16)]), r#"'1e+16'"#, r#"str(1e16)"#);
        chk(bad, b("str", &[f(1e-5)]), r#"'1e-05'"#, r#"str(1e-5)"#);
        chk(bad, b("str", &[f(0.0001)]), r#"'0.0001'"#, r#"str(0.0001)"#);
        chk(bad, b("str", &[f(123456789012345678.0)]), r#"'1.2345678901234568e+17'"#, r#"str(123456789012345678.0)"#);
        chk(bad, b("str", &[f(-0.0)]), r#"'-0.0'"#, r#"str(-0.0)"#);
        chk(bad, b("str", &[f(0.1 + 0.2)]), r#"'0.30000000000000004'"#, r#"str(0.1 + 0.2)"#);
        chk(bad, b("str", &[f(1.0/3.0)]), r#"'0.3333333333333333'"#, r#"str(1/3)"#);
        chk(bad, b("str", &[f(2.5e-300)]), r#"'2.5e-300'"#, r#"str(2.5e-300)"#);
        chk(bad, b("str", &[f(1.7976931348623157e308)]), r#"'1.7976931348623157e+308'"#, r#"str(1.7976931348623157e308)"#);
        chk(bad, b("str", &[t(&[i(1), s("a"), f(2.0)])]), r#""(1, 'a', 2.0)""#, r#"str((1, 'a', 2.0))"#);
        chk(bad, b("str", &[t(&[i(1)])]), r#"'(1,)'"#, r#"str((1,))"#);
        chk(bad, b("str", &[nn()]), r#"'None'"#, r#"str(None)"#);
        chk(bad, b("str", &[by(b"it's")]), r#"'b"it\'s"'"#, r#"str(b'it\'s')"#);
        chk(bad, b("repr", &[s("it's")]), r#"'"it\'s"'"#, r#"repr("it's")"#);
        chk(bad, b("repr", &[s("a\nb\x01")]), r#""'a\\nb\\x01'""#, r#"repr('a\nb\x01')"#);
        chk(bad, b("repr", &[s("caf\u{e9}")]), r#""'café'""#, r#"repr('caf\u00e9')"#);
        chk(bad, b("ascii", &[s("caf\u{e9} \u{1f600}")]), r#""'caf\\xe9 \\U0001f600'""#, r#"ascii('caf\u00e9 \U0001f600')"#);
        chk(bad, b("bool", &[s("")]), r#"False"#, r#"bool('')"#);
        chk(bad, b("bool", &[f(0.0)]), r#"False"#, r#"bool(0.0)"#);
        chk(bad, b("bool", &[t(&[i(0)])]), r#"True"#, r#"bool((0,))"#);
        chk(bad, b("chr", &[i(955)]), r#"'λ'"#, r#"chr(955)"#);
        chk(bad, b("chr", &[i(-1)]), r#"<err>"#, r#"chr(-1)"#);
        chk(bad, b("ord", &[s("\u{e9}")]), r#"233"#, r#"ord('\u00e9')"#);
        chk(bad, b("ord", &[s("ab")]), r#"<err>"#, r#"ord('ab')"#);
        chk(bad, b("hex", &[i(-255)]), r#"'-0xff'"#, r#"hex(-255)"#);
        chk(bad, b("oct", &[i(8)]), r#"'0o10'"#, r#"oct(8)"#);
        chk(bad, b("bin", &[i(10)]), r#"'0b1010'"#, r#"bin(10)"#);
        chk(bad, b("hex", &[i(i64::MIN)]), r#"'-0x8000000000000000'"#, r#"hex(-9223372036854775808)"#);
        chk(bad, b("len", &[s("h\u{e9}llo")]), r#"5"#, r#"len('h\u00e9llo')"#);
        chk(bad, b("len", &[t(&[i(1), i(2)])]), r#"2"#, r#"len((1, 2))"#);
        chk(bad, b("format", &[f(3.14159), s(".2f")]), r#"'3.14'"#, r#"format(3.14159, '.2f')"#);
        chk(bad, b("format", &[i(42)]), r#"'42'"#, r#"format(42)"#);
        chk(bad, bk("int", &[s("ff")], &[("base", i(16))]), r#"255"#, r#"int('ff', base=16)"#);
        chk(bad, bk("round", &[f(2.567)], &[("ndigits", i(1))]), r#"2.6"#, r#"round(2.567, ndigits=1)"#);
        chk(bad, fv(f(1234.5), r#",.2f"#), r#"'1,234.50'"#, r#"format(1234.5, ',.2f')"#);
        chk(bad, fv(f(1234.5), r#"_.3f"#), r#"'1_234.500'"#, r#"format(1234.5, '_.3f')"#);
        chk(bad, fv(f(1234.5), r#"10.1f"#), r#"'    1234.5'"#, r#"format(1234.5, '10.1f')"#);
        chk(bad, fv(f(1234.5), r#"<10.1f"#), r#"'1234.5    '"#, r#"format(1234.5, '<10.1f')"#);
        chk(bad, fv(f(1234.5), r#"^12.2e"#), r#"'  1.23e+03  '"#, r#"format(1234.5, '^12.2e')"#);
        chk(bad, fv(f(1234.5), r#"+.3g"#), r#"'+1.23e+03'"#, r#"format(1234.5, '+.3g')"#);
        chk(bad, fv(f(1234.5), r#"e"#), r#"'1.234500e+03'"#, r#"format(1234.5, 'e')"#);
        chk(bad, fv(f(1234.5), r#"E"#), r#"'1.234500E+03'"#, r#"format(1234.5, 'E')"#);
        chk(bad, fv(f(1234.5), r#"g"#), r#"'1234.5'"#, r#"format(1234.5, 'g')"#);
        chk(bad, fv(f(1234.5), r#"G"#), r#"'1234.5'"#, r#"format(1234.5, 'G')"#);
        chk(bad, fv(f(1234.5), r#"%"#), r#"'123450.000000%'"#, r#"format(1234.5, '%')"#);
        chk(bad, fv(f(1234.5), r#".1%"#), r#"'123450.0%'"#, r#"format(1234.5, '.1%')"#);
        chk(bad, fv(f(1234.5), r#"015,.2f"#), r#"'0,000,001,234.50'"#, r#"format(1234.5, '015,.2f')"#);
        chk(bad, fv(f(1234.5), r#"#.0f"#), r#"'1234.'"#, r#"format(1234.5, '#.0f')"#);
        chk(bad, fv(f(1234.5), r#".0f"#), r#"'1234'"#, r#"format(1234.5, '.0f')"#);
        chk(bad, fv(f(1234.5), r#""#), r#"'1234.5'"#, r#"format(1234.5, '')"#);
        chk(bad, fv(f(1234.5), r#".3"#), r#"'1.23e+03'"#, r#"format(1234.5, '.3')"#);
        chk(bad, fv(f(1234.5), r#".10g"#), r#"'1234.5'"#, r#"format(1234.5, '.10g')"#);
        chk(bad, fv(f(1234.5), r#"n"#), r#"'1234.5'"#, r#"format(1234.5, 'n')"#);
        chk(bad, fv(f(1234.5), r#"=+12.1f"#), r#"'+     1234.5'"#, r#"format(1234.5, '=+12.1f')"#);
        chk(bad, fv(f(1234.5), r#"*>12,"#), r#"'*****1,234.5'"#, r#"format(1234.5, '*>12,')"#);
        chk(bad, fv(f(0.000012345), r#""#), r#"'1.2345e-05'"#, r#"format(0.000012345, '')"#);
        chk(bad, fv(f(0.000012345), r#"g"#), r#"'1.2345e-05'"#, r#"format(0.000012345, 'g')"#);
        chk(bad, fv(f(0.000012345), r#".2g"#), r#"'1.2e-05'"#, r#"format(0.000012345, '.2g')"#);
        chk(bad, fv(f(0.000012345), r#"e"#), r#"'1.234500e-05'"#, r#"format(0.000012345, 'e')"#);
        chk(bad, fv(f(0.000012345), r#".3e"#), r#"'1.234e-05'"#, r#"format(0.000012345, '.3e')"#);
        chk(bad, fv(f(0.000012345), r#"f"#), r#"'0.000012'"#, r#"format(0.000012345, 'f')"#);
        chk(bad, fv(f(0.000012345), r#"#g"#), r#"'1.23450e-05'"#, r#"format(0.000012345, '#g')"#);
        chk(bad, fv(f(0.000012345), r#".0e"#), r#"'1e-05'"#, r#"format(0.000012345, '.0e')"#);
        chk(bad, fv(f(-0.0), r#""#), r#"'-0.0'"#, r#"format(-0.0, '')"#);
        chk(bad, fv(f(-0.0), r#"f"#), r#"'-0.000000'"#, r#"format(-0.0, 'f')"#);
        chk(bad, fv(f(-0.0), r#"z.1f"#), r#"'0.0'"#, r#"format(-0.0, 'z.1f')"#);
        chk(bad, fv(f(-0.0), r#"+.2f"#), r#"'-0.00'"#, r#"format(-0.0, '+.2f')"#);
        chk(bad, fv(f(-0.0), r#"g"#), r#"'-0'"#, r#"format(-0.0, 'g')"#);
        chk(bad, fv(f(-0.0001), r#"z.2f"#), r#"'0.00'"#, r#"format(-0.0001, 'z.2f')"#);
        chk(bad, fv(f(-0.0001), r#".2f"#), r#"'-0.00'"#, r#"format(-0.0001, '.2f')"#);
        chk(bad, fv(f(-0.0001), r#"z.0e"#), r#"'-1e-04'"#, r#"format(-0.0001, 'z.0e')"#);
        chk(bad, fv(f(f64::INFINITY), r#""#), r#"'inf'"#, r#"format(float('inf'), '')"#);
        chk(bad, fv(f(f64::INFINITY), r#"f"#), r#"'inf'"#, r#"format(float('inf'), 'f')"#);
        chk(bad, fv(f(f64::INFINITY), r#"010"#), r#"'0000000inf'"#, r#"format(float('inf'), '010')"#);
        chk(bad, fv(f(f64::INFINITY), r#"F"#), r#"'INF'"#, r#"format(float('inf'), 'F')"#);
        chk(bad, fv(f(f64::INFINITY), r#"+e"#), r#"'+inf'"#, r#"format(float('inf'), '+e')"#);
        chk(bad, fv(f(f64::INFINITY), r#"^9"#), r#"'   inf   '"#, r#"format(float('inf'), '^9')"#);
        chk(bad, fv(f(f64::NAN), r#""#), r#"'nan'"#, r#"format(float('nan'), '')"#);
        chk(bad, fv(f(f64::NAN), r#"f"#), r#"'nan'"#, r#"format(float('nan'), 'f')"#);
        chk(bad, fv(f(f64::NAN), r#"+08"#), r#"'+0000nan'"#, r#"format(float('nan'), '+08')"#);
        chk(bad, fv(f(f64::NAN), r#"G"#), r#"'NAN'"#, r#"format(float('nan'), 'G')"#);
        chk(bad, fv(f(1e16), r#""#), r#"'1e+16'"#, r#"format(1e16, '')"#);
        chk(bad, fv(f(1e16), r#".3"#), r#"'1e+16'"#, r#"format(1e16, '.3')"#);
        chk(bad, fv(f(1e16), r#"f"#), r#"'10000000000000000.000000'"#, r#"format(1e16, 'f')"#);
        chk(bad, fv(f(1e16), r#"g"#), r#"'1e+16'"#, r#"format(1e16, 'g')"#);
        chk(bad, fv(f(1e16), r#",.0f"#), r#"'10,000,000,000,000,000'"#, r#"format(1e16, ',.0f')"#);
        chk(bad, fv(f(123.0), r#".3"#), r#"'1.23e+02'"#, r#"format(123.0, '.3')"#);
        chk(bad, fv(f(123.0), r#".2"#), r#"'1.2e+02'"#, r#"format(123.0, '.2')"#);
        chk(bad, fv(f(123.0), r#".4"#), r#"'123.0'"#, r#"format(123.0, '.4')"#);
        chk(bad, fv(f(123.0), r#"#.3g"#), r#"'123.'"#, r#"format(123.0, '#.3g')"#);
        chk(bad, fv(f(123.0), r#"g"#), r#"'123'"#, r#"format(123.0, 'g')"#);
        chk(bad, fv(f(0.5), r#".0f"#), r#"'0'"#, r#"format(0.5, '.0f')"#);
        chk(bad, fv(f(0.5), r#".0e"#), r#"'5e-01'"#, r#"format(0.5, '.0e')"#);
        chk(bad, fv(f(0.5), r#".0%"#), r#"'50%'"#, r#"format(0.5, '.0%')"#);
        chk(bad, fv(f(0.5), r#".1g"#), r#"'0.5'"#, r#"format(0.5, '.1g')"#);
        chk(bad, fv(f(2.5), r#".0f"#), r#"'2'"#, r#"format(2.5, '.0f')"#);
        chk(bad, fv(f(1.5), r#".0f"#), r#"'2'"#, r#"format(1.5, '.0f')"#);
        chk(bad, fv(i(12345678), r#","#), r#"'12,345,678'"#, r#"format(12345678, ',')"#);
        chk(bad, fv(i(12345678), r#"_"#), r#"'12_345_678'"#, r#"format(12345678, '_')"#);
        chk(bad, fv(i(12345678), r#"015,"#), r#"'000,012,345,678'"#, r#"format(12345678, '015,')"#);
        chk(bad, fv(i(12345678), r#"08,"#), r#"'12,345,678'"#, r#"format(12345678, '08,')"#);
        chk(bad, fv(i(12345678), r#"09,"#), r#"'12,345,678'"#, r#"format(12345678, '09,')"#);
        chk(bad, fv(i(12345678), r#"x"#), r#"'bc614e'"#, r#"format(12345678, 'x')"#);
        chk(bad, fv(i(12345678), r#"#x"#), r#"'0xbc614e'"#, r#"format(12345678, '#x')"#);
        chk(bad, fv(i(12345678), r#"#X"#), r#"'0XBC614E'"#, r#"format(12345678, '#X')"#);
        chk(bad, fv(i(12345678), r#"_x"#), r#"'bc_614e'"#, r#"format(12345678, '_x')"#);
        chk(bad, fv(i(12345678), r#"#_b"#), r#"'0b1011_1100_0110_0001_0100_1110'"#, r#"format(12345678, '#_b')"#);
        chk(bad, fv(i(12345678), r#"o"#), r#"'57060516'"#, r#"format(12345678, 'o')"#);
        chk(bad, fv(i(12345678), r#"#o"#), r#"'0o57060516'"#, r#"format(12345678, '#o')"#);
        chk(bad, fv(i(12345678), r#"+d"#), r#"'+12345678'"#, r#"format(12345678, '+d')"#);
        chk(bad, fv(i(12345678), r#" d"#), r#"' 12345678'"#, r#"format(12345678, ' d')"#);
        chk(bad, fv(i(12345678), r#"e"#), r#"'1.234568e+07'"#, r#"format(12345678, 'e')"#);
        chk(bad, fv(i(12345678), r#".2f"#), r#"'12345678.00'"#, r#"format(12345678, '.2f')"#);
        chk(bad, fv(i(12345678), r#"%"#), r#"'1234567800.000000%'"#, r#"format(12345678, '%')"#);
        chk(bad, fv(i(12345678), r#"n"#), r#"'12345678'"#, r#"format(12345678, 'n')"#);
        chk(bad, fv(i(12345678), r#"c"#), r#"<err>"#, r#"format(12345678, 'c')"#);
        chk(bad, fv(i(12345678), r#">12"#), r#"'    12345678'"#, r#"format(12345678, '>12')"#);
        chk(bad, fv(i(12345678), r#"^12"#), r#"'  12345678  '"#, r#"format(12345678, '^12')"#);
        chk(bad, fv(i(12345678), r#"<12"#), r#"'12345678    '"#, r#"format(12345678, '<12')"#);
        chk(bad, fv(i(12345678), r#"=12"#), r#"'    12345678'"#, r#"format(12345678, '=12')"#);
        chk(bad, fv(i(12345678), r#"*^13,"#), r#"'*12,345,678**'"#, r#"format(12345678, '*^13,')"#);
        chk(bad, fv(i(12345678), r#"010x"#), r#"'0000bc614e'"#, r#"format(12345678, '010x')"#);
        chk(bad, fv(i(12345678), r#"#010x"#), r#"'0x00bc614e'"#, r#"format(12345678, '#010x')"#);
        chk(bad, fv(i(12345678), r#",x"#), r#"<err>"#, r#"format(12345678, ',x')"#);
        chk(bad, fv(i(12345678), r#".2"#), r#"<err>"#, r#"format(12345678, '.2')"#);
        chk(bad, fv(i(12345678), r#"z"#), r#"<err>"#, r#"format(12345678, 'z')"#);
        chk(bad, fv(i(-42), r#""#), r#"'-42'"#, r#"format(-42, '')"#);
        chk(bad, fv(i(-42), r#"05"#), r#"'-0042'"#, r#"format(-42, '05')"#);
        chk(bad, fv(i(-42), r#"+05"#), r#"'-0042'"#, r#"format(-42, '+05')"#);
        chk(bad, fv(i(-42), r#"x"#), r#"'-2a'"#, r#"format(-42, 'x')"#);
        chk(bad, fv(i(-42), r#"#x"#), r#"'-0x2a'"#, r#"format(-42, '#x')"#);
        chk(bad, fv(i(-42), r#"<6"#), r#"'-42   '"#, r#"format(-42, '<6')"#);
        chk(bad, fv(i(-42), r#"=6"#), r#"'-   42'"#, r#"format(-42, '=6')"#);
        chk(bad, fv(i(-42), r#"b"#), r#"'-101010'"#, r#"format(-42, 'b')"#);
        chk(bad, fv(i(-42), r#"08_b"#), r#"'-10_1010'"#, r#"format(-42, '08_b')"#);
        chk(bad, fv(i(-42), r#"c"#), r#"<err>"#, r#"format(-42, 'c')"#);
        chk(bad, fv(i(1234), r#"08,"#), r#"'0,001,234'"#, r#"format(1234, '08,')"#);
        chk(bad, fv(i(1234), r#"09,"#), r#"'0,001,234'"#, r#"format(1234, '09,')"#);
        chk(bad, fv(i(1234), r#"010,"#), r#"'00,001,234'"#, r#"format(1234, '010,')"#);
        chk(bad, fv(i(1234), r#"07,"#), r#"'001,234'"#, r#"format(1234, '07,')"#);
        chk(bad, fv(i(1234), r#"012_"#), r#"'0_000_001_234'"#, r#"format(1234, '012_')"#);
        chk(bad, fv(i(1234), r#"0="#), r#"'1234'"#, r#"format(1234, '0=')"#);
        chk(bad, fv(i(65), r#"c"#), r#"'A'"#, r#"format(65, 'c')"#);
        chk(bad, fv(i(65), r#"05c"#), r#"'0000A'"#, r#"format(65, '05c')"#);
        chk(bad, fv(i(65), r#"^5c"#), r#"'  A  '"#, r#"format(65, '^5c')"#);
        chk(bad, fv(i(65), r#"+c"#), r#"<err>"#, r#"format(65, '+c')"#);
        chk(bad, fv(bo(true), r#""#), r#"'True'"#, r#"format(True, '')"#);
        chk(bad, fv(bo(true), r#">5"#), r#"'    1'"#, r#"format(True, '>5')"#);
        chk(bad, fv(bo(true), r#"d"#), r#"'1'"#, r#"format(True, 'd')"#);
        chk(bad, fv(bo(true), r#"x"#), r#"'1'"#, r#"format(True, 'x')"#);
        chk(bad, fv(bo(true), r#".2f"#), r#"'1.00'"#, r#"format(True, '.2f')"#);
        chk(bad, fv(bo(true), r#"s"#), r#"<err>"#, r#"format(True, 's')"#);
        chk(bad, fv(s("abc"), r#""#), r#"'abc'"#, r#"format('abc', '')"#);
        chk(bad, fv(s("abc"), r#"10"#), r#"'abc       '"#, r#"format('abc', '10')"#);
        chk(bad, fv(s("abc"), r#">10"#), r#"'       abc'"#, r#"format('abc', '>10')"#);
        chk(bad, fv(s("abc"), r#"^10"#), r#"'   abc    '"#, r#"format('abc', '^10')"#);
        chk(bad, fv(s("abc"), r#"*^9"#), r#"'***abc***'"#, r#"format('abc', '*^9')"#);
        chk(bad, fv(s("abc"), r#"05"#), r#"'abc00'"#, r#"format('abc', '05')"#);
        chk(bad, fv(s("abc"), r#".2"#), r#"'ab'"#, r#"format('abc', '.2')"#);
        chk(bad, fv(s("abc"), r#"x<6.1"#), r#"'axxxxx'"#, r#"format('abc', 'x<6.1')"#);
        chk(bad, fv(s("abc"), r#"=5"#), r#"<err>"#, r#"format('abc', '=5')"#);
        chk(bad, fv(s("abc"), r#"+"#), r#"<err>"#, r#"format('abc', '+')"#);
        chk(bad, fv(s("abc"), r#","#), r#"<err>"#, r#"format('abc', ',')"#);
        chk(bad, fv(s("abc"), r#"s"#), r#"'abc'"#, r#"format('abc', 's')"#);
        chk(bad, fv(s("abc"), r#"#"#), r#"<err>"#, r#"format('abc', '#')"#);
        chk(bad, fv(s("abc"), r#"d"#), r#"<err>"#, r#"format('abc', 'd')"#);
        chk(bad, fv(s("h\u{e9}"), r#"5"#), r#"'hé   '"#, r#"format('h\u00e9', '5')"#);
        chk(bad, fv(s("h\u{e9}"), r#"^6"#), r#"'  hé  '"#, r#"format('h\u00e9', '^6')"#);
        chk(bad, fv(nn(), r#""#), r#"'None'"#, r#"format(None, '')"#);
        chk(bad, fv(nn(), r#"5"#), r#"<err>"#, r#"format(None, '5')"#);
        chk(bad, fv(t(&[i(1), i(2)]), r#""#), r#"'(1, 2)'"#, r#"format((1, 2), '')"#);
        chk(bad, fv(t(&[i(1), i(2)]), r#"10"#), r#"<err>"#, r#"format((1, 2), '10')"#);
        chk(bad, fv(f(1e-310), r#""#), r#"'1e-310'"#, r#"format(1e-310, '')"#);
        chk(bad, fv(f(1e-310), r#"e"#), r#"'1.000000e-310'"#, r#"format(1e-310, 'e')"#);
        chk(bad, fv(f(1e-310), r#".20e"#), r#"'9.99999999999996944933e-311'"#, r#"format(1e-310, '.20e')"#);
        chk(bad, fv(f(9.995), r#".2f"#), r#"'9.99'"#, r#"format(9.995, '.2f')"#);
        chk(bad, fv(f(9.995), r#".3g"#), r#"'9.99'"#, r#"format(9.995, '.3g')"#);
        chk(bad, fv(f(1e22), r#"f"#), r#"'10000000000000000000000.000000'"#, r#"format(1e22, 'f')"#);
        chk(bad, fv(f(1e22), r#""#), r#"'1e+22'"#, r#"format(1e22, '')"#);
        chk(bad, fv(f(0.1), r#".30f"#), r#"'0.100000000000000005551115123126'"#, r#"format(0.1, '.30f')"#);
        chk(bad, fv(f(0.1), r#".17g"#), r#"'0.10000000000000001'"#, r#"format(0.1, '.17g')"#);
        chk(bad, fv(f(0.1), r#".55e"#), r#"'1.0000000000000000555111512312578270211815834045410156250e-01'"#, r#"format(0.1, '.55e')"#);
        chk(bad, sf("{} + {} = {}", &[i(1), f(2.5), s("x")], &[]), r#"'1 + 2.5 = x'"#, r#"'{} + {} = {}'.format(1, 2.5, 'x')"#);
        chk(bad, sf("{1}{0}{1}", &[s("a"), s("b")], &[]), r#"'bab'"#, r#"'{1}{0}{1}'.format('a', 'b')"#);
        chk(bad, sf("{name}:{n:>5}", &[], &[("name", s("k")), ("n", i(3))]), r#"'k:    3'"#, r#"'{name}:{n:>5}'.format(name='k', n=3)"#);
        chk(bad, sf("{!r} {!s} {!a}", &[s("x"), s("y"), s("\u{e9}")], &[]), r#""'x' y '\\xe9'""#, r#"'{!r} {!s} {!a}'.format('x', 'y', '\u00e9')"#);
        chk(bad, sf("{:{}}|", &[s("ab"), i(5)], &[]), r#"'ab   |'"#, r#"'{:{}}|'.format('ab', 5)"#);
        chk(bad, sf("{:{}.{}f}", &[f(3.14159), i(10), i(2)], &[]), r#"'      3.14'"#, r#"'{:{}.{}f}'.format(3.14159, 10, 2)"#);
        chk(bad, sf("{0:{1}}", &[i(7), s(">4")], &[]), r#"'   7'"#, r#"'{0:{1}}'.format(7, '>4')"#);
        chk(bad, sf("{{}} {{{}}}", &[i(1)], &[]), r#"'{} {1}'"#, r#"'{{}} {{{}}}'.format(1)"#);
        chk(bad, sf("{0} {}", &[i(1), i(2)], &[]), r#"<err>"#, r#"'{0} {}'.format(1, 2)"#);
        chk(bad, sf("{} {0}", &[i(1), i(2)], &[]), r#"<err>"#, r#"'{} {0}'.format(1, 2)"#);
        chk(bad, sf("{}", &[], &[]), r#"<err>"#, r#"'{}'.format()"#);
        chk(bad, sf("}", &[], &[]), r#"<err>"#, r#"'}'.format()"#);
        chk(bad, sf("{", &[], &[]), r#"<err>"#, r#"'{'.format()"#);
        chk(bad, sf("{!x}", &[i(1)], &[]), r#"<err>"#, r#"'{!x}'.format(1)"#);
        chk(bad, sf("{:,}", &[i(1234567)], &[]), r#"'1,234,567'"#, r#"'{:,}'.format(1234567)"#);
        chk(bad, sf("{0!r:>8}", &[s("ab")], &[]), r#""    'ab'""#, r#"'{0!r:>8}'.format('ab')"#);
        chk(bad, sf("{:%}", &[f(0.25)], &[]), r#"'25.000000%'"#, r#"'{:%}'.format(0.25)"#);
        chk(bad, sf("{:d}", &[f(1.5)], &[]), r#"<err>"#, r#"'{:d}'.format(1.5)"#);
        chk(bad, sf("{x}", &[], &[("x", nn())]), r#"'None'"#, r#"'{x}'.format(**{'x': None})"#);
        chk(bad, sf("{:{:{}}}", &[i(1), i(2), i(3)], &[]), r#"<err>"#, r#"'{:{:{}}}'.format(1, 2, 3)"#);
        chk(bad, pf("%d items", &[i(5)]), r#"'5 items'"#, r#"'%d items' % 5"#);
        chk(bad, pf("%5.2f|%-6s|%x", &[f(3.14159), s("ab"), i(255)]), r#"' 3.14|ab    |ff'"#, r#"'%5.2f|%-6s|%x' % (3.14159, 'ab', 255)"#);
        chk(bad, pf("%05d", &[i(-42)]), r#"'-0042'"#, r#"'%05d' % -42"#);
        chk(bad, pf("%+d % d", &[i(5), i(5)]), r#"'+5  5'"#, r#"'%+d % d' % (5, 5)"#);
        chk(bad, pf("%#o %#x %#X", &[i(8), i(255), i(255)]), r#"'0o10 0xff 0XFF'"#, r#"'%#o %#x %#X' % (8, 255, 255)"#);
        chk(bad, pf("%.3d", &[i(7)]), r#"'007'"#, r#"'%.3d' % 7"#);
        chk(bad, pf("%05.3d", &[i(5)]), r#"'00005'"#, r#"'%05.3d' % 5"#);
        chk(bad, pf("%#.5x", &[i(255)]), r#"'0x000ff'"#, r#"'%#.5x' % 255"#);
        chk(bad, pf("%e", &[f(12345.678)]), r#"'1.234568e+04'"#, r#"'%e' % 12345.678"#);
        chk(bad, pf("%.2E", &[f(0.000123)]), r#"'1.23E-04'"#, r#"'%.2E' % 0.000123"#);
        chk(bad, pf("%g %g %g", &[i(100000), i(1000000), f(0.0001)]), r#"'100000 1e+06 0.0001'"#, r#"'%g %g %g' % (100000, 1000000, 0.0001)"#);
        chk(bad, pf("%#g", &[f(1.5)]), r#"'1.50000'"#, r#"'%#g' % 1.5"#);
        chk(bad, pf("%.0f %.0f", &[f(0.5), f(1.5)]), r#"'0 2'"#, r#"'%.0f %.0f' % (0.5, 1.5)"#);
        chk(bad, pf("%c%c", &[i(65), s("b")]), r#"'Ab'"#, r#"'%c%c' % (65, 'b')"#);
        chk(bad, pf("%r %s %a", &[s("x"), f(1.0), s("\u{e9}")]), r#""'x' 1.0 '\\xe9'""#, r#"'%r %s %a' % ('x', 1.0, '\u00e9')"#);
        chk(bad, pf("%*d|%-*d", &[i(5), i(1), i(4), i(2)]), r#"'    1|2   '"#, r#"'%*d|%-*d' % (5, 1, 4, 2)"#);
        chk(bad, pf("%.*f", &[i(2), f(3.14159)]), r#"'3.14'"#, r#"'%.*f' % (2, 3.14159)"#);
        chk(bad, pf("%d", &[f(3.99)]), r#"'3'"#, r#"'%d' % 3.99"#);
        chk(bad, pf("%x", &[f(3.0)]), r#"<err>"#, r#"'%x' % 3.0"#);
        chk(bad, pf("%d", &[s("5")]), r#"<err>"#, r#"'%d' % '5'"#);
        chk(bad, pf("%s %s", &[s("a")]), r#"<err>"#, r#"'%s %s' % ('a',)"#);
        chk(bad, pf("%s", &[s("a"), s("b")]), r#"<err>"#, r#"'%s' % ('a', 'b')"#);
        chk(bad, pf("100%%", &[]), r#"'100%'"#, r#"'100%%' % ()"#);
        chk(bad, pf("%.2s", &[s("abcdef")]), r#"'ab'"#, r#"'%.2s' % 'abcdef'"#);
        chk(bad, pf("%010.3f", &[f(-3.14159)]), r#"'-00003.142'"#, r#"'%010.3f' % -3.14159"#);
        chk(bad, pf("%010f", &[f(f64::INFINITY)]), r#"'0000000inf'"#, r#"'%010f' % float('inf')"#);
        chk(bad, pf("%s", &[bo(true)]), r#"'True'"#, r#"'%s' % True"#);
        chk(bad, pf("%d", &[bo(true)]), r#"'1'"#, r#"'%d' % True"#);
        chk(bad, pf("%5s|%-5s|", &[s("ab"), s("cd")]), r#"'   ab|cd   |'"#, r#"'%5s|%-5s|' % ('ab', 'cd')"#);
        chk(bad, pm("%(a)s-%(b)05.1f", &[("a", s("x")), ("b", f(2.25))]), r#"'x-002.2'"#, r#"'%(a)s-%(b)05.1f' % {'a': 'x', 'b': 2.25}"#);
        chk(bad, pm("%(a)s %s", &[("a", i(1))]), r#"<err>"#, r#"'%(a)s %s' % {'a': 1}"#);
        chk(bad, pm("%(missing)s", &[("a", i(1))]), r#"<err>"#, r#"'%(missing)s' % {'a': 1}"#);
        chk(bad, pf("%i|%u", &[i(-7), i(7)]), r#"'-7|7'"#, r#"'%i|%u' % (-7, 7)"#);
        chk(bad, pf("% 05d", &[i(42)]), r#"' 0042'"#, r#"'% 05d' % 42"#);
        chk(bad, pf("%-+6d|", &[i(42)]), r#"'+42   |'"#, r#"'%-+6d|' % 42"#);
        chk(bad, pf("%G", &[f(1e-10)]), r#"'1E-10'"#, r#"'%G' % 1e-10"#);
        chk(bad, sm("hello world", "title", &[]), r#"'Hello World'"#, r#"'hello world'.title()"#);
        chk(bad, sm("they're bill's 2nd", "title", &[]), r#""They'Re Bill'S 2Nd""#, r#""they're bill's 2nd".title()"#);
        chk(bad, sm("hELLO wORLD", "capitalize", &[]), r#"'Hello world'"#, r#"'hELLO wORLD'.capitalize()"#);
        chk(bad, sm("Hello", "swapcase", &[]), r#"'hELLO'"#, r#"'Hello'.swapcase()"#);
        chk(bad, sm("MiXeD", "casefold", &[]), r#"'mixed'"#, r#"'MiXeD'.casefold()"#);
        chk(bad, sm("Straße", "upper", &[]), r#"'STRASSE'"#, r#"'Straße'.upper()"#);
        chk(bad, sm("ÀÉÎ", "lower", &[]), r#"'àéî'"#, r#"'ÀÉÎ'.lower()"#);
        chk(bad, sm("ab", "center", &[i(5), s("*")]), r#"'**ab*'"#, r#"'ab'.center(5, '*')"#);
        chk(bad, sm("ab", "center", &[i(6), s("*")]), r#"'**ab**'"#, r#"'ab'.center(6, '*')"#);
        chk(bad, sm("abc", "center", &[i(6), s("*")]), r#"'*abc**'"#, r#"'abc'.center(6, '*')"#);
        chk(bad, sm("abc", "center", &[i(8)]), r#"'  abc   '"#, r#"'abc'.center(8)"#);
        chk(bad, sm("abc", "ljust", &[i(6), s("-")]), r#"'abc---'"#, r#"'abc'.ljust(6, '-')"#);
        chk(bad, sm("abc", "rjust", &[i(2)]), r#"'abc'"#, r#"'abc'.rjust(2)"#);
        chk(bad, sm("abc", "rjust", &[i(5), s("xy")]), r#"<err>"#, r#"'abc'.rjust(5, 'xy')"#);
        chk(bad, sm("banana", "count", &[s("an")]), r#"2"#, r#"'banana'.count('an')"#);
        chk(bad, sm("banana", "count", &[s("a"), i(2)]), r#"2"#, r#"'banana'.count('a', 2)"#);
        chk(bad, sm("banana", "count", &[s("a"), i(-3), i(-1)]), r#"1"#, r#"'banana'.count('a', -3, -1)"#);
        chk(bad, sm("abc", "count", &[s("")]), r#"4"#, r#"'abc'.count('')"#);
        chk(bad, sm("abc", "count", &[s(""), i(4)]), r#"0"#, r#"'abc'.count('', 4)"#);
        chk(bad, sm("aaaa", "count", &[s("aa")]), r#"2"#, r#"'aaaa'.count('aa')"#);
        chk(bad, sm("héllo", "encode", &[]), r#"b'h\xc3\xa9llo'"#, r#"'héllo'.encode()"#);
        chk(bad, sm("héllo", "encode", &[s("latin-1")]), r#"b'h\xe9llo'"#, r#"'héllo'.encode('latin-1')"#);
        chk(bad, sm("héllo", "encode", &[s("ascii")]), r#"<err>"#, r#"'héllo'.encode('ascii')"#);
        chk(bad, sm("report.csv", "endswith", &[s(".csv")]), r#"True"#, r#"'report.csv'.endswith('.csv')"#);
        chk(bad, sm("report.csv", "endswith", &[t(&[s(".txt"), s(".csv")])]), r#"True"#, r#"'report.csv'.endswith(('.txt', '.csv'))"#);
        chk(bad, sm("report.csv", "startswith", &[s("port"), i(2)]), r#"True"#, r#"'report.csv'.startswith('port', 2)"#);
        chk(bad, sm("report.csv", "startswith", &[s("rep"), i(0), i(2)]), r#"False"#, r#"'report.csv'.startswith('rep', 0, 2)"#);
        chk(bad, sm("abc", "startswith", &[s(""), i(5)]), r#"False"#, r#"'abc'.startswith('', 5)"#);
        chk(bad, sm("abc", "endswith", &[s(""), i(3)]), r#"True"#, r#"'abc'.endswith('', 3)"#);
        chk(bad, sm("a\tbc\tdef\n\tg", "expandtabs", &[]), r#"'a       bc      def\n        g'"#, r#"'a\tbc\tdef\n\tg'.expandtabs()"#);
        chk(bad, sm("a\tb", "expandtabs", &[i(4)]), r#"'a   b'"#, r#"'a\tb'.expandtabs(4)"#);
        chk(bad, sm("a\tb", "expandtabs", &[i(0)]), r#"'ab'"#, r#"'a\tb'.expandtabs(0)"#);
        chk(bad, sm("héllo wörld", "find", &[s("wö")]), r#"6"#, r#"'héllo wörld'.find('wö')"#);
        chk(bad, sm("hello", "find", &[s("l"), i(3)]), r#"3"#, r#"'hello'.find('l', 3)"#);
        chk(bad, sm("hello", "find", &[s("z")]), r#"-1"#, r#"'hello'.find('z')"#);
        chk(bad, sm("hello", "find", &[s(""), i(5)]), r#"5"#, r#"'hello'.find('', 5)"#);
        chk(bad, sm("hello", "find", &[s(""), i(6)]), r#"-1"#, r#"'hello'.find('', 6)"#);
        chk(bad, sm("hello", "rfind", &[s("l")]), r#"3"#, r#"'hello'.rfind('l')"#);
        chk(bad, sm("hello", "rfind", &[s("l"), i(0), i(3)]), r#"2"#, r#"'hello'.rfind('l', 0, 3)"#);
        chk(bad, sm("hello", "index", &[s("z")]), r#"<err>"#, r#"'hello'.index('z')"#);
        chk(bad, sm("hello", "rindex", &[s("h")]), r#"0"#, r#"'hello'.rindex('h')"#);
        chk(bad, sm("hello", "find", &[s("l"), nn(), i(-1)]), r#"2"#, r#"'hello'.find('l', None, -1)"#);
        chk(bad, sm("abc123", "isalnum", &[]), r#"True"#, r#"'abc123'.isalnum()"#);
        chk(bad, sm("abc 123", "isalnum", &[]), r#"False"#, r#"'abc 123'.isalnum()"#);
        chk(bad, sm("", "isalpha", &[]), r#"False"#, r#"''.isalpha()"#);
        chk(bad, sm("0123", "isdigit", &[]), r#"True"#, r#"'0123'.isdigit()"#);
        chk(bad, sm("0123", "isdecimal", &[]), r#"True"#, r#"'0123'.isdecimal()"#);
        chk(bad, sm("_id9", "isidentifier", &[]), r#"True"#, r#"'_id9'.isidentifier()"#);
        chk(bad, sm("9id", "isidentifier", &[]), r#"False"#, r#"'9id'.isidentifier()"#);
        chk(bad, sm("abc1", "islower", &[]), r#"True"#, r#"'abc1'.islower()"#);
        chk(bad, sm("123", "islower", &[]), r#"False"#, r#"'123'.islower()"#);
        chk(bad, sm("ABC D", "isupper", &[]), r#"True"#, r#"'ABC D'.isupper()"#);
        chk(bad, sm("Hello World", "istitle", &[]), r#"True"#, r#"'Hello World'.istitle()"#);
        chk(bad, sm("Hello world", "istitle", &[]), r#"False"#, r#"'Hello world'.istitle()"#);
        chk(bad, sm("HeLlo", "istitle", &[]), r#"False"#, r#"'HeLlo'.istitle()"#);
        chk(bad, sm(" \t\n\x1c", "isspace", &[]), r#"True"#, r#"' \t\n\x1c'.isspace()"#);
        chk(bad, sm("ab\n", "isprintable", &[]), r#"False"#, r#"'ab\n'.isprintable()"#);
        chk(bad, sm("é€", "isprintable", &[]), r#"True"#, r#"'é€'.isprintable()"#);
        chk(bad, sm("é", "isascii", &[]), r#"False"#, r#"'é'.isascii()"#);
        chk(bad, sm(", ", "join", &[t(&[s("a"), s("b"), s("c")])]), r#"'a, b, c'"#, r#"', '.join(('a', 'b', 'c'))"#);
        chk(bad, sm("-", "join", &[s("xyz")]), r#"'x-y-z'"#, r#"'-'.join('xyz')"#);
        chk(bad, sm("-", "join", &[t(&[i(1), i(2)])]), r#"<err>"#, r#"'-'.join((1, 2))"#);
        chk(bad, sm("a=b=c", "partition", &[s("=")]), r#"('a', '=', 'b=c')"#, r#"'a=b=c'.partition('=')"#);
        chk(bad, sm("a=b=c", "rpartition", &[s("=")]), r#"('a=b', '=', 'c')"#, r#"'a=b=c'.rpartition('=')"#);
        chk(bad, sm("abc", "partition", &[s("x")]), r#"('abc', '', '')"#, r#"'abc'.partition('x')"#);
        chk(bad, sm("abc", "rpartition", &[s("x")]), r#"('', '', 'abc')"#, r#"'abc'.rpartition('x')"#);
        chk(bad, sm("abc", "partition", &[s("")]), r#"<err>"#, r#"'abc'.partition('')"#);
        chk(bad, sm("test_case", "removeprefix", &[s("test_")]), r#"'case'"#, r#"'test_case'.removeprefix('test_')"#);
        chk(bad, sm("file.py", "removesuffix", &[s(".txt")]), r#"'file.py'"#, r#"'file.py'.removesuffix('.txt')"#);
        chk(bad, sm("aaa", "replace", &[s("a"), s("b"), i(2)]), r#"'bba'"#, r#"'aaa'.replace('a', 'b', 2)"#);
        chk(bad, sm("abc", "replace", &[s(""), s("-")]), r#"'-a-b-c-'"#, r#"'abc'.replace('', '-')"#);
        chk(bad, sm("abc", "replace", &[s(""), s("-"), i(2)]), r#"'-a-bc'"#, r#"'abc'.replace('', '-', 2)"#);
        chk(bad, sm("", "replace", &[s(""), s("x")]), r#"'x'"#, r#"''.replace('', 'x')"#);
        chk(bad, sm("aaaa", "replace", &[s("aa"), s("b")]), r#"'bb'"#, r#"'aaaa'.replace('aa', 'b')"#);
        chk(bad, sm("  a b  c  ", "split", &[]), r#"['a', 'b', 'c']"#, r#"'  a b  c  '.split()"#);
        chk(bad, sm("  a b  c  ", "split", &[nn(), i(1)]), r#"['a', 'b  c  ']"#, r#"'  a b  c  '.split(None, 1)"#);
        chk(bad, sm("  a b  c  ", "rsplit", &[nn(), i(1)]), r#"['  a b', 'c']"#, r#"'  a b  c  '.rsplit(None, 1)"#);
        chk(bad, sm("a,b,,c", "split", &[s(",")]), r#"['a', 'b', '', 'c']"#, r#"'a,b,,c'.split(',')"#);
        chk(bad, sm("a,b,,c", "split", &[s(","), i(2)]), r#"['a', 'b', ',c']"#, r#"'a,b,,c'.split(',', 2)"#);
        chk(bad, sm("a,b,,c", "rsplit", &[s(","), i(1)]), r#"['a,b,', 'c']"#, r#"'a,b,,c'.rsplit(',', 1)"#);
        chk(bad, sm("", "split", &[s(",")]), r#"['']"#, r#"''.split(',')"#);
        chk(bad, sm("", "split", &[]), r#"[]"#, r#"''.split()"#);
        chk(bad, sm("a<>b<>c", "split", &[s("<>")]), r#"['a', 'b', 'c']"#, r#"'a<>b<>c'.split('<>')"#);
        chk(bad, sm("abc", "split", &[s("")]), r#"<err>"#, r#"'abc'.split('')"#);
        chk(bad, sm("one\ntwo\r\nthree\rfour\x0b", "splitlines", &[]), r#"['one', 'two', 'three', 'four']"#, r#"'one\ntwo\r\nthree\rfour\x0b'.splitlines()"#);
        chk(bad, sm("one\ntwo\r\n", "splitlines", &[bo(true)]), r#"['one\n', 'two\r\n']"#, r#"'one\ntwo\r\n'.splitlines(True)"#);
        chk(bad, sm("", "splitlines", &[]), r#"[]"#, r#"''.splitlines()"#);
        chk(bad, sm("  xx  ", "strip", &[]), r#"'xx'"#, r#"'  xx  '.strip()"#);
        chk(bad, sm("xxhixx", "lstrip", &[s("x")]), r#"'hixx'"#, r#"'xxhixx'.lstrip('x')"#);
        chk(bad, sm("www.example.com", "strip", &[s("cmowz.")]), r#"'example'"#, r#"'www.example.com'.strip('cmowz.')"#);
        chk(bad, sm("  xx  ", "rstrip", &[]), r#"'  xx'"#, r#"'  xx  '.rstrip()"#);
        chk(bad, sm("42", "zfill", &[i(5)]), r#"'00042'"#, r#"'42'.zfill(5)"#);
        chk(bad, sm("-42", "zfill", &[i(5)]), r#"'-0042'"#, r#"'-42'.zfill(5)"#);
        chk(bad, sm("+", "zfill", &[i(3)]), r#"'+00'"#, r#"'+'.zfill(3)"#);
        chk(bad, sm("abc", "zfill", &[i(2)]), r#"'abc'"#, r#"'abc'.zfill(2)"#);
        chk(bad, sm("{}-{}", "format", &[s("a"), i(1)]), r#"'a-1'"#, r#"'{}-{}'.format('a', 1)"#);
        chk(bad, sm("é", "upper", &[]), r#"'É'"#, r#"'é'.upper()"#);
        chk(bad, smk("a,b,c", "split", &[], &[("sep", s(",")), ("maxsplit", i(1))]), r#"['a', 'b,c']"#, r#"'a,b,c'.split(sep=',', maxsplit=1)"#);
        chk(bad, smk("abc", "replace", &[s("b"), s("x")], &[("count", i(0))]), r#"'abc'"#, r#"'abc'.replace('b', 'x', count=0)"#);
        chk(bad, smk("{x}{y}", "format", &[], &[("x", i(1)), ("y", s("z"))]), r#"'1z'"#, r#"'{x}{y}'.format(x=1, y='z')"#);
        chk(bad, smk("a\tb", "expandtabs", &[], &[("tabsize", i(2))]), r#"'a b'"#, r#"'a\tb'.expandtabs(tabsize=2)"#);
        chk(bad, m("ceil", &[f(2.1)]), r#"3"#, r#"math.ceil(2.1)"#);
        chk(bad, m("ceil", &[f(-2.1)]), r#"-2"#, r#"math.ceil(-2.1)"#);
        chk(bad, m("floor", &[f(-2.1)]), r#"-3"#, r#"math.floor(-2.1)"#);
        chk(bad, m("floor", &[i(5)]), r#"5"#, r#"math.floor(5)"#);
        chk(bad, m("trunc", &[f(-2.7)]), r#"-2"#, r#"math.trunc(-2.7)"#);
        chk(bad, m("floor", &[f(f64::INFINITY)]), r#"<err>"#, r#"math.floor(float('inf'))"#);
        chk(bad, m("sqrt", &[i(2)]), r#"1.4142135623730951"#, r#"math.sqrt(2)"#);
        chk(bad, m("sqrt", &[i(-1)]), r#"<err>"#, r#"math.sqrt(-1)"#);
        chk(bad, m("isqrt", &[i(99)]), r#"9"#, r#"math.isqrt(99)"#);
        chk(bad, m("isqrt", &[i(i64::MAX)]), r#"3037000499"#, r#"math.isqrt(9223372036854775807)"#);
        chk(bad, m("pow", &[i(2), f(0.5)]), r#"1.4142135623730951"#, r#"math.pow(2, 0.5)"#);
        chk(bad, m("pow", &[f(0.0), i(-1)]), r#"<err>"#, r#"math.pow(0.0, -1)"#);
        chk(bad, m("exp", &[i(1)]), r#"2.718281828459045"#, r#"math.exp(1)"#);
        chk(bad, m("exp", &[i(1000)]), r#"<err>"#, r#"math.exp(1000)"#);
        chk(bad, m("log", &[i(100), i(10)]), r#"2.0"#, r#"math.log(100, 10)"#);
        chk(bad, m("log", &[i(8), i(2)]), r#"3.0"#, r#"math.log(8, 2)"#);
        chk(bad, m("log", &[f(2.718281828459045)]), r#"1.0"#, r#"math.log(2.718281828459045)"#);
        chk(bad, m("log", &[i(0)]), r#"<err>"#, r#"math.log(0)"#);
        chk(bad, m("log", &[i(10), i(1)]), r#"<err>"#, r#"math.log(10, 1)"#);
        chk(bad, m("log2", &[i(1024)]), r#"10.0"#, r#"math.log2(1024)"#);
        chk(bad, m("log2", &[i(10)]), r#"3.321928094887362"#, r#"math.log2(10)"#);
        chk(bad, m("log10", &[i(1000)]), r#"3.0"#, r#"math.log10(1000)"#);
        chk(bad, m("log10", &[i(2)]), r#"0.3010299956639812"#, r#"math.log10(2)"#);
        chk(bad, m("fabs", &[i(-3)]), r#"3.0"#, r#"math.fabs(-3)"#);
        chk(bad, m("factorial", &[i(20)]), r#"2432902008176640000"#, r#"math.factorial(20)"#);
        chk(bad, m("factorial", &[i(21)]), r#"<err>"#, r#"math.factorial(21) [declined; Python gives 51090942171709440000]"#);
        chk(bad, m("factorial", &[f(5.0)]), r#"<err>"#, r#"math.factorial(5.0)"#);
        chk(bad, m("gcd", &[i(12), i(-18), i(30)]), r#"6"#, r#"math.gcd(12, -18, 30)"#);
        chk(bad, m("gcd", &[]), r#"0"#, r#"math.gcd()"#);
        chk(bad, m("lcm", &[i(4), i(6), i(10)]), r#"60"#, r#"math.lcm(4, 6, 10)"#);
        chk(bad, m("lcm", &[i(4), i(0)]), r#"0"#, r#"math.lcm(4, 0)"#);
        chk(bad, m("comb", &[i(52), i(5)]), r#"2598960"#, r#"math.comb(52, 5)"#);
        chk(bad, m("comb", &[i(5), i(7)]), r#"0"#, r#"math.comb(5, 7)"#);
        chk(bad, m("comb", &[i(60), i(30)]), r#"118264581564861424"#, r#"math.comb(60, 30)"#);
        chk(bad, m("perm", &[i(5), i(2)]), r#"20"#, r#"math.perm(5, 2)"#);
        chk(bad, m("perm", &[i(5)]), r#"120"#, r#"math.perm(5)"#);
        chk(bad, m("copysign", &[i(3), f(-0.0)]), r#"-3.0"#, r#"math.copysign(3, -0.0)"#);
        chk(bad, m("fmod", &[i(-7), i(3)]), r#"-1.0"#, r#"math.fmod(-7, 3)"#);
        chk(bad, m("fmod", &[i(7), i(0)]), r#"<err>"#, r#"math.fmod(7, 0)"#);
        chk(bad, m("hypot", &[i(3), i(4)]), r#"5.0"#, r#"math.hypot(3, 4)"#);
        chk(bad, m("hypot", &[i(1), i(1)]), r#"1.4142135623730951"#, r#"math.hypot(1, 1)"#);
        chk(bad, m("hypot", &[i(3), i(4), i(5)]), r#"7.0710678118654755"#, r#"math.hypot(3, 4, 5)"#);
        chk(bad, m("hypot", &[i(-7)]), r#"7.0"#, r#"math.hypot(-7)"#);
        chk(bad, m("isclose", &[f(0.1 + 0.2), f(0.3)]), r#"True"#, r#"math.isclose(0.1 + 0.2, 0.3)"#);
        chk(bad, m("isclose", &[f(1.0), f(1.1)]), r#"False"#, r#"math.isclose(1.0, 1.1)"#);
        chk(bad, m("isfinite", &[f(1e308)]), r#"True"#, r#"math.isfinite(1e308)"#);
        chk(bad, m("isinf", &[f(f64::NEG_INFINITY)]), r#"True"#, r#"math.isinf(float('-inf'))"#);
        chk(bad, m("isnan", &[f(f64::NAN)]), r#"True"#, r#"math.isnan(float('nan'))"#);
        chk(bad, m("prod", &[t(&[i(1), i(2), i(3), i(4)])]), r#"24"#, r#"math.prod((1, 2, 3, 4))"#);
        chk(bad, m("prod", &[t(&[i(2), f(0.5), i(3)])]), r#"3.0"#, r#"math.prod((2, 0.5, 3))"#);
        chk(bad, m("prod", &[t(&[]), i(5)]), r#"<err>"#, r#"math.prod((), 5)"#);
        chk(bad, m("fsum", &[t(&[f(0.1); 10])]), r#"1.0"#, r#"math.fsum((0.1,) * 10)"#);
        chk(bad, m("fsum", &[t(&[f(1e-16), i(1), f(1e16)])]), r#"1.0000000000000002e+16"#, r#"math.fsum((1e-16, 1, 1e16))"#);
        chk(bad, m("fsum", &[t(&[f(1e308), f(1e308)])]), r#"<err>"#, r#"math.fsum((1e308, 1e308))"#);
        chk(bad, m("degrees", &[f(std::f64::consts::PI)]), r#"180.0"#, r#"math.degrees(math.pi)"#);
        chk(bad, m("radians", &[i(180)]), r#"3.141592653589793"#, r#"math.radians(180)"#);
        chk(bad, m("radians", &[i(45)]), r#"0.7853981633974483"#, r#"math.radians(45)"#);
        chk(bad, m("sin", &[f(0.5)]), r#"0.479425538604203"#, r#"math.sin(0.5)"#);
        chk(bad, m("cos", &[i(1)]), r#"0.5403023058681398"#, r#"math.cos(1)"#);
        chk(bad, m("tan", &[f(0.25)]), r#"0.25534192122103627"#, r#"math.tan(0.25)"#);
        chk(bad, m("sin", &[f(f64::INFINITY)]), r#"<err>"#, r#"math.sin(float('inf'))"#);
        chk(bad, op("add", &[bo(true), i(1)]), r#"2"#, r#"operator.add(True, 1)"#);
        chk(bad, op("add", &[bo(true), bo(true)]), r#"2"#, r#"operator.add(True, True)"#);
        chk(bad, op("add", &[f(0.1), f(0.2)]), r#"0.30000000000000004"#, r#"operator.add(0.1, 0.2)"#);
        chk(bad, op("add", &[s("ab"), s("cd")]), r#"'abcd'"#, r#"operator.add('ab', 'cd')"#);
        chk(bad, op("add", &[t(&[i(1)]), t(&[i(2), i(3)])]), r#"(1, 2, 3)"#, r#"operator.add((1,), (2, 3))"#);
        chk(bad, op("add", &[s("a"), i(1)]), r#"<err>"#, r#"operator.add('a', 1)"#);
        chk(bad, op("add", &[i(i64::MAX), i(1)]), r#"<err>"#, r#"operator.add(9223372036854775807, 1) [declined; Python gives 9223372036854775808]"#);
        chk(bad, op("sub", &[i(1), f(2.5)]), r#"-1.5"#, r#"operator.sub(1, 2.5)"#);
        chk(bad, op("mul", &[s("ab"), i(3)]), r#"'ababab'"#, r#"operator.mul('ab', 3)"#);
        chk(bad, op("mul", &[i(2), t(&[i(1), i(2)])]), r#"(1, 2, 1, 2)"#, r#"operator.mul(2, (1, 2))"#);
        chk(bad, op("mul", &[s("ab"), i(-1)]), r#"''"#, r#"operator.mul('ab', -1)"#);
        chk(bad, op("mul", &[s("ab"), bo(true)]), r#"'ab'"#, r#"operator.mul('ab', True)"#);
        chk(bad, op("mul", &[s("a"), f(2.0)]), r#"<err>"#, r#"operator.mul('a', 2.0)"#);
        chk(bad, op("truediv", &[i(1), i(3)]), r#"0.3333333333333333"#, r#"operator.truediv(1, 3)"#);
        chk(bad, op("truediv", &[i(10), i(2)]), r#"5.0"#, r#"operator.truediv(10, 2)"#);
        chk(bad, op("truediv", &[i(0), i(-5)]), r#"-0.0"#, r#"operator.truediv(0, -5)"#);
        chk(bad, op("truediv", &[i(9007199254740993), i(1)]), r#"9007199254740992.0"#, r#"operator.truediv(9007199254740993, 1)"#);
        chk(bad, op("truediv", &[i(1), i(0)]), r#"<err>"#, r#"operator.truediv(1, 0)"#);
        chk(bad, op("truediv", &[f(1.0), f(0.0)]), r#"<err>"#, r#"operator.truediv(1.0, 0.0)"#);
        chk(bad, op("floordiv", &[i(-7), i(2)]), r#"-4"#, r#"operator.floordiv(-7, 2)"#);
        chk(bad, op("floordiv", &[f(7.0), i(-2)]), r#"-4.0"#, r#"operator.floordiv(7.0, -2)"#);
        chk(bad, op("floordiv", &[i(i64::MIN), i(-1)]), r#"<err>"#, r#"operator.floordiv(-9223372036854775808, -1) [declined; Python gives 9223372036854775808]"#);
        chk(bad, op("mod", &[i(-7), i(3)]), r#"2"#, r#"operator.mod(-7, 3)"#);
        chk(bad, op("mod", &[i(7), i(-3)]), r#"-2"#, r#"operator.mod(7, -3)"#);
        chk(bad, op("mod", &[f(-7.5), i(2)]), r#"0.5"#, r#"operator.mod(-7.5, 2)"#);
        chk(bad, op("mod", &[f(5.0), f(-2.5)]), r#"-0.0"#, r#"operator.mod(5.0, -2.5)"#);
        chk(bad, op("mod", &[s("%s-%d"), t(&[s("a"), i(3)])]), r#"'a-3'"#, r#"operator.mod('%s-%d', ('a', 3))"#);
        chk(bad, op("mod", &[s("%s!"), s("hi")]), r#"'hi!'"#, r#"operator.mod('%s!', 'hi')"#);
        chk(bad, op("pow", &[i(2), i(10)]), r#"1024"#, r#"operator.pow(2, 10)"#);
        chk(bad, op("pow", &[i(2), i(-1)]), r#"0.5"#, r#"operator.pow(2, -1)"#);
        chk(bad, op("pow", &[i(4), f(0.5)]), r#"2.0"#, r#"operator.pow(4, 0.5)"#);
        chk(bad, op("pow", &[i(1), i(1000000000000)]), r#"1"#, r#"operator.pow(1, 10**12)"#);
        chk(bad, op("neg", &[bo(true)]), r#"-1"#, r#"operator.neg(True)"#);
        chk(bad, op("pos", &[bo(false)]), r#"0"#, r#"operator.pos(False)"#);
        chk(bad, op("abs", &[f(-0.0)]), r#"0.0"#, r#"operator.abs(-0.0)"#);
        chk(bad, op("invert", &[i(5)]), r#"-6"#, r#"operator.invert(5)"#);
        chk(bad, op("invert", &[bo(true)]), r#"-2"#, r#"operator.invert(True)"#);
        chk(bad, op("eq", &[i(1), f(1.0)]), r#"True"#, r#"operator.eq(1, 1.0)"#);
        chk(bad, op("eq", &[bo(true), i(1)]), r#"True"#, r#"operator.eq(True, 1)"#);
        chk(bad, op("eq", &[s("1"), i(1)]), r#"False"#, r#"operator.eq('1', 1)"#);
        chk(bad, op("eq", &[t(&[i(1), f(2.0)]), t(&[f(1.0), i(2)])]), r#"True"#, r#"operator.eq((1, 2.0), (1.0, 2))"#);
        chk(bad, op("eq", &[i(9007199254740993), f(9007199254740992.0)]), r#"False"#, r#"operator.eq(9007199254740993, 9007199254740992.0)"#);
        chk(bad, op("ne", &[f(f64::NAN), f(f64::NAN)]), r#"True"#, r#"operator.ne(float('nan'), float('nan'))"#);
        chk(bad, op("lt", &[i(1), f(1.5)]), r#"True"#, r#"operator.lt(1, 1.5)"#);
        chk(bad, op("lt", &[s("apple"), s("banana")]), r#"True"#, r#"operator.lt('apple', 'banana')"#);
        chk(bad, op("lt", &[s("Z"), s("a")]), r#"True"#, r#"operator.lt('Z', 'a')"#);
        chk(bad, op("le", &[t(&[i(1), i(2)]), t(&[i(1), i(2), i(0)])]), r#"True"#, r#"operator.le((1, 2), (1, 2, 0))"#);
        chk(bad, op("gt", &[t(&[i(1), s("b")]), t(&[i(1), s("a")])]), r#"True"#, r#"operator.gt((1, 'b'), (1, 'a'))"#);
        chk(bad, op("ge", &[f(f64::NAN), i(1)]), r#"False"#, r#"operator.ge(float('nan'), 1)"#);
        chk(bad, op("lt", &[i(1), s("a")]), r#"<err>"#, r#"operator.lt(1, 'a')"#);
        chk(bad, op("lt", &[nn(), nn()]), r#"<err>"#, r#"operator.lt(None, None)"#);
        chk(bad, op("and_", &[bo(true), bo(false)]), r#"False"#, r#"operator.and_(True, False)"#);
        chk(bad, op("and_", &[i(12), i(10)]), r#"8"#, r#"operator.and_(12, 10)"#);
        chk(bad, op("or_", &[bo(true), i(2)]), r#"3"#, r#"operator.or_(True, 2)"#);
        chk(bad, op("xor", &[bo(true), bo(true)]), r#"False"#, r#"operator.xor(True, True)"#);
        chk(bad, op("lshift", &[i(1), i(62)]), r#"4611686018427387904"#, r#"operator.lshift(1, 62)"#);
        chk(bad, op("lshift", &[i(1), i(63)]), r#"<err>"#, r#"operator.lshift(1, 63) [declined; Python gives 9223372036854775808]"#);
        chk(bad, op("lshift", &[i(-1), i(63)]), r#"-9223372036854775808"#, r#"operator.lshift(-1, 63)"#);
        chk(bad, op("rshift", &[i(-9), i(1)]), r#"-5"#, r#"operator.rshift(-9, 1)"#);
        chk(bad, op("rshift", &[i(-9), i(100)]), r#"-1"#, r#"operator.rshift(-9, 100)"#);
        chk(bad, op("rshift", &[i(1), i(-1)]), r#"<err>"#, r#"operator.rshift(1, -1)"#);
        chk(bad, op("not_", &[s("")]), r#"True"#, r#"operator.not_('')"#);
        chk(bad, op("truth", &[t(&[i(0)])]), r#"True"#, r#"operator.truth((0,))"#);
        chk(bad, op("contains", &[s("hello"), s("ell")]), r#"True"#, r#"operator.contains('hello', 'ell')"#);
        chk(bad, op("contains", &[t(&[i(1), i(2)]), f(2.0)]), r#"True"#, r#"operator.contains((1, 2), 2.0)"#);
        chk(bad, op("contains", &[by(b"abc"), i(98)]), r#"True"#, r#"operator.contains(b'abc', 98)"#);
        chk(bad, op("contains", &[s("abc"), i(1)]), r#"<err>"#, r#"operator.contains('abc', 1)"#);
        chk(bad, op("concat", &[s("a"), s("b")]), r#"'ab'"#, r#"operator.concat('a', 'b')"#);
        chk(bad, op("concat", &[i(1), i(2)]), r#"<err>"#, r#"operator.concat(1, 2)"#);
        chk(bad, op("add", &[by(b"ab"), by(b"c")]), r#"b'abc'"#, r#"operator.add(b'ab', b'c')"#);
        chk(bad, op("mul", &[f(1e308), i(10)]), r#"inf"#, r#"operator.mul(1e308, 10)"#);
        chk(bad, st("mean", &[i(1), i(2), i(3)]), r#"2"#, r#"statistics.mean([1, 2, 3])"#);
        chk(bad, st("mean", &[i(1), i(2), i(3), i(4)]), r#"2.5"#, r#"statistics.mean([1, 2, 3, 4])"#);
        chk(bad, st("mean", &[i(1), i(2)]), r#"1.5"#, r#"statistics.mean([1, 2])"#);
        chk(bad, st("mean", &[f(1.5), f(2.5)]), r#"2.0"#, r#"statistics.mean([1.5, 2.5])"#);
        chk(bad, st("mean", &[f(0.1), f(0.2), f(0.3)]), r#"0.2"#, r#"statistics.mean([0.1, 0.2, 0.3])"#);
        chk(bad, st("mean", &[i(1), i(2), i(4)]), r#"2.3333333333333335"#, r#"statistics.mean([1, 2, 4])"#);
        chk(bad, st("mean", &[bo(true), bo(false)]), r#"0.5"#, r#"statistics.mean([True, False])"#);
        chk(bad, st("mean", &[]), r#"<err>"#, r#"statistics.mean([])"#);
        chk(bad, st("fmean", &[i(1), i(2), i(3), i(4)]), r#"2.5"#, r#"statistics.fmean([1, 2, 3, 4])"#);
        chk(bad, st("fmean", &[f(0.1), f(0.2), f(0.3)]), r#"0.19999999999999998"#, r#"statistics.fmean([0.1, 0.2, 0.3])"#);
        chk(bad, st("median", &[i(3), i(1), i(2)]), r#"2"#, r#"statistics.median([3, 1, 2])"#);
        chk(bad, st("median", &[i(4), i(1), i(2), i(3)]), r#"2.5"#, r#"statistics.median([4, 1, 2, 3])"#);
        chk(bad, st("median", &[i(1), i(3)]), r#"2.0"#, r#"statistics.median([1, 3])"#);
        chk(bad, st("median", &[f(1.5), i(2)]), r#"1.75"#, r#"statistics.median([1.5, 2])"#);
        chk(bad, st("median_low", &[i(4), i(1), i(2), i(3)]), r#"2"#, r#"statistics.median_low([4, 1, 2, 3])"#);
        chk(bad, st("median_high", &[i(4), i(1), i(2), i(3)]), r#"3"#, r#"statistics.median_high([4, 1, 2, 3])"#);
        chk(bad, st("median_low", &[s("b"), s("a"), s("c")]), r#"'b'"#, r#"statistics.median_low(['b', 'a', 'c'])"#);
        chk(bad, st("mode", &[i(1), i(2), i(2), i(3), i(3)]), r#"2"#, r#"statistics.mode([1, 2, 2, 3, 3])"#);
        chk(bad, st("mode", &[s("a"), s("b"), s("b")]), r#"'b'"#, r#"statistics.mode(['a', 'b', 'b'])"#);
        chk(bad, st("mode", &[f(1.0), i(1), i(2)]), r#"1.0"#, r#"statistics.mode([1.0, 1, 2])"#);
        chk(bad, st("pvariance", &[i(1), i(2), i(3), i(4)]), r#"1.25"#, r#"statistics.pvariance([1, 2, 3, 4])"#);
        chk(bad, st("variance", &[i(1), i(2), i(3)]), r#"1"#, r#"statistics.variance([1, 2, 3])"#);
        chk(bad, st("variance", &[i(1), i(2), i(3), i(4)]), r#"1.6666666666666667"#, r#"statistics.variance([1, 2, 3, 4])"#);
        chk(bad, st("variance", &[f(1.5), f(2.5), f(2.5), f(2.75), f(3.25), f(4.75)]), r#"1.16875"#, r#"statistics.variance([1.5, 2.5, 2.5, 2.75, 3.25, 4.75])"#);
        chk(bad, st("pvariance", &[f(0.1), f(0.2), f(0.3)]), r#"0.006666666666666665"#, r#"statistics.pvariance([0.1, 0.2, 0.3])"#);
        chk(bad, st("variance", &[i(5)]), r#"<err>"#, r#"statistics.variance([5])"#);
        chk(bad, st("stdev", &[i(1), i(2), i(3)]), r#"1.0"#, r#"statistics.stdev([1, 2, 3])"#);
        chk(bad, st("stdev", &[i(1), i(2), i(3), i(4)]), r#"1.2909944487358056"#, r#"statistics.stdev([1, 2, 3, 4])"#);
        chk(bad, st("pstdev", &[i(1), i(2), i(3), i(4)]), r#"1.118033988749895"#, r#"statistics.pstdev([1, 2, 3, 4])"#);
        chk(bad, st("pstdev", &[i(2), i(4), i(4), i(4), i(5), i(5), i(7), i(9)]), r#"2.0"#, r#"statistics.pstdev([2, 4, 4, 4, 5, 5, 7, 9])"#);
        chk(bad, st("stdev", &[f(0.1), f(0.25), f(0.7)]), r#"0.31224989991991986"#, r#"statistics.stdev([0.1, 0.25, 0.7])"#);
        chk(bad, st("pstdev", &[f(1.5), f(2.5)]), r#"0.5"#, r#"statistics.pstdev([1.5, 2.5])"#);
        chk(bad, st("median", &[i(2), i(1), f(f64::NAN)]), r#"<err>"#, r#"statistics.median([2, 1, float('nan')]) [declined; Python gives nan]"#);
        chk(bad, fv(f(-56.868), r#"<12g"#), r#"'-56.868     '"#, r#"format(-56.868, '<12g')"#);
        chk(bad, fv(f(-816.2276007338658), r#".2g"#), r#"'-8.2e+02'"#, r#"format(-816.2276007338658, '.2g')"#);
        chk(bad, fv(f(-13.0), r#".2%"#), r#"'-1300.00%'"#, r#"format(-13.0, '.2%')"#);
        chk(bad, fv(f(-30.0), r#".6g"#), r#"'-30'"#, r#"format(-30.0, '.6g')"#);
        chk(bad, fv(f(1.6202460310847344e-16), r#"015.3f"#), r#"'00000000000.000'"#, r#"format(1.6202460310847344e-16, '015.3f')"#);
        chk(bad, fv(f(-127.1657553638546), r#"z.1f"#), r#"'-127.2'"#, r#"format(-127.1657553638546, 'z.1f')"#);
        chk(bad, fv(f(43415.0), r#".1f"#), r#"'43415.0'"#, r#"format(43415.0, '.1f')"#);
        chk(bad, fv(f(-918.3026798267641), r#".1g"#), r#"'-9e+02'"#, r#"format(-918.3026798267641, '.1g')"#);
        chk(bad, fv(f(1.1861619215761983), r#".3"#), r#"'1.19'"#, r#"format(1.1861619215761983, '.3')"#);
        chk(bad, fv(f(1.0884221835119273e-247), r#".1f"#), r#"'0.0'"#, r#"format(1.0884221835119273e-247, '.1f')"#);
        chk(bad, fv(f(-125.86212576262915), r#".3e"#), r#"'-1.259e+02'"#, r#"format(-125.86212576262915, '.3e')"#);
        chk(bad, fv(f(72666.5), r#".6g"#), r#"'72666.5'"#, r#"format(72666.5, '.6g')"#);
        chk(bad, fv(f(1.7990308760234574e-207), r#""#), r#"'1.7990308760234574e-207'"#, r#"format(1.7990308760234574e-207, '')"#);
        chk(bad, fv(f(50.01), r#".15g"#), r#"'50.01'"#, r#"format(50.01, '.15g')"#);
        chk(bad, fv(f(34368.75), r#".1g"#), r#"'3e+04'"#, r#"format(34368.75, '.1g')"#);
        chk(bad, fv(f(104.9685899011907), r#".6g"#), r#"'104.969'"#, r#"format(104.9685899011907, '.6g')"#);
        chk(bad, fv(f(-99.4), r#"^+18,.3f"#), r#"'     -99.400      '"#, r#"format(-99.4, '^+18,.3f')"#);
        chk(bad, fv(f(2.9906854168032784e-112), r#"^+18,.3f"#), r#"'      +0.000      '"#, r#"format(2.9906854168032784e-112, '^+18,.3f')"#);
        chk(bad, fv(f(-5240070.689791803), r#"g"#), r#"'-5.24007e+06'"#, r#"format(-5240070.689791803, 'g')"#);
        chk(bad, fv(f(-478.3163021970711), r#"#.3g"#), r#"'-478.'"#, r#"format(-478.3163021970711, '#.3g')"#);
        chk(bad, fv(f(-27393.146917264603), r#"<12g"#), r#"'-27393.1    '"#, r#"format(-27393.146917264603, '<12g')"#);
        chk(bad, fv(f(-23368.25), r#""#), r#"'-23368.25'"#, r#"format(-23368.25, '')"#);
        chk(bad, fv(f(-907.6435783853784), r#"+,.2f"#), r#"'-907.64'"#, r#"format(-907.6435783853784, '+,.2f')"#);
        chk(bad, fv(f(24.7057), r#".0e"#), r#"'2e+01'"#, r#"format(24.7057, '.0e')"#);
        chk(bad, fv(f(-124280.0), r#".6g"#), r#"'-124280'"#, r#"format(-124280.0, '.6g')"#);
        chk(bad, fv(f(0.41674327638422903), r#"_.4f"#), r#"'0.4167'"#, r#"format(0.41674327638422903, '_.4f')"#);
        chk(bad, fv(f(-466.0820134452066), r#""#), r#"'-466.0820134452066'"#, r#"format(-466.0820134452066, '')"#);
        chk(bad, fv(f(89.0), r#"#.3g"#), r#"'89.0'"#, r#"format(89.0, '#.3g')"#);
        chk(bad, fv(f(-52.81014380435022), r#".2%"#), r#"'-5281.01%'"#, r#"format(-52.81014380435022, '.2%')"#);
        chk(bad, fv(f(475.7569974617859), r#"e"#), r#"'4.757570e+02'"#, r#"format(475.7569974617859, 'e')"#);
        chk(bad, fv(f(-740.77427839916), r#".2g"#), r#"'-7.4e+02'"#, r#"format(-740.77427839916, '.2g')"#);
        chk(bad, fv(f(2.4091248734435477e-193), r#"g"#), r#"'2.40912e-193'"#, r#"format(2.4091248734435477e-193, 'g')"#);
        chk(bad, fv(f(6.203425466080926e-227), r#"+,.2f"#), r#"'+0.00'"#, r#"format(6.203425466080926e-227, '+,.2f')"#);
        chk(bad, fv(f(-58.5), r#".3"#), r#"'-58.5'"#, r#"format(-58.5, '.3')"#);
        chk(bad, fv(f(-990.9376839009365), r#".17g"#), r#"'-990.93768390093646'"#, r#"format(-990.9376839009365, '.17g')"#);
        chk(bad, fv(f(-4.87), r#".1"#), r#"'-5e+00'"#, r#"format(-4.87, '.1')"#);
        chk(bad, fv(f(27.93), r#".12f"#), r#"'27.930000000000'"#, r#"format(27.93, '.12f')"#);
        chk(bad, fv(f(15930.0), r#".2g"#), r#"'1.6e+04'"#, r#"format(15930.0, '.2g')"#);
        chk(bad, fv(f(-590.6616317786691), r#".2g"#), r#"'-5.9e+02'"#, r#"format(-590.6616317786691, '.2g')"#);
        chk(bad, fv(f(3.5936867014154277e+21), r#".3f"#), r#"'3593686701415427735552.000'"#, r#"format(3.5936867014154277e+21, '.3f')"#);
        chk(bad, fv(f(58408.125), r#"<12g"#), r#"'58408.1     '"#, r#"format(58408.125, '<12g')"#);
        chk(bad, fv(f(13.8), r#".6g"#), r#"'13.8'"#, r#"format(13.8, '.6g')"#);
        chk(bad, fv(f(-477.52160750211294), r#"g"#), r#"'-477.522'"#, r#"format(-477.52160750211294, 'g')"#);
        chk(bad, fv(f(1.4091602602504732e-56), r#".3"#), r#"'1.41e-56'"#, r#"format(1.4091602602504732e-56, '.3')"#);
        chk(bad, fv(f(35.7539), r#".2%"#), r#"'3575.39%'"#, r#"format(35.7539, '.2%')"#);
        chk(bad, fv(f(-96.0), r#".2f"#), r#"'-96.00'"#, r#"format(-96.0, '.2f')"#);
        chk(bad, fv(f(-81.8399), r#".2f"#), r#"'-81.84'"#, r#"format(-81.8399, '.2f')"#);
        chk(bad, fv(f(693.5458949532899), r#".6g"#), r#"'693.546'"#, r#"format(693.5458949532899, '.6g')"#);
        chk(bad, fv(f(46.979144161663044), r#".3f"#), r#"'46.979'"#, r#"format(46.979144161663044, '.3f')"#);
        chk(bad, fv(f(15.680523509170825), r#".1"#), r#"'2e+01'"#, r#"format(15.680523509170825, '.1')"#);
        chk(bad, fv(f(1.3939768688945558e+23), r#"_.4f"#), r#"'139_397_686_889_455_584_018_432.0000'"#, r#"format(1.3939768688945558e+23, '_.4f')"#);
        chk(bad, fv(f(-33060.375), r#"#.3g"#), r#"'-3.31e+04'"#, r#"format(-33060.375, '#.3g')"#);
        chk(bad, fv(f(-980.9067295712066), r#".12f"#), r#"'-980.906729571207'"#, r#"format(-980.9067295712066, '.12f')"#);
        chk(bad, fv(f(-3.5329064621642664e-06), r#">20.5e"#), r#"'        -3.53291e-06'"#, r#"format(-3.5329064621642664e-06, '>20.5e')"#);
        chk(bad, fv(f(925.8694762554212), r#".17g"#), r#"'925.8694762554212'"#, r#"format(925.8694762554212, '.17g')"#);
        chk(bad, fv(f(912.0595162794971), r#""#), r#"'912.0595162794971'"#, r#"format(912.0595162794971, '')"#);
        chk(bad, fv(f(61.83), r#".3f"#), r#"'61.830'"#, r#"format(61.83, '.3f')"#);
        chk(bad, fv(f(-2.5489372256647267e-10), r#".10e"#), r#"'-2.5489372257e-10'"#, r#"format(-2.5489372256647267e-10, '.10e')"#);
        chk(bad, fv(f(-0.0001088302881659923), r#".3e"#), r#"'-1.088e-04'"#, r#"format(-0.0001088302881659923, '.3e')"#);
        chk(bad, fv(f(1.2644887405367847e-168), r#""#), r#"'1.2644887405367847e-168'"#, r#"format(1.2644887405367847e-168, '')"#);
        chk(bad, fv(f(3.633460463721984e-41), r#"e"#), r#"'3.633460e-41'"#, r#"format(3.633460463721984e-41, 'e')"#);
        chk(bad, fv(f(1.669723430330257e-286), r#".1g"#), r#"'2e-286'"#, r#"format(1.669723430330257e-286, '.1g')"#);
        chk(bad, fv(f(48.4), r#".2f"#), r#"'48.40'"#, r#"format(48.4, '.2f')"#);
        chk(bad, fv(f(-147.03149342071129), r#".2%"#), r#"'-14703.15%'"#, r#"format(-147.03149342071129, '.2%')"#);
        chk(bad, fv(f(-114968.29168829747), r#"_.4f"#), r#"'-114_968.2917'"#, r#"format(-114968.29168829747, '_.4f')"#);
        chk(bad, fv(f(-29330.875), r#".0e"#), r#"'-3e+04'"#, r#"format(-29330.875, '.0e')"#);
        chk(bad, fv(f(8.601503790611218e-238), r#".1"#), r#"'9e-238'"#, r#"format(8.601503790611218e-238, '.1')"#);
        chk(bad, fv(f(-22784.75), r#"_.4f"#), r#"'-22_784.7500'"#, r#"format(-22784.75, '_.4f')"#);
        chk(bad, fv(f(0.008191517000683142), r#"g"#), r#"'0.00819152'"#, r#"format(0.008191517000683142, 'g')"#);
        chk(bad, fv(f(-71807.375), r#"+,.2f"#), r#"'-71,807.38'"#, r#"format(-71807.375, '+,.2f')"#);
        chk(bad, fv(f(355.51384742724144), r#".3"#), r#"'3.56e+02'"#, r#"format(355.51384742724144, '.3')"#);
        chk(bad, fv(f(-714.320038395728), r#"015.3f"#), r#"'-0000000714.320'"#, r#"format(-714.320038395728, '015.3f')"#);
        chk(bad, fv(f(-2234169068633.555), r#".6g"#), r#"'-2.23417e+12'"#, r#"format(-2234169068633.555, '.6g')"#);
        chk(bad, fv(f(5.3747731042606403e-17), r#".1g"#), r#"'5e-17'"#, r#"format(5.3747731042606403e-17, '.1g')"#);
        chk(bad, fv(f(1.5602617155137053e-156), r#".1f"#), r#"'0.0'"#, r#"format(1.5602617155137053e-156, '.1f')"#);
        chk(bad, fv(f(-88.2737), r#".1g"#), r#"'-9e+01'"#, r#"format(-88.2737, '.1g')"#);
        chk(bad, fv(f(94.7), r#"+,.2f"#), r#"'+94.70'"#, r#"format(94.7, '+,.2f')"#);
        chk(bad, fv(f(-26664.125), r#".12f"#), r#"'-26664.125000000000'"#, r#"format(-26664.125, '.12f')"#);
        chk(bad, fv(f(-68175.75), r#"^+18,.3f"#), r#"'   -68,175.750    '"#, r#"format(-68175.75, '^+18,.3f')"#);
        chk(bad, fv(f(85669.0), r#"^+18,.3f"#), r#"'   +85,669.000    '"#, r#"format(85669.0, '^+18,.3f')"#);
        chk(bad, fv(f(-2.644501896707397e+17), r#".12f"#), r#"'-264450189670739712.000000000000'"#, r#"format(-2.644501896707397e+17, '.12f')"#);
        chk(bad, fv(f(3.742964526360339e-24), r#"_.4f"#), r#"'0.0000'"#, r#"format(3.742964526360339e-24, '_.4f')"#);
        chk(bad, fv(f(8.852023083261412e-149), r#".2%"#), r#"'0.00%'"#, r#"format(8.852023083261412e-149, '.2%')"#);
        chk(bad, fv(f(-98.0), r#"015.3f"#), r#"'-0000000098.000'"#, r#"format(-98.0, '015.3f')"#);
        chk(bad, fv(f(69.74), r#"g"#), r#"'69.74'"#, r#"format(69.74, 'g')"#);
        chk(bad, fv(f(58588.25), r#".1g"#), r#"'6e+04'"#, r#"format(58588.25, '.1g')"#);
        chk(bad, fv(f(87.87810213624039), r#"_.4f"#), r#"'87.8781'"#, r#"format(87.87810213624039, '_.4f')"#);
        chk(bad, fv(f(-41.7), r#".1"#), r#"'-4e+01'"#, r#"format(-41.7, '.1')"#);
        chk(bad, fv(f(-358.379355896789), r#".1f"#), r#"'-358.4'"#, r#"format(-358.379355896789, '.1f')"#);
        chk(bad, fv(f(34.0), r#".1"#), r#"'3e+01'"#, r#"format(34.0, '.1')"#);
        chk(bad, fv(f(-133851839979901.66), r#".15g"#), r#"'-133851839979902'"#, r#"format(-133851839979901.66, '.15g')"#);
        chk(bad, fv(f(742.2149891779115), r#"<12g"#), r#"'742.215     '"#, r#"format(742.2149891779115, '<12g')"#);
        chk(bad, fv(f(-964.7267187389854), r#".1f"#), r#"'-964.7'"#, r#"format(-964.7267187389854, '.1f')"#);
        chk(bad, fv(f(59927813280.55721), r#".2f"#), r#"'59927813280.56'"#, r#"format(59927813280.55721, '.2f')"#);
        chk(bad, fv(f(1.6936444745417956e+20), r#".3e"#), r#"'1.694e+20'"#, r#"format(1.6936444745417956e+20, '.3e')"#);
        chk(bad, fv(f(5.755717659976259e-124), r#".3f"#), r#"'0.000'"#, r#"format(5.755717659976259e-124, '.3f')"#);
        chk(bad, fv(f(6.380766102822263e-178), r#".10e"#), r#"'6.3807661028e-178'"#, r#"format(6.380766102822263e-178, '.10e')"#);
        chk(bad, fv(f(43.37), r#".0f"#), r#"'43'"#, r#"format(43.37, '.0f')"#);
        chk(bad, fv(f(-7.055880816396098e-19), r#"g"#), r#"'-7.05588e-19'"#, r#"format(-7.055880816396098e-19, 'g')"#);
        chk(bad, fv(f(1.3075406763143875e-46), r#">20.5e"#), r#"'         1.30754e-46'"#, r#"format(1.3075406763143875e-46, '>20.5e')"#);
        chk(bad, fv(f(23.535), r#"015.3f"#), r#"'00000000023.535'"#, r#"format(23.535, '015.3f')"#);
        chk(bad, fv(f(-882.4331890660644), r#"g"#), r#"'-882.433'"#, r#"format(-882.4331890660644, 'g')"#);
        chk(bad, fv(f(-917.7761483714899), r#".1"#), r#"'-9e+02'"#, r#"format(-917.7761483714899, '.1')"#);
        chk(bad, fv(f(-5.169346160120112e-13), r#"+,.2f"#), r#"'-0.00'"#, r#"format(-5.169346160120112e-13, '+,.2f')"#);
        chk(bad, fv(f(14.7379), r#"+,.2f"#), r#"'+14.74'"#, r#"format(14.7379, '+,.2f')"#);
        chk(bad, fv(f(-17.0), r#"=+14.2E"#), r#"'-     1.70E+01'"#, r#"format(-17.0, '=+14.2E')"#);
        chk(bad, fv(f(354.72955711049985), r#".0f"#), r#"'355'"#, r#"format(354.72955711049985, '.0f')"#);
        chk(bad, fv(f(640.1820136980703), r#"#.3g"#), r#"'640.'"#, r#"format(640.1820136980703, '#.3g')"#);
        chk(bad, fv(f(-78.0), r#".2f"#), r#"'-78.00'"#, r#"format(-78.0, '.2f')"#);
        chk(bad, fv(f(708.2199582162666), r#".2g"#), r#"'7.1e+02'"#, r#"format(708.2199582162666, '.2g')"#);
        chk(bad, fv(f(1.0297636415035644e-206), r#".1f"#), r#"'0.0'"#, r#"format(1.0297636415035644e-206, '.1f')"#);
        chk(bad, fv(f(-1.7046759769211756e+21), r#"+,.2f"#), r#"'-1,704,675,976,921,175,556,096.00'"#, r#"format(-1.7046759769211756e+21, '+,.2f')"#);
        chk(bad, fv(f(-107939.75), r#".2g"#), r#"'-1.1e+05'"#, r#"format(-107939.75, '.2g')"#);
        chk(bad, fv(f(47.0892), r#".3f"#), r#"'47.089'"#, r#"format(47.0892, '.3f')"#);
        chk(bad, fv(f(-251.64419784966105), r#".6g"#), r#"'-251.644'"#, r#"format(-251.64419784966105, '.6g')"#);
        chk(bad, fv(f(5.057588587337311e-52), r#".1g"#), r#"'5e-52'"#, r#"format(5.057588587337311e-52, '.1g')"#);
        chk(bad, fv(f(-44.35), r#".2%"#), r#"'-4435.00%'"#, r#"format(-44.35, '.2%')"#);
        chk(bad, fv(f(-51585629909047.64), r#".2%"#), r#"'-5158562990904764.00%'"#, r#"format(-51585629909047.64, '.2%')"#);
        chk(bad, fv(f(-829.5683906251679), r#"g"#), r#"'-829.568'"#, r#"format(-829.5683906251679, 'g')"#);
        chk(bad, fv(f(1.7740027133891776e-40), r#"_.4f"#), r#"'0.0000'"#, r#"format(1.7740027133891776e-40, '_.4f')"#);
        chk(bad, fv(f(-642.9568849547998), r#"+,.2f"#), r#"'-642.96'"#, r#"format(-642.9568849547998, '+,.2f')"#);
        chk(bad, fv(f(1.2519714290622905e+22), r#"+,.2f"#), r#"'+12,519,714,290,622,904,926,208.00'"#, r#"format(1.2519714290622905e+22, '+,.2f')"#);
        chk(bad, fv(f(1.7343007801151188e-305), r#".3"#), r#"'1.73e-305'"#, r#"format(1.7343007801151188e-305, '.3')"#);
        chk(bad, fv(f(-673.4357630154038), r#"^+18,.3f"#), r#"'     -673.436     '"#, r#"format(-673.4357630154038, '^+18,.3f')"#);
        chk(bad, fv(f(-785.6218613535968), r#"015.3f"#), r#"'-0000000785.622'"#, r#"format(-785.6218613535968, '015.3f')"#);
        chk(bad, fv(f(1.955753209919555e-38), r#"+,.2f"#), r#"'+0.00'"#, r#"format(1.955753209919555e-38, '+,.2f')"#);
        chk(bad, fv(f(107913.25), r#".2%"#), r#"'10791325.00%'"#, r#"format(107913.25, '.2%')"#);
        chk(bad, fv(f(-6.118193568835006e+18), r#""#), r#"'-6.118193568835006e+18'"#, r#"format(-6.118193568835006e+18, '')"#);
        chk(bad, fv(f(27.61), r#".3f"#), r#"'27.610'"#, r#"format(27.61, '.3f')"#);
        chk(bad, fv(f(-525.8282306007077), r#"^+18,.3f"#), r#"'     -525.828     '"#, r#"format(-525.8282306007077, '^+18,.3f')"#);
        chk(bad, fv(f(625.4314740004781), r#".10e"#), r#"'6.2543147400e+02'"#, r#"format(625.4314740004781, '.10e')"#);
        chk(bad, fv(f(3.2563744911569834e-237), r#"#.3g"#), r#"'3.26e-237'"#, r#"format(3.2563744911569834e-237, '#.3g')"#);
        chk(bad, fv(f(2868.875), r#".0f"#), r#"'2869'"#, r#"format(2868.875, '.0f')"#);
        chk(bad, fv(f(-962.1895631613986), r#"z.1f"#), r#"'-962.2'"#, r#"format(-962.1895631613986, 'z.1f')"#);
        chk(bad, fv(f(6.152373976824581e-219), r#".6g"#), r#"'6.15237e-219'"#, r#"format(6.152373976824581e-219, '.6g')"#);
        chk(bad, fv(f(90.10361190705817), r#"_.4f"#), r#"'90.1036'"#, r#"format(90.10361190705817, '_.4f')"#);
        chk(bad, fv(f(1.8536124155386193e-110), r#"015.3f"#), r#"'00000000000.000'"#, r#"format(1.8536124155386193e-110, '015.3f')"#);
        chk(bad, fv(f(6.190066693874773e-150), r#"z.1f"#), r#"'0.0'"#, r#"format(6.190066693874773e-150, 'z.1f')"#);
        chk(bad, fv(f(679453805.0013361), r#".3"#), r#"'6.79e+08'"#, r#"format(679453805.0013361, '.3')"#);
        chk(bad, fv(f(69989.875), r#".17g"#), r#"'69989.875'"#, r#"format(69989.875, '.17g')"#);
        chk(bad, fv(f(-51555.625), r#"015.3f"#), r#"'-0000051555.625'"#, r#"format(-51555.625, '015.3f')"#);
        chk(bad, fv(f(-403.9665590957244), r#"z.1f"#), r#"'-404.0'"#, r#"format(-403.9665590957244, 'z.1f')"#);
        chk(bad, fv(f(446.8395552110642), r#"e"#), r#"'4.468396e+02'"#, r#"format(446.8395552110642, 'e')"#);
        chk(bad, fv(f(-154.55379715057302), r#"e"#), r#"'-1.545538e+02'"#, r#"format(-154.55379715057302, 'e')"#);
        chk(bad, fv(f(-665.8114676580165), r#".2f"#), r#"'-665.81'"#, r#"format(-665.8114676580165, '.2f')"#);
        chk(bad, fv(f(98727.875), r#".17g"#), r#"'98727.875'"#, r#"format(98727.875, '.17g')"#);
        chk(bad, fv(f(-271.19897936196), r#".3"#), r#"'-2.71e+02'"#, r#"format(-271.19897936196, '.3')"#);
        chk(bad, fv(f(-1.758519547709245e-11), r#"=+14.2E"#), r#"'-     1.76E-11'"#, r#"format(-1.758519547709245e-11, '=+14.2E')"#);
        chk(bad, fv(f(1.1146002346981755e-55), r#"_.4f"#), r#"'0.0000'"#, r#"format(1.1146002346981755e-55, '_.4f')"#);
        chk(bad, fv(f(4.079550189569421e-111), r#"+,.2f"#), r#"'+0.00'"#, r#"format(4.079550189569421e-111, '+,.2f')"#);
        chk(bad, fv(f(705.33324733075), r#".3e"#), r#"'7.053e+02'"#, r#"format(705.33324733075, '.3e')"#);
        chk(bad, fv(f(-5.482595725977075e+21), r#".1"#), r#"'-5e+21'"#, r#"format(-5.482595725977075e+21, '.1')"#);
        chk(bad, fv(f(120543.5), r#".2f"#), r#"'120543.50'"#, r#"format(120543.5, '.2f')"#);
        chk(bad, fv(f(4.0352457434312365e+17), r#".1"#), r#"'4e+17'"#, r#"format(4.0352457434312365e+17, '.1')"#);
        chk(bad, fv(f(-8831.75), r#"e"#), r#"'-8.831750e+03'"#, r#"format(-8831.75, 'e')"#);
        chk(bad, fv(f(65.1443), r#".15g"#), r#"'65.1443'"#, r#"format(65.1443, '.15g')"#);
        chk(bad, fv(f(-94.55), r#">20.5e"#), r#"'        -9.45500e+01'"#, r#"format(-94.55, '>20.5e')"#);
        chk(bad, fv(f(-563.10034045505), r#".6g"#), r#"'-563.1'"#, r#"format(-563.10034045505, '.6g')"#);
        chk(bad, fv(f(-12477.5), r#".6g"#), r#"'-12477.5'"#, r#"format(-12477.5, '.6g')"#);
        chk(bad, fv(f(70.56), r#"z.1f"#), r#"'70.6'"#, r#"format(70.56, 'z.1f')"#);
        chk(bad, fv(f(48.0), r#"_.4f"#), r#"'48.0000'"#, r#"format(48.0, '_.4f')"#);
        chk(bad, fv(f(180026447.69469732), r#"#.3g"#), r#"'1.80e+08'"#, r#"format(180026447.69469732, '#.3g')"#);
        chk(bad, fv(f(913.4907497918148), r#"015.3f"#), r#"'00000000913.491'"#, r#"format(913.4907497918148, '015.3f')"#);
        chk(bad, fv(f(-122362.25), r#".10e"#), r#"'-1.2236225000e+05'"#, r#"format(-122362.25, '.10e')"#);
        chk(bad, fv(f(76.01), r#">20.5e"#), r#"'         7.60100e+01'"#, r#"format(76.01, '>20.5e')"#);
        chk(bad, fv(f(87624.125), r#".2f"#), r#"'87624.12'"#, r#"format(87624.125, '.2f')"#);
        chk(bad, fv(f(198.061836454101), r#".1g"#), r#"'2e+02'"#, r#"format(198.061836454101, '.1g')"#);
        chk(bad, fv(f(-0.0009427301574865897), r#".3"#), r#"'-0.000943'"#, r#"format(-0.0009427301574865897, '.3')"#);
        chk(bad, fv(f(10.0), r#".3e"#), r#"'1.000e+01'"#, r#"format(10.0, '.3e')"#);
        chk(bad, fv(f(-97230.25), r#".1"#), r#"'-1e+05'"#, r#"format(-97230.25, '.1')"#);
        chk(bad, fv(f(1.550279418282103e-272), r#"=+14.2E"#), r#"'+    1.55E-272'"#, r#"format(1.550279418282103e-272, '=+14.2E')"#);
        chk(bad, fv(f(-98.689), r#"z.1f"#), r#"'-98.7'"#, r#"format(-98.689, 'z.1f')"#);
        chk(bad, fv(f(282534374936.9299), r#""#), r#"'282534374936.9299'"#, r#"format(282534374936.9299, '')"#);
        chk(bad, fv(f(5.5865586853539365e+19), r#".2f"#), r#"'55865586853539364864.00'"#, r#"format(5.5865586853539365e+19, '.2f')"#);
        chk(bad, fv(f(53415.125), r#".15g"#), r#"'53415.125'"#, r#"format(53415.125, '.15g')"#);
        chk(bad, fv(f(-3.2567252095183778e+19), r#""#), r#"'-3.2567252095183778e+19'"#, r#"format(-3.2567252095183778e+19, '')"#);
        chk(bad, fv(f(-5.824073454104606e-18), r#".6g"#), r#"'-5.82407e-18'"#, r#"format(-5.824073454104606e-18, '.6g')"#);
        chk(bad, fv(f(4.4295773795353664e-06), r#".3f"#), r#"'0.000'"#, r#"format(4.4295773795353664e-06, '.3f')"#);
        chk(bad, fv(f(872.6176530074392), r#"015.3f"#), r#"'00000000872.618'"#, r#"format(872.6176530074392, '015.3f')"#);
        chk(bad, fv(f(644.4212411967287), r#"+,.2f"#), r#"'+644.42'"#, r#"format(644.4212411967287, '+,.2f')"#);
        chk(bad, fv(f(-882.750108695433), r#".1g"#), r#"'-9e+02'"#, r#"format(-882.750108695433, '.1g')"#);
        chk(bad, fv(f(38.0), r#"#.3g"#), r#"'38.0'"#, r#"format(38.0, '#.3g')"#);
        chk(bad, fv(f(919.1678598509388), r#".2g"#), r#"'9.2e+02'"#, r#"format(919.1678598509388, '.2g')"#);
        chk(bad, fv(f(-63.6196), r#".1"#), r#"'-6e+01'"#, r#"format(-63.6196, '.1')"#);
        chk(bad, fv(f(-29915.0), r#".17g"#), r#"'-29915'"#, r#"format(-29915.0, '.17g')"#);
        chk(bad, fv(f(-97.0), r#".17g"#), r#"'-97'"#, r#"format(-97.0, '.17g')"#);
        chk(bad, fv(f(12.465), r#".15g"#), r#"'12.465'"#, r#"format(12.465, '.15g')"#);
        chk(bad, fv(f(-798.8464406640289), r#">20.5e"#), r#"'        -7.98846e+02'"#, r#"format(-798.8464406640289, '>20.5e')"#);
        chk(bad, fv(f(782.2522493605154), r#".17g"#), r#"'782.25224936051541'"#, r#"format(782.2522493605154, '.17g')"#);
        chk(bad, fv(f(13.5), r#"^+18,.3f"#), r#"'     +13.500      '"#, r#"format(13.5, '^+18,.3f')"#);
        chk(bad, fv(f(73.0), r#"e"#), r#"'7.300000e+01'"#, r#"format(73.0, 'e')"#);
        chk(bad, fv(f(700.16363215111), r#".3f"#), r#"'700.164'"#, r#"format(700.16363215111, '.3f')"#);
        chk(bad, fv(f(-157.29177176331018), r#".10e"#), r#"'-1.5729177176e+02'"#, r#"format(-157.29177176331018, '.10e')"#);
        chk(bad, fv(f(-49303.25), r#".12f"#), r#"'-49303.250000000000'"#, r#"format(-49303.25, '.12f')"#);
        chk(bad, fv(f(87711.625), r#"+,.2f"#), r#"'+87,711.62'"#, r#"format(87711.625, '+,.2f')"#);
        chk(bad, fv(f(-495.5685703264419), r#".3f"#), r#"'-495.569'"#, r#"format(-495.5685703264419, '.3f')"#);
        chk(bad, fv(f(-10.0), r#".3f"#), r#"'-10.000'"#, r#"format(-10.0, '.3f')"#);
        chk(bad, fv(f(-424.1577135773571), r#"015.3f"#), r#"'-0000000424.158'"#, r#"format(-424.1577135773571, '015.3f')"#);
        chk(bad, fv(f(0.028362441134297353), r#"+,.2f"#), r#"'+0.03'"#, r#"format(0.028362441134297353, '+,.2f')"#);
        chk(bad, fv(f(51.0), r#""#), r#"'51.0'"#, r#"format(51.0, '')"#);
        chk(bad, fv(f(-802.5877228841289), r#".2f"#), r#"'-802.59'"#, r#"format(-802.5877228841289, '.2f')"#);
        chk(bad, fv(f(5.8), r#".0e"#), r#"'6e+00'"#, r#"format(5.8, '.0e')"#);
        chk(bad, fv(f(1.8410325551557529e-214), r#".2%"#), r#"'0.00%'"#, r#"format(1.8410325551557529e-214, '.2%')"#);
        chk(bad, fv(f(6.94543226174605e-288), r#".6g"#), r#"'6.94543e-288'"#, r#"format(6.94543226174605e-288, '.6g')"#);
        chk(bad, fv(f(4.132621827860623e-53), r#""#), r#"'4.132621827860623e-53'"#, r#"format(4.132621827860623e-53, '')"#);
        chk(bad, fv(f(-71.0), r#""#), r#"'-71.0'"#, r#"format(-71.0, '')"#);
        chk(bad, fv(f(1.6021208074433833e-14), r#".2%"#), r#"'0.00%'"#, r#"format(1.6021208074433833e-14, '.2%')"#);
        chk(bad, fv(f(-1.192998508189509e+18), r#".15g"#), r#"'-1.19299850818951e+18'"#, r#"format(-1.192998508189509e+18, '.15g')"#);
        chk(bad, fv(f(-68.8781), r#".2g"#), r#"'-69'"#, r#"format(-68.8781, '.2g')"#);
        chk(bad, fv(f(-282.8411477718711), r#""#), r#"'-282.8411477718711'"#, r#"format(-282.8411477718711, '')"#);
        chk(bad, fv(f(-21.0), r#""#), r#"'-21.0'"#, r#"format(-21.0, '')"#);
        chk(bad, fv(f(2766.1033427939233), r#""#), r#"'2766.1033427939233'"#, r#"format(2766.1033427939233, '')"#);
        chk(bad, fv(f(-30.0), r#".6g"#), r#"'-30'"#, r#"format(-30.0, '.6g')"#);
        chk(bad, fv(f(894.1945690160633), r#"015.3f"#), r#"'00000000894.195'"#, r#"format(894.1945690160633, '015.3f')"#);
        chk(bad, fv(f(81.1385), r#"=+14.2E"#), r#"'+     8.11E+01'"#, r#"format(81.1385, '=+14.2E')"#);
        chk(bad, fv(f(36233.0), r#".3f"#), r#"'36233.000'"#, r#"format(36233.0, '.3f')"#);
        chk(bad, fv(f(-28526.375), r#"=+14.2E"#), r#"'-     2.85E+04'"#, r#"format(-28526.375, '=+14.2E')"#);
        chk(bad, fv(f(86.943), r#".3"#), r#"'86.9'"#, r#"format(86.943, '.3')"#);
        chk(bad, fv(f(43.74861004071043), r#"e"#), r#"'4.374861e+01'"#, r#"format(43.74861004071043, 'e')"#);
        chk(bad, fv(f(-9463.875), r#".1f"#), r#"'-9463.9'"#, r#"format(-9463.875, '.1f')"#);
        chk(bad, fv(f(-333.0830061509928), r#"<12g"#), r#"'-333.083    '"#, r#"format(-333.0830061509928, '<12g')"#);
        chk(bad, fv(f(-6.850283761020772e-16), r#"=+14.2E"#), r#"'-     6.85E-16'"#, r#"format(-6.850283761020772e-16, '=+14.2E')"#);
        chk(bad, fv(f(6.2670807728310106e-211), r#".10e"#), r#"'6.2670807728e-211'"#, r#"format(6.2670807728310106e-211, '.10e')"#);
        chk(bad, fv(f(-102.8978645821976), r#"<12g"#), r#"'-102.898    '"#, r#"format(-102.8978645821976, '<12g')"#);
        chk(bad, fv(f(331.15320231897977), r#"g"#), r#"'331.153'"#, r#"format(331.15320231897977, 'g')"#);
        chk(bad, fv(f(4.258002109278308e-08), r#"<12g"#), r#"'4.258e-08   '"#, r#"format(4.258002109278308e-08, '<12g')"#);
        chk(bad, fv(f(99120.625), r#".2%"#), r#"'9912062.50%'"#, r#"format(99120.625, '.2%')"#);
        chk(bad, fv(f(2.4436758909558793e-188), r#".2%"#), r#"'0.00%'"#, r#"format(2.4436758909558793e-188, '.2%')"#);
        chk(bad, fv(f(582.8618160783255), r#".2g"#), r#"'5.8e+02'"#, r#"format(582.8618160783255, '.2g')"#);
        chk(bad, fv(f(4.1660265248795285e-15), r#".15g"#), r#"'4.16602652487953e-15'"#, r#"format(4.1660265248795285e-15, '.15g')"#);
        chk(bad, fv(f(1.5439164374086149e-84), r#"e"#), r#"'1.543916e-84'"#, r#"format(1.5439164374086149e-84, 'e')"#);
        chk(bad, fv(f(79237.25), r#".3e"#), r#"'7.924e+04'"#, r#"format(79237.25, '.3e')"#);
        chk(bad, fv(f(-47.7825), r#".3"#), r#"'-47.8'"#, r#"format(-47.7825, '.3')"#);
        chk(bad, fv(f(284.02219506930055), r#"e"#), r#"'2.840222e+02'"#, r#"format(284.02219506930055, 'e')"#);
        chk(bad, fv(f(23.0), r#"#.3g"#), r#"'23.0'"#, r#"format(23.0, '#.3g')"#);
        chk(bad, fv(f(-244.01044743599005), r#".1f"#), r#"'-244.0'"#, r#"format(-244.01044743599005, '.1f')"#);
        chk(bad, fv(f(-66.0), r#".15g"#), r#"'-66'"#, r#"format(-66.0, '.15g')"#);
        chk(bad, fv(f(99.0), r#".1f"#), r#"'99.0'"#, r#"format(99.0, '.1f')"#);
        chk(bad, fv(f(3.052276128676385e-190), r#""#), r#"'3.052276128676385e-190'"#, r#"format(3.052276128676385e-190, '')"#);
        chk(bad, fv(f(-6.545166042564313e-13), r#"_.4f"#), r#"'-0.0000'"#, r#"format(-6.545166042564313e-13, '_.4f')"#);
        chk(bad, fv(f(40.1753), r#".1"#), r#"'4e+01'"#, r#"format(40.1753, '.1')"#);
        chk(bad, fv(f(327.5441804999589), r#".1g"#), r#"'3e+02'"#, r#"format(327.5441804999589, '.1g')"#);
        chk(bad, fv(f(937.6911546213018), r#".15g"#), r#"'937.691154621302'"#, r#"format(937.6911546213018, '.15g')"#);
        chk(bad, fv(f(-635.7222138673113), r#"015.3f"#), r#"'-0000000635.722'"#, r#"format(-635.7222138673113, '015.3f')"#);
        chk(bad, fv(f(36.74), r#".2f"#), r#"'36.74'"#, r#"format(36.74, '.2f')"#);
        chk(bad, fv(f(-35.68), r#".12f"#), r#"'-35.680000000000'"#, r#"format(-35.68, '.12f')"#);
        chk(bad, fv(f(-106722.0), r#"=+14.2E"#), r#"'-     1.07E+05'"#, r#"format(-106722.0, '=+14.2E')"#);
        chk(bad, fv(f(-104451.875), r#".2%"#), r#"'-10445187.50%'"#, r#"format(-104451.875, '.2%')"#);
        chk(bad, fv(f(-94.77110471612195), r#".15g"#), r#"'-94.771104716122'"#, r#"format(-94.77110471612195, '.15g')"#);
        chk(bad, fv(f(29.59), r#".3e"#), r#"'2.959e+01'"#, r#"format(29.59, '.3e')"#);
        chk(bad, fv(f(8.721602679015909e-237), r#"015.3f"#), r#"'00000000000.000'"#, r#"format(8.721602679015909e-237, '015.3f')"#);
        chk(bad, fv(f(-5.005), r#".1g"#), r#"'-5'"#, r#"format(-5.005, '.1g')"#);
        chk(bad, fv(f(-3.8065108570339436e+18), r#".6g"#), r#"'-3.80651e+18'"#, r#"format(-3.8065108570339436e+18, '.6g')"#);
        chk(bad, fv(f(2679445051.5928884), r#".15g"#), r#"'2679445051.59289'"#, r#"format(2679445051.5928884, '.15g')"#);
        chk(bad, fv(f(49.67), r#".0e"#), r#"'5e+01'"#, r#"format(49.67, '.0e')"#);
        chk(bad, fv(f(-810.6884414591946), r#"#.3g"#), r#"'-811.'"#, r#"format(-810.6884414591946, '#.3g')"#);
        chk(bad, fv(f(1.1398129581781317e-141), r#"_.4f"#), r#"'0.0000'"#, r#"format(1.1398129581781317e-141, '_.4f')"#);
        chk(bad, fv(f(2.5252166636990403e-42), r#".0e"#), r#"'3e-42'"#, r#"format(2.5252166636990403e-42, '.0e')"#);
        chk(bad, fv(f(104337.75), r#"_.4f"#), r#"'104_337.7500'"#, r#"format(104337.75, '_.4f')"#);
        chk(bad, fv(f(346.45752173366145), r#"#.3g"#), r#"'346.'"#, r#"format(346.45752173366145, '#.3g')"#);
        chk(bad, b("round", &[f(334.0110489265344), i(5)]), r#"334.01105"#, r#"round(334.0110489265344, 5)"#);
        chk(bad, b("str", &[f(334.0110489265344)]), r#"'334.0110489265344'"#, r#"str(334.0110489265344)"#);
        chk(bad, b("round", &[f(583.9000262860272), i(-3)]), r#"1000.0"#, r#"round(583.9000262860272, -3)"#);
        chk(bad, b("str", &[f(583.9000262860272)]), r#"'583.9000262860272'"#, r#"str(583.9000262860272)"#);
        chk(bad, b("round", &[f(-2.432587344132331e-09), i(5)]), r#"-0.0"#, r#"round(-2.432587344132331e-09, 5)"#);
        chk(bad, b("str", &[f(-2.432587344132331e-09)]), r#"'-2.432587344132331e-09'"#, r#"str(-2.432587344132331e-09)"#);
        chk(bad, b("round", &[f(34.2), i(0)]), r#"34.0"#, r#"round(34.2, 0)"#);
        chk(bad, b("str", &[f(34.2)]), r#"'34.2'"#, r#"str(34.2)"#);
        chk(bad, b("round", &[f(-862.5279444129872), i(7)]), r#"-862.5279444"#, r#"round(-862.5279444129872, 7)"#);
        chk(bad, b("str", &[f(-862.5279444129872)]), r#"'-862.5279444129872'"#, r#"str(-862.5279444129872)"#);
        chk(bad, b("round", &[f(2.0180718154186964e-82), i(2)]), r#"0.0"#, r#"round(2.0180718154186964e-82, 2)"#);
        chk(bad, b("str", &[f(2.0180718154186964e-82)]), r#"'2.0180718154186964e-82'"#, r#"str(2.0180718154186964e-82)"#);
        chk(bad, b("round", &[f(83727.0), i(2)]), r#"83727.0"#, r#"round(83727.0, 2)"#);
        chk(bad, b("str", &[f(83727.0)]), r#"'83727.0'"#, r#"str(83727.0)"#);
        chk(bad, b("round", &[f(3.0887836528516124e-29), i(-2)]), r#"0.0"#, r#"round(3.0887836528516124e-29, -2)"#);
        chk(bad, b("str", &[f(3.0887836528516124e-29)]), r#"'3.0887836528516124e-29'"#, r#"str(3.0887836528516124e-29)"#);
        chk(bad, b("round", &[f(-484.85245887294525), i(0)]), r#"-485.0"#, r#"round(-484.85245887294525, 0)"#);
        chk(bad, b("str", &[f(-484.85245887294525)]), r#"'-484.85245887294525'"#, r#"str(-484.85245887294525)"#);
        chk(bad, b("round", &[f(108784.875), i(3)]), r#"108784.875"#, r#"round(108784.875, 3)"#);
        chk(bad, b("str", &[f(108784.875)]), r#"'108784.875'"#, r#"str(108784.875)"#);
        chk(bad, b("round", &[f(-223.20357372876458), i(-1)]), r#"-220.0"#, r#"round(-223.20357372876458, -1)"#);
        chk(bad, b("str", &[f(-223.20357372876458)]), r#"'-223.20357372876458'"#, r#"str(-223.20357372876458)"#);
        chk(bad, b("round", &[f(93254.875), i(4)]), r#"93254.875"#, r#"round(93254.875, 4)"#);
        chk(bad, b("str", &[f(93254.875)]), r#"'93254.875'"#, r#"str(93254.875)"#);
        chk(bad, b("round", &[f(6.592765155025257e+23), i(1)]), r#"6.592765155025257e+23"#, r#"round(6.592765155025257e+23, 1)"#);
        chk(bad, b("str", &[f(6.592765155025257e+23)]), r#"'6.592765155025257e+23'"#, r#"str(6.592765155025257e+23)"#);
        chk(bad, b("round", &[f(-101.57389062458708), i(-1)]), r#"-100.0"#, r#"round(-101.57389062458708, -1)"#);
        chk(bad, b("str", &[f(-101.57389062458708)]), r#"'-101.57389062458708'"#, r#"str(-101.57389062458708)"#);
        chk(bad, b("round", &[f(5.8350063580547895e-16), i(4)]), r#"0.0"#, r#"round(5.8350063580547895e-16, 4)"#);
        chk(bad, b("str", &[f(5.8350063580547895e-16)]), r#"'5.8350063580547895e-16'"#, r#"str(5.8350063580547895e-16)"#);
        chk(bad, b("round", &[f(196.25786034348334), i(-2)]), r#"200.0"#, r#"round(196.25786034348334, -2)"#);
        chk(bad, b("str", &[f(196.25786034348334)]), r#"'196.25786034348334'"#, r#"str(196.25786034348334)"#);
        chk(bad, b("round", &[f(-2.343037675928233e+19), i(-2)]), r#"-2.343037675928233e+19"#, r#"round(-2.343037675928233e+19, -2)"#);
        chk(bad, b("str", &[f(-2.343037675928233e+19)]), r#"'-2.343037675928233e+19'"#, r#"str(-2.343037675928233e+19)"#);
        chk(bad, b("round", &[f(-39661.375), i(1)]), r#"-39661.4"#, r#"round(-39661.375, 1)"#);
        chk(bad, b("str", &[f(-39661.375)]), r#"'-39661.375'"#, r#"str(-39661.375)"#);
        chk(bad, b("round", &[f(2.209962141878696e-10), i(8)]), r#"0.0"#, r#"round(2.209962141878696e-10, 8)"#);
        chk(bad, b("str", &[f(2.209962141878696e-10)]), r#"'2.209962141878696e-10'"#, r#"str(2.209962141878696e-10)"#);
        chk(bad, b("round", &[f(52142.625), i(7)]), r#"52142.625"#, r#"round(52142.625, 7)"#);
        chk(bad, b("str", &[f(52142.625)]), r#"'52142.625'"#, r#"str(52142.625)"#);
        chk(bad, b("round", &[f(49912.68009031177), i(0)]), r#"49913.0"#, r#"round(49912.68009031177, 0)"#);
        chk(bad, b("str", &[f(49912.68009031177)]), r#"'49912.68009031177'"#, r#"str(49912.68009031177)"#);
        chk(bad, b("round", &[f(702.8986832750027), i(1)]), r#"702.9"#, r#"round(702.8986832750027, 1)"#);
        chk(bad, b("str", &[f(702.8986832750027)]), r#"'702.8986832750027'"#, r#"str(702.8986832750027)"#);
        chk(bad, b("round", &[f(-274.2357189551184), i(5)]), r#"-274.23572"#, r#"round(-274.2357189551184, 5)"#);
        chk(bad, b("str", &[f(-274.2357189551184)]), r#"'-274.2357189551184'"#, r#"str(-274.2357189551184)"#);
        chk(bad, b("round", &[f(8.355035229831984e-221), i(7)]), r#"0.0"#, r#"round(8.355035229831984e-221, 7)"#);
        chk(bad, b("str", &[f(8.355035229831984e-221)]), r#"'8.355035229831984e-221'"#, r#"str(8.355035229831984e-221)"#);
        chk(bad, b("round", &[f(-77.0), i(0)]), r#"-77.0"#, r#"round(-77.0, 0)"#);
        chk(bad, b("str", &[f(-77.0)]), r#"'-77.0'"#, r#"str(-77.0)"#);
        chk(bad, b("round", &[f(64.1), i(4)]), r#"64.1"#, r#"round(64.1, 4)"#);
        chk(bad, b("str", &[f(64.1)]), r#"'64.1'"#, r#"str(64.1)"#);
        chk(bad, b("round", &[f(5.742356493048519e-144), i(-2)]), r#"0.0"#, r#"round(5.742356493048519e-144, -2)"#);
        chk(bad, b("str", &[f(5.742356493048519e-144)]), r#"'5.742356493048519e-144'"#, r#"str(5.742356493048519e-144)"#);
        chk(bad, b("round", &[f(4.040046734692611e-08), i(6)]), r#"0.0"#, r#"round(4.040046734692611e-08, 6)"#);
        chk(bad, b("str", &[f(4.040046734692611e-08)]), r#"'4.040046734692611e-08'"#, r#"str(4.040046734692611e-08)"#);
        chk(bad, b("round", &[f(3.4973286186239666e-174), i(4)]), r#"0.0"#, r#"round(3.4973286186239666e-174, 4)"#);
        chk(bad, b("str", &[f(3.4973286186239666e-174)]), r#"'3.4973286186239666e-174'"#, r#"str(3.4973286186239666e-174)"#);
        chk(bad, b("round", &[f(79.2), i(7)]), r#"79.2"#, r#"round(79.2, 7)"#);
        chk(bad, b("str", &[f(79.2)]), r#"'79.2'"#, r#"str(79.2)"#);
        chk(bad, b("round", &[f(-525.6573809579099), i(2)]), r#"-525.66"#, r#"round(-525.6573809579099, 2)"#);
        chk(bad, b("str", &[f(-525.6573809579099)]), r#"'-525.6573809579099'"#, r#"str(-525.6573809579099)"#);
        chk(bad, b("round", &[f(3.1104775720387565e+17), i(1)]), r#"3.1104775720387565e+17"#, r#"round(3.1104775720387565e+17, 1)"#);
        chk(bad, b("str", &[f(3.1104775720387565e+17)]), r#"'3.1104775720387565e+17'"#, r#"str(3.1104775720387565e+17)"#);
        chk(bad, b("round", &[f(60023.125), i(5)]), r#"60023.125"#, r#"round(60023.125, 5)"#);
        chk(bad, b("str", &[f(60023.125)]), r#"'60023.125'"#, r#"str(60023.125)"#);
        chk(bad, b("round", &[f(-94351.0), i(-1)]), r#"-94350.0"#, r#"round(-94351.0, -1)"#);
        chk(bad, b("str", &[f(-94351.0)]), r#"'-94351.0'"#, r#"str(-94351.0)"#);
        chk(bad, b("round", &[f(7.904672270178675e-252), i(-2)]), r#"0.0"#, r#"round(7.904672270178675e-252, -2)"#);
        chk(bad, b("str", &[f(7.904672270178675e-252)]), r#"'7.904672270178675e-252'"#, r#"str(7.904672270178675e-252)"#);
        chk(bad, b("round", &[f(746.7374529589656), i(2)]), r#"746.74"#, r#"round(746.7374529589656, 2)"#);
        chk(bad, b("str", &[f(746.7374529589656)]), r#"'746.7374529589656'"#, r#"str(746.7374529589656)"#);
        chk(bad, b("round", &[f(-28.751099170457362), i(1)]), r#"-28.8"#, r#"round(-28.751099170457362, 1)"#);
        chk(bad, b("str", &[f(-28.751099170457362)]), r#"'-28.751099170457362'"#, r#"str(-28.751099170457362)"#);
        chk(bad, b("round", &[f(882.7731239181467), i(5)]), r#"882.77312"#, r#"round(882.7731239181467, 5)"#);
        chk(bad, b("str", &[f(882.7731239181467)]), r#"'882.7731239181467'"#, r#"str(882.7731239181467)"#);
        chk(bad, b("round", &[f(2164003555662124.5), i(0)]), r#"2164003555662124.0"#, r#"round(2164003555662124.5, 0)"#);
        chk(bad, b("str", &[f(2164003555662124.5)]), r#"'2164003555662124.5'"#, r#"str(2164003555662124.5)"#);
        chk(bad, b("round", &[f(1.5355238055153e-57), i(7)]), r#"0.0"#, r#"round(1.5355238055153e-57, 7)"#);
        chk(bad, b("str", &[f(1.5355238055153e-57)]), r#"'1.5355238055153e-57'"#, r#"str(1.5355238055153e-57)"#);
        chk(bad, b("round", &[f(-180.38536154326664), i(4)]), r#"-180.3854"#, r#"round(-180.38536154326664, 4)"#);
        chk(bad, b("str", &[f(-180.38536154326664)]), r#"'-180.38536154326664'"#, r#"str(-180.38536154326664)"#);
        chk(bad, b("round", &[f(-85.53), i(-3)]), r#"-0.0"#, r#"round(-85.53, -3)"#);
        chk(bad, b("str", &[f(-85.53)]), r#"'-85.53'"#, r#"str(-85.53)"#);
        chk(bad, b("round", &[f(687.2042591647084), i(0)]), r#"687.0"#, r#"round(687.2042591647084, 0)"#);
        chk(bad, b("str", &[f(687.2042591647084)]), r#"'687.2042591647084'"#, r#"str(687.2042591647084)"#);
        chk(bad, b("round", &[f(-12324.25), i(8)]), r#"-12324.25"#, r#"round(-12324.25, 8)"#);
        chk(bad, b("str", &[f(-12324.25)]), r#"'-12324.25'"#, r#"str(-12324.25)"#);
        chk(bad, b("round", &[f(386.08972682969033), i(1)]), r#"386.1"#, r#"round(386.08972682969033, 1)"#);
        chk(bad, b("str", &[f(386.08972682969033)]), r#"'386.08972682969033'"#, r#"str(386.08972682969033)"#);
        chk(bad, b("round", &[f(-76.46), i(1)]), r#"-76.5"#, r#"round(-76.46, 1)"#);
        chk(bad, b("str", &[f(-76.46)]), r#"'-76.46'"#, r#"str(-76.46)"#);
        chk(bad, b("round", &[f(233.13457734265398), i(0)]), r#"233.0"#, r#"round(233.13457734265398, 0)"#);
        chk(bad, b("str", &[f(233.13457734265398)]), r#"'233.13457734265398'"#, r#"str(233.13457734265398)"#);
        chk(bad, b("round", &[f(153242102090.3791), i(5)]), r#"153242102090.3791"#, r#"round(153242102090.3791, 5)"#);
        chk(bad, b("str", &[f(153242102090.3791)]), r#"'153242102090.3791'"#, r#"str(153242102090.3791)"#);
        chk(bad, b("round", &[f(-30.89), i(-2)]), r#"-0.0"#, r#"round(-30.89, -2)"#);
        chk(bad, b("str", &[f(-30.89)]), r#"'-30.89'"#, r#"str(-30.89)"#);
        chk(bad, b("round", &[f(-99.4), i(0)]), r#"-99.0"#, r#"round(-99.4, 0)"#);
        chk(bad, b("str", &[f(-99.4)]), r#"'-99.4'"#, r#"str(-99.4)"#);
        chk(bad, b("round", &[f(-79480.5), i(3)]), r#"-79480.5"#, r#"round(-79480.5, 3)"#);
        chk(bad, b("str", &[f(-79480.5)]), r#"'-79480.5'"#, r#"str(-79480.5)"#);
        chk(bad, b("round", &[f(370.8653383861631), i(5)]), r#"370.86534"#, r#"round(370.8653383861631, 5)"#);
        chk(bad, b("str", &[f(370.8653383861631)]), r#"'370.8653383861631'"#, r#"str(370.8653383861631)"#);
        chk(bad, b("round", &[f(79.8), i(6)]), r#"79.8"#, r#"round(79.8, 6)"#);
        chk(bad, b("str", &[f(79.8)]), r#"'79.8'"#, r#"str(79.8)"#);
        chk(bad, b("round", &[f(0.00028140586192416754), i(5)]), r#"0.00028"#, r#"round(0.00028140586192416754, 5)"#);
        chk(bad, b("str", &[f(0.00028140586192416754)]), r#"'0.00028140586192416754'"#, r#"str(0.00028140586192416754)"#);
        chk(bad, b("round", &[f(26.3923), i(-1)]), r#"30.0"#, r#"round(26.3923, -1)"#);
        chk(bad, b("str", &[f(26.3923)]), r#"'26.3923'"#, r#"str(26.3923)"#);
        chk(bad, b("round", &[f(-0.7156773614229539), i(6)]), r#"-0.715677"#, r#"round(-0.7156773614229539, 6)"#);
        chk(bad, b("str", &[f(-0.7156773614229539)]), r#"'-0.7156773614229539'"#, r#"str(-0.7156773614229539)"#);
        chk(bad, b("round", &[f(-0.002522121669177873), i(0)]), r#"-0.0"#, r#"round(-0.002522121669177873, 0)"#);
        chk(bad, b("str", &[f(-0.002522121669177873)]), r#"'-0.002522121669177873'"#, r#"str(-0.002522121669177873)"#);
        chk(bad, b("round", &[f(-902.1882605340754), i(-2)]), r#"-900.0"#, r#"round(-902.1882605340754, -2)"#);
        chk(bad, b("str", &[f(-902.1882605340754)]), r#"'-902.1882605340754'"#, r#"str(-902.1882605340754)"#);
        chk(bad, b("round", &[f(9.041699618060795e-157), i(3)]), r#"0.0"#, r#"round(9.041699618060795e-157, 3)"#);
        chk(bad, b("str", &[f(9.041699618060795e-157)]), r#"'9.041699618060795e-157'"#, r#"str(9.041699618060795e-157)"#);
        chk(bad, b("round", &[f(-68.616), i(1)]), r#"-68.6"#, r#"round(-68.616, 1)"#);
        chk(bad, b("str", &[f(-68.616)]), r#"'-68.616'"#, r#"str(-68.616)"#);
        chk(bad, b("round", &[f(580.6417306300998), i(8)]), r#"580.64173063"#, r#"round(580.6417306300998, 8)"#);
        chk(bad, b("str", &[f(580.6417306300998)]), r#"'580.6417306300998'"#, r#"str(580.6417306300998)"#);
        chk(bad, b("round", &[f(70.006), i(-2)]), r#"100.0"#, r#"round(70.006, -2)"#);
        chk(bad, b("str", &[f(70.006)]), r#"'70.006'"#, r#"str(70.006)"#);
        chk(bad, b("round", &[f(-2.401881419966492e-10), i(4)]), r#"-0.0"#, r#"round(-2.401881419966492e-10, 4)"#);
        chk(bad, b("str", &[f(-2.401881419966492e-10)]), r#"'-2.401881419966492e-10'"#, r#"str(-2.401881419966492e-10)"#);
        chk(bad, b("round", &[f(9.35116869256047e-160), i(-3)]), r#"0.0"#, r#"round(9.35116869256047e-160, -3)"#);
        chk(bad, b("str", &[f(9.35116869256047e-160)]), r#"'9.35116869256047e-160'"#, r#"str(9.35116869256047e-160)"#);
        chk(bad, b("round", &[f(-56.889), i(8)]), r#"-56.889"#, r#"round(-56.889, 8)"#);
        chk(bad, b("str", &[f(-56.889)]), r#"'-56.889'"#, r#"str(-56.889)"#);
        chk(bad, b("round", &[f(-370.81242083589586), i(5)]), r#"-370.81242"#, r#"round(-370.81242083589586, 5)"#);
        chk(bad, b("str", &[f(-370.81242083589586)]), r#"'-370.81242083589586'"#, r#"str(-370.81242083589586)"#);
        chk(bad, b("round", &[f(1.5473658106876214e-07), i(7)]), r#"2e-07"#, r#"round(1.5473658106876214e-07, 7)"#);
        chk(bad, b("str", &[f(1.5473658106876214e-07)]), r#"'1.5473658106876214e-07'"#, r#"str(1.5473658106876214e-07)"#);
        chk(bad, b("round", &[f(49880.0), i(4)]), r#"49880.0"#, r#"round(49880.0, 4)"#);
        chk(bad, b("str", &[f(49880.0)]), r#"'49880.0'"#, r#"str(49880.0)"#);
        chk(bad, b("round", &[f(387150309.47971183), i(-2)]), r#"387150300.0"#, r#"round(387150309.47971183, -2)"#);
        chk(bad, b("str", &[f(387150309.47971183)]), r#"'387150309.47971183'"#, r#"str(387150309.47971183)"#);
        chk(bad, b("round", &[f(-591.0024331719277), i(-2)]), r#"-600.0"#, r#"round(-591.0024331719277, -2)"#);
        chk(bad, b("str", &[f(-591.0024331719277)]), r#"'-591.0024331719277'"#, r#"str(-591.0024331719277)"#);
        chk(bad, b("round", &[f(-87501547877793.55), i(-3)]), r#"-87501547878000.0"#, r#"round(-87501547877793.55, -3)"#);
        chk(bad, b("str", &[f(-87501547877793.55)]), r#"'-87501547877793.55'"#, r#"str(-87501547877793.55)"#);
        chk(bad, b("round", &[f(-57389.375), i(8)]), r#"-57389.375"#, r#"round(-57389.375, 8)"#);
        chk(bad, b("str", &[f(-57389.375)]), r#"'-57389.375'"#, r#"str(-57389.375)"#);
        chk(bad, b("round", &[f(336.0085200409817), i(-1)]), r#"340.0"#, r#"round(336.0085200409817, -1)"#);
        chk(bad, b("str", &[f(336.0085200409817)]), r#"'336.0085200409817'"#, r#"str(336.0085200409817)"#);
        chk(bad, b("round", &[f(8.880971955959888e-98), i(-3)]), r#"0.0"#, r#"round(8.880971955959888e-98, -3)"#);
        chk(bad, b("str", &[f(8.880971955959888e-98)]), r#"'8.880971955959888e-98'"#, r#"str(8.880971955959888e-98)"#);
        chk(bad, b("round", &[f(-11.860120077219381), i(7)]), r#"-11.8601201"#, r#"round(-11.860120077219381, 7)"#);
        chk(bad, b("str", &[f(-11.860120077219381)]), r#"'-11.860120077219381'"#, r#"str(-11.860120077219381)"#);
        chk(bad, b("round", &[f(708.146469303372), i(-1)]), r#"710.0"#, r#"round(708.146469303372, -1)"#);
        chk(bad, b("str", &[f(708.146469303372)]), r#"'708.146469303372'"#, r#"str(708.146469303372)"#);
        chk(bad, b("round", &[f(0.00010347247563298462), i(3)]), r#"0.0"#, r#"round(0.00010347247563298462, 3)"#);
        chk(bad, b("str", &[f(0.00010347247563298462)]), r#"'0.00010347247563298462'"#, r#"str(0.00010347247563298462)"#);
        chk(bad, b("round", &[f(-943.9631445133032), i(-3)]), r#"-1000.0"#, r#"round(-943.9631445133032, -3)"#);
        chk(bad, b("str", &[f(-943.9631445133032)]), r#"'-943.9631445133032'"#, r#"str(-943.9631445133032)"#);
        chk(bad, b("round", &[f(-843.5479071360717), i(0)]), r#"-844.0"#, r#"round(-843.5479071360717, 0)"#);
        chk(bad, b("str", &[f(-843.5479071360717)]), r#"'-843.5479071360717'"#, r#"str(-843.5479071360717)"#);
        chk(bad, b("round", &[f(-1.8469066078926136e-14), i(6)]), r#"-0.0"#, r#"round(-1.8469066078926136e-14, 6)"#);
        chk(bad, b("str", &[f(-1.8469066078926136e-14)]), r#"'-1.8469066078926136e-14'"#, r#"str(-1.8469066078926136e-14)"#);
        chk(bad, pf(r#"% .3f"#, &[f(-27968.875)]), r#"'-27968.875'"#, r#"'% .3f' % (-27968.875,)"#);
        chk(bad, pf(r#"%.2f"#, &[f(69.2)]), r#"'69.20'"#, r#"'%.2f' % (69.2,)"#);
        chk(bad, pf(r#"%+.1f"#, &[f(50.0)]), r#"'+50.0'"#, r#"'%+.1f' % (50.0,)"#);
        chk(bad, pf(r#"%.0e"#, &[f(2.3467708333416715e-38)]), r#"'2e-38'"#, r#"'%.0e' % (2.3467708333416715e-38,)"#);
        chk(bad, pf(r#"%+.1f"#, &[f(-15.8)]), r#"'-15.8'"#, r#"'%+.1f' % (-15.8,)"#);
        chk(bad, pf(r#"%.0e"#, &[f(-47.1)]), r#"'-5e+01'"#, r#"'%.0e' % (-47.1,)"#);
        chk(bad, pf(r#"%#.3g"#, &[f(19.9495)]), r#"'19.9'"#, r#"'%#.3g' % (19.9495,)"#);
        chk(bad, pf(r#"%10.3e"#, &[f(716029785734940.4)]), r#"' 7.160e+14'"#, r#"'%10.3e' % (716029785734940.4,)"#);
        chk(bad, pf(r#"%#.3g"#, &[f(5.479790247183496e-44)]), r#"'5.48e-44'"#, r#"'%#.3g' % (5.479790247183496e-44,)"#);
        chk(bad, pf(r#"%.2f"#, &[f(-638.5728565674654)]), r#"'-638.57'"#, r#"'%.2f' % (-638.5728565674654,)"#);
        chk(bad, pf(r#"%.2f"#, &[f(-740.9774470283139)]), r#"'-740.98'"#, r#"'%.2f' % (-740.9774470283139,)"#);
        chk(bad, pf(r#"%.2f"#, &[f(951.1323290840833)]), r#"'951.13'"#, r#"'%.2f' % (951.1323290840833,)"#);
        chk(bad, pf(r#"%10.3e"#, &[f(4.8558212782014485e-241)]), r#"'4.856e-241'"#, r#"'%10.3e' % (4.8558212782014485e-241,)"#);
        chk(bad, pf(r#"%.0e"#, &[f(-111839.375)]), r#"'-1e+05'"#, r#"'%.0e' % (-111839.375,)"#);
        chk(bad, pf(r#"%.2f"#, &[f(674.093212885831)]), r#"'674.09'"#, r#"'%.2f' % (674.093212885831,)"#);
        chk(bad, pf(r#"%08.2f"#, &[f(612.8021997496276)]), r#"'00612.80'"#, r#"'%08.2f' % (612.8021997496276,)"#);
        chk(bad, pf(r#"%#.3g"#, &[f(1.0352596840693327e-197)]), r#"'1.04e-197'"#, r#"'%#.3g' % (1.0352596840693327e-197,)"#);
        chk(bad, pf(r#"%.2f"#, &[f(-0.020712116786132002)]), r#"'-0.02'"#, r#"'%.2f' % (-0.020712116786132002,)"#);
        chk(bad, pf(r#"%08.2f"#, &[f(-758.2263275900675)]), r#"'-0758.23'"#, r#"'%08.2f' % (-758.2263275900675,)"#);
        chk(bad, pf(r#"%.2f"#, &[f(-80.5)]), r#"'-80.50'"#, r#"'%.2f' % (-80.5,)"#);
        chk(bad, pf(r#"%.0e"#, &[f(-75219.5)]), r#"'-8e+04'"#, r#"'%.0e' % (-75219.5,)"#);
        chk(bad, pf(r#"%+.1f"#, &[f(-1.0947016043334973e-10)]), r#"'-0.0'"#, r#"'%+.1f' % (-1.0947016043334973e-10,)"#);
        chk(bad, pf(r#"%-12.4g"#, &[f(-602.7563273167334)]), r#"'-602.8      '"#, r#"'%-12.4g' % (-602.7563273167334,)"#);
        chk(bad, pf(r#"%f"#, &[f(431.0807119580179)]), r#"'431.080712'"#, r#"'%f' % (431.0807119580179,)"#);
        chk(bad, pf(r#"%08.2f"#, &[f(61.0)]), r#"'00061.00'"#, r#"'%08.2f' % (61.0,)"#);
        chk(bad, pf(r#"%#.3g"#, &[f(-3573.5)]), r#"'-3.57e+03'"#, r#"'%#.3g' % (-3573.5,)"#);
        chk(bad, pf(r#"%#.3g"#, &[f(-697.4258094569088)]), r#"'-697.'"#, r#"'%#.3g' % (-697.4258094569088,)"#);
        chk(bad, pf(r#"%G"#, &[f(102687.75)]), r#"'102688'"#, r#"'%G' % (102687.75,)"#);
        chk(bad, pf(r#"%08.2f"#, &[f(32.5)]), r#"'00032.50'"#, r#"'%08.2f' % (32.5,)"#);
        chk(bad, pf(r#"%.0e"#, &[f(-19.3)]), r#"'-2e+01'"#, r#"'%.0e' % (-19.3,)"#);
        chk(bad, pf(r#"%#.3g"#, &[f(1.3861928874757786e-73)]), r#"'1.39e-73'"#, r#"'%#.3g' % (1.3861928874757786e-73,)"#);
        chk(bad, pf(r#"%08.2f"#, &[f(-93.81)]), r#"'-0093.81'"#, r#"'%08.2f' % (-93.81,)"#);
        chk(bad, pf(r#"%#.3g"#, &[f(-27.0)]), r#"'-27.0'"#, r#"'%#.3g' % (-27.0,)"#);
        chk(bad, pf(r#"%G"#, &[f(916.6703498619922)]), r#"'916.67'"#, r#"'%G' % (916.6703498619922,)"#);
        chk(bad, pf(r#"%+.1f"#, &[f(77.95301512371157)]), r#"'+78.0'"#, r#"'%+.1f' % (77.95301512371157,)"#);
        chk(bad, pf(r#"% .3f"#, &[f(883.0328839820731)]), r#"' 883.033'"#, r#"'% .3f' % (883.0328839820731,)"#);
        chk(bad, pf(r#"%+.1f"#, &[f(6.120028787626546e-156)]), r#"'+0.0'"#, r#"'%+.1f' % (6.120028787626546e-156,)"#);
        chk(bad, pf(r#"%10.3e"#, &[f(-11781.0)]), r#"'-1.178e+04'"#, r#"'%10.3e' % (-11781.0,)"#);
        chk(bad, pf(r#"%08.2f"#, &[f(46118.75)]), r#"'46118.75'"#, r#"'%08.2f' % (46118.75,)"#);
        chk(bad, pf(r#"%G"#, &[f(614.7253824406573)]), r#"'614.725'"#, r#"'%G' % (614.7253824406573,)"#);
        chk(bad, pf(r#"%+.1f"#, &[f(-4.786377315875874e-10)]), r#"'-0.0'"#, r#"'%+.1f' % (-4.786377315875874e-10,)"#);
        chk(bad, pf(r#"% .3f"#, &[f(-80.67722358649388)]), r#"'-80.677'"#, r#"'% .3f' % (-80.67722358649388,)"#);
        chk(bad, pf(r#"%.0e"#, &[f(-765.174671684859)]), r#"'-8e+02'"#, r#"'%.0e' % (-765.174671684859,)"#);
        chk(bad, pf(r#"%+.1f"#, &[f(6747.5)]), r#"'+6747.5'"#, r#"'%+.1f' % (6747.5,)"#);
        chk(bad, pf(r#"%.2f"#, &[f(31.66)]), r#"'31.66'"#, r#"'%.2f' % (31.66,)"#);
        chk(bad, pf(r#"%10.3e"#, &[f(1.1394079175060883e-19)]), r#"' 1.139e-19'"#, r#"'%10.3e' % (1.1394079175060883e-19,)"#);
        chk(bad, pf(r#"%-12.4g"#, &[f(1.7490751568179906e-11)]), r#"'1.749e-11   '"#, r#"'%-12.4g' % (1.7490751568179906e-11,)"#);
        chk(bad, pf(r#"%08.2f"#, &[f(-933.233833229891)]), r#"'-0933.23'"#, r#"'%08.2f' % (-933.233833229891,)"#);
        chk(bad, pf(r#"%.0e"#, &[f(9748797579.47626)]), r#"'1e+10'"#, r#"'%.0e' % (9748797579.47626,)"#);
        chk(bad, pf(r#"%10.3e"#, &[f(-11.94435171140924)]), r#"'-1.194e+01'"#, r#"'%10.3e' % (-11.94435171140924,)"#);
        chk(bad, pf(r#"%.0e"#, &[f(-76.3)]), r#"'-8e+01'"#, r#"'%.0e' % (-76.3,)"#);
        chk(bad, pf(r#"%10.3e"#, &[f(100337.875)]), r#"' 1.003e+05'"#, r#"'%10.3e' % (100337.875,)"#);
        chk(bad, pf(r#"% .3f"#, &[f(-99.5)]), r#"'-99.500'"#, r#"'% .3f' % (-99.5,)"#);
        chk(bad, pf(r#"%08.2f"#, &[f(2.0395498548439752e-255)]), r#"'00000.00'"#, r#"'%08.2f' % (2.0395498548439752e-255,)"#);
        chk(bad, pf(r#"%G"#, &[f(9.587286309823526e-175)]), r#"'9.58729E-175'"#, r#"'%G' % (9.587286309823526e-175,)"#);
        chk(bad, pf(r#"%10.3e"#, &[f(738.0050998488018)]), r#"' 7.380e+02'"#, r#"'%10.3e' % (738.0050998488018,)"#);
        chk(bad, pf(r#"%10.3e"#, &[f(3.590962871515264e+21)]), r#"' 3.591e+21'"#, r#"'%10.3e' % (3.590962871515264e+21,)"#);
        chk(bad, pf(r#"%#.3g"#, &[f(79525.5)]), r#"'7.95e+04'"#, r#"'%#.3g' % (79525.5,)"#);
        chk(bad, pf(r#"%.0e"#, &[f(1.503391270696581e-08)]), r#"'2e-08'"#, r#"'%.0e' % (1.503391270696581e-08,)"#);
        chk(bad, pf(r#"%+.1f"#, &[f(6.101801682006051e-121)]), r#"'+0.0'"#, r#"'%+.1f' % (6.101801682006051e-121,)"#);
        chk(bad, m("log10", &[f(950710.3462896895)]), r#"5.978048220229591"#, r#"math.log10(950710.3462896895)"#);
        chk(bad, m("degrees", &[f(132134615717786.81)]), r#"7570755808212175.0"#, r#"math.degrees(132134615717786.81)"#);
        chk(bad, m("exp", &[f(462538.2846139814)]), r#"<err>"#, r#"math.exp(462538.2846139814)"#);
        chk(bad, m("log2", &[f(-10.372415745860323)]), r#"<err>"#, r#"math.log2(-10.372415745860323)"#);
        chk(bad, m("tan", &[f(24.986617439269935)]), r#"-0.1471727691544614"#, r#"math.tan(24.986617439269935)"#);
        chk(bad, m("tan", &[f(2.1010388274159545e+23)]), r#"-0.23176362320247504"#, r#"math.tan(2.1010388274159545e+23)"#);
        chk(bad, m("log10", &[f(0.012355404298110939)]), r#"-1.9081430388846599"#, r#"math.log10(0.012355404298110939)"#);
        chk(bad, m("sqrt", &[f(8.635000071917823e-05)]), r#"0.00929247010859751"#, r#"math.sqrt(8.635000071917823e-05)"#);
        chk(bad, m("log10", &[f(1.1785570540411094e-05)]), r#"-4.928649388400196"#, r#"math.log10(1.1785570540411094e-05)"#);
        chk(bad, m("cos", &[f(439442.9771438896)]), r#"-0.9904445629321372"#, r#"math.cos(439442.9771438896)"#);
        chk(bad, m("log", &[f(809111.3510276107)]), r#"13.60369182689986"#, r#"math.log(809111.3510276107)"#);
        chk(bad, m("exp", &[f(-8.291107343525773)]), r#"0.0002507366573448118"#, r#"math.exp(-8.291107343525773)"#);
        chk(bad, m("sin", &[f(73977.76212498819)]), r#"-0.4454544095975989"#, r#"math.sin(73977.76212498819)"#);
        chk(bad, m("sin", &[f(5.410478908600622e+17)]), r#"0.971145363793876"#, r#"math.sin(5.410478908600622e+17)"#);
        chk(bad, m("radians", &[f(795945.3989227719)]), r#"13891.867877302097"#, r#"math.radians(795945.3989227719)"#);
        chk(bad, m("log", &[f(18.99019026806083)]), r#"2.9439225441558245"#, r#"math.log(18.99019026806083)"#);
        chk(bad, m("log", &[f(390764.5168936456)]), r#"12.875860398952925"#, r#"math.log(390764.5168936456)"#);
        chk(bad, m("exp", &[f(-36.14328387955388)]), r#"2.009885204700762e-16"#, r#"math.exp(-36.14328387955388)"#);
        chk(bad, m("cos", &[f(-42.809422692512555)]), r#"0.3875034174519749"#, r#"math.cos(-42.809422692512555)"#);
        chk(bad, m("radians", &[f(9.30665239176558e-21)]), r#"1.6243172657491457e-22"#, r#"math.radians(9.30665239176558e-21)"#);
        chk(bad, m("sin", &[f(73557.49051730843)]), r#"0.23782516562722492"#, r#"math.sin(73557.49051730843)"#);
        chk(bad, m("cos", &[f(-6.510144559205031)]), r#"0.9743551148015903"#, r#"math.cos(-6.510144559205031)"#);
        chk(bad, m("radians", &[f(2.1169051907114664e-29)]), r#"3.6946965530473573e-31"#, r#"math.radians(2.1169051907114664e-29)"#);
        chk(bad, m("cos", &[f(813260.5103152246)]), r#"-0.9054630309892385"#, r#"math.cos(813260.5103152246)"#);
        chk(bad, m("log", &[f(-11.509773164751692)]), r#"<err>"#, r#"math.log(-11.509773164751692)"#);
        chk(bad, m("sqrt", &[f(543082.0822995272)]), r#"736.9410304084902"#, r#"math.sqrt(543082.0822995272)"#);
        chk(bad, m("sin", &[f(41566.853150757364)]), r#"-0.42661891360613385"#, r#"math.sin(41566.853150757364)"#);
        chk(bad, m("radians", &[f(-26.275684041987425)]), r#"-0.45859719974641255"#, r#"math.radians(-26.275684041987425)"#);
        chk(bad, m("radians", &[f(15.114265780965141)]), r#"0.2637937018993538"#, r#"math.radians(15.114265780965141)"#);
        chk(bad, m("exp", &[f(1.3980055823974127e-11)]), r#"1.0000000000139802"#, r#"math.exp(1.3980055823974127e-11)"#);
        chk(bad, m("log", &[f(-3.2781827409118947)]), r#"<err>"#, r#"math.log(-3.2781827409118947)"#);
        chk(bad, m("sin", &[f(46.65050138783083)]), r#"0.45590468399188316"#, r#"math.sin(46.65050138783083)"#);
        chk(bad, m("cos", &[f(9.266020971319435)]), r#"-0.9874245548661564"#, r#"math.cos(9.266020971319435)"#);
        chk(bad, m("tan", &[f(1.1263069343552036e+21)]), r#"3.6932533045411455"#, r#"math.tan(1.1263069343552036e+21)"#);
        chk(bad, m("sqrt", &[f(67248755927.35182)]), r#"259323.65092168478"#, r#"math.sqrt(67248755927.35182)"#);
        chk(bad, m("cos", &[f(529077.5624890843)]), r#"-0.3643192404648809"#, r#"math.cos(529077.5624890843)"#);
        chk(bad, m("log2", &[f(38.73660122305776)]), r#"5.275625471292463"#, r#"math.log2(38.73660122305776)"#);
        chk(bad, m("degrees", &[f(901898.7694435756)]), r#"51674993.037159376"#, r#"math.degrees(901898.7694435756)"#);
        chk(bad, m("sqrt", &[f(1.688012304271382)]), r#"1.2992352767191098"#, r#"math.sqrt(1.688012304271382)"#);
        chk(bad, m("tan", &[f(500822.69188423065)]), r#"-0.6611492111690682"#, r#"math.tan(500822.69188423065)"#);
        chk(bad, m("degrees", &[f(490947.6642213028)]), r#"28129229.12168654"#, r#"math.degrees(490947.6642213028)"#);
        chk(bad, m("degrees", &[f(3.791439335275707e-22)]), r#"2.1723347219118433e-20"#, r#"math.degrees(3.791439335275707e-22)"#);
        chk(bad, m("sqrt", &[f(2.162219600054453e+18)]), r#"1470448775.0528588"#, r#"math.sqrt(2.162219600054453e+18)"#);
        chk(bad, m("log", &[f(440981.5466908493)]), r#"12.996758309315052"#, r#"math.log(440981.5466908493)"#);
        chk(bad, m("tan", &[f(3.604504427111462e+21)]), r#"21.620931821184673"#, r#"math.tan(3.604504427111462e+21)"#);
        chk(bad, m("degrees", &[f(212730.1213440904)]), r#"12188538.128322251"#, r#"math.degrees(212730.1213440904)"#);
        chk(bad, m("log2", &[f(8.407204827121262)]), r#"3.071626222318749"#, r#"math.log2(8.407204827121262)"#);
        chk(bad, m("sin", &[f(491413.0934015773)]), r#"-0.8968775390596019"#, r#"math.sin(491413.0934015773)"#);
        chk(bad, m("tan", &[f(-25.742163709028766)]), r#"-0.6980595781399342"#, r#"math.tan(-25.742163709028766)"#);
        chk(bad, m("cos", &[f(197193.00403228367)]), r#"0.0544176737209756"#, r#"math.cos(197193.00403228367)"#);
        chk(bad, m("tan", &[f(-48.204640022535905)]), r#"-1.8745991611738038"#, r#"math.tan(-48.204640022535905)"#);
        chk(bad, m("log10", &[f(33.7536183229664)]), r#"1.528320335195742"#, r#"math.log10(33.7536183229664)"#);
        chk(bad, m("exp", &[f(11.228702034575292)]), r#"75259.84725455604"#, r#"math.exp(11.228702034575292)"#);
        chk(bad, m("sqrt", &[f(3.4230951796817635e-18)]), r#"1.8501608523806149e-09"#, r#"math.sqrt(3.4230951796817635e-18)"#);
        chk(bad, m("tan", &[f(1.642233831154151e+26)]), r#"0.5299586024685952"#, r#"math.tan(1.642233831154151e+26)"#);
        chk(bad, m("degrees", &[f(668223.692151429)]), r#"38286397.330926076"#, r#"math.degrees(668223.692151429)"#);
        chk(bad, m("radians", &[f(7961272.204639479)]), r#"138950.41261846668"#, r#"math.radians(7961272.204639479)"#);
        chk(bad, m("radians", &[f(6.356790216900705e-29)]), r#"1.1094691914348179e-30"#, r#"math.radians(6.356790216900705e-29)"#);
        chk(bad, m("sqrt", &[f(5.387111013193282e-07)]), r#"0.0007339694144304163"#, r#"math.sqrt(5.387111013193282e-07)"#);
        chk(bad, m("sqrt", &[f(5.7572645952774826e-15)]), r#"7.587664064306935e-08"#, r#"math.sqrt(5.7572645952774826e-15)"#);
        chk(bad, m("log", &[f(4.000799922339283)]), r#"1.3864943217112597"#, r#"math.log(4.000799922339283)"#);
        chk(bad, m("cos", &[f(1.6946245063909904)]), r#"-0.12351197058079179"#, r#"math.cos(1.6946245063909904)"#);
        chk(bad, m("log2", &[f(12.201896641195674)]), r#"3.609033510066956"#, r#"math.log2(12.201896641195674)"#);
        chk(bad, m("exp", &[f(3.1056180167984024)]), r#"22.323010708684276"#, r#"math.exp(3.1056180167984024)"#);
        chk(bad, m("sqrt", &[f(806403.8735149489)]), r#"897.9999295740222"#, r#"math.sqrt(806403.8735149489)"#);
        chk(bad, m("log2", &[f(1.7267472187052746e-11)]), r#"-35.753152143877664"#, r#"math.log2(1.7267472187052746e-11)"#);
        chk(bad, m("radians", &[f(44357.997969982964)]), r#"774.1931141691636"#, r#"math.radians(44357.997969982964)"#);
        chk(bad, m("radians", &[f(441499.3988405431)]), r#"7705.618155343112"#, r#"math.radians(441499.3988405431)"#);
        chk(bad, m("exp", &[f(5.943612324010625e-08)]), r#"1.000000059436125"#, r#"math.exp(5.943612324010625e-08)"#);
        chk(bad, m("log10", &[f(243164.30955243096)]), r#"5.385899831694863"#, r#"math.log10(243164.30955243096)"#);
        chk(bad, m("exp", &[f(29057312419453.27)]), r#"<err>"#, r#"math.exp(29057312419453.27)"#);
        chk(bad, m("log", &[f(553811.1552496474)]), r#"13.224579032617696"#, r#"math.log(553811.1552496474)"#);
        chk(bad, m("cos", &[f(1.2450110895058004e+29)]), r#"-0.9406939765268384"#, r#"math.cos(1.2450110895058004e+29)"#);
        chk(bad, m("sqrt", &[f(1.8154369376188388e+21)]), r#"42607944536.422295"#, r#"math.sqrt(1.8154369376188388e+21)"#);
        chk(bad, m("log10", &[f(299170.017913319)]), r#"5.475918067443597"#, r#"math.log10(299170.017913319)"#);
        chk(bad, m("log10", &[f(40.93809378825071)]), r#"1.6121276166235647"#, r#"math.log10(40.93809378825071)"#);
        chk(bad, m("log", &[f(740622.3184439443)]), r#"13.515246082632673"#, r#"math.log(740622.3184439443)"#);
        chk(bad, m("cos", &[f(377881197504.30334)]), r#"-0.2769426951662534"#, r#"math.cos(377881197504.30334)"#);
        chk(bad, m("exp", &[f(-13.89183855746797)]), r#"9.265122611533092e-07"#, r#"math.exp(-13.89183855746797)"#);
        chk(bad, m("exp", &[f(12.918739885861285)]), r#"407884.72873651923"#, r#"math.exp(12.918739885861285)"#);
        chk(bad, m("sin", &[f(-32.06461244165527)]), r#"-0.6041397543696894"#, r#"math.sin(-32.06461244165527)"#);
        chk(bad, m("degrees", &[f(8.556384385167725)]), r#"490.24471316175044"#, r#"math.degrees(8.556384385167725)"#);
        chk(bad, m("log2", &[f(4926648756244.273)]), r#"42.16374375751015"#, r#"math.log2(4926648756244.273)"#);
        chk(bad, m("exp", &[f(-0.2976952222196516)]), r#"0.7425276111819535"#, r#"math.exp(-0.2976952222196516)"#);
        chk(bad, m("tan", &[f(12.25273159224551)]), r#"-0.32434462660314134"#, r#"math.tan(12.25273159224551)"#);
        chk(bad, m("sin", &[f(480750.6323336409)]), r#"-0.8459050710946868"#, r#"math.sin(480750.6323336409)"#);
        chk(bad, m("degrees", &[f(0.001235886166044275)]), r#"0.07081106127294143"#, r#"math.degrees(0.001235886166044275)"#);
        chk(bad, m("sin", &[f(46.2029980377508)]), r#"0.7961415534909986"#, r#"math.sin(46.2029980377508)"#);
        chk(bad, m("log", &[f(-5.806578677475805)]), r#"<err>"#, r#"math.log(-5.806578677475805)"#);
        chk(bad, m("tan", &[f(26.41202647838162)]), r#"3.3326760290157207"#, r#"math.tan(26.41202647838162)"#);
        chk(bad, m("log10", &[f(3.0299029942013945e-11)]), r#"-10.518571275708073"#, r#"math.log10(3.0299029942013945e-11)"#);
        chk(bad, m("log10", &[f(-14.09030054714254)]), r#"<err>"#, r#"math.log10(-14.09030054714254)"#);
        chk(bad, m("sqrt", &[f(-44.8318993963543)]), r#"<err>"#, r#"math.sqrt(-44.8318993963543)"#);
        chk(bad, m("log2", &[f(132283.13770239064)]), r#"17.01326964562574"#, r#"math.log2(132283.13770239064)"#);
        chk(bad, m("log10", &[f(7.348370259287023e+17)]), r#"17.86619103080117"#, r#"math.log10(7.348370259287023e+17)"#);
        chk(bad, m("sqrt", &[f(2.8487965830430135e+29)]), r#"533741190376292.2"#, r#"math.sqrt(2.8487965830430135e+29)"#);
        chk(bad, m("log10", &[f(14.641295521214985)]), r#"1.1655795065599337"#, r#"math.log10(14.641295521214985)"#);
        chk(bad, m("cos", &[f(857332.9253114363)]), r#"0.14362993711925007"#, r#"math.cos(857332.9253114363)"#);
        chk(bad, m("degrees", &[f(124283.64779052648)]), r#"7120928.480887586"#, r#"math.degrees(124283.64779052648)"#);
        chk(bad, m("tan", &[f(3.520087560978067e+17)]), r#"-4.936176533787017"#, r#"math.tan(3.520087560978067e+17)"#);
        chk(bad, m("log2", &[f(8.720950428859285e-28)]), r#"-89.88950128511433"#, r#"math.log2(8.720950428859285e-28)"#);
        chk(bad, m("sin", &[f(9457910.234438313)]), r#"0.861158439770481"#, r#"math.sin(9457910.234438313)"#);
        chk(bad, m("degrees", &[f(-17.61618791600396)]), r#"-1009.333218696388"#, r#"math.degrees(-17.61618791600396)"#);
        chk(bad, m("radians", &[f(0.042741464486780956)]), r#"0.000745979282418556"#, r#"math.radians(0.042741464486780956)"#);
        chk(bad, m("log", &[f(460962.3611772642)]), r#"13.041071672609263"#, r#"math.log(460962.3611772642)"#);
        chk(bad, m("log", &[f(7845473091.114844)]), r#"22.7832025261079"#, r#"math.log(7845473091.114844)"#);
        chk(bad, m("degrees", &[f(791906.5999186204)]), r#"45372905.94389197"#, r#"math.degrees(791906.5999186204)"#);
        chk(bad, m("tan", &[f(39.09802164193607)]), r#"5.760382928645368"#, r#"math.tan(39.09802164193607)"#);
        chk(bad, m("sin", &[f(7.675869724369454e-25)]), r#"7.675869724369454e-25"#, r#"math.sin(7.675869724369454e-25)"#);
        chk(bad, m("cos", &[f(6.115248113102415e+28)]), r#"-0.7950008651226536"#, r#"math.cos(6.115248113102415e+28)"#);
        chk(bad, m("sin", &[f(-6.350257412407267)]), r#"-0.06702182735709865"#, r#"math.sin(-6.350257412407267)"#);
        chk(bad, m("degrees", &[f(1.5637734879017105e-29)]), r#"8.959762097122011e-28"#, r#"math.degrees(1.5637734879017105e-29)"#);
        chk(bad, m("radians", &[f(807377.7429130162)]), r#"14091.399921152448"#, r#"math.radians(807377.7429130162)"#);
        chk(bad, m("cos", &[f(-11.214629699342893)]), r#"0.21730770462468996"#, r#"math.cos(-11.214629699342893)"#);
        chk(bad, m("cos", &[f(194111.6358087537)]), r#"0.4615353004319448"#, r#"math.cos(194111.6358087537)"#);
        chk(bad, m("radians", &[f(2.2664805982609384e+29)]), r#"3.955754887222424e+27"#, r#"math.radians(2.2664805982609384e+29)"#);
        chk(bad, m("log", &[f(13.415955614329874)]), r#"2.5964447161023365"#, r#"math.log(13.415955614329874)"#);
        chk(bad, m("log10", &[f(1.265689452462925e-19)]), r#"-18.89767283904694"#, r#"math.log10(1.265689452462925e-19)"#);
        chk(bad, m("degrees", &[f(-35.054645042627854)]), r#"-2008.4832132717697"#, r#"math.degrees(-35.054645042627854)"#);
        chk(bad, m("tan", &[f(1.3076267220350549e-08)]), r#"1.307626722035055e-08"#, r#"math.tan(1.3076267220350549e-08)"#);
        chk(bad, st("pstdev", &[f(6.84), f(11.21), f(-3.32)]), r#"6.0868108781747665"#, r#"statistics.pstdev([6.84, 11.21, -3.32])"#);
        chk(bad, st("variance", &[i(7), i(-22), i(29), i(-37), i(2)]), r#"664.7"#, r#"statistics.variance([7, -22, 29, -37, 2])"#);
        chk(bad, st("median_low", &[f(5.0), f(15.24), f(1.296)]), r#"5.0"#, r#"statistics.median_low([5.0, 15.24, 1.296])"#);
        chk(bad, st("pvariance", &[i(-47), i(-13), i(-15), i(37)]), r#"902.75"#, r#"statistics.pvariance([-47, -13, -15, 37])"#);
        chk(bad, st("stdev", &[i(7)]), r#"<err>"#, r#"statistics.stdev([7])"#);
        chk(bad, st("mean", &[i(37)]), r#"37"#, r#"statistics.mean([37])"#);
        chk(bad, st("median_high", &[f(18.08), f(3.1), f(14.8), f(-7.077), f(-3.1)]), r#"3.1"#, r#"statistics.median_high([18.08, 3.1, 14.8, -7.077, -3.1])"#);
        chk(bad, st("pstdev", &[f(-12.0), f(-14.64), f(6.886), f(-5.0)]), r#"8.330013010193921"#, r#"statistics.pstdev([-12.0, -14.64, 6.886, -5.0])"#);
        chk(bad, st("median", &[f(0.524), f(-11.59), f(-5.32), f(-15.42), f(8.2), f(16.084), f(-11.9)]), r#"-5.32"#, r#"statistics.median([0.524, -11.59, -5.32, -15.42, 8.2, 16.084, -11.9])"#);
        chk(bad, st("variance", &[i(-6)]), r#"<err>"#, r#"statistics.variance([-6])"#);
        chk(bad, st("pstdev", &[i(4), i(-23), i(24), i(-38), i(10), i(34), i(20)]), r#"24.153421188354265"#, r#"statistics.pstdev([4, -23, 24, -38, 10, 34, 20])"#);
        chk(bad, st("variance", &[f(-20.0), f(-18.0)]), r#"2.0"#, r#"statistics.variance([-20.0, -18.0])"#);
        chk(bad, st("variance", &[f(-5.656), f(-16.11), f(-10.0)]), r#"27.581425333333332"#, r#"statistics.variance([-5.656, -16.11, -10.0])"#);
        chk(bad, st("median", &[i(46), i(44), i(14), i(42), i(41), i(-14), i(11)]), r#"41"#, r#"statistics.median([46, 44, 14, 42, 41, -14, 11])"#);
        chk(bad, st("mode", &[i(-41), i(-35), i(24), i(17), i(-4), i(6)]), r#"-41"#, r#"statistics.mode([-41, -35, 24, 17, -4, 6])"#);
        chk(bad, st("mean", &[f(7.0), f(8.3), f(-11.997)]), r#"1.1010000000000002"#, r#"statistics.mean([7.0, 8.3, -11.997])"#);
        chk(bad, st("fmean", &[f(11.1), f(-12.1), f(-4.4), f(-2.0), f(8.6), f(-2.6), f(-7.4)]), r#"-1.2571428571428573"#, r#"statistics.fmean([11.1, -12.1, -4.4, -2.0, 8.6, -2.6, -7.4])"#);
        chk(bad, st("variance", &[i(8)]), r#"<err>"#, r#"statistics.variance([8])"#);
        chk(bad, st("mean", &[f(-3.0), f(-19.847), f(0.61), f(6.0), f(-6.0), f(13.381), f(18.0)]), r#"1.306285714285714"#, r#"statistics.mean([-3.0, -19.847, 0.61, 6.0, -6.0, 13.381, 18.0])"#);
        chk(bad, st("variance", &[f(6.78), f(-8.445), f(-13.4)]), r#"110.59750833333334"#, r#"statistics.variance([6.78, -8.445, -13.4])"#);
        chk(bad, st("pstdev", &[i(-14), i(31), i(-35), i(17), i(4), i(-14)]), r#"21.84350907910377"#, r#"statistics.pstdev([-14, 31, -35, 17, 4, -14])"#);
        chk(bad, st("stdev", &[i(10), i(20), i(-26), i(-43), i(21), i(33)]), r#"30.058276730378275"#, r#"statistics.stdev([10, 20, -26, -43, 21, 33])"#);
        chk(bad, st("fmean", &[f(-2.533), f(-18.8), f(2.67), f(6.704), f(-5.0)]), r#"-3.3918"#, r#"statistics.fmean([-2.533, -18.8, 2.67, 6.704, -5.0])"#);
        chk(bad, st("median_low", &[f(2.93), f(-14.9), f(18.0), f(-5.801)]), r#"-5.801"#, r#"statistics.median_low([2.93, -14.9, 18.0, -5.801])"#);
        chk(bad, st("variance", &[i(-11), i(-41), i(-24), i(-28), i(23)]), r#"594.7"#, r#"statistics.variance([-11, -41, -24, -28, 23])"#);
        chk(bad, st("median", &[f(18.9)]), r#"18.9"#, r#"statistics.median([18.9])"#);
        chk(bad, st("variance", &[f(-1.0), f(-5.0)]), r#"8.0"#, r#"statistics.variance([-1.0, -5.0])"#);
        chk(bad, st("pstdev", &[f(-16.0), f(-2.136), f(11.1), f(-14.3), f(3.96), f(-1.6), f(10.04)]), r#"9.97065876684675"#, r#"statistics.pstdev([-16.0, -2.136, 11.1, -14.3, 3.96, -1.6, 10.04])"#);
        chk(bad, st("mean", &[i(-4)]), r#"-4"#, r#"statistics.mean([-4])"#);
        chk(bad, st("median_high", &[i(-21), i(19), i(7), i(12), i(50), i(-48)]), r#"12"#, r#"statistics.median_high([-21, 19, 7, 12, 50, -48])"#);
        chk(bad, st("median_low", &[f(18.7), f(-15.0), f(-12.0), f(5.5), f(-12.99)]), r#"-12.0"#, r#"statistics.median_low([18.7, -15.0, -12.0, 5.5, -12.99])"#);
        chk(bad, st("variance", &[f(15.7), f(19.849), f(-0.39), f(-4.164)]), r#"139.26376358333331"#, r#"statistics.variance([15.7, 19.849, -0.39, -4.164])"#);
        chk(bad, st("median_high", &[f(0.535), f(12.0), f(18.6), f(7.0), f(15.0), f(10.9), f(17.412)]), r#"12.0"#, r#"statistics.median_high([0.535, 12.0, 18.6, 7.0, 15.0, 10.9, 17.412])"#);
        chk(bad, st("variance", &[f(12.0), f(-12.57), f(-0.0), f(-15.0), f(-10.46)]), r#"125.22608000000001"#, r#"statistics.variance([12.0, -12.57, -0.0, -15.0, -10.46])"#);
        chk(bad, st("stdev", &[i(-18), i(31), i(-48)]), r#"39.878983604567125"#, r#"statistics.stdev([-18, 31, -48])"#);
        chk(bad, st("mean", &[i(-9)]), r#"-9"#, r#"statistics.mean([-9])"#);
        chk(bad, st("median_high", &[f(4.867), f(9.14)]), r#"9.14"#, r#"statistics.median_high([4.867, 9.14])"#);
        chk(bad, st("pvariance", &[i(36), i(-34)]), r#"1225"#, r#"statistics.pvariance([36, -34])"#);
        chk(bad, st("pstdev", &[i(-50), i(-14)]), r#"18.0"#, r#"statistics.pstdev([-50, -14])"#);
        chk(bad, st("median_low", &[f(19.68)]), r#"19.68"#, r#"statistics.median_low([19.68])"#);
        chk(bad, st("variance", &[i(-7), i(0), i(1)]), r#"19"#, r#"statistics.variance([-7, 0, 1])"#);
        chk(bad, st("median_high", &[f(11.7), f(15.32), f(-16.0), f(3.967), f(-18.2), f(-12.0)]), r#"3.967"#, r#"statistics.median_high([11.7, 15.32, -16.0, 3.967, -18.2, -12.0])"#);
        chk(bad, st("mean", &[f(-5.1), f(-10.6), f(-13.2), f(-19.1), f(12.14)]), r#"-7.172"#, r#"statistics.mean([-5.1, -10.6, -13.2, -19.1, 12.14])"#);
        chk(bad, st("pvariance", &[f(-13.921), f(17.0), f(-3.96), f(18.78), f(-10.3)]), r#"189.14105616"#, r#"statistics.pvariance([-13.921, 17.0, -3.96, 18.78, -10.3])"#);
        chk(bad, st("fmean", &[i(2), i(-8)]), r#"-3.0"#, r#"statistics.fmean([2, -8])"#);
        chk(bad, st("fmean", &[i(-43), i(19), i(8), i(24), i(-22), i(-28)]), r#"-7.0"#, r#"statistics.fmean([-43, 19, 8, 24, -22, -28])"#);
        chk(bad, st("fmean", &[i(43), i(-48)]), r#"-2.5"#, r#"statistics.fmean([43, -48])"#);
        chk(bad, st("median_low", &[f(19.9), f(-6.582)]), r#"-6.582"#, r#"statistics.median_low([19.9, -6.582])"#);
        chk(bad, st("pstdev", &[i(-29), i(47), i(-14), i(50), i(-10), i(30), i(16)]), r#"28.89601342463584"#, r#"statistics.pstdev([-29, 47, -14, 50, -10, 30, 16])"#);
        chk(bad, st("median_high", &[i(-17), i(-3), i(21)]), r#"-3"#, r#"statistics.median_high([-17, -3, 21])"#);
        chk(bad, st("pstdev", &[f(19.34), f(12.5)]), r#"3.42"#, r#"statistics.pstdev([19.34, 12.5])"#);
        chk(bad, st("median_low", &[f(-5.4), f(-1.2), f(-20.0), f(-2.19), f(-4.2), f(16.349), f(5.0)]), r#"-2.19"#, r#"statistics.median_low([-5.4, -1.2, -20.0, -2.19, -4.2, 16.349, 5.0])"#);
        chk(bad, st("fmean", &[f(3.0), f(-4.098), f(6.81), f(16.765)]), r#"5.61925"#, r#"statistics.fmean([3.0, -4.098, 6.81, 16.765])"#);
        chk(bad, st("stdev", &[f(-10.79), f(13.0)]), r#"16.822070324427965"#, r#"statistics.stdev([-10.79, 13.0])"#);
        chk(bad, st("median_low", &[i(-17), i(-6), i(16), i(-27), i(-40), i(35), i(45)]), r#"-6"#, r#"statistics.median_low([-17, -6, 16, -27, -40, 35, 45])"#);
        chk(bad, st("median", &[i(-16), i(0), i(-25)]), r#"-16"#, r#"statistics.median([-16, 0, -25])"#);
        chk(bad, st("median_high", &[f(13.439), f(-6.05), f(4.292), f(14.0)]), r#"13.439"#, r#"statistics.median_high([13.439, -6.05, 4.292, 14.0])"#);
        chk(bad, st("mean", &[i(-34), i(43), i(29), i(35)]), r#"18.25"#, r#"statistics.mean([-34, 43, 29, 35])"#);
        chk(bad, st("fmean", &[f(-11.018), f(-19.31), f(1.0)]), r#"-9.776"#, r#"statistics.fmean([-11.018, -19.31, 1.0])"#);
        chk(bad, st("median_low", &[i(36), i(8)]), r#"8"#, r#"statistics.median_low([36, 8])"#);
        chk(bad, sm("1a ", "count", &[s(""), i(-5), i(3)]), r#"4"#, r#"'1a '.count('', -5, 3)"#);
        chk(bad, sm("\t,a  b", "center", &[i(8)]), r#"' \t,a  b '"#, r#"'\t,a  b'.center(8)"#);
        chk(bad, sm(" X,", "endswith", &[s(" "), i(3), i(9)]), r#"False"#, r#"' X,'.endswith(' ', 3, 9)"#);
        chk(bad, sm(",1x", "title", &[]), r#"',1X'"#, r#"',1x'.title()"#);
        chk(bad, sm("\t b", "rfind", &[s(",b"), i(-1), i(6)]), r#"-1"#, r#"'\t b'.rfind(',b', -1, 6)"#);
        chk(bad, sm("Xb,\t,1X ,", "split", &[s("a "), i(2)]), r#"['Xb,\t,1X ,']"#, r#"'Xb,\t,1X ,'.split('a ', 2)"#);
        chk(bad, sm("bxa-X1", "startswith", &[s("b "), i(1), i(-4)]), r#"False"#, r#"'bxa-X1'.startswith('b ', 1, -4)"#);
        chk(bad, sm("bab,\ta\t", "find", &[s(""), i(1), i(6)]), r#"1"#, r#"'bab,\ta\t'.find('', 1, 6)"#);
        chk(bad, sm("aba-", "rstrip", &[s("b ")]), r#"'aba-'"#, r#"'aba-'.rstrip('b ')"#);
        chk(bad, sm("x", "capitalize", &[]), r#"'X'"#, r#"'x'.capitalize()"#);
        chk(bad, sm("b\t ,\t, ", "strip", &[s("b")]), r#"'\t ,\t, '"#, r#"'b\t ,\t, '.strip('b')"#);
        chk(bad, sm("1x1xxx-a", "title", &[]), r#"'1X1Xxx-A'"#, r#"'1x1xxx-a'.title()"#);
        chk(bad, sm(",", "swapcase", &[]), r#"','"#, r#"','.swapcase()"#);
        chk(bad, sm("", "rfind", &[s(""), i(5), i(-2)]), r#"-1"#, r#"''.rfind('', 5, -2)"#);
        chk(bad, sm("abxX,", "zfill", &[i(7)]), r#"'00abxX,'"#, r#"'abxX,'.zfill(7)"#);
        chk(bad, sm(" ,", "partition", &[s("")]), r#"<err>"#, r#"' ,'.partition('')"#);
        chk(bad, sm("-  ", "swapcase", &[]), r#"'-  '"#, r#"'-  '.swapcase()"#);
        chk(bad, sm("  xxx1", "rpartition", &[s(",,")]), r#"('', '', '  xxx1')"#, r#"'  xxx1'.rpartition(',,')"#);
        chk(bad, sm("1-Xx", "title", &[]), r#"'1-Xx'"#, r#"'1-Xx'.title()"#);
        chk(bad, sm("X1-,", "partition", &[s("a")]), r#"('X1-,', '', '')"#, r#"'X1-,'.partition('a')"#);
        chk(bad, sm("b1 ,xx,a ", "partition", &[s(" a")]), r#"('b1 ,xx,a ', '', '')"#, r#"'b1 ,xx,a '.partition(' a')"#);
        chk(bad, sm("\taabXa\t 1", "rstrip", &[]), r#"'\taabXa\t 1'"#, r#"'\taabXa\t 1'.rstrip()"#);
        chk(bad, sm("1", "lstrip", &[s("a,")]), r#"'1'"#, r#"'1'.lstrip('a,')"#);
        chk(bad, sm(" bX,,-", "istitle", &[]), r#"False"#, r#"' bX,,-'.istitle()"#);
        chk(bad, sm("b", "rsplit", &[nn(), i(2)]), r#"['b']"#, r#"'b'.rsplit(None, 2)"#);
        chk(bad, sm(",x,a", "replace", &[s(""), s("_"), i(2)]), r#"'_,_x,a'"#, r#"',x,a'.replace('', '_', 2)"#);
        chk(bad, sm("", "title", &[]), r#"''"#, r#"''.title()"#);
        chk(bad, sm("\tx-1\t", "title", &[]), r#"'\tX-1\t'"#, r#"'\tx-1\t'.title()"#);
        chk(bad, sm(" Xab XX,", "capitalize", &[]), r#"' xab xx,'"#, r#"' Xab XX,'.capitalize()"#);
        chk(bad, sm("1-b \t-X1x", "endswith", &[s("ba"), i(0), i(-5)]), r#"False"#, r#"'1-b \t-X1x'.endswith('ba', 0, -5)"#);
        chk(bad, sm("1\t -", "partition", &[s(", ")]), r#"('1\t -', '', '')"#, r#"'1\t -'.partition(', ')"#);
        chk(bad, sm("\tX--x  X", "partition", &[s(" ,")]), r#"('\tX--x  X', '', '')"#, r#"'\tX--x  X'.partition(' ,')"#);
        chk(bad, sm("11 x\t", "endswith", &[s("a"), i(-5), i(4)]), r#"False"#, r#"'11 x\t'.endswith('a', -5, 4)"#);
        chk(bad, sm(",X,1", "count", &[s("a"), i(4), i(7)]), r#"0"#, r#"',X,1'.count('a', 4, 7)"#);
        chk(bad, sm("\t1b,bb-\t", "count", &[s(""), i(-5), i(-5)]), r#"1"#, r#"'\t1b,bb-\t'.count('', -5, -5)"#);
        chk(bad, sm("-X bxaXxb", "swapcase", &[]), r#"'-x BXAxXB'"#, r#"'-X bxaXxb'.swapcase()"#);
        chk(bad, sm("1", "center", &[i(3)]), r#"' 1 '"#, r#"'1'.center(3)"#);
        chk(bad, sm("Xa-X", "endswith", &[s("a "), i(-2), i(7)]), r#"False"#, r#"'Xa-X'.endswith('a ', -2, 7)"#);
        chk(bad, sm(" -X\t,", "rpartition", &[s("  ")]), r#"('', '', ' -X\t,')"#, r#"' -X\t,'.rpartition('  ')"#);
        chk(bad, sm("", "split", &[nn(), i(0)]), r#"[]"#, r#"''.split(None, 0)"#);
        chk(bad, sm("-", "endswith", &[s("a"), i(3), i(7)]), r#"False"#, r#"'-'.endswith('a', 3, 7)"#);
        chk(bad, sm("bb bxx \t\t", "rstrip", &[s(",")]), r#"'bb bxx \t\t'"#, r#"'bb bxx \t\t'.rstrip(',')"#);
        chk(bad, sm("bx,-Xax", "rfind", &[s(" ,"), i(0), i(-4)]), r#"-1"#, r#"'bx,-Xax'.rfind(' ,', 0, -4)"#);
        chk(bad, sm("XXa-b", "center", &[i(10)]), r#"'  XXa-b   '"#, r#"'XXa-b'.center(10)"#);
        chk(bad, sm(",-b\txb", "capitalize", &[]), r#"',-b\txb'"#, r#"',-b\txb'.capitalize()"#);
        chk(bad, sm(",,b x\t-", "endswith", &[s(""), i(0), i(-4)]), r#"True"#, r#"',,b x\t-'.endswith('', 0, -4)"#);
        chk(bad, sm("b", "lstrip", &[s(" b")]), r#"''"#, r#"'b'.lstrip(' b')"#);
        chk(bad, sm(", xxaaX", "title", &[]), r#"', Xxaax'"#, r#"', xxaaX'.title()"#);
        chk(bad, sm("baX\tbX-X", "title", &[]), r#"'Bax\tBx-X'"#, r#"'baX\tbX-X'.title()"#);
        chk(bad, sm(", bb", "capitalize", &[]), r#"', bb'"#, r#"', bb'.capitalize()"#);
        chk(bad, sm("", "rpartition", &[s(" ")]), r#"('', '', '')"#, r#"''.rpartition(' ')"#);
        chk(bad, sm("b,\t,\t", "istitle", &[]), r#"False"#, r#"'b,\t,\t'.istitle()"#);
        chk(bad, sm(",\t1XX ", "rstrip", &[s("")]), r#"',\t1XX '"#, r#"',\t1XX '.rstrip('')"#);
        chk(bad, sm("1 ", "strip", &[]), r#"'1'"#, r#"'1 '.strip()"#);
        chk(bad, sm(" x-1X", "partition", &[s(",")]), r#"(' x-1X', '', '')"#, r#"' x-1X'.partition(',')"#);
        chk(bad, sm("", "capitalize", &[]), r#"''"#, r#"''.capitalize()"#);
        chk(bad, sm("-\t", "swapcase", &[]), r#"'-\t'"#, r#"'-\t'.swapcase()"#);
        chk(bad, sm("X-   \tx--", "split", &[nn(), i(0)]), r#"['X-   \tx--']"#, r#"'X-   \tx--'.split(None, 0)"#);
        chk(bad, sm("XX  X", "split", &[nn(), i(-1)]), r#"['XX', 'X']"#, r#"'XX  X'.split(None, -1)"#);
        chk(bad, sm("1Xx ", "endswith", &[s(""), i(1), i(-2)]), r#"True"#, r#"'1Xx '.endswith('', 1, -2)"#);
        chk(bad, sm(",---b11-", "title", &[]), r#"',---B11-'"#, r#"',---b11-'.title()"#);
        chk(bad, sm(" bX,1 1b", "swapcase", &[]), r#"' Bx,1 1B'"#, r#"' bX,1 1b'.swapcase()"#);
        chk(bad, sm("-bxa1", "partition", &[s(",")]), r#"('-bxa1', '', '')"#, r#"'-bxa1'.partition(',')"#);
        chk(bad, sm("- x1", "split", &[s("b,"), i(2)]), r#"['- x1']"#, r#"'- x1'.split('b,', 2)"#);
        chk(bad, sm(",-", "startswith", &[s(" ,"), i(2), i(9)]), r#"False"#, r#"',-'.startswith(' ,', 2, 9)"#);
        chk(bad, sm("b", "center", &[i(8)]), r#"'   b    '"#, r#"'b'.center(8)"#);
        chk(bad, sm("X", "swapcase", &[]), r#"'x'"#, r#"'X'.swapcase()"#);
        chk(bad, sm("X a,\t", "replace", &[s(""), s("_"), i(0)]), r#"'X a,\t'"#, r#"'X a,\t'.replace('', '_', 0)"#);
        chk(bad, sm("", "capitalize", &[]), r#"''"#, r#"''.capitalize()"#);
        chk(bad, sm("bxbb\t ,", "find", &[s(""), i(-1), i(2)]), r#"-1"#, r#"'bxbb\t ,'.find('', -1, 2)"#);
        chk(bad, sm("bX,X, b-", "rstrip", &[s("ab")]), r#"'bX,X, b-'"#, r#"'bX,X, b-'.rstrip('ab')"#);
        chk(bad, sm("", "rstrip", &[]), r#"''"#, r#"''.rstrip()"#);
        chk(bad, sm("x\t", "rsplit", &[nn(), i(2)]), r#"['x']"#, r#"'x\t'.rsplit(None, 2)"#);
        chk(bad, sm("a  ", "startswith", &[s("ba"), i(-4), i(4)]), r#"False"#, r#"'a  '.startswith('ba', -4, 4)"#);
        chk(bad, sm("", "replace", &[s(""), s("_"), i(0)]), r#"''"#, r#"''.replace('', '_', 0)"#);
        chk(bad, sm("b\tX1-,\tX1", "find", &[s(",b"), i(-2), i(-3)]), r#"-1"#, r#"'b\tX1-,\tX1'.find(',b', -2, -3)"#);
        chk(bad, sm("-\tX", "capitalize", &[]), r#"'-\tx'"#, r#"'-\tX'.capitalize()"#);
        chk(bad, sm("b\t-1abxb", "rstrip", &[]), r#"'b\t-1abxb'"#, r#"'b\t-1abxb'.rstrip()"#);
        chk(bad, sm("a\ta\t11, ", "rsplit", &[s(" "), i(0)]), r#"['a\ta\t11, ']"#, r#"'a\ta\t11, '.rsplit(' ', 0)"#);
        chk(bad, sm("", "split", &[nn(), i(0)]), r#"[]"#, r#"''.split(None, 0)"#);
        chk(bad, sm("-\t 1b\tXa", "find", &[s(""), i(4), i(6)]), r#"4"#, r#"'-\t 1b\tXa'.find('', 4, 6)"#);
        chk(bad, sm("-X\tx", "rpartition", &[s("aa")]), r#"('', '', '-X\tx')"#, r#"'-X\tx'.rpartition('aa')"#);
        chk(bad, sm("ab1X 1", "capitalize", &[]), r#"'Ab1x 1'"#, r#"'ab1X 1'.capitalize()"#);
        chk(bad, sm("x \t,-a--1", "zfill", &[i(2)]), r#"'x \t,-a--1'"#, r#"'x \t,-a--1'.zfill(2)"#);
        chk(bad, sm(" a ", "endswith", &[s(" a"), i(3), i(1)]), r#"False"#, r#"' a '.endswith(' a', 3, 1)"#);
        chk(bad, sm("1ba1-aX", "endswith", &[s(",,"), i(4), i(3)]), r#"False"#, r#"'1ba1-aX'.endswith(',,', 4, 3)"#);
        chk(bad, sm("1,axb ", "rpartition", &[s("")]), r#"<err>"#, r#"'1,axb '.rpartition('')"#);
        chk(bad, sm("", "capitalize", &[]), r#"''"#, r#"''.capitalize()"#);
        chk(bad, sm(" xxXb\t,bb", "center", &[i(6)]), r#"' xxXb\t,bb'"#, r#"' xxXb\t,bb'.center(6)"#);
        chk(bad, sm("1\t\t", "zfill", &[i(8)]), r#"'000001\t\t'"#, r#"'1\t\t'.zfill(8)"#);
        chk(bad, sm("", "istitle", &[]), r#"False"#, r#"''.istitle()"#);
        chk(bad, sm("\tXa-1\tbb", "rstrip", &[s("")]), r#"'\tXa-1\tbb'"#, r#"'\tXa-1\tbb'.rstrip('')"#);
        chk(bad, sm("", "strip", &[]), r#"''"#, r#"''.strip()"#);
        chk(bad, sm("a1b1", "lstrip", &[]), r#"'a1b1'"#, r#"'a1b1'.lstrip()"#);
        chk(bad, sm("\t", "partition", &[s("")]), r#"<err>"#, r#"'\t'.partition('')"#);
        chk(bad, sm(" \t -bXax", "lstrip", &[s(",")]), r#"' \t -bXax'"#, r#"' \t -bXax'.lstrip(',')"#);
        chk(bad, sm("X,,\t\t", "rfind", &[s(","), i(-4), i(9)]), r#"2"#, r#"'X,,\t\t'.rfind(',', -4, 9)"#);
        chk(bad, sm(" 1xX1 b ", "startswith", &[s(" "), i(-4), i(-5)]), r#"False"#, r#"' 1xX1 b '.startswith(' ', -4, -5)"#);
        chk(bad, sm("\t", "zfill", &[i(6)]), r#"'00000\t'"#, r#"'\t'.zfill(6)"#);
        chk(bad, sm("1a--\t\t\t-", "capitalize", &[]), r#"'1a--\t\t\t-'"#, r#"'1a--\t\t\t-'.capitalize()"#);
        chk(bad, sm("x\t-\t, ", "strip", &[s("  ")]), r#"'x\t-\t,'"#, r#"'x\t-\t, '.strip('  ')"#);
        chk(bad, sm("1bx", "replace", &[s(" "), s("_"), i(0)]), r#"'1bx'"#, r#"'1bx'.replace(' ', '_', 0)"#);
        chk(bad, sm(",X1\t -\t", "strip", &[s("")]), r#"',X1\t -\t'"#, r#"',X1\t -\t'.strip('')"#);
        chk(bad, sm("-", "zfill", &[i(10)]), r#"'-000000000'"#, r#"'-'.zfill(10)"#);
        chk(bad, sm("1-b", "count", &[s(""), i(3), i(1)]), r#"0"#, r#"'1-b'.count('', 3, 1)"#);
        chk(bad, sm(",bXa-ab", "swapcase", &[]), r#"',BxA-AB'"#, r#"',bXa-ab'.swapcase()"#);
        chk(bad, sm("X", "zfill", &[i(1)]), r#"'X'"#, r#"'X'.zfill(1)"#);
        chk(bad, sm("X", "lstrip", &[s("a")]), r#"'X'"#, r#"'X'.lstrip('a')"#);
        chk(bad, sm("a-a, \t", "capitalize", &[]), r#"'A-a, \t'"#, r#"'a-a, \t'.capitalize()"#);
        chk(bad, sm("-1,xxb", "partition", &[s("")]), r#"<err>"#, r#"'-1,xxb'.partition('')"#);
        chk(bad, sm(",b -1\t a\t", "capitalize", &[]), r#"',b -1\t a\t'"#, r#"',b -1\t a\t'.capitalize()"#);
        chk(bad, sm("ax--,a\t -", "strip", &[]), r#"'ax--,a\t -'"#, r#"'ax--,a\t -'.strip()"#);
        chk(bad, sm("-   \tb", "strip", &[]), r#"'-   \tb'"#, r#"'-   \tb'.strip()"#);
        chk(bad, sm("", "rsplit", &[s("b"), i(0)]), r#"['']"#, r#"''.rsplit('b', 0)"#);
        chk(bad, sm("-1 X1 x", "partition", &[s(",")]), r#"('-1 X1 x', '', '')"#, r#"'-1 X1 x'.partition(',')"#);
        chk(bad, sm("xX x11", "rpartition", &[s("")]), r#"<err>"#, r#"'xX x11'.rpartition('')"#);
        chk(bad, sm("b", "find", &[s(""), i(-5), i(6)]), r#"0"#, r#"'b'.find('', -5, 6)"#);
        chk(bad, sm("1axaX\tb", "capitalize", &[]), r#"'1axax\tb'"#, r#"'1axaX\tb'.capitalize()"#);
        chk(bad, sm("-- xX-", "count", &[s(" ,"), i(2), i(3)]), r#"0"#, r#"'-- xX-'.count(' ,', 2, 3)"#);
        chk(bad, sm(",b", "capitalize", &[]), r#"',b'"#, r#"',b'.capitalize()"#);
        chk(bad, b("int", &[s("12"), i(10)]), r#"12"#, r#"int('12', 10)"#);
        chk(bad, b("int", &[s("12"), i(0)]), r#"12"#, r#"int('12', 0)"#);
        chk(bad, b("int", &[s("12"), i(16)]), r#"18"#, r#"int('12', 16)"#);
        chk(bad, b("int", &[s(" -0x1F "), i(10)]), r#"<err>"#, r#"int(' -0x1F ', 10)"#);
        chk(bad, b("int", &[s(" -0x1F "), i(0)]), r#"-31"#, r#"int(' -0x1F ', 0)"#);
        chk(bad, b("int", &[s(" -0x1F "), i(16)]), r#"-31"#, r#"int(' -0x1F ', 16)"#);
        chk(bad, b("int", &[s("0o17"), i(10)]), r#"<err>"#, r#"int('0o17', 10)"#);
        chk(bad, b("int", &[s("0o17"), i(0)]), r#"15"#, r#"int('0o17', 0)"#);
        chk(bad, b("int", &[s("0o17"), i(16)]), r#"<err>"#, r#"int('0o17', 16)"#);
        chk(bad, b("int", &[s("1_2_3"), i(10)]), r#"123"#, r#"int('1_2_3', 10)"#);
        chk(bad, b("int", &[s("1_2_3"), i(0)]), r#"123"#, r#"int('1_2_3', 0)"#);
        chk(bad, b("int", &[s("1_2_3"), i(16)]), r#"291"#, r#"int('1_2_3', 16)"#);
        chk(bad, b("int", &[s("_12"), i(10)]), r#"<err>"#, r#"int('_12', 10)"#);
        chk(bad, b("int", &[s("_12"), i(0)]), r#"<err>"#, r#"int('_12', 0)"#);
        chk(bad, b("int", &[s("_12"), i(16)]), r#"<err>"#, r#"int('_12', 16)"#);
        chk(bad, b("int", &[s("12_"), i(10)]), r#"<err>"#, r#"int('12_', 10)"#);
        chk(bad, b("int", &[s("12_"), i(0)]), r#"<err>"#, r#"int('12_', 0)"#);
        chk(bad, b("int", &[s("12_"), i(16)]), r#"<err>"#, r#"int('12_', 16)"#);
        chk(bad, b("int", &[s("0b"), i(10)]), r#"<err>"#, r#"int('0b', 10)"#);
        chk(bad, b("int", &[s("0b"), i(0)]), r#"<err>"#, r#"int('0b', 0)"#);
        chk(bad, b("int", &[s("0b"), i(16)]), r#"11"#, r#"int('0b', 16)"#);
        chk(bad, b("int", &[s("+7"), i(10)]), r#"7"#, r#"int('+7', 10)"#);
        chk(bad, b("int", &[s("+7"), i(0)]), r#"7"#, r#"int('+7', 0)"#);
        chk(bad, b("int", &[s("+7"), i(16)]), r#"7"#, r#"int('+7', 16)"#);
        chk(bad, b("int", &[s("1e3"), i(10)]), r#"<err>"#, r#"int('1e3', 10)"#);
        chk(bad, b("int", &[s("1e3"), i(0)]), r#"<err>"#, r#"int('1e3', 0)"#);
        chk(bad, b("int", &[s("1e3"), i(16)]), r#"483"#, r#"int('1e3', 16)"#);
        chk(bad, b("int", &[s("٣"), i(10)]), r#"<err>"#, r#"int('٣', 10) [declined; Python gives 3]"#);
        chk(bad, b("int", &[s("٣"), i(0)]), r#"<err>"#, r#"int('٣', 0) [declined; Python gives 3]"#);
        chk(bad, b("int", &[s("٣"), i(16)]), r#"<err>"#, r#"int('٣', 16) [declined; Python gives 3]"#);
        chk(bad, b("int", &[s("  "), i(10)]), r#"<err>"#, r#"int('  ', 10)"#);
        chk(bad, b("int", &[s("  "), i(0)]), r#"<err>"#, r#"int('  ', 0)"#);
        chk(bad, b("int", &[s("  "), i(16)]), r#"<err>"#, r#"int('  ', 16)"#);
        chk(bad, b("int", &[s("0_0"), i(10)]), r#"0"#, r#"int('0_0', 10)"#);
        chk(bad, b("int", &[s("0_0"), i(0)]), r#"0"#, r#"int('0_0', 0)"#);
        chk(bad, b("int", &[s("0_0"), i(16)]), r#"0"#, r#"int('0_0', 16)"#);
        chk(bad, b("int", &[s("00"), i(10)]), r#"0"#, r#"int('00', 10)"#);
        chk(bad, b("int", &[s("00"), i(0)]), r#"0"#, r#"int('00', 0)"#);
        chk(bad, b("int", &[s("00"), i(16)]), r#"0"#, r#"int('00', 16)"#);
        chk(bad, b("int", &[s("07"), i(10)]), r#"7"#, r#"int('07', 10)"#);
        chk(bad, b("int", &[s("07"), i(0)]), r#"<err>"#, r#"int('07', 0)"#);
        chk(bad, b("int", &[s("07"), i(16)]), r#"7"#, r#"int('07', 16)"#);
        chk(bad, b("float", &[s("1.5")]), r#"1.5"#, r#"float('1.5')"#);
        chk(bad, b("float", &[s(" -2.5e-3 ")]), r#"-0.0025"#, r#"float(' -2.5e-3 ')"#);
        chk(bad, b("float", &[s("1_0.0_1")]), r#"10.01"#, r#"float('1_0.0_1')"#);
        chk(bad, b("float", &[s("1__0")]), r#"<err>"#, r#"float('1__0')"#);
        chk(bad, b("float", &[s("nan")]), r#"nan"#, r#"float('nan')"#);
        chk(bad, b("float", &[s("-Inf")]), r#"-inf"#, r#"float('-Inf')"#);
        chk(bad, b("float", &[s("inF")]), r#"inf"#, r#"float('inF')"#);
        chk(bad, b("float", &[s("e5")]), r#"<err>"#, r#"float('e5')"#);
        chk(bad, b("float", &[s("1e")]), r#"<err>"#, r#"float('1e')"#);
        chk(bad, b("float", &[s(".")]), r#"<err>"#, r#"float('.')"#);
        chk(bad, b("float", &[s("1.e5")]), r#"100000.0"#, r#"float('1.e5')"#);
        chk(bad, b("float", &[s("0x10")]), r#"<err>"#, r#"float('0x10')"#);
        chk(bad, b("float", &[s("1e-400")]), r#"0.0"#, r#"float('1e-400')"#);
        chk(bad, b("float", &[s("+.5E+2")]), r#"50.0"#, r#"float('+.5E+2')"#);
        chk(bad, b("float", &[s("4_")]), r#"<err>"#, r#"float('4_')"#);
        chk(bad, fv(i(-795332644453), r#"e"#), r#"'-7.953326e+11'"#, r#"format(-795332644453, 'e')"#);
        chk(bad, fv(i(-229361981029), r#""#), r#"'-229361981029'"#, r#"format(-229361981029, '')"#);
        chk(bad, fv(i(197700684539), r#"+015,"#), r#"'+197,700,684,539'"#, r#"format(197700684539, '+015,')"#);
        chk(bad, fv(i(-352674596478), r#"b"#), r#"'-101001000011101000010110100011001111110'"#, r#"format(-352674596478, 'b')"#);
        chk(bad, fv(i(503952969565), r#","#), r#"'503,952,969,565'"#, r#"format(503952969565, ',')"#);
        chk(bad, fv(i(-994791813616), r#"x"#), r#"'-e79e3671f0'"#, r#"format(-994791813616, 'x')"#);
        chk(bad, fv(i(-126504685574), r#""#), r#"'-126504685574'"#, r#"format(-126504685574, '')"#);
        chk(bad, fv(i(694824342870), r#"#_x"#), r#"'0xa1_c6c2_1556'"#, r#"format(694824342870, '#_x')"#);
        chk(bad, fv(i(355348146075), r#"#_x"#), r#"'0x52_bc66_679b'"#, r#"format(355348146075, '#_x')"#);
        chk(bad, fv(i(636646448352), r#"_"#), r#"'636_646_448_352'"#, r#"format(636646448352, '_')"#);
        chk(bad, fv(i(81625440324), r#"x"#), r#"'1301416044'"#, r#"format(81625440324, 'x')"#);
        chk(bad, fv(i(807650450267), r#"#o"#), r#"'0o13601355755533'"#, r#"format(807650450267, '#o')"#);
        chk(bad, fv(i(-517301839404), r#"%"#), r#"'-51730183940400.000000%'"#, r#"format(-517301839404, '%')"#);
        chk(bad, fv(i(608014740492), r#"e"#), r#"'6.080147e+11'"#, r#"format(608014740492, 'e')"#);
        chk(bad, fv(i(-905500461984), r#"#_x"#), r#"'-0xd2_d408_8ba0'"#, r#"format(-905500461984, '#_x')"#);
        chk(bad, fv(i(197384316339), r#"+015,"#), r#"'+197,384,316,339'"#, r#"format(197384316339, '+015,')"#);
        chk(bad, fv(i(934376907472), r#","#), r#"'934,376,907,472'"#, r#"format(934376907472, ',')"#);
        chk(bad, fv(i(-251344184364), r#""#), r#"'-251344184364'"#, r#"format(-251344184364, '')"#);
        chk(bad, fv(i(346795456710), r#"b"#), r#"'101000010111110100111101011100011000110'"#, r#"format(346795456710, 'b')"#);
        chk(bad, fv(i(-785171967713), r#"#_x"#), r#"'-0xb6_cfe5_6ee1'"#, r#"format(-785171967713, '#_x')"#);
        chk(bad, fv(i(-248524685528), r#"%"#), r#"'-24852468552800.000000%'"#, r#"format(-248524685528, '%')"#);
        chk(bad, fv(i(-592460808012), r#"x"#), r#"'-89f16a7f4c'"#, r#"format(-592460808012, 'x')"#);
        chk(bad, fv(i(-38752273893), r#"%"#), r#"'-3875227389300.000000%'"#, r#"format(-38752273893, '%')"#);
        chk(bad, fv(i(167023660027), r#"^20"#), r#"'    167023660027    '"#, r#"format(167023660027, '^20')"#);
        chk(bad, fv(i(-163030757736), r#"_"#), r#"'-163_030_757_736'"#, r#"format(-163030757736, '_')"#);
        chk(bad, fv(i(-267479840050), r#"b"#), r#"'-11111001000111000010100111010100110010'"#, r#"format(-267479840050, 'b')"#);
        chk(bad, fv(i(-116686385404), r#","#), r#"'-116,686,385,404'"#, r#"format(-116686385404, ',')"#);
        chk(bad, fv(i(-869368353848), r#","#), r#"'-869,368,353,848'"#, r#"format(-869368353848, ',')"#);
        chk(bad, fv(i(527086453789), r#"%"#), r#"'52708645378900.000000%'"#, r#"format(527086453789, '%')"#);
        chk(bad, fv(i(-161844166329), r#"^20"#), r#"'   -161844166329    '"#, r#"format(-161844166329, '^20')"#);
        chk(bad, fv(i(-367398627390), r#"_"#), r#"'-367_398_627_390'"#, r#"format(-367398627390, '_')"#);
        chk(bad, fv(i(914954744318), r#""#), r#"'914954744318'"#, r#"format(914954744318, '')"#);
        chk(bad, fv(i(-910474356664), r#"_"#), r#"'-910_474_356_664'"#, r#"format(-910474356664, '_')"#);
        chk(bad, fv(i(-273506756067), r#"^20"#), r#"'   -273506756067    '"#, r#"format(-273506756067, '^20')"#);
        chk(bad, fv(i(-368137361041), r#"%"#), r#"'-36813736104100.000000%'"#, r#"format(-368137361041, '%')"#);
        chk(bad, fv(i(264786339572), r#"+015,"#), r#"'+264,786,339,572'"#, r#"format(264786339572, '+015,')"#);
        chk(bad, fv(i(-152600372432), r#"^20"#), r#"'   -152600372432    '"#, r#"format(-152600372432, '^20')"#);
        chk(bad, fv(i(-993939674702), r#"#_x"#), r#"'-0xe7_6b6b_d64e'"#, r#"format(-993939674702, '#_x')"#);
        chk(bad, fv(i(-127942627142), r#"b"#), r#"'-1110111001001111110011000111101000110'"#, r#"format(-127942627142, 'b')"#);
        chk(bad, fv(i(472541611676), r#"#_x"#), r#"'0x6e_05ac_c69c'"#, r#"format(472541611676, '#_x')"#);
    }
}
