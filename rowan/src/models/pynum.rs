//! `decimal.Decimal` and `fractions.Fraction` as literal values (`Key::Dec`, `Key::Frac`),
//! computed exactly as CPython does in the default context (28 digits, ROUND_HALF_EVEN):
//! construction, arithmetic, comparisons, `str` / `repr` / `int` / `float` / `round` / `abs`,
//! `quantize` and the common methods. A result outside what is simulated (a coefficient past
//! 38 digits, a changed context, NaN / Infinity) is None: the caller keeps it unknown.

use std::cell::Cell;
use std::cmp::Ordering;

use crate::intern::{self, key, key_bool, key_float, key_int, Key, KeyId};

const PREC: u32 = 28;

thread_local! {
    /// `getcontext()` / `localcontext()` / `setcontext()` was called: the context may differ
    static OPAQUE: Cell<bool> = const { Cell::new(false) };
}

pub fn set_opaque(v: bool) {
    OPAQUE.with(|o| o.set(v));
}

fn opaque() -> bool {
    OPAQUE.with(|o| o.get())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dec {
    pub neg: bool,
    pub coef: u128,
    pub exp: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rounding {
    HalfEven,
    HalfUp,
    HalfDown,
    Up,
    Down,
    Ceiling,
    Floor,
    Up05,
}

impl Rounding {
    pub fn from_name(s: &str) -> Option<Rounding> {
        Some(match s {
            "ROUND_HALF_EVEN" => Rounding::HalfEven,
            "ROUND_HALF_UP" => Rounding::HalfUp,
            "ROUND_HALF_DOWN" => Rounding::HalfDown,
            "ROUND_UP" => Rounding::Up,
            "ROUND_DOWN" => Rounding::Down,
            "ROUND_CEILING" => Rounding::Ceiling,
            "ROUND_FLOOR" => Rounding::Floor,
            "ROUND_05UP" => Rounding::Up05,
            _ => return None,
        })
    }
}

fn pow10(n: u32) -> Option<u128> {
    10u128.checked_pow(n)
}

fn ndigits(c: u128) -> u32 {
    if c == 0 {
        1
    } else {
        c.ilog10() + 1
    }
}

/// Drop the last `k` digits of `coef` with rounding (sign `neg`).
fn shed(coef: u128, k: u32, neg: bool, mode: Rounding) -> Option<u128> {
    if k == 0 {
        return Some(coef);
    }
    if k > 38 {
        // everything goes: the rest is below one unit
        let up = coef != 0 && match mode {
            Rounding::Up => true,
            Rounding::Ceiling => !neg,
            Rounding::Floor => neg,
            _ => false,
        };
        return Some(if up { 1 } else { 0 });
    }
    let p = pow10(k)?;
    let q = coef / p;
    let r = coef % p;
    let half = p / 2;
    let up = match mode {
        Rounding::Down => false,
        Rounding::Up => r != 0,
        Rounding::Ceiling => r != 0 && !neg,
        Rounding::Floor => r != 0 && neg,
        Rounding::HalfUp => r >= half,
        Rounding::HalfDown => r > half,
        Rounding::HalfEven => r > half || (r == half && q % 2 == 1),
        Rounding::Up05 => r != 0 && (q % 10 == 0 || q % 10 == 5),
    };
    Some(if up { q + 1 } else { q })
}

impl Dec {
    pub fn from_int(i: i128) -> Dec {
        Dec { neg: i < 0, coef: i.unsigned_abs(), exp: 0 }
    }

    pub fn zero() -> Dec {
        Dec { neg: false, coef: 0, exp: 0 }
    }

    pub fn parse(s: &str) -> Option<Dec> {
        let t: String = s.trim().chars().filter(|c| *c != '_').collect();
        let (neg, body) = match t.strip_prefix('-') {
            Some(r) => (true, r),
            None => (false, t.strip_prefix('+').unwrap_or(&t)),
        };
        let (mant, ex) = match body.find(['e', 'E']) {
            Some(i) => (&body[..i], body[i + 1..].parse::<i32>().ok()?),
            None => (body, 0),
        };
        let (ip, fp) = match mant.split_once('.') {
            Some((a, b)) => (a, b),
            None => (mant, ""),
        };
        if (ip.is_empty() && fp.is_empty()) || !ip.chars().all(|c| c.is_ascii_digit()) || !fp.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        let digits = format!("{}{}", ip, fp);
        let digits = digits.trim_start_matches('0');
        if digits.len() > 38 {
            return None;
        }
        let coef = if digits.is_empty() { 0 } else { digits.parse::<u128>().ok()? };
        Some(Dec { neg, coef, exp: ex.checked_sub(fp.len() as i32)? })
    }

    /// The exact value of a float (when its digits fit).
    pub fn from_f64(f: f64) -> Option<Dec> {
        if !f.is_finite() {
            return None;
        }
        let neg = f.is_sign_negative();
        let a = f.abs();
        if a == 0.0 {
            return Some(Dec { neg, coef: 0, exp: 0 });
        }
        let bits = a.to_bits();
        let e = ((bits >> 52) & 0x7ff) as i32;
        let m = bits & ((1u64 << 52) - 1);
        let (mut mant, mut e2) = if e == 0 { (m as u128, -1074) } else { ((m | (1u64 << 52)) as u128, e - 1075) };
        while mant % 2 == 0 && e2 < 0 {
            mant /= 2;
            e2 += 1;
        }
        if e2 >= 0 {
            let c = mant.checked_mul(2u128.checked_pow(e2 as u32)?)?;
            return Some(Dec { neg, coef: c, exp: 0 });
        }
        let k = (-e2) as u32;
        let c = mant.checked_mul(5u128.checked_pow(k)?)?;
        Some(Dec { neg, coef: c, exp: -(k as i32) })
    }

    pub fn is_zero(&self) -> bool {
        self.coef == 0
    }

    /// Round to the context precision.
    fn fit(self, mode: Rounding) -> Option<Dec> {
        let n = ndigits(self.coef);
        if n <= PREC {
            return Some(self);
        }
        let k = n - PREC;
        let mut c = shed(self.coef, k, self.neg, mode)?;
        let mut e = self.exp + k as i32;
        if ndigits(c) > PREC {
            c /= 10;
            e += 1;
        }
        Some(Dec { neg: self.neg, coef: c, exp: e })
    }

    fn aligned(a: Dec, b: Dec) -> Option<(i128, i128, i32)> {
        let e = a.exp.min(b.exp);
        let ca = a.coef.checked_mul(pow10((a.exp - e) as u32)?)?;
        let cb = b.coef.checked_mul(pow10((b.exp - e) as u32)?)?;
        let sa = i128::try_from(ca).ok()?;
        let sb = i128::try_from(cb).ok()?;
        Some((if a.neg { -sa } else { sa }, if b.neg { -sb } else { sb }, e))
    }

    pub fn add(a: Dec, b: Dec) -> Option<Dec> {
        let (x, y, e) = Dec::aligned(a, b)?;
        let s = x.checked_add(y)?;
        let neg = if s == 0 { a.neg && b.neg } else { s < 0 };
        Dec { neg, coef: s.unsigned_abs(), exp: e }.fit(Rounding::HalfEven)
    }

    pub fn neg(self) -> Dec {
        Dec { neg: !self.neg, ..self }
    }

    pub fn mul(a: Dec, b: Dec) -> Option<Dec> {
        let c = a.coef.checked_mul(b.coef)?;
        Dec { neg: a.neg != b.neg, coef: c, exp: a.exp.checked_add(b.exp)? }.fit(Rounding::HalfEven)
    }

    pub fn div(a: Dec, b: Dec) -> Option<Dec> {
        if b.is_zero() {
            return None;
        }
        let neg = a.neg != b.neg;
        let ideal = a.exp - b.exp;
        if a.is_zero() {
            return Some(Dec { neg, coef: 0, exp: ideal });
        }
        // enough digits for the precision and a rounding digit
        let shift = (PREC as i32 + ndigits(b.coef) as i32 - ndigits(a.coef) as i32 + 1).max(0) as u32;
        let num = a.coef.checked_mul(pow10(shift)?)?;
        let q = num / b.coef;
        let r = num % b.coef;
        let mut e = ideal - shift as i32;
        if r == 0 {
            // exact: back towards the ideal exponent
            let mut c = q;
            while e < ideal && c % 10 == 0 && c != 0 {
                c /= 10;
                e += 1;
            }
            return Dec { neg, coef: c, exp: e }.fit(Rounding::HalfEven);
        }
        // inexact: q with a sticky digit, rounded to the precision
        let c = q.checked_mul(10)?.checked_add(1)?;
        e -= 1;
        Dec { neg, coef: c, exp: e }.fit(Rounding::HalfEven)
    }

    pub fn cmp(a: Dec, b: Dec) -> Option<Ordering> {
        if a.is_zero() && b.is_zero() {
            return Some(Ordering::Equal);
        }
        let (x, y, _) = Dec::aligned(a, b).or_else(|| {
            // very different exponents: the signs and magnitudes decide
            None
        })?;
        Some(x.cmp(&y))
    }

    /// Rescale to exponent `exp` (quantize); None where the result needs more than 28 digits.
    pub fn rescale(self, exp: i32, mode: Rounding) -> Option<Dec> {
        let d = if exp < self.exp {
            let c = self.coef.checked_mul(pow10((self.exp - exp) as u32)?)?;
            Dec { neg: self.neg, coef: c, exp }
        } else {
            let k = (exp - self.exp) as u32;
            Dec { neg: self.neg, coef: shed(self.coef, k, self.neg, mode)?, exp }
        };
        if ndigits(d.coef) > PREC {
            return None;
        }
        Some(d)
    }

    pub fn normalize(self) -> Option<Dec> {
        let mut d = self.fit(Rounding::HalfEven)?;
        if d.coef == 0 {
            return Some(Dec { neg: d.neg, coef: 0, exp: 0 });
        }
        while d.coef % 10 == 0 {
            d.coef /= 10;
            d.exp += 1;
        }
        Some(d)
    }

    pub fn to_int(self) -> Option<i128> {
        let mag = if self.exp >= 0 {
            self.coef.checked_mul(pow10(self.exp as u32)?)?
        } else {
            let k = (-self.exp) as u32;
            if k > 38 {
                0
            } else {
                self.coef / pow10(k)?
            }
        };
        let m = i128::try_from(mag).ok()?;
        Some(if self.neg { -m } else { m })
    }

    pub fn to_f64(self) -> Option<f64> {
        let s = format!("{}{}e{}", if self.neg { "-" } else { "" }, self.coef, self.exp);
        s.parse::<f64>().ok()
    }

    /// `str(d)`: the to-scientific-string conversion.
    pub fn to_str(self) -> String {
        let d = self.coef.to_string();
        let n = d.len() as i32;
        let adjusted = self.exp + n - 1;
        let sign = if self.neg { "-" } else { "" };
        if self.exp <= 0 && adjusted >= -6 {
            if self.exp == 0 {
                return format!("{}{}", sign, d);
            }
            let point = n + self.exp;
            if point > 0 {
                return format!("{}{}.{}", sign, &d[..point as usize], &d[point as usize..]);
            }
            return format!("{}0.{}{}", sign, "0".repeat((-point) as usize), d);
        }
        let mut out = format!("{}{}", sign, &d[..1]);
        if n > 1 {
            out.push('.');
            out.push_str(&d[1..]);
        }
        out.push('E');
        if adjusted >= 0 {
            out.push('+');
        }
        out.push_str(&adjusted.to_string());
        out
    }

    /// `d.as_integer_ratio()`.
    pub fn ratio(self) -> Option<(i128, i128)> {
        let (n, d) = if self.exp >= 0 {
            (self.coef.checked_mul(pow10(self.exp as u32)?)?, 1u128)
        } else {
            (self.coef, pow10((-self.exp) as u32)?)
        };
        let g = gcd(n, d).max(1);
        let n = i128::try_from(n / g).ok()?;
        let d = i128::try_from(d / g).ok()?;
        Some((if self.neg { -n } else { n }, d))
    }
}

fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

// ----------------------------------------------------------------------- fractions

pub fn frac(n: i128, d: i128) -> Option<(i128, i128)> {
    if d == 0 {
        return None;
    }
    let g = gcd(n.unsigned_abs(), d.unsigned_abs()) as i128;
    let g = if g == 0 { 1 } else { g };
    let (mut n, mut d) = (n / g, d / g);
    if d < 0 {
        n = n.checked_neg()?;
        d = d.checked_neg()?;
    }
    Some((n, d))
}

fn frac_add(a: (i128, i128), b: (i128, i128)) -> Option<(i128, i128)> {
    frac(a.0.checked_mul(b.1)?.checked_add(b.0.checked_mul(a.1)?)?, a.1.checked_mul(b.1)?)
}

fn frac_mul(a: (i128, i128), b: (i128, i128)) -> Option<(i128, i128)> {
    frac(a.0.checked_mul(b.0)?, a.1.checked_mul(b.1)?)
}

fn frac_div(a: (i128, i128), b: (i128, i128)) -> Option<(i128, i128)> {
    if b.0 == 0 {
        return None;
    }
    frac(a.0.checked_mul(b.1)?, a.1.checked_mul(b.0)?)
}

fn frac_cmp(a: (i128, i128), b: (i128, i128)) -> Option<Ordering> {
    Some(a.0.checked_mul(b.1)?.cmp(&b.0.checked_mul(a.1)?))
}

fn floor_div(n: i128, d: i128) -> i128 {
    let q = n / d;
    if (n % d != 0) && ((n < 0) != (d < 0)) {
        q - 1
    } else {
        q
    }
}

fn frac_to_f64(f: (i128, i128)) -> f64 {
    let (n, d) = f;
    if n.unsigned_abs() < (1u128 << 53) && d.unsigned_abs() < (1u128 << 53) {
        return n as f64 / d as f64;
    }
    // correctly rounded through a decimal string of enough digits
    let q = Dec::div(Dec::from_int(n), Dec::from_int(d)).and_then(|x| x.to_f64());
    q.unwrap_or(n as f64 / d as f64)
}

fn frac_str(f: (i128, i128)) -> String {
    if f.1 == 1 {
        f.0.to_string()
    } else {
        format!("{}/{}", f.0, f.1)
    }
}

/// The exact fraction of a float.
fn f64_frac(x: f64) -> Option<(i128, i128)> {
    if !x.is_finite() {
        return None;
    }
    let d = Dec::from_f64(x)?;
    d.ratio()
}

pub fn frac_parse(s: &str) -> Option<(i128, i128)> {
    let t = s.trim();
    if let Some((a, b)) = t.split_once('/') {
        let n: i128 = a.trim().parse().ok()?;
        let d: i128 = b.trim().parse().ok()?;
        return frac(n, d);
    }
    Dec::parse(t)?.ratio()
}

/// `Fraction.limit_denominator(max)`.
fn limit_denominator(f: (i128, i128), max: i128) -> Option<(i128, i128)> {
    if max < 1 {
        return None;
    }
    if f.1 <= max {
        return Some(f);
    }
    let (mut p0, mut q0, mut p1, mut q1) = (0i128, 1i128, 1i128, 0i128);
    let (mut n, mut d) = f;
    loop {
        let a = floor_div(n, d);
        let q2 = q0 + a * q1;
        if q2 > max {
            break;
        }
        let (np0, nq0, np1, nq1) = (p1, q1, p0 + a * p1, q2);
        p0 = np0;
        q0 = nq0;
        p1 = np1;
        q1 = nq1;
        let nn = d;
        d = n - a * d;
        n = nn;
        if d == 0 {
            break;
        }
    }
    let k = (max - q0) / q1;
    let b1 = frac(p0 + k * p1, q0 + k * q1)?;
    let b2 = frac(p1, q1)?;
    let d1 = frac_add(b1, (-f.0, f.1))?;
    let d2 = frac_add(b2, (-f.0, f.1))?;
    let abs = |x: (i128, i128)| (x.0.abs(), x.1);
    Some(if frac_cmp(abs(d2), abs(d1))? != Ordering::Greater { b2 } else { b1 })
}

// --------------------------------------------------------------------- keys

pub fn dec_key(d: Dec) -> KeyId {
    key(Key::Dec(d.neg, d.coef, d.exp))
}

pub fn frac_key(f: (i128, i128)) -> KeyId {
    key(Key::Frac(f.0, f.1))
}

/// Attributes and methods of Decimal / Fraction values.
pub const NAMES: &[&str] = &["quantize", "to_integral_value", "to_integral", "normalize", "copy_abs", "copy_negate", "is_zero", "is_signed", "adjusted", "as_integer_ratio", "scaleb", "numerator", "denominator", "limit_denominator", "is_integer"];

pub fn as_dec(k: KeyId) -> Option<Dec> {
    intern::with_key(k, |kk| match kk {
        Key::Dec(n, c, e) => Some(Dec { neg: *n, coef: *c, exp: *e }),
        _ => None,
    })
}

pub fn as_frac(k: KeyId) -> Option<(i128, i128)> {
    intern::with_key(k, |kk| match kk {
        Key::Frac(n, d) => Some((*n, *d)),
        _ => None,
    })
}

pub fn is_num_key(k: KeyId) -> bool {
    intern::with_key(k, |kk| matches!(kk, Key::Dec(..) | Key::Frac(..)))
}

enum Op {
    I(i128),
    F(f64),
    D(Dec),
    Q((i128, i128)),
}

fn operand(k: KeyId) -> Option<Op> {
    intern::with_key(k, |kk| match kk {
        Key::Int(i) => Some(Op::I(*i as i128)),
        Key::Bool(b) => Some(Op::I(*b as i128)),
        Key::Float(b) => Some(Op::F(f64::from_bits(*b))),
        Key::Dec(n, c, e) => Some(Op::D(Dec { neg: *n, coef: *c, exp: *e })),
        Key::Frac(n, d) => Some(Op::Q((*n, *d))),
        _ => None,
    })
}

fn int_key(i: i128) -> Option<KeyId> {
    Some(key_int(i64::try_from(i).ok()?))
}

/// `a OP b` when a Decimal / Fraction is involved: Some(result) / Some(None) unknown (or the
/// operation raises); None: not ours.
pub fn binop(op: &str, a: KeyId, b: KeyId) -> Option<Option<KeyId>> {
    if !is_num_key(a) && !is_num_key(b) {
        return None;
    }
    if matches!(op, "eq" | "ne" | "lt" | "le" | "gt" | "ge") {
        return Some(compare(op, a, b).map(key_bool));
    }
    let (Some(x), Some(y)) = (operand(a), operand(b)) else { return Some(None) };
    Some(arith(op, x, y))
}

fn arith(op: &str, x: Op, y: Op) -> Option<KeyId> {
    match (x, y) {
        (Op::D(_), Op::F(_)) | (Op::F(_), Op::D(_)) | (Op::D(_), Op::Q(_)) | (Op::Q(_), Op::D(_)) => None,
        (Op::D(a), Op::I(i)) => dec_op(op, a, Dec::from_int(i)),
        (Op::I(i), Op::D(b)) => dec_op(op, Dec::from_int(i), b),
        (Op::D(a), Op::D(b)) => dec_op(op, a, b),
        (Op::Q(a), Op::I(i)) => frac_op(op, a, (i, 1)),
        (Op::I(i), Op::Q(b)) => frac_op(op, (i, 1), b),
        (Op::Q(a), Op::Q(b)) => frac_op(op, a, b),
        (Op::Q(a), Op::F(f)) => float_op(op, frac_to_f64(a), f),
        (Op::F(f), Op::Q(b)) => float_op(op, f, frac_to_f64(b)),
        _ => None,
    }
}

fn float_op(op: &str, a: f64, b: f64) -> Option<KeyId> {
    let r = match op {
        "add" => a + b,
        "sub" => a - b,
        "mul" => a * b,
        "truediv" if b != 0.0 => a / b,
        _ => return None,
    };
    Some(key_float(r))
}

fn dec_op(op: &str, a: Dec, b: Dec) -> Option<KeyId> {
    if opaque() {
        return None;
    }
    let r = match op {
        "add" => Dec::add(a, b)?,
        "sub" => Dec::add(a, b.neg())?,
        "mul" => Dec::mul(a, b)?,
        "truediv" => Dec::div(a, b)?,
        "pow" => {
            // a small non-negative integral power, by repeated multiplication (exact results only)
            let n = b.to_int().filter(|n| b.exp >= 0 && (0..=64).contains(n))?;
            let mut acc = Dec::from_int(1);
            for _ in 0..n {
                let c = acc.coef.checked_mul(a.coef)?;
                if ndigits(c) > PREC {
                    return None;
                }
                acc = Dec { neg: acc.neg != a.neg, coef: c, exp: acc.exp + a.exp };
            }
            acc
        }
        _ => return None,
    };
    Some(dec_key(r))
}

fn frac_op(op: &str, a: (i128, i128), b: (i128, i128)) -> Option<KeyId> {
    let r = match op {
        "add" => frac_add(a, b)?,
        "sub" => frac_add(a, (b.0.checked_neg()?, b.1))?,
        "mul" => frac_mul(a, b)?,
        "truediv" => frac_div(a, b)?,
        "floordiv" => {
            let q = frac_div(a, b)?;
            return int_key(floor_div(q.0, q.1));
        }
        "mod" => {
            let q = frac_div(a, b)?;
            let fl = floor_div(q.0, q.1);
            frac_add(a, frac_mul(b, (fl.checked_neg()?, 1))?)?
        }
        "pow" if b.1 == 1 => {
            let n = b.0;
            if !(-64..=64).contains(&n) {
                return None;
            }
            let mut acc = (1i128, 1i128);
            for _ in 0..n.unsigned_abs() {
                acc = frac_mul(acc, a)?;
            }
            if n < 0 {
                frac_div((1, 1), acc)?
            } else {
                acc
            }
        }
        "pow" => return Some(key_float(frac_to_f64(a).powf(frac_to_f64(b)))),
        _ => return None,
    };
    Some(frac_key(r))
}

/// Comparisons involving a Decimal / Fraction (exact, as Python compares them).
pub fn compare(op: &str, a: KeyId, b: KeyId) -> Option<bool> {
    let ord = order(a, b);
    match op {
        "eq" | "ne" => {
            let eq = match ord {
                Some(o) => o == Ordering::Equal,
                // a number never equals a non-number
                None if operand(a).is_none() || operand(b).is_none() => false,
                None => return None,
            };
            Some(if op == "eq" { eq } else { !eq })
        }
        _ => {
            let o = ord?;
            Some(match op {
                "lt" => o == Ordering::Less,
                "le" => o != Ordering::Greater,
                "gt" => o == Ordering::Greater,
                _ => o != Ordering::Less,
            })
        }
    }
}

pub fn order(a: KeyId, b: KeyId) -> Option<Ordering> {
    let as_q = |o: Op| -> Option<(i128, i128)> {
        match o {
            Op::I(i) => Some((i, 1)),
            Op::F(f) => f64_frac(f),
            Op::D(d) => d.ratio(),
            Op::Q(q) => Some(q),
        }
    };
    match (operand(a)?, operand(b)?) {
        (Op::D(x), Op::D(y)) => Dec::cmp(x, y),
        (x, y) => frac_cmp(as_q(x)?, as_q(y)?),
    }
}

pub fn truthy(k: KeyId) -> Option<bool> {
    if let Some(d) = as_dec(k) {
        return Some(!d.is_zero());
    }
    as_frac(k).map(|f| f.0 != 0)
}

pub fn str_of(k: KeyId) -> Option<String> {
    if let Some(d) = as_dec(k) {
        return Some(d.to_str());
    }
    as_frac(k).map(frac_str)
}

pub fn repr_of(k: KeyId) -> Option<String> {
    if let Some(d) = as_dec(k) {
        return Some(format!("Decimal('{}')", d.to_str()));
    }
    as_frac(k).map(|f| format!("Fraction({}, {})", f.0, f.1))
}

/// Builtins on a Decimal / Fraction argument (`str`, `repr`, `int`, `float`, `abs`, `round`,
/// `bool`): Some(result) / Some(None) unknown; None: not ours.
pub fn builtin(name: &str, args: &[KeyId]) -> Option<Option<KeyId>> {
    let a = *args.first()?;
    if !is_num_key(a) && !(name == "round" && args.len() == 2 && is_num_key(args[0])) {
        return None;
    }
    let lit = |s: String| intern::key_str_lit(&s);
    Some(match name {
        "str" => str_of(a).map(lit),
        "repr" | "ascii" => repr_of(a).map(lit),
        "bool" => truthy(a).map(key_bool),
        "float" => as_dec(a).and_then(|d| d.to_f64()).or_else(|| as_frac(a).map(frac_to_f64)).map(key_float),
        "int" => as_dec(a).and_then(|d| d.to_int()).or_else(|| as_frac(a).map(|f| f.0 / f.1)).and_then(int_key),
        "abs" => {
            if let Some(d) = as_dec(a) {
                Some(dec_key(Dec { neg: false, ..d }))
            } else {
                as_frac(a).map(|f| frac_key((f.0.abs(), f.1)))
            }
        }
        "round" => {
            let nd = args.get(1).and_then(|k| intern::key_as_int(*k));
            if let Some(d) = as_dec(a) {
                match nd {
                    None if args.len() == 1 => d.rescale(0, Rounding::HalfEven).and_then(|r| r.to_int()).and_then(int_key),
                    Some(n) => d.rescale(-(n as i32), Rounding::HalfEven).map(dec_key),
                    None => None,
                }
            } else {
                let f = as_frac(a)?;
                match nd {
                    None if args.len() == 1 => {
                        // half to even
                        let fl = floor_div(f.0, f.1);
                        let rem = frac_add(f, (-fl, 1))?;
                        let r = match frac_cmp(rem, (1, 2))? {
                            Ordering::Less => fl,
                            Ordering::Greater => fl + 1,
                            Ordering::Equal => {
                                if fl % 2 == 0 {
                                    fl
                                } else {
                                    fl + 1
                                }
                            }
                        };
                        int_key(r)
                    }
                    _ => None,
                }
            }
        }
        _ => None,
    })
}

/// `Decimal(x)`.
pub fn decimal_new(arg: Option<KeyId>) -> Option<KeyId> {
    let Some(a) = arg else { return Some(dec_key(Dec::zero())) };
    if let Some(d) = as_dec(a) {
        return Some(dec_key(d));
    }
    let d = match intern::key_get(a) {
        Key::Int(i) => Some(Dec::from_int(i as i128)),
        Key::Bool(b) => Some(Dec::from_int(b as i128)),
        Key::Float(b) => Dec::from_f64(f64::from_bits(b)),
        Key::Str(s) => Dec::parse(&s),
        _ => None,
    };
    d.map(dec_key)
}

/// `Fraction(a, b)`.
pub fn fraction_new(a: Option<KeyId>, b: Option<KeyId>) -> Option<KeyId> {
    let Some(a) = a else { return Some(frac_key((0, 1))) };
    let q = |k: KeyId| -> Option<(i128, i128)> {
        intern::with_key(k, |kk| match kk {
            Key::Int(i) => Some((*i as i128, 1)),
            Key::Bool(b) => Some((*b as i128, 1)),
            Key::Float(b) => f64_frac(f64::from_bits(*b)),
            Key::Str(s) => frac_parse(s),
            Key::Dec(n, c, e) => Dec { neg: *n, coef: *c, exp: *e }.ratio(),
            Key::Frac(n, d) => Some((*n, *d)),
            _ => None,
        })
    };
    let x = q(a)?;
    match b {
        None => Some(frac_key(x)),
        Some(b) => {
            let y = q(b)?;
            frac_div(x, y).map(frac_key)
        }
    }
}

/// Methods / attributes of a Decimal or Fraction literal: Some(result) / Some(None) unknown.
pub fn method(recv: KeyId, name: &str, args: &[KeyId], kwargs: &[(&str, KeyId)]) -> Option<KeyId> {
    let kw = |n: &str| kwargs.iter().find(|(k, _)| *k == n).map(|(_, v)| *v);
    let rounding = |k: Option<KeyId>| -> Option<Rounding> {
        match k {
            None => Some(Rounding::HalfEven),
            Some(k) if k == intern::key_none() => Some(Rounding::HalfEven),
            Some(k) => Rounding::from_name(&intern::key_as_str(k)?),
        }
    };
    if let Some(d) = as_dec(recv) {
        if opaque() {
            return None;
        }
        return match name {
            "quantize" => {
                let e = as_dec(*args.first().or(kw("exp").as_ref())?)?.exp;
                let r = rounding(args.get(1).copied().or(kw("rounding")))?;
                d.rescale(e, r).map(dec_key)
            }
            "to_integral_value" | "to_integral" => {
                let r = rounding(args.first().copied().or(kw("rounding")))?;
                if d.exp >= 0 {
                    Some(dec_key(d))
                } else {
                    d.rescale(0, r).map(dec_key)
                }
            }
            "normalize" => d.normalize().map(dec_key),
            "copy_abs" => Some(dec_key(Dec { neg: false, ..d })),
            "copy_negate" => Some(dec_key(d.neg())),
            "is_zero" => Some(key_bool(d.is_zero())),
            "is_signed" => Some(key_bool(d.neg)),
            "is_finite" => Some(key_bool(true)),
            "is_nan" | "is_infinite" => Some(key_bool(false)),
            "adjusted" => int_key((d.exp + ndigits(d.coef) as i32 - 1) as i128),
            "as_integer_ratio" => {
                let (n, q) = d.ratio()?;
                Some(intern::key_tuple(vec![int_key(n)?, int_key(q)?]))
            }
            "compare" => {
                let o = order(recv, *args.first()?)?;
                Some(dec_key(Dec::from_int(match o {
                    Ordering::Less => -1,
                    Ordering::Equal => 0,
                    Ordering::Greater => 1,
                })))
            }
            "scaleb" => {
                let n = intern::key_as_int(*args.first()?)?;
                Dec { exp: d.exp.checked_add(i32::try_from(n).ok()?)?, ..d }.fit(Rounding::HalfEven).map(dec_key)
            }
            _ => None,
        };
    }
    let f = as_frac(recv)?;
    match name {
        "numerator" => int_key(f.0),
        "denominator" => int_key(f.1),
        "limit_denominator" => {
            let m = match args.first().copied().or(kw("max_denominator")) {
                Some(k) => intern::key_as_int(k)? as i128,
                None => 1_000_000,
            };
            limit_denominator(f, m).map(frac_key)
        }
        "as_integer_ratio" => Some(intern::key_tuple(vec![int_key(f.0)?, int_key(f.1)?])),
        "is_integer" => Some(key_bool(f.1 == 1)),
        _ => None,
    }
}

pub fn attr(recv: KeyId, name: &str) -> Option<Option<KeyId>> {
    if !is_num_key(recv) {
        return None;
    }
    match name {
        "numerator" | "denominator" if as_frac(recv).is_some() => Some(method(recv, name, &[], &[])),
        "real" => Some(Some(recv)),
        "imag" => Some(if as_dec(recv).is_some() { Some(dec_key(Dec::zero())) } else { Some(key_int(0)) }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Dec {
        Dec::parse(s).unwrap()
    }

    #[test]
    fn decimal_like_cpython() {
        // expected values from CPython 3.14's decimal module
        assert_eq!(Dec::add(d("1.10"), d("2.20")).unwrap().to_str(), "3.30");
        assert_eq!(d("2.675").rescale(-2, Rounding::HalfUp).unwrap().to_str(), "2.68");
        assert_eq!(d("2.675").rescale(-2, Rounding::HalfEven).unwrap().to_str(), "2.68");
        assert_eq!(d("2.665").rescale(-2, Rounding::HalfEven).unwrap().to_str(), "2.66");
        assert_eq!(Dec::div(d("1"), d("3")).unwrap().to_str(), "0.3333333333333333333333333333");
        assert_eq!(Dec::div(d("1"), d("4")).unwrap().to_str(), "0.25");
        assert_eq!(Dec::div(d("10"), d("2")).unwrap().to_str(), "5");
        assert_eq!(Dec::div(d("1.00"), d("4")).unwrap().to_str(), "0.25");
        assert_eq!(Dec::div(d("2"), d("3")).unwrap().to_str(), "0.6666666666666666666666666667");
        assert_eq!(Dec::mul(d("1.5"), d("2.0")).unwrap().to_str(), "3.00");
        assert_eq!(d("1E+2").to_str(), "1E+2");
        assert_eq!(d("0.0000001").to_str(), "1E-7");
        assert_eq!(d("0.000001").to_str(), "0.000001");
        assert_eq!(d("-0").to_str(), "-0");
        assert_eq!(Dec::add(d("0.1"), d("-0.1")).unwrap().to_str(), "0.0");
        assert_eq!(d("120.00").normalize().unwrap().to_str(), "1.2E+2");
        assert_eq!(Dec::from_f64(0.5).unwrap().to_str(), "0.5");
        assert_eq!(Dec::cmp(d("1.0"), d("1.00")), Some(Ordering::Equal));
        assert_eq!(limit_denominator(frac_parse("3.141592653589793").unwrap(), 1000), Some((355, 113)));
        assert_eq!(frac_str(frac_add((1, 3), (1, 6)).unwrap()), "1/2");
    }
}
