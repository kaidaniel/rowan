//! A backtracking matcher for Python's `re` syntax on known strings: literals and escapes,
//! `.`, classes (ranges, negation, `\d \w \s` and their negations), anchors `^ $ \A \Z \b \B`,
//! groups (capturing, `(?P<name>…)`, `(?:…)`), alternation, greedy and lazy quantifiers
//! (`* + ? {m,n}`), back-references (`\1`, `(?P=name)`), look-ahead / fixed-width look-behind,
//! and the flags I M S X A (inline `(?imsx)` at the start or scoped `(?i:…)`). Positions are
//! char indices. A pattern using anything else does not parse (`None`: the caller keeps the
//! result unknown); a match that takes too many steps gives up the same way.

pub const IGNORECASE: u32 = 2;
pub const MULTILINE: u32 = 8;
pub const DOTALL: u32 = 16;
pub const VERBOSE: u32 = 64;
pub const ASCII: u32 = 256;

#[derive(Debug, Clone)]
enum ClassItem {
    Range(char, char),
    Digit(bool),
    Word(bool),
    Space(bool),
}

#[derive(Debug, Clone)]
enum Node {
    Char(char, bool),
    Any(bool),
    Class(Vec<ClassItem>, bool, bool, bool),
    Bol(bool),
    Eol(bool),
    StrStart,
    StrEnd,
    WordB(bool, bool),
    Group(Option<usize>, Box<Node>),
    Alt(Vec<Node>),
    Seq(Vec<Node>),
    Rep(Box<Node>, usize, Option<usize>, bool),
    Backref(usize, bool),
    Look(bool, bool, Box<Node>, usize),
}

/// A compiled pattern: its tree, groups (and names) and flags.
#[derive(Debug, Clone)]
pub struct Regex {
    root: Node,
    pub ngroups: usize,
    pub names: Vec<(String, usize)>,
    pub flags: u32,
}

/// The span of each group of a match (None: the group did not take part).
pub type Spans = Vec<Option<(usize, usize)>>;

struct Parser<'a> {
    s: &'a [char],
    i: usize,
    flags: u32,
    ngroups: usize,
    names: Vec<(String, usize)>,
}

fn is_word(c: char, ascii: bool) -> bool {
    if ascii {
        c.is_ascii_alphanumeric() || c == '_'
    } else {
        c.is_alphanumeric() || c == '_'
    }
}

fn is_digit(c: char, ascii: bool) -> bool {
    c.is_ascii_digit() || (!ascii && unicode_decimal(c))
}

/// Unicode decimal digits (category Nd) outside ASCII: the common blocks.
fn unicode_decimal(c: char) -> bool {
    let u = c as u32;
    const STARTS: &[u32] = &[0x660, 0x6f0, 0x7c0, 0x966, 0x9e6, 0xa66, 0xae6, 0xb66, 0xbe6, 0xc66, 0xce6, 0xd66, 0xde6, 0xe50, 0xed0, 0xf20, 0x1040, 0x1090, 0x17e0, 0x1810, 0xff10];
    STARTS.iter().any(|s| u >= *s && u < s + 10)
}

fn is_space(c: char, ascii: bool) -> bool {
    if ascii {
        matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0b' | '\x0c')
    } else {
        c.is_whitespace() || ('\x1c'..='\x1f').contains(&c)
    }
}

fn fold(c: char) -> char {
    let mut l = c.to_lowercase();
    match (l.next(), l.next()) {
        (Some(x), None) => x,
        _ => c,
    }
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<char> {
        self.s.get(self.i).copied()
    }

    fn icase(&self) -> bool {
        self.flags & IGNORECASE != 0
    }

    fn ascii(&self) -> bool {
        self.flags & ASCII != 0
    }

    fn skip_verbose(&mut self) {
        if self.flags & VERBOSE == 0 {
            return;
        }
        loop {
            match self.peek() {
                Some(c) if c.is_whitespace() => self.i += 1,
                Some('#') => {
                    while let Some(c) = self.peek() {
                        self.i += 1;
                        if c == '\n' {
                            break;
                        }
                    }
                }
                _ => break,
            }
        }
    }

    fn alt(&mut self) -> Option<Node> {
        let mut alts = vec![self.seq()?];
        while self.peek() == Some('|') {
            self.i += 1;
            alts.push(self.seq()?);
        }
        Some(if alts.len() == 1 { alts.pop().unwrap() } else { Node::Alt(alts) })
    }

    fn seq(&mut self) -> Option<Node> {
        let mut items: Vec<Node> = Vec::new();
        loop {
            self.skip_verbose();
            match self.peek() {
                None | Some('|') | Some(')') => break,
                _ => {}
            }
            let atom = self.atom()?;
            let atom = self.quant(atom)?;
            items.push(atom);
        }
        Some(if items.len() == 1 { items.pop().unwrap() } else { Node::Seq(items) })
    }

    fn number(&mut self) -> Option<usize> {
        let st = self.i;
        while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
            self.i += 1;
        }
        if st == self.i {
            return None;
        }
        self.s[st..self.i].iter().collect::<String>().parse().ok()
    }

    fn quant(&mut self, atom: Node) -> Option<Node> {
        self.skip_verbose();
        let (lo, hi) = match self.peek() {
            Some('*') => {
                self.i += 1;
                (0, None)
            }
            Some('+') => {
                self.i += 1;
                (1, None)
            }
            Some('?') => {
                self.i += 1;
                (0, Some(1))
            }
            Some('{') => {
                let save = self.i;
                self.i += 1;
                let lo = self.number();
                let (lo, hi) = if self.peek() == Some(',') {
                    self.i += 1;
                    (lo.unwrap_or(0), self.number())
                } else {
                    match lo {
                        Some(n) => (n, Some(n)),
                        None => {
                            self.i = save;
                            return Some(atom);
                        }
                    }
                };
                if self.peek() != Some('}') {
                    // not a quantifier: a literal `{`
                    self.i = save;
                    return Some(atom);
                }
                self.i += 1;
                if let Some(h) = hi {
                    if h < lo {
                        return None;
                    }
                }
                (lo, hi)
            }
            _ => return Some(atom),
        };
        if matches!(atom, Node::Bol(_) | Node::Eol(_) | Node::StrStart | Node::StrEnd | Node::WordB(..) | Node::Rep(..)) {
            return None;
        }
        let mut greedy = true;
        match self.peek() {
            Some('?') => {
                self.i += 1;
                greedy = false;
            }
            Some('+') => return None,
            _ => {}
        }
        Some(Node::Rep(Box::new(atom), lo, hi, greedy))
    }

    fn escape_char(&mut self, e: char) -> Option<char> {
        Some(match e {
            'n' => '\n',
            't' => '\t',
            'r' => '\r',
            'f' => '\x0c',
            'v' => '\x0b',
            'a' => '\x07',
            '0' => {
                let mut v = 0u32;
                let mut n = 0;
                while n < 2 && matches!(self.peek(), Some(c) if ('0'..='7').contains(&c)) {
                    v = v * 8 + self.peek().unwrap().to_digit(8).unwrap();
                    self.i += 1;
                    n += 1;
                }
                char::from_u32(v)?
            }
            'x' | 'u' | 'U' => {
                let n = match e {
                    'x' => 2,
                    'u' => 4,
                    _ => 8,
                };
                if self.i + n > self.s.len() {
                    return None;
                }
                let h: String = self.s[self.i..self.i + n].iter().collect();
                self.i += n;
                char::from_u32(u32::from_str_radix(&h, 16).ok()?)?
            }
            c if c.is_ascii_alphanumeric() => return None,
            c => c,
        })
    }

    fn atom(&mut self) -> Option<Node> {
        let c = self.peek()?;
        self.i += 1;
        let ic = self.icase();
        Some(match c {
            '.' => Node::Any(self.flags & DOTALL != 0),
            '^' => Node::Bol(self.flags & MULTILINE != 0),
            '$' => Node::Eol(self.flags & MULTILINE != 0),
            '[' => self.class()?,
            '(' => self.group()?,
            '*' | '+' | '?' => return None,
            '\\' => {
                let e = self.peek()?;
                self.i += 1;
                let a = self.ascii();
                match e {
                    'd' => Node::Class(vec![ClassItem::Digit(false)], false, ic, a),
                    'D' => Node::Class(vec![ClassItem::Digit(true)], false, ic, a),
                    'w' => Node::Class(vec![ClassItem::Word(false)], false, ic, a),
                    'W' => Node::Class(vec![ClassItem::Word(true)], false, ic, a),
                    's' => Node::Class(vec![ClassItem::Space(false)], false, ic, a),
                    'S' => Node::Class(vec![ClassItem::Space(true)], false, ic, a),
                    'b' => Node::WordB(true, a),
                    'B' => Node::WordB(false, a),
                    'A' => Node::StrStart,
                    'Z' => Node::StrEnd,
                    '1'..='9' => {
                        let mut n = e.to_digit(10).unwrap() as usize;
                        if let Some(d) = self.peek().and_then(|c| c.to_digit(10)) {
                            if n * 10 + d as usize <= self.ngroups {
                                n = n * 10 + d as usize;
                                self.i += 1;
                            }
                        }
                        if n > self.ngroups {
                            return None;
                        }
                        Node::Backref(n, ic)
                    }
                    other => Node::Char(self.escape_char(other)?, ic),
                }
            }
            c => Node::Char(c, ic),
        })
    }

    fn class(&mut self) -> Option<Node> {
        let mut neg = false;
        if self.peek() == Some('^') {
            neg = true;
            self.i += 1;
        }
        let a = self.ascii();
        let mut items = Vec::new();
        let mut first = true;
        loop {
            let c = self.peek()?;
            if c == ']' && !first {
                self.i += 1;
                break;
            }
            first = false;
            self.i += 1;
            let lo = if c == '\\' {
                let e = self.peek()?;
                self.i += 1;
                match e {
                    'd' => {
                        items.push(ClassItem::Digit(false));
                        continue;
                    }
                    'D' => {
                        items.push(ClassItem::Digit(true));
                        continue;
                    }
                    'w' => {
                        items.push(ClassItem::Word(false));
                        continue;
                    }
                    'W' => {
                        items.push(ClassItem::Word(true));
                        continue;
                    }
                    's' => {
                        items.push(ClassItem::Space(false));
                        continue;
                    }
                    'S' => {
                        items.push(ClassItem::Space(true));
                        continue;
                    }
                    'b' => '\x08',
                    other => self.escape_char(other)?,
                }
            } else {
                c
            };
            if self.peek() == Some('-') && self.s.get(self.i + 1).map(|x| *x != ']').unwrap_or(false) {
                self.i += 1;
                let hc = self.peek()?;
                self.i += 1;
                let hi = if hc == '\\' {
                    let e = self.peek()?;
                    self.i += 1;
                    self.escape_char(e)?
                } else {
                    hc
                };
                if hi < lo {
                    return None;
                }
                items.push(ClassItem::Range(lo, hi));
            } else {
                items.push(ClassItem::Range(lo, lo));
            }
        }
        Some(Node::Class(items, neg, self.icase(), a))
    }

    fn flag_letters(&mut self) -> Option<(u32, u32)> {
        let mut on = 0;
        let mut off = 0;
        let mut minus = false;
        while let Some(c) = self.peek() {
            let f = match c {
                'i' => IGNORECASE,
                'm' => MULTILINE,
                's' => DOTALL,
                'x' => VERBOSE,
                'a' => ASCII,
                'u' => 0,
                '-' => {
                    minus = true;
                    self.i += 1;
                    continue;
                }
                _ => break,
            };
            if minus {
                off |= f;
            } else {
                on |= f;
            }
            self.i += 1;
        }
        Some((on, off))
    }

    fn group(&mut self) -> Option<Node> {
        let gstart = self.i - 1;
        if self.peek() == Some('?') {
            self.i += 1;
            let c = self.peek()?;
            match c {
                ':' => {
                    self.i += 1;
                    let inner = self.alt()?;
                    self.close()?;
                    return Some(Node::Group(None, Box::new(inner)));
                }
                'P' => {
                    self.i += 1;
                    match self.peek()? {
                        '<' => {
                            self.i += 1;
                            let st = self.i;
                            while matches!(self.peek(), Some(c) if c != '>') {
                                self.i += 1;
                            }
                            let name: String = self.s[st..self.i].iter().collect();
                            self.i += 1;
                            if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') || name.chars().next().unwrap().is_ascii_digit() || self.names.iter().any(|(n, _)| *n == name) {
                                return None;
                            }
                            self.ngroups += 1;
                            let g = self.ngroups;
                            self.names.push((name, g));
                            let inner = self.alt()?;
                            self.close()?;
                            return Some(Node::Group(Some(g), Box::new(inner)));
                        }
                        '=' => {
                            self.i += 1;
                            let st = self.i;
                            while matches!(self.peek(), Some(c) if c != ')') {
                                self.i += 1;
                            }
                            let name: String = self.s[st..self.i].iter().collect();
                            self.close()?;
                            let g = self.names.iter().find(|(n, _)| *n == name)?.1;
                            return Some(Node::Backref(g, self.icase()));
                        }
                        _ => return None,
                    }
                }
                '=' | '!' => {
                    self.i += 1;
                    let inner = self.alt()?;
                    self.close()?;
                    return Some(Node::Look(true, c == '!', Box::new(inner), 0));
                }
                '<' => {
                    self.i += 1;
                    let c2 = self.peek()?;
                    if c2 != '=' && c2 != '!' {
                        return None;
                    }
                    self.i += 1;
                    let inner = self.alt()?;
                    self.close()?;
                    let w = width(&inner)?;
                    return Some(Node::Look(false, c2 == '!', Box::new(inner), w));
                }
                '#' => {
                    while matches!(self.peek(), Some(c) if c != ')') {
                        self.i += 1;
                    }
                    self.close()?;
                    return Some(Node::Seq(vec![]));
                }
                _ => {
                    // `(?i)` / `(?i:…)`
                    let (on, off) = self.flag_letters()?;
                    match self.peek()? {
                        ')' => {
                            // a global flag group: only at the very start
                            if gstart != 0 {
                                return None;
                            }
                            self.i += 1;
                            self.flags = (self.flags | on) & !off;
                            return Some(Node::Seq(vec![]));
                        }
                        ':' => {
                            self.i += 1;
                            let saved = self.flags;
                            self.flags = (self.flags | on) & !off;
                            let inner = self.alt();
                            self.flags = saved;
                            let inner = inner?;
                            self.close()?;
                            return Some(Node::Group(None, Box::new(inner)));
                        }
                        _ => return None,
                    }
                }
            }
        }
        self.ngroups += 1;
        let g = self.ngroups;
        let inner = self.alt()?;
        self.close()?;
        Some(Node::Group(Some(g), Box::new(inner)))
    }

    fn close(&mut self) -> Option<()> {
        if self.peek() == Some(')') {
            self.i += 1;
            Some(())
        } else {
            None
        }
    }
}

/// The fixed width of a node (look-behind), None when it varies.
fn width(n: &Node) -> Option<usize> {
    Some(match n {
        Node::Char(..) | Node::Any(_) | Node::Class(..) => 1,
        Node::Bol(_) | Node::Eol(_) | Node::StrStart | Node::StrEnd | Node::WordB(..) | Node::Look(..) => 0,
        Node::Group(_, x) => width(x)?,
        Node::Seq(xs) => xs.iter().map(width).sum::<Option<usize>>()?,
        Node::Alt(xs) => {
            let w = width(&xs[0])?;
            for x in &xs[1..] {
                if width(x)? != w {
                    return None;
                }
            }
            w
        }
        Node::Rep(x, lo, Some(hi), _) if lo == hi => width(x)? * lo,
        _ => return None,
    })
}

/// Pending work after a node matched.
enum Cont<'a> {
    Done,
    Seq(&'a [Node], &'a Cont<'a>),
    Close(usize, usize, &'a Cont<'a>),
    More(&'a Node, usize, Option<usize>, bool, usize, usize, &'a Cont<'a>),
    EndAt(usize),
}

struct Ctx<'s> {
    s: &'s [char],
    caps: Spans,
    steps: usize,
    aborted: bool,
    end: usize,
    /// the match must end here (fullmatch / endpos)
    need_end: Option<usize>,
    /// the match may not be empty at this position (after an empty match)
    not_empty_at: Option<usize>,
    start: usize,
    limit: usize,
}

const STEP_LIMIT: usize = 200_000;

impl Regex {
    pub fn new(pat: &str, flags: u32) -> Option<Regex> {
        let s: Vec<char> = pat.chars().collect();
        let mut p = Parser { s: &s, i: 0, flags, ngroups: 0, names: Vec::new() };
        let root = p.alt()?;
        if p.i != s.len() {
            return None;
        }
        Some(Regex { root, ngroups: p.ngroups, names: p.names, flags: p.flags })
    }

    pub fn group_index(&self, name: &str) -> Option<usize> {
        self.names.iter().find(|(n, _)| n == name).map(|(_, g)| *g)
    }

    /// Match at `start` (anchored), the spans of the match and its groups; Err(()) when the
    /// matcher gave up.
    pub fn match_at(&self, s: &[char], start: usize, endpos: usize, full: bool, not_empty_at: Option<usize>) -> Result<Option<Spans>, ()> {
        let mut ctx = Ctx { s: &s[..endpos], caps: vec![None; self.ngroups + 1], steps: 0, aborted: false, end: 0, need_end: if full { Some(endpos) } else { None }, not_empty_at, start, limit: STEP_LIMIT };
        let ok = ctx.m(&self.root, start, &Cont::Done);
        if ctx.aborted {
            return Err(());
        }
        if !ok {
            return Ok(None);
        }
        ctx.caps[0] = Some((start, ctx.end));
        Ok(Some(ctx.caps))
    }

    /// The first match at or after `pos`.
    pub fn search(&self, s: &[char], pos: usize, endpos: usize, not_empty_at: Option<usize>) -> Result<Option<Spans>, ()> {
        let mut i = pos;
        while i <= endpos {
            if let Some(m) = self.match_at(s, i, endpos, false, not_empty_at)? {
                return Ok(Some(m));
            }
            i += 1;
        }
        Ok(None)
    }

    /// All successive non-overlapping matches (`findall` / `finditer` / `sub`).
    pub fn find_all(&self, s: &[char], pos: usize, endpos: usize, limit: usize) -> Result<Vec<Spans>, ()> {
        let mut out = Vec::new();
        let mut at = pos;
        let mut prev_empty_at: Option<usize> = None;
        while at <= endpos && (limit == 0 || out.len() < limit) {
            let Some(m) = self.search(s, at, endpos, prev_empty_at)? else { break };
            let (a, b) = m[0].unwrap();
            prev_empty_at = if a == b { Some(b) } else { None };
            at = b;
            out.push(m);
            if out.len() > 10_000 {
                return Err(());
            }
        }
        Ok(out)
    }
}

impl<'s> Ctx<'s> {
    fn tick(&mut self) -> bool {
        self.steps += 1;
        if self.steps > self.limit {
            self.aborted = true;
        }
        !self.aborted
    }

    fn class_has(&self, items: &[ClassItem], neg: bool, icase: bool, ascii: bool, c: char) -> bool {
        let test = |c: char| {
            items.iter().any(|it| match it {
                ClassItem::Range(lo, hi) => *lo <= c && c <= *hi,
                ClassItem::Digit(n) => is_digit(c, ascii) != *n,
                ClassItem::Word(n) => is_word(c, ascii) != *n,
                ClassItem::Space(n) => is_space(c, ascii) != *n,
            })
        };
        let mut hit = test(c);
        if !hit && icase {
            let l = fold(c);
            let u = c.to_uppercase().next().unwrap_or(c);
            hit = test(l) || test(u);
        }
        hit != neg
    }

    fn word_at(&self, i: usize, ascii: bool) -> bool {
        i < self.s.len() && is_word(self.s[i], ascii)
    }

    fn m(&mut self, n: &Node, i: usize, k: &Cont) -> bool {
        if !self.tick() {
            return false;
        }
        let s = self.s;
        match n {
            Node::Char(c, ic) => i < s.len() && (s[i] == *c || (*ic && fold(s[i]) == fold(*c))) && self.run(k, i + 1),
            Node::Any(dotall) => i < s.len() && (*dotall || s[i] != '\n') && self.run(k, i + 1),
            Node::Class(items, neg, ic, a) => i < s.len() && self.class_has(items, *neg, *ic, *a, s[i]) && self.run(k, i + 1),
            Node::Bol(multi) => (i == 0 || (*multi && s[i - 1] == '\n')) && self.run(k, i),
            Node::Eol(multi) => (i == s.len() || (i + 1 == s.len() && s[i] == '\n') || (*multi && s[i] == '\n')) && self.run(k, i),
            Node::StrStart => i == 0 && self.run(k, i),
            Node::StrEnd => i == s.len() && self.run(k, i),
            Node::WordB(want, a) => {
                let before = i > 0 && is_word(s[i - 1], *a);
                let after = self.word_at(i, *a);
                ((before != after) == *want) && self.run(k, i)
            }
            Node::Seq(xs) => {
                if xs.is_empty() {
                    self.run(k, i)
                } else {
                    self.m(&xs[0], i, &Cont::Seq(&xs[1..], k))
                }
            }
            Node::Alt(xs) => {
                for x in xs {
                    let saved = self.caps.clone();
                    if self.m(x, i, k) {
                        return true;
                    }
                    self.caps = saved;
                    if self.aborted {
                        return false;
                    }
                }
                false
            }
            Node::Group(None, x) => self.m(x, i, k),
            Node::Group(Some(g), x) => {
                let old = self.caps[*g];
                if self.m(x, i, &Cont::Close(*g, i, k)) {
                    return true;
                }
                self.caps[*g] = old;
                false
            }
            Node::Rep(x, lo, hi, greedy) => self.rep(x, *lo, *hi, *greedy, 0, i, usize::MAX, k),
            Node::Backref(g, ic) => {
                let Some((a, b)) = self.caps[*g] else { return false };
                let len = b - a;
                if i + len > s.len() {
                    return false;
                }
                let same = (0..len).all(|j| s[a + j] == s[i + j] || (*ic && fold(s[a + j]) == fold(s[i + j])));
                same && self.run(k, i + len)
            }
            Node::Look(ahead, neg, x, w) => {
                let saved = self.caps.clone();
                let (saved_end, saved_need, saved_ne) = (self.end, self.need_end, self.not_empty_at);
                self.need_end = None;
                self.not_empty_at = None;
                let hit = if *ahead {
                    self.m(x, i, &Cont::EndAt(usize::MAX))
                } else if i >= *w {
                    self.m(x, i - w, &Cont::EndAt(i))
                } else {
                    false
                };
                self.end = saved_end;
                self.need_end = saved_need;
                self.not_empty_at = saved_ne;
                if self.aborted {
                    return false;
                }
                if *neg {
                    self.caps = saved;
                    !hit && self.run(k, i)
                } else if hit {
                    if self.run(k, i) {
                        return true;
                    }
                    self.caps = saved;
                    false
                } else {
                    self.caps = saved;
                    false
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn rep(&mut self, x: &Node, lo: usize, hi: Option<usize>, greedy: bool, count: usize, i: usize, last: usize, k: &Cont) -> bool {
        // an iteration that matched nothing ends the loop
        if count > lo && i == last {
            return self.run(k, i);
        }
        let can_more = hi.map(|h| count < h).unwrap_or(true);
        if count < lo {
            return self.m(x, i, &Cont::More(x, lo, hi, greedy, count + 1, i, k));
        }
        if greedy {
            if can_more {
                let saved = self.caps.clone();
                if self.m(x, i, &Cont::More(x, lo, hi, greedy, count + 1, i, k)) {
                    return true;
                }
                self.caps = saved;
                if self.aborted {
                    return false;
                }
            }
            self.run(k, i)
        } else {
            let saved = self.caps.clone();
            if self.run(k, i) {
                return true;
            }
            self.caps = saved;
            if self.aborted {
                return false;
            }
            can_more && self.m(x, i, &Cont::More(x, lo, hi, greedy, count + 1, i, k))
        }
    }

    fn run(&mut self, k: &Cont, i: usize) -> bool {
        match k {
            Cont::Done => {
                if let Some(e) = self.need_end {
                    if i != e {
                        return false;
                    }
                }
                if self.not_empty_at == Some(self.start) && i == self.start {
                    return false;
                }
                self.end = i;
                true
            }
            Cont::EndAt(p) => *p == usize::MAX || i == *p,
            Cont::Seq(xs, next) => {
                if xs.is_empty() {
                    self.run(next, i)
                } else {
                    self.m(&xs[0], i, &Cont::Seq(&xs[1..], next))
                }
            }
            Cont::Close(g, start, next) => {
                let old = self.caps[*g];
                self.caps[*g] = Some((*start, i));
                if self.run(next, i) {
                    return true;
                }
                self.caps[*g] = old;
                false
            }
            Cont::More(x, lo, hi, greedy, count, last, next) => self.rep(x, *lo, *hi, *greedy, *count, i, *last, next),
        }
    }
}

/// `re.escape(s)`.
pub fn escape(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_alphanumeric() || c == '_' || !c.is_ascii() {
            out.push(c);
        } else {
            out.push('\\');
            out.push(c);
        }
    }
    out
}

/// A replacement template (`\1`, `\g<name>`, `\g<1>`, `\n`…) expanded for a match; None for
/// an invalid template.
pub fn expand(re: &Regex, tmpl: &str, s: &[char], m: &Spans) -> Option<String> {
    let t: Vec<char> = tmpl.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let group = |g: usize, out: &mut String| -> Option<()> {
        if g > re.ngroups {
            return None;
        }
        if let Some((a, b)) = m[g] {
            out.extend(&s[a..b]);
        }
        Some(())
    };
    while i < t.len() {
        let c = t[i];
        if c != '\\' {
            out.push(c);
            i += 1;
            continue;
        }
        i += 1;
        let e = *t.get(i)?;
        i += 1;
        match e {
            'g' => {
                if t.get(i) != Some(&'<') {
                    return None;
                }
                let st = i + 1;
                let en = t[st..].iter().position(|c| *c == '>')? + st;
                let name: String = t[st..en].iter().collect();
                i = en + 1;
                let g = match name.parse::<usize>() {
                    Ok(n) => n,
                    Err(_) => re.group_index(&name)?,
                };
                group(g, &mut out)?;
            }
            '0' => out.push('\0'),
            '1'..='9' => {
                let mut g = e.to_digit(10).unwrap() as usize;
                if let Some(d) = t.get(i).and_then(|c| c.to_digit(10)) {
                    g = g * 10 + d as usize;
                    i += 1;
                }
                group(g, &mut out)?;
            }
            'n' => out.push('\n'),
            't' => out.push('\t'),
            'r' => out.push('\r'),
            'f' => out.push('\x0c'),
            'v' => out.push('\x0b'),
            'a' => out.push('\x07'),
            'b' => out.push('\x08'),
            '\\' => out.push('\\'),
            c if c.is_ascii_alphabetic() => return None,
            c => {
                out.push('\\');
                out.push(c);
            }
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    fn find(p: &str, s: &str) -> Option<String> {
        let r = Regex::new(p, 0).unwrap();
        let cs = chars(s);
        r.search(&cs, 0, cs.len(), None).unwrap().map(|m| {
            let (a, b) = m[0].unwrap();
            cs[a..b].iter().collect()
        })
    }

    fn all(p: &str, s: &str) -> Vec<String> {
        let r = Regex::new(p, 0).unwrap();
        let cs = chars(s);
        r.find_all(&cs, 0, cs.len(), 0).unwrap().iter().map(|m| {
            let (a, b) = m[0].unwrap();
            cs[a..b].iter().collect()
        }).collect()
    }

    #[test]
    fn matches_like_cpython() {
        // expected values from CPython 3.14
        assert_eq!(find(r"(?P<level>[A-Z]+) (?P<code>\d+)(?: (?P<msg>.*))?", "ERROR 42 disk full").as_deref(), Some("ERROR 42 disk full"));
        assert_eq!(find(r"a+?", "aaa").as_deref(), Some("a"));
        assert_eq!(find(r"(a|ab)(c|bcd)(d*)", "abcd").as_deref(), Some("abcd"));
        assert_eq!(find(r"\bfoo\b", "a foo b").as_deref(), Some("foo"));
        assert_eq!(find(r"(\w)\1", "abccd").as_deref(), Some("cc"));
        assert_eq!(find(r"(?<=\$)\d+", "cost $42").as_deref(), Some("42"));
        assert_eq!(find(r"\d+(?!px)", "12px 34em").as_deref(), Some("1"));
        assert_eq!(find(r"x*", "abc").as_deref(), Some(""));
        assert_eq!(find(r"^$", "").as_deref(), Some(""));
        assert_eq!(find(r"a{2,3}", "aaaa").as_deref(), Some("aaa"));
        assert_eq!(find(r"a{,2}", "aaaa").as_deref(), Some("aa"));
        assert_eq!(find(r"[^\d\s]+", "12 ab3").as_deref(), Some("ab"));
        assert_eq!(find(r"(?i)hello", "Say HELLO").as_deref(), Some("HELLO"));
        assert_eq!(find(r"colou?r", "the color").as_deref(), Some("color"));
        assert_eq!(all(r"x*", "axx"), vec!["", "xx", ""]);
        assert_eq!(all(r"\b", "ab cd"), vec!["", "", "", ""]);
        assert_eq!(all(r"[a-z]+", "ab 12 cd ef"), vec!["ab", "cd", "ef"]);
        assert_eq!(all(r"a|", "bab"), vec!["", "a", "", ""]);
        let r = Regex::new(r"(\d{4})-(\d{2})-(\d{2})", 0).unwrap();
        let cs = chars("2023-11-05");
        let m = r.match_at(&cs, 0, cs.len(), true, None).unwrap().unwrap();
        assert_eq!(expand(&r, r"\3/\2/\1", &cs, &m).as_deref(), Some("05/11/2023"));
        assert!(Regex::new(r"(?P<a>x)(?P<a>y)", 0).is_none());
        assert!(Regex::new(r"a**", 0).is_none());
        assert_eq!(escape("a.b*c"), r"a\.b\*c");
    }
}
