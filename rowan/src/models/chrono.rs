//! `datetime` values as literals (`Key::Chrono(kind, value, tz)`), computed as CPython does:
//! `date` (proleptic ordinal), `datetime` (microseconds since 0001-01-01, naive or with a fixed
//! UTC offset), `time` (microseconds of the day), `timedelta` (microseconds), `timezone`
//! (offset seconds). Construction and validation, `fromisoformat` / `strptime` / `strftime` /
//! `isoformat` / `str` / `repr`, arithmetic and comparisons, attributes and the common
//! methods. The clock (`now()`, `today()`, `fromtimestamp`) stays unknown.

use crate::intern::{self, key, key_bool, key_float, key_int, Key, KeyId};

pub const DATE: u8 = 0;
pub const DATETIME: u8 = 1;
pub const TIME: u8 = 2;
pub const DELTA: u8 = 3;
pub const TZ: u8 = 4;
pub const NAIVE: i32 = i32::MIN;

const US_DAY: i64 = 86_400_000_000;
/// (timedelta goes to 999999999 days; past 100 million days the value is not simulated)
const MAX_DELTA_US: i64 = 100_000_000 * US_DAY;

pub fn chrono_key(kind: u8, v: i64, tz: i32) -> KeyId {
    key(Key::Chrono(kind, v, tz))
}

/// Attributes and methods of datetime values.
pub const NAMES: &[&str] = &[
    "year", "month", "day", "hour", "minute", "second", "microsecond", "tzinfo", "days", "seconds", "microseconds", "weekday", "isoweekday", "isocalendar", "toordinal",
    "date", "time", "timetz", "isoformat", "strftime", "ctime", "replace", "utcoffset", "timestamp", "astimezone", "total_seconds", "tzname",
];

pub fn as_chrono(k: KeyId) -> Option<(u8, i64, i32)> {
    intern::with_key(k, |kk| match kk {
        Key::Chrono(a, b, c) => Some((*a, *b, *c)),
        _ => None,
    })
}

pub fn is_chrono_key(k: KeyId) -> bool {
    as_chrono(k).is_some()
}

// ---------------------------------------------------------------- calendar

fn is_leap(y: i64) -> bool {
    y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
}

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if is_leap(y) {
                29
            } else {
                28
            }
        }
    }
}

fn days_before_year(y: i64) -> i64 {
    let y = y - 1;
    y * 365 + y / 4 - y / 100 + y / 400
}

fn days_before_month(y: i64, m: i64) -> i64 {
    let mut d = 0;
    for mm in 1..m {
        d += days_in_month(y, mm);
    }
    d
}

/// `date(y, m, d).toordinal()`; None when invalid.
pub fn ymd_to_ord(y: i64, m: i64, d: i64) -> Option<i64> {
    if !(1..=9999).contains(&y) || !(1..=12).contains(&m) || d < 1 || d > days_in_month(y, m) {
        return None;
    }
    Some(days_before_year(y) + days_before_month(y, m) + d)
}

pub fn ord_to_ymd(n: i64) -> (i64, i64, i64) {
    // CPython's _ord2ymd
    let mut n = n - 1;
    let n400 = n / 146_097;
    n %= 146_097;
    let mut year = n400 * 400 + 1;
    let n100 = n / 36_524;
    n %= 36_524;
    let n4 = n / 1_461;
    n %= 1_461;
    let n1 = n / 365;
    n %= 365;
    year += n100 * 100 + n4 * 4 + n1;
    if n1 == 4 || n100 == 4 {
        return (year - 1, 12, 31);
    }
    let leap = n1 == 3 && (n4 != 24 || n100 == 3);
    let mut month = (n + 50) >> 5;
    let before = |y: i64, m: i64| days_before_month(y, m);
    let mut preceding = before(if leap { 2000 } else { 2001 }, month);
    if preceding > n {
        month -= 1;
        preceding -= days_in_month(if leap { 2000 } else { 2001 }, month);
    }
    let _ = leap;
    (year, month, n - preceding + 1)
}

fn weekday(ord: i64) -> i64 {
    (ord + 6) % 7
}

const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
const DAYS: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

/// The parts of a date / datetime / time value.
#[derive(Clone, Copy, Debug)]
pub struct Parts {
    pub y: i64,
    pub mo: i64,
    pub d: i64,
    pub h: i64,
    pub mi: i64,
    pub s: i64,
    pub us: i64,
    pub ord: i64,
}

pub fn parts(kind: u8, v: i64) -> Parts {
    let (ord, tod) = match kind {
        DATE => (v, 0),
        DATETIME => (v.div_euclid(US_DAY) + 1, v.rem_euclid(US_DAY)),
        _ => (1, v),
    };
    let (y, mo, d) = ord_to_ymd(ord);
    Parts { y, mo, d, h: tod / 3_600_000_000, mi: tod / 60_000_000 % 60, s: tod / 1_000_000 % 60, us: tod % 1_000_000, ord }
}

fn dt_value(ord: i64, h: i64, mi: i64, s: i64, us: i64) -> Option<i64> {
    if !(0..24).contains(&h) || !(0..60).contains(&mi) || !(0..60).contains(&s) || !(0..1_000_000).contains(&us) {
        return None;
    }
    Some((ord - 1) * US_DAY + ((h * 60 + mi) * 60 + s) * 1_000_000 + us)
}

fn offset_text(off: i32, colon: bool) -> String {
    if off == NAIVE {
        return String::new();
    }
    let sign = if off < 0 { '-' } else { '+' };
    let a = off.unsigned_abs();
    let (h, m, s) = (a / 3600, a / 60 % 60, a % 60);
    let sep = if colon { ":" } else { "" };
    if s != 0 {
        format!("{}{:02}{}{:02}{}{:02}", sign, h, sep, m, sep, s)
    } else {
        format!("{}{:02}{}{:02}", sign, h, sep, m)
    }
}

fn time_text(p: &Parts, timespec: &str) -> Option<String> {
    Some(match timespec {
        "auto" => {
            if p.us != 0 {
                format!("{:02}:{:02}:{:02}.{:06}", p.h, p.mi, p.s, p.us)
            } else {
                format!("{:02}:{:02}:{:02}", p.h, p.mi, p.s)
            }
        }
        "hours" => format!("{:02}", p.h),
        "minutes" => format!("{:02}:{:02}", p.h, p.mi),
        "seconds" => format!("{:02}:{:02}:{:02}", p.h, p.mi, p.s),
        "milliseconds" => format!("{:02}:{:02}:{:02}.{:03}", p.h, p.mi, p.s, p.us / 1000),
        "microseconds" => format!("{:02}:{:02}:{:02}.{:06}", p.h, p.mi, p.s, p.us),
        _ => return None,
    })
}

pub fn isoformat(k: KeyId, sep: &str, timespec: &str) -> Option<String> {
    let (kind, v, tz) = as_chrono(k)?;
    let p = parts(kind, v);
    Some(match kind {
        DATE => format!("{:04}-{:02}-{:02}", p.y, p.mo, p.d),
        DATETIME => format!("{:04}-{:02}-{:02}{}{}{}", p.y, p.mo, p.d, sep, time_text(&p, timespec)?, offset_text(tz, true)),
        TIME => format!("{}{}", time_text(&p, timespec)?, offset_text(tz, true)),
        _ => return None,
    })
}

fn delta_text(us: i64) -> String {
    let days = us.div_euclid(US_DAY);
    let rest = us.rem_euclid(US_DAY);
    let (h, m, s, f) = (rest / 3_600_000_000, rest / 60_000_000 % 60, rest / 1_000_000 % 60, rest % 1_000_000);
    let mut out = String::new();
    if days != 0 {
        out = format!("{} day{}, ", days, if days.abs() != 1 { "s" } else { "" });
    }
    out.push_str(&format!("{}:{:02}:{:02}", h, m, s));
    if f != 0 {
        out.push_str(&format!(".{:06}", f));
    }
    out
}

fn tz_name(off: i32) -> String {
    if off == 0 {
        "UTC".into()
    } else {
        format!("UTC{}", offset_text(off, true))
    }
}

pub fn str_of(k: KeyId) -> Option<String> {
    let (kind, v, _) = as_chrono(k)?;
    match kind {
        DATE | TIME => isoformat(k, "T", "auto"),
        DATETIME => isoformat(k, " ", "auto"),
        DELTA => Some(delta_text(v)),
        _ => Some(tz_name(v as i32)),
    }
}

fn tz_repr(off: i32) -> String {
    if off == 0 {
        "datetime.timezone.utc".into()
    } else {
        format!("datetime.timezone({})", delta_repr(off as i64 * 1_000_000))
    }
}

fn delta_repr(us: i64) -> String {
    let days = us.div_euclid(US_DAY);
    let rest = us.rem_euclid(US_DAY);
    let (s, f) = (rest / 1_000_000, rest % 1_000_000);
    let mut parts = Vec::new();
    if days != 0 {
        parts.push(format!("days={}", days));
    }
    if s != 0 {
        parts.push(format!("seconds={}", s));
    }
    if f != 0 {
        parts.push(format!("microseconds={}", f));
    }
    if parts.is_empty() {
        parts.push("0".into());
    }
    format!("datetime.timedelta({})", parts.join(", "))
}

pub fn repr_of(k: KeyId) -> Option<String> {
    let (kind, v, tz) = as_chrono(k)?;
    Some(repr_raw(kind, v, tz))
}

/// `repr` of a value from its parts (no key lookups: usable while the interner is borrowed).
pub fn repr_raw(kind: u8, v: i64, tz: i32) -> String {
    let p = parts(kind, v);
    let tzs = if tz != NAIVE { format!(", tzinfo={}", tz_repr(tz)) } else { String::new() };
    match kind {
        DATE => format!("datetime.date({}, {}, {})", p.y, p.mo, p.d),
        DATETIME => {
            let mut xs = vec![p.y, p.mo, p.d, p.h, p.mi];
            if p.s != 0 || p.us != 0 {
                xs.push(p.s);
            }
            if p.us != 0 {
                xs.push(p.us);
            }
            format!("datetime.datetime({}{})", xs.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", "), tzs)
        }
        TIME => {
            let mut xs = vec![p.h, p.mi];
            if p.s != 0 || p.us != 0 {
                xs.push(p.s);
            }
            if p.us != 0 {
                xs.push(p.us);
            }
            format!("datetime.time({}{})", xs.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", "), tzs)
        }
        DELTA => delta_repr(v),
        _ => tz_repr(v as i32),
    }
}

/// `strftime` with the C locale's directives.
pub fn strftime(k: KeyId, fmt: &str) -> Option<String> {
    let (kind, v, tz) = as_chrono(k)?;
    if kind == DELTA || kind == TZ {
        return None;
    }
    let p = parts(kind, v);
    let (y, mo, d) = if kind == TIME { (1900, 1, 1) } else { (p.y, p.mo, p.d) };
    let ord = if kind == TIME { ymd_to_ord(1900, 1, 1)? } else { p.ord };
    let wd = weekday(ord);
    let yday = ord - days_before_year(y);
    let mut out = String::new();
    let mut it = fmt.chars();
    while let Some(c) = it.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        let Some(d0) = it.next() else {
            out.push('%');
            break;
        };
        let piece = match d0 {
            'Y' => format!("{}", y),
            'y' => format!("{:02}", y % 100),
            'm' => format!("{:02}", mo),
            'd' => format!("{:02}", d),
            'e' => format!("{:2}", d),
            'H' => format!("{:02}", p.h),
            'I' => format!("{:02}", if p.h % 12 == 0 { 12 } else { p.h % 12 }),
            'p' => (if p.h < 12 { "AM" } else { "PM" }).to_string(),
            'M' => format!("{:02}", p.mi),
            'S' => format!("{:02}", p.s),
            'f' => format!("{:06}", p.us),
            'a' => DAYS[wd as usize][..3].to_string(),
            'A' => DAYS[wd as usize].to_string(),
            'w' => format!("{}", (wd + 1) % 7),
            'u' => format!("{}", wd + 1),
            'b' | 'h' => MONTHS[(mo - 1) as usize][..3].to_string(),
            'B' => MONTHS[(mo - 1) as usize].to_string(),
            'j' => format!("{:03}", yday),
            'z' => offset_text(tz, false),
            ':' => {
                if it.next()? != 'z' {
                    return None;
                }
                offset_text(tz, true)
            }
            'Z' => {
                if tz == NAIVE {
                    String::new()
                } else {
                    tz_name(tz)
                }
            }
            'F' => format!("{}-{:02}-{:02}", y, mo, d),
            'T' => format!("{:02}:{:02}:{:02}", p.h, p.mi, p.s),
            'D' => format!("{:02}/{:02}/{:02}", mo, d, y % 100),
            'R' => format!("{:02}:{:02}", p.h, p.mi),
            'c' => format!("{} {} {:2} {:02}:{:02}:{:02} {}", &DAYS[wd as usize][..3], &MONTHS[(mo - 1) as usize][..3], d, p.h, p.mi, p.s, y),
            'x' => format!("{:02}/{:02}/{:02}", mo, d, y % 100),
            'X' => format!("{:02}:{:02}:{:02}", p.h, p.mi, p.s),
            '%' => "%".into(),
            'n' => "\n".into(),
            't' => "\t".into(),
            _ => return None,
        };
        out.push_str(&piece);
    }
    Some(out)
}

/// `datetime.strptime(text, fmt)` for the common directives.
pub fn strptime(text: &str, fmt: &str) -> Option<KeyId> {
    let t: Vec<char> = text.chars().collect();
    let mut i = 0;
    let (mut y, mut mo, mut d, mut h, mut mi, mut s, mut us) = (1900i64, 1i64, 1i64, 0i64, 0i64, 0i64, 0i64);
    let mut pm: Option<bool> = None;
    let mut tz = NAIVE;
    let mut f = fmt.chars().peekable();
    let num = |t: &[char], i: &mut usize, max: usize| -> Option<i64> {
        let st = *i;
        while *i < t.len() && *i - st < max && t[*i].is_ascii_digit() {
            *i += 1;
        }
        if *i == st {
            return None;
        }
        t[st..*i].iter().collect::<String>().parse().ok()
    };
    while let Some(c) = f.next() {
        if c == '%' {
            let dir = f.next()?;
            match dir {
                'Y' => y = num(&t, &mut i, 4)?,
                'y' => {
                    let v = num(&t, &mut i, 2)?;
                    y = if v < 69 { 2000 + v } else { 1900 + v };
                }
                'm' => mo = num(&t, &mut i, 2)?,
                'd' => d = num(&t, &mut i, 2)?,
                'H' => h = num(&t, &mut i, 2)?,
                'I' => h = num(&t, &mut i, 2)?,
                'M' => mi = num(&t, &mut i, 2)?,
                'S' => s = num(&t, &mut i, 2)?,
                'f' => {
                    let st = i;
                    let v = num(&t, &mut i, 6)?;
                    us = v * 10i64.pow(6 - (i - st) as u32);
                }
                'p' => {
                    let w: String = t.get(i..i + 2)?.iter().collect::<String>().to_uppercase();
                    pm = Some(match w.as_str() {
                        "AM" => false,
                        "PM" => true,
                        _ => return None,
                    });
                    i += 2;
                }
                'b' | 'B' | 'h' => {
                    let rest: String = t[i..].iter().collect::<String>().to_lowercase();
                    let mut hit = None;
                    for (k, name) in MONTHS.iter().enumerate() {
                        let full = name.to_lowercase();
                        if dir == 'B' && rest.starts_with(&full) {
                            hit = Some((k, full.len()));
                            break;
                        }
                        if dir != 'B' && rest.starts_with(&full[..3]) {
                            hit = Some((k, 3));
                            break;
                        }
                    }
                    let (k, n) = hit?;
                    mo = k as i64 + 1;
                    i += n;
                }
                'a' | 'A' => {
                    let rest: String = t[i..].iter().collect::<String>().to_lowercase();
                    let mut n = None;
                    for name in DAYS.iter() {
                        let full = name.to_lowercase();
                        if dir == 'A' && rest.starts_with(&full) {
                            n = Some(full.len());
                            break;
                        }
                        if dir == 'a' && rest.starts_with(&full[..3]) {
                            n = Some(3);
                            break;
                        }
                    }
                    i += n?;
                }
                'j' => {
                    let v = num(&t, &mut i, 3)?;
                    let o = ymd_to_ord(y, 1, 1)? + v - 1;
                    let (_, m2, d2) = ord_to_ymd(o);
                    mo = m2;
                    d = d2;
                }
                'z' => {
                    if t.get(i) == Some(&'Z') {
                        tz = 0;
                        i += 1;
                    } else {
                        let sign = match t.get(i)? {
                            '+' => 1,
                            '-' => -1,
                            _ => return None,
                        };
                        i += 1;
                        let hh = num(&t, &mut i, 2)?;
                        if t.get(i) == Some(&':') {
                            i += 1;
                        }
                        let mm = num(&t, &mut i, 2)?;
                        tz = sign * (hh * 3600 + mm * 60) as i32;
                    }
                }
                '%' => {
                    if t.get(i) != Some(&'%') {
                        return None;
                    }
                    i += 1;
                }
                _ => return None,
            }
        } else if c.is_whitespace() {
            while i < t.len() && t[i].is_whitespace() {
                i += 1;
            }
        } else {
            if t.get(i) != Some(&c) {
                return None;
            }
            i += 1;
        }
    }
    if i != t.len() {
        return None;
    }
    if let Some(p) = pm {
        if !(1..=12).contains(&h) {
            return None;
        }
        h = h % 12 + if p { 12 } else { 0 };
    }
    let o = ymd_to_ord(y, mo, d)?;
    Some(chrono_key(DATETIME, dt_value(o, h, mi, s, us)?, tz))
}

/// `fromisoformat` of the class `kind` (3.11+ formats: `YYYY-MM-DD`, `YYYYMMDD`, a `T` or
/// space then `HH[:MM[:SS[.ffffff]]]`, then `Z` / `±HH[:MM]`).
pub fn fromisoformat(kind: u8, text: &str) -> Option<KeyId> {
    let s = text.trim();
    let date_part = |d: &str| -> Option<i64> {
        let (y, m, dd) = if d.len() == 10 && d.as_bytes()[4] == b'-' && d.as_bytes()[7] == b'-' {
            (&d[..4], &d[5..7], &d[8..])
        } else if d.len() == 8 && d.chars().all(|c| c.is_ascii_digit()) {
            (&d[..4], &d[4..6], &d[6..])
        } else {
            return None;
        };
        ymd_to_ord(y.parse().ok()?, m.parse().ok()?, dd.parse().ok()?)
    };
    let time_part = |t: &str| -> Option<(i64, i32)> {
        let (body, tz) = if let Some(b) = t.strip_suffix('Z') {
            (b, 0)
        } else if let Some(i) = t.rfind(['+', '-']) {
            let off = &t[i + 1..];
            let sign = if &t[i..i + 1] == "-" { -1 } else { 1 };
            let digits: String = off.chars().filter(|c| *c != ':').collect();
            if digits.len() < 2 || !digits.chars().all(|c| c.is_ascii_digit()) {
                return None;
            }
            let hh: i32 = digits[..2].parse().ok()?;
            let mm: i32 = if digits.len() >= 4 { digits[2..4].parse().ok()? } else { 0 };
            let ss: i32 = if digits.len() >= 6 { digits[4..6].parse().ok()? } else { 0 };
            (&t[..i], sign * (hh * 3600 + mm * 60 + ss))
        } else {
            (t, NAIVE)
        };
        let (hms, frac) = match body.split_once(['.', ',']) {
            Some((a, b)) => (a, Some(b)),
            None => (body, None),
        };
        let fs: Vec<&str> = if hms.contains(':') {
            hms.split(':').collect()
        } else {
            (0..hms.len() / 2).map(|i| &hms[2 * i..2 * i + 2]).collect()
        };
        if fs.is_empty() || fs.len() > 3 || fs.iter().any(|x| x.len() != 2 || !x.chars().all(|c| c.is_ascii_digit())) {
            return None;
        }
        let h: i64 = fs[0].parse().ok()?;
        let m: i64 = fs.get(1).map(|x| x.parse().unwrap_or(0)).unwrap_or(0);
        let sec: i64 = fs.get(2).map(|x| x.parse().unwrap_or(0)).unwrap_or(0);
        let us = match frac {
            Some(f) if !f.is_empty() && f.chars().all(|c| c.is_ascii_digit()) => {
                let mut f6: String = f.chars().take(6).collect();
                while f6.len() < 6 {
                    f6.push('0');
                }
                f6.parse::<i64>().ok()?
            }
            Some(_) => return None,
            None => 0,
        };
        Some((dt_value(1, h, m, sec, us)?, tz))
    };
    match kind {
        DATE => Some(chrono_key(DATE, date_part(s)?, NAIVE)),
        TIME => {
            let (v, tz) = time_part(s)?;
            Some(chrono_key(TIME, v, tz))
        }
        _ => {
            let (d, t) = match s.find(|c| c == 'T' || c == ' ') {
                Some(i) if i >= 8 => (&s[..i], Some(&s[i + 1..])),
                _ => (s, None),
            };
            let o = date_part(d)?;
            let (tv, tz) = match t {
                Some(t) => time_part(t)?,
                None => (0, NAIVE),
            };
            Some(chrono_key(DATETIME, (o - 1) * US_DAY + tv, tz))
        }
    }
}

// --------------------------------------------------------------- arithmetic

fn delta(us: i64) -> Option<KeyId> {
    if us.abs() > MAX_DELTA_US {
        return None;
    }
    Some(chrono_key(DELTA, us, NAIVE))
}

/// UTC instant of an aware datetime (micros), the value of a naive one.
fn instant(v: i64, tz: i32) -> i64 {
    if tz == NAIVE {
        v
    } else {
        v - tz as i64 * 1_000_000
    }
}

fn in_range_dt(v: i64) -> bool {
    v >= 0 && v < 3_652_059 * US_DAY
}

/// Operators on datetime values: Some(result) / Some(None) unknown or raising; None: not ours.
pub fn binop(op: &str, a: KeyId, b: KeyId) -> Option<Option<KeyId>> {
    let (ca, cb) = (as_chrono(a), as_chrono(b));
    if ca.is_none() && cb.is_none() {
        return None;
    }
    if matches!(op, "eq" | "ne" | "lt" | "le" | "gt" | "ge") {
        return Some(compare(op, a, b).map(key_bool));
    }
    let num = |k: KeyId| -> Option<f64> {
        intern::with_key(k, |kk| match kk {
            Key::Int(i) => Some(*i as f64),
            Key::Float(f) => Some(f64::from_bits(*f)),
            _ => None,
        })
    };
    let int = |k: KeyId| intern::with_key(k, |kk| if let Key::Int(i) = kk { Some(*i) } else { None });
    Some((|| -> Option<KeyId> {
        match (ca, cb, op) {
            (Some((DELTA, x, _)), Some((DELTA, y, _)), "add") => delta(x.checked_add(y)?),
            (Some((DELTA, x, _)), Some((DELTA, y, _)), "sub") => delta(x.checked_sub(y)?),
            (Some((DELTA, x, _)), Some((DELTA, y, _)), "truediv") if y != 0 => Some(key_float(x as f64 / y as f64)),
            (Some((DELTA, x, _)), Some((DELTA, y, _)), "floordiv") if y != 0 => Some(key_int(x.div_euclid(y))),
            (Some((DELTA, x, _)), Some((DELTA, y, _)), "mod") if y != 0 => delta(x.rem_euclid(y)),
            (Some((DELTA, x, _)), None, "mul") => match int(b) {
                Some(i) => delta(x.checked_mul(i)?),
                None => delta(round_half_even(x as f64 * num(b)?)?),
            },
            (None, Some((DELTA, y, _)), "mul") => match int(a) {
                Some(i) => delta(y.checked_mul(i)?),
                None => delta(round_half_even(y as f64 * num(a)?)?),
            },
            (Some((DELTA, x, _)), None, "truediv") => {
                let n = num(b)?;
                if n == 0.0 {
                    return None;
                }
                match int(b) {
                    // exact division rounds half to even
                    Some(i) => delta(div_round_half_even(x, i)),
                    None => delta(round_half_even(x as f64 / n)?),
                }
            }
            (Some((DELTA, x, _)), None, "floordiv") => {
                let i = int(b)?;
                if i == 0 {
                    return None;
                }
                delta(x.div_euclid(i))
            }
            (Some((DATETIME, x, tz)), Some((DELTA, y, _)), "add") | (Some((DELTA, y, _)), Some((DATETIME, x, tz)), "add") => {
                let v = x.checked_add(y)?;
                in_range_dt(v).then(|| chrono_key(DATETIME, v, tz))
            }
            (Some((DATETIME, x, tz)), Some((DELTA, y, _)), "sub") => {
                let v = x.checked_sub(y)?;
                in_range_dt(v).then(|| chrono_key(DATETIME, v, tz))
            }
            (Some((DATETIME, x, t1)), Some((DATETIME, y, t2)), "sub") => {
                if (t1 == NAIVE) != (t2 == NAIVE) {
                    return None;
                }
                delta(instant(x, t1) - instant(y, t2))
            }
            (Some((DATE, x, _)), Some((DELTA, y, _)), "add") | (Some((DELTA, y, _)), Some((DATE, x, _)), "add") => {
                let o = x + y.div_euclid(US_DAY);
                (1..=3_652_059).contains(&o).then(|| chrono_key(DATE, o, NAIVE))
            }
            (Some((DATE, x, _)), Some((DELTA, y, _)), "sub") => {
                let o = x - y.div_euclid(US_DAY);
                (1..=3_652_059).contains(&o).then(|| chrono_key(DATE, o, NAIVE))
            }
            (Some((DATE, x, _)), Some((DATE, y, _)), "sub") => delta((x - y) * US_DAY),
            _ => None,
        }
    })())
}

fn round_half_even(x: f64) -> Option<i64> {
    if !x.is_finite() {
        return None;
    }
    let r = x.round();
    let r = if (x - x.trunc()).abs() == 0.5 && r % 2.0 != 0.0 { r - x.signum() } else { r };
    Some(r as i64)
}

fn div_round_half_even(x: i64, d: i64) -> i64 {
    let q = x.div_euclid(d);
    let r = x.rem_euclid(d);
    let twice = 2 * r;
    let da = d.abs();
    if twice > da || (twice == da && q % 2 != 0) {
        q + 1
    } else {
        q
    }
}

pub fn compare(op: &str, a: KeyId, b: KeyId) -> Option<bool> {
    let (ca, cb) = (as_chrono(a), as_chrono(b));
    let ord = match (ca, cb) {
        (Some((k1, x, t1)), Some((k2, y, t2))) if k1 == k2 => {
            if k1 == DATETIME || k1 == TIME {
                if (t1 == NAIVE) != (t2 == NAIVE) {
                    // naive vs aware: == is False, ordering raises
                    return if matches!(op, "eq" | "ne") { Some(op == "ne") } else { None };
                }
                Some(instant(x, t1).cmp(&instant(y, t2)))
            } else {
                Some(x.cmp(&y))
            }
        }
        // a datetime is never equal to a date (or another kind)
        _ => None,
    };
    match (op, ord) {
        ("eq", Some(o)) => Some(o.is_eq()),
        ("ne", Some(o)) => Some(!o.is_eq()),
        ("eq", None) => Some(false),
        ("ne", None) => Some(true),
        ("lt", Some(o)) => Some(o.is_lt()),
        ("le", Some(o)) => Some(o.is_le()),
        ("gt", Some(o)) => Some(o.is_gt()),
        ("ge", Some(o)) => Some(o.is_ge()),
        _ => None,
    }
}

pub fn truthy(k: KeyId) -> Option<bool> {
    let (kind, v, _) = as_chrono(k)?;
    Some(kind != DELTA || v != 0)
}

// ------------------------------------------------------------ construction

fn kw<'a>(kwargs: &'a [(&str, KeyId)], n: &str) -> Option<KeyId> {
    kwargs.iter().find(|(k, _)| *k == n).map(|(_, v)| *v)
}

fn int_of(k: KeyId) -> Option<i64> {
    intern::with_key(k, |kk| match kk {
        Key::Int(i) => Some(*i),
        _ => None,
    })
}

fn tz_of(k: Option<KeyId>) -> Option<i32> {
    match k {
        None => Some(NAIVE),
        Some(k) if k == intern::key_none() => Some(NAIVE),
        Some(k) => match as_chrono(k)? {
            (TZ, v, _) => Some(v as i32),
            _ => None,
        },
    }
}

/// `date(…)`, `datetime(…)`, `time(…)`, `timedelta(…)`, `timezone(…)`: Some(Ok(key)),
/// Some(Err(())) raises ValueError, None: not simulated.
pub fn construct(class: &str, args: &[KeyId], kwargs: &[(&str, KeyId)]) -> Option<Result<KeyId, ()>> {
    let names: &[&str] = match class {
        "date" => &["year", "month", "day"],
        "datetime" => &["year", "month", "day", "hour", "minute", "second", "microsecond", "tzinfo"],
        "time" => &["hour", "minute", "second", "microsecond", "tzinfo"],
        "timedelta" => &["days", "seconds", "microseconds", "milliseconds", "minutes", "hours", "weeks"],
        "timezone" => &["offset", "name"],
        _ => return None,
    };
    if args.len() > names.len() {
        return None;
    }
    let mut vals: Vec<Option<KeyId>> = vec![None; names.len()];
    for (i, a) in args.iter().enumerate() {
        vals[i] = Some(*a);
    }
    for (k, v) in kwargs {
        let i = names.iter().position(|n| n == k)?;
        vals[i] = Some(*v);
    }
    let get = |i: usize, d: i64| -> Option<i64> {
        match vals[i] {
            None => Some(d),
            Some(k) => int_of(k),
        }
    };
    Some(match class {
        "date" => {
            let (y, m, d) = (get(0, -1)?, get(1, -1)?, get(2, -1)?);
            match ymd_to_ord(y, m, d) {
                Some(o) => Ok(chrono_key(DATE, o, NAIVE)),
                None => Err(()),
            }
        }
        "datetime" => {
            let (y, m, d) = (get(0, -1)?, get(1, -1)?, get(2, -1)?);
            let tz = tz_of(vals[7])?;
            match ymd_to_ord(y, m, d).and_then(|o| dt_value(o, get(3, 0).unwrap_or(-1), get(4, 0).unwrap_or(-1), get(5, 0).unwrap_or(-1), get(6, 0).unwrap_or(-1))) {
                Some(v) => Ok(chrono_key(DATETIME, v, tz)),
                None => {
                    if [3, 4, 5, 6].iter().any(|i| vals[*i].is_some() && get(*i, 0).is_none()) {
                        return None;
                    }
                    Err(())
                }
            }
        }
        "time" => {
            let tz = tz_of(vals[4])?;
            match dt_value(1, get(0, 0)?, get(1, 0)?, get(2, 0)?, get(3, 0)?) {
                Some(v) => Ok(chrono_key(TIME, v, tz)),
                None => Err(()),
            }
        }
        "timedelta" => {
            let mult: [f64; 7] = [86_400e6, 1e6, 1.0, 1e3, 60e6, 3600e6, 7.0 * 86_400e6];
            let mut total_i: i128 = 0;
            let mut total_f: f64 = 0.0;
            let mut any_float = false;
            for (i, v) in vals.iter().enumerate() {
                let Some(k) = v else { continue };
                match intern::key_get(*k) {
                    Key::Int(n) => total_i += n as i128 * mult[i] as i128,
                    Key::Float(b) => {
                        any_float = true;
                        total_f += f64::from_bits(b) * mult[i];
                    }
                    _ => return None,
                }
            }
            let us = if any_float { total_i as f64 + total_f } else { total_i as f64 };
            let us = if any_float { round_half_even(us)? as i128 } else { total_i };
            let us = i64::try_from(us).ok()?;
            match delta(us) {
                Some(k) => Ok(k),
                None => Err(()),
            }
        }
        _ => {
            // timezone(timedelta)
            let off = as_chrono(vals[0]?)?;
            if off.0 != DELTA || vals[1].is_some() {
                return None;
            }
            if off.1 % 1_000_000 != 0 || off.1.abs() >= US_DAY {
                return Some(Err(()));
            }
            Ok(chrono_key(TZ, off.1 / 1_000_000, NAIVE))
        }
    })
}

/// Attributes of a value (`d.year`, `td.days`, `dt.tzinfo`).
pub fn attr(k: KeyId, name: &str) -> Option<Option<KeyId>> {
    let (kind, v, tz) = as_chrono(k)?;
    let p = parts(kind, v);
    let i = |x: i64| Some(Some(key_int(x)));
    match (kind, name) {
        (DATE | DATETIME, "year") => i(p.y),
        (DATE | DATETIME, "month") => i(p.mo),
        (DATE | DATETIME, "day") => i(p.d),
        (DATETIME | TIME, "hour") => i(p.h),
        (DATETIME | TIME, "minute") => i(p.mi),
        (DATETIME | TIME, "second") => i(p.s),
        (DATETIME | TIME, "microsecond") => i(p.us),
        (DATETIME | TIME, "fold") => i(0),
        (DATETIME | TIME, "tzinfo") => Some(Some(if tz == NAIVE { intern::key_none() } else { chrono_key(TZ, tz as i64, NAIVE) })),
        (DELTA, "days") => i(v.div_euclid(US_DAY)),
        (DELTA, "seconds") => i(v.rem_euclid(US_DAY) / 1_000_000),
        (DELTA, "microseconds") => i(v.rem_euclid(1_000_000)),
        _ => None,
    }
}

/// Methods of a value; Some(result) / None not simulated.
pub fn method(k: KeyId, name: &str, args: &[KeyId], kwargs: &[(&str, KeyId)]) -> Option<KeyId> {
    let (kind, v, tz) = as_chrono(k)?;
    let p = parts(kind, v);
    let s = |x: String| intern::key_str_lit(&x);
    match (kind, name) {
        (DATE | DATETIME, "weekday") => Some(key_int(weekday(p.ord))),
        (DATE | DATETIME, "isoweekday") => Some(key_int(weekday(p.ord) + 1)),
        (DATE | DATETIME, "toordinal") => Some(key_int(p.ord)),
        // (`isocalendar`: an `IsoCalendarDate`, a named tuple, whose repr and attributes are
        // not a tuple's: not computed)
        (DATETIME, "date") => Some(chrono_key(DATE, p.ord, NAIVE)),
        (DATETIME, "time") => Some(chrono_key(TIME, v.rem_euclid(US_DAY), NAIVE)),
        (DATETIME, "timetz") => Some(chrono_key(TIME, v.rem_euclid(US_DAY), tz)),
        (DATE | DATETIME | TIME, "isoformat") => {
            let sep = match args.first().copied().or(kw(kwargs, "sep")) {
                Some(x) => intern::key_as_str(x)?.to_string(),
                None => "T".into(),
            };
            let ts = match (if kind == TIME { args.first() } else { args.get(1) }).copied().or(kw(kwargs, "timespec")) {
                Some(x) => intern::key_as_str(x)?.to_string(),
                None => "auto".into(),
            };
            isoformat(k, &sep, &ts).map(s)
        }
        (DATE | DATETIME | TIME, "strftime" | "__format__") => {
            let f = intern::key_as_str(*args.first()?)?;
            if name == "__format__" && f.is_empty() {
                return str_of(k).map(s);
            }
            strftime(k, &f).map(s)
        }
        (DATE | DATETIME, "ctime") => strftime(k, "%c").map(s),
        (DATE | DATETIME | TIME, "replace") => {
            let mut parts = p;
            let mut newtz = tz;
            for (n, x) in kwargs {
                match *n {
                    "tzinfo" => newtz = tz_of(Some(*x))?,
                    _ => {
                        let val = int_of(*x)?;
                        match *n {
                            "year" => parts.y = val,
                            "month" => parts.mo = val,
                            "day" => parts.d = val,
                            "hour" => parts.h = val,
                            "minute" => parts.mi = val,
                            "second" => parts.s = val,
                            "microsecond" => parts.us = val,
                            _ => return None,
                        }
                    }
                }
            }
            if !args.is_empty() {
                return None;
            }
            match kind {
                DATE => Some(chrono_key(DATE, ymd_to_ord(parts.y, parts.mo, parts.d)?, NAIVE)),
                DATETIME => Some(chrono_key(DATETIME, dt_value(ymd_to_ord(parts.y, parts.mo, parts.d)?, parts.h, parts.mi, parts.s, parts.us)?, newtz)),
                _ => Some(chrono_key(TIME, dt_value(1, parts.h, parts.mi, parts.s, parts.us)?, newtz)),
            }
        }
        (DATETIME, "utcoffset") | (TIME, "utcoffset") => Some(if tz == NAIVE { intern::key_none() } else { delta(tz as i64 * 1_000_000)? }),
        (DATETIME, "timestamp") if tz != NAIVE => {
            let epoch = (ymd_to_ord(1970, 1, 1)? - 1) * US_DAY;
            Some(key_float((instant(v, tz) - epoch) as f64 / 1e6))
        }
        (DATETIME, "astimezone") if tz != NAIVE => {
            let to = tz_of(args.first().copied())?;
            if to == NAIVE {
                return None;
            }
            let u = instant(v, tz) + to as i64 * 1_000_000;
            in_range_dt(u).then(|| chrono_key(DATETIME, u, to))
        }
        (DELTA, "total_seconds") => Some(key_float(v as f64 / 1e6)),
        (TZ, "utcoffset") => delta(v * 1_000_000),
        (TZ, "tzname") => Some(s(tz_name(v as i32))),
        _ => None,
    }
}

/// Class-level calls: `datetime.fromisoformat(…)`, `date.fromordinal(…)`, `datetime.strptime(…)`,
/// `datetime.combine(d, t)`; the clock ones are None (unknown).
pub fn class_method(class: &str, name: &str, args: &[KeyId]) -> Option<Result<KeyId, ()>> {
    let kind = match class {
        "date" => DATE,
        "datetime" => DATETIME,
        "time" => TIME,
        _ => return None,
    };
    Some(match name {
        "fromisoformat" => match fromisoformat(kind, &intern::key_as_str(*args.first()?)?) {
            Some(k) => Ok(k),
            None => Err(()),
        },
        "strptime" if kind == DATETIME => {
            let (t, f) = (intern::key_as_str(*args.first()?)?, intern::key_as_str(*args.get(1)?)?);
            match strptime(&t, &f) {
                Some(k) => Ok(k),
                None => {
                    // a directive not simulated: unknown; a mismatch: ValueError
                    if f.contains("%c") || f.contains("%x") || f.contains("%X") || f.contains("%U") || f.contains("%W") || f.contains("%G") || f.contains("%V") {
                        return None;
                    }
                    Err(())
                }
            }
        }
        "fromordinal" => {
            let o = int_of(*args.first()?)?;
            if !(1..=3_652_059).contains(&o) {
                return Some(Err(()));
            }
            Ok(if kind == DATE { chrono_key(DATE, o, NAIVE) } else { chrono_key(DATETIME, (o - 1) * US_DAY, NAIVE) })
        }
        "combine" if kind == DATETIME => {
            let (d, t) = (as_chrono(*args.first()?)?, as_chrono(*args.get(1)?)?);
            let o = match d.0 {
                DATE => d.1,
                DATETIME => d.1.div_euclid(US_DAY) + 1,
                _ => return None,
            };
            if t.0 != TIME {
                return None;
            }
            Ok(chrono_key(DATETIME, (o - 1) * US_DAY + t.1, t.2))
        }
        _ => return None,
    })
}

/// Class constants (`timezone.utc`, `date.min`, `timedelta.max`, …).
pub fn constant(qual: &str) -> Option<KeyId> {
    Some(match qual {
        "datetime.timezone.utc" | "datetime.UTC" => chrono_key(TZ, 0, NAIVE),
        "datetime.date.min" => chrono_key(DATE, 1, NAIVE),
        "datetime.date.max" => chrono_key(DATE, 3_652_059, NAIVE),
        "datetime.datetime.min" => chrono_key(DATETIME, 0, NAIVE),
        "datetime.datetime.max" => chrono_key(DATETIME, 3_652_059 * US_DAY - 1, NAIVE),
        "datetime.timedelta.resolution" => chrono_key(DELTA, 1, NAIVE),
        "datetime.MINYEAR" => key_int(1),
        "datetime.MAXYEAR" => key_int(9999),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn datetime_like_cpython() {
        // expected values from CPython 3.14
        let d = construct("date", &[key_int(2024), key_int(2), key_int(28)], &[]).unwrap().unwrap();
        let two = construct("timedelta", &[], &[("days", key_int(2))]).unwrap().unwrap();
        let d2 = binop("add", d, two).unwrap().unwrap();
        assert_eq!(str_of(d2).unwrap(), "2024-03-01");
        assert_eq!(method(d2, "weekday", &[], &[]), Some(key_int(4)));
        assert_eq!(strftime(d2, "%a %d %b %Y").unwrap(), "Fri 01 Mar 2024");
        assert_eq!(str_of(construct("timedelta", &[], &[("seconds", key_int(-1))]).unwrap().unwrap()).unwrap(), "-1 day, 23:59:59");
        assert_eq!(str_of(construct("timedelta", &[], &[("hours", key_int(25)), ("minutes", key_int(1))]).unwrap().unwrap()).unwrap(), "1 day, 1:01:00");
        assert_eq!(str_of(construct("timedelta", &[], &[("days", key_int(-3)), ("microseconds", key_int(5))]).unwrap().unwrap()).unwrap(), "-3 days, 0:00:00.000005");
        let dt = fromisoformat(DATETIME, "2024-03-01T09:00:00").unwrap();
        assert_eq!(str_of(dt).unwrap(), "2024-03-01 09:00:00");
        assert_eq!(str_of(fromisoformat(DATETIME, "2024-03-01T09:00:00.5+02:00").unwrap()).unwrap(), "2024-03-01 09:00:00.500000+02:00");
        assert_eq!(repr_of(fromisoformat(DATETIME, "2024-01-02T03:04").unwrap()).unwrap(), "datetime.datetime(2024, 1, 2, 3, 4)");
        assert_eq!(str_of(strptime("05/06/2023 14:30", "%d/%m/%Y %H:%M").unwrap()).unwrap(), "2023-06-05 14:30:00");
        assert_eq!(method(d2, "isocalendar", &[], &[]), None);
        assert_eq!(ord_to_ymd(ymd_to_ord(2000, 12, 31).unwrap()), (2000, 12, 31));
        assert_eq!(ord_to_ymd(ymd_to_ord(1900, 3, 1).unwrap()), (1900, 3, 1));
        assert!(construct("date", &[key_int(2023), key_int(2), key_int(29)], &[]).unwrap().is_err());
    }
}
