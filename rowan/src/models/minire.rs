//! A small backtracking regular-expression matcher for the patterns programs give pandas
//! (`filter(regex=)`, `str.extract` group names): literals, `.`, classes `[a-z]` / `[^…]`,
//! escapes `\d \w \s \D \W \S` and escaped punctuation, groups `( … )` (also `(?:…)` and
//! `(?P<name>…)`), alternation, `* + ?` and `{m,n}` (greedy), anchors `^ $`. Anything else
//! (look-around, back-references, flags) is not parsed: callers treat the pattern as unknown.

#[derive(Debug, Clone)]
enum Node {
    Char(char),
    Any,
    Class(Vec<(char, char)>, bool),
    Start,
    End,
    Group(Box<Node>, Option<String>),
    Alt(Vec<Node>),
    Seq(Vec<Node>),
    Repeat(Box<Node>, usize, Option<usize>),
}

/// A compiled pattern of the small matcher.
pub struct Regex {
    root: Node,
}

struct Parser<'a> {
    s: &'a [char],
    i: usize,
}

fn class_of(c: char) -> Option<(Vec<(char, char)>, bool)> {
    let digits = vec![('0', '9')];
    let word = vec![('a', 'z'), ('A', 'Z'), ('0', '9'), ('_', '_')];
    let space = vec![(' ', ' '), ('\t', '\t'), ('\n', '\n'), ('\r', '\r'), ('\x0b', '\x0c')];
    Some(match c {
        'd' => (digits, false),
        'D' => (digits, true),
        'w' => (word, false),
        'W' => (word, true),
        's' => (space, false),
        'S' => (space, true),
        _ => return None,
    })
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<char> {
        self.s.get(self.i).copied()
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
        let mut items = Vec::new();
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            let atom = self.atom()?;
            items.push(self.quant(atom)?);
        }
        Some(Node::Seq(items))
    }

    fn quant(&mut self, atom: Node) -> Option<Node> {
        let (lo, hi) = match self.peek() {
            Some('*') => (0, None),
            Some('+') => (1, None),
            Some('?') => (0, Some(1)),
            Some('{') => {
                let rest: String = self.s[self.i + 1..].iter().collect();
                let end = rest.find('}')?;
                let body = &rest[..end];
                let (a, b) = match body.split_once(',') {
                    Some((a, b)) => (a.trim().parse().ok()?, if b.trim().is_empty() { None } else { Some(b.trim().parse().ok()?) }),
                    None => {
                        let n: usize = body.trim().parse().ok()?;
                        (n, Some(n))
                    }
                };
                self.i += end + 1;
                (a, b)
            }
            _ => return Some(atom),
        };
        self.i += 1;
        if self.peek() == Some('?') || self.peek() == Some('+') {
            return None; // lazy / possessive: not parsed
        }
        Some(Node::Repeat(Box::new(atom), lo, hi))
    }

    fn atom(&mut self) -> Option<Node> {
        let c = self.peek()?;
        self.i += 1;
        Some(match c {
            '.' => Node::Any,
            '^' => Node::Start,
            '$' => Node::End,
            '(' => {
                let mut name = None;
                if self.peek() == Some('?') {
                    self.i += 1;
                    match self.peek() {
                        Some(':') => self.i += 1,
                        Some('P') if self.s.get(self.i + 1) == Some(&'<') => {
                            self.i += 2;
                            let mut n = String::new();
                            while let Some(ch) = self.peek() {
                                self.i += 1;
                                if ch == '>' {
                                    break;
                                }
                                n.push(ch);
                            }
                            name = Some(n);
                        }
                        Some('<') if !matches!(self.s.get(self.i + 1), Some('=') | Some('!')) => {
                            self.i += 1;
                            let mut n = String::new();
                            while let Some(ch) = self.peek() {
                                self.i += 1;
                                if ch == '>' {
                                    break;
                                }
                                n.push(ch);
                            }
                            name = Some(n);
                        }
                        _ => return None,
                    }
                }
                let inner = self.alt()?;
                if self.peek() != Some(')') {
                    return None;
                }
                self.i += 1;
                Node::Group(Box::new(inner), name)
            }
            '[' => {
                let mut neg = false;
                if self.peek() == Some('^') {
                    neg = true;
                    self.i += 1;
                }
                let mut ranges = Vec::new();
                let mut first = true;
                loop {
                    let ch = self.peek()?;
                    self.i += 1;
                    if ch == ']' && !first {
                        break;
                    }
                    first = false;
                    let lo = if ch == '\\' {
                        let e = self.peek()?;
                        self.i += 1;
                        if let Some((cls, false)) = class_of(e) {
                            ranges.extend(cls);
                            continue;
                        }
                        if class_of(e).is_some() {
                            return None;
                        }
                        unescape(e)?
                    } else {
                        ch
                    };
                    if self.peek() == Some('-') && self.s.get(self.i + 1).map(|x| *x != ']').unwrap_or(false) {
                        self.i += 1;
                        let hi = self.peek()?;
                        self.i += 1;
                        ranges.push((lo, hi));
                    } else {
                        ranges.push((lo, lo));
                    }
                }
                Node::Class(ranges, neg)
            }
            '\\' => {
                let e = self.peek()?;
                self.i += 1;
                if let Some((cls, neg)) = class_of(e) {
                    Node::Class(cls, neg)
                } else {
                    Node::Char(unescape(e)?)
                }
            }
            '*' | '+' | '?' | '{' | ')' => return None,
            _ => Node::Char(c),
        })
    }
}

fn unescape(e: char) -> Option<char> {
    Some(match e {
        'n' => '\n',
        't' => '\t',
        'r' => '\r',
        c if !c.is_alphanumeric() => c,
        _ => return None,
    })
}

impl Regex {
    pub fn new(pat: &str) -> Option<Regex> {
        let chars: Vec<char> = pat.chars().collect();
        let mut p = Parser { s: &chars, i: 0 };
        let root = p.alt()?;
        if p.i != chars.len() {
            return None;
        }
        Some(Regex { root })
    }

    /// `re.search(pat, s) is not None`
    pub fn search(&self, s: &str) -> bool {
        let cs: Vec<char> = s.chars().collect();
        (0..=cs.len()).any(|start| m(&self.root, &cs, start, &mut |_| true))
    }

    /// The names of the named groups, in order; with the count of all groups.
    pub fn groups(&self) -> (Vec<Option<String>>, usize) {
        fn walk(n: &Node, out: &mut Vec<Option<String>>) {
            match n {
                Node::Group(inner, name) => {
                    out.push(name.clone());
                    walk(inner, out);
                }
                Node::Alt(xs) | Node::Seq(xs) => xs.iter().for_each(|x| walk(x, out)),
                Node::Repeat(x, _, _) => walk(x, out),
                _ => {}
            }
        }
        let mut out = Vec::new();
        walk(&self.root, &mut out);
        let n = out.len();
        (out, n)
    }
}

/// Match node `n` at position `i`; `k` continues with the end position.
fn m(n: &Node, s: &[char], i: usize, k: &mut dyn FnMut(usize) -> bool) -> bool {
    match n {
        Node::Char(c) => i < s.len() && s[i] == *c && k(i + 1),
        Node::Any => i < s.len() && s[i] != '\n' && k(i + 1),
        Node::Class(rs, neg) => i < s.len() && (rs.iter().any(|(a, b)| *a <= s[i] && s[i] <= *b) != *neg) && k(i + 1),
        Node::Start => i == 0 && k(i),
        Node::End => (i == s.len() || (i + 1 == s.len() && s[i] == '\n')) && k(i),
        Node::Group(inner, _) => m(inner, s, i, k),
        Node::Alt(xs) => xs.iter().any(|x| m(x, s, i, k)),
        Node::Seq(xs) => seq(xs, s, i, k),
        Node::Repeat(x, lo, hi) => rep(x, *lo, *hi, s, i, 0, k),
    }
}

fn seq(xs: &[Node], s: &[char], i: usize, k: &mut dyn FnMut(usize) -> bool) -> bool {
    match xs.split_first() {
        None => k(i),
        Some((first, rest)) => m(first, s, i, &mut |j| seq(rest, s, j, k)),
    }
}

fn rep(x: &Node, lo: usize, hi: Option<usize>, s: &[char], i: usize, count: usize, k: &mut dyn FnMut(usize) -> bool) -> bool {
    // greedy: one more first (only when it consumes), then stop
    if hi.map(|h| count < h).unwrap_or(true) && m(x, s, i, &mut |j| j > i && rep(x, lo, hi, s, j, count + 1, k)) {
        return true;
    }
    count >= lo && k(i)
}

#[cfg(test)]
mod tests {
    use super::Regex;

    #[test]
    fn basics() {
        let r = Regex::new("^q").unwrap();
        assert!(r.search("qty") && !r.search("aq"));
        let r = Regex::new("_(net|gross)$").unwrap();
        assert!(r.search("amt_net") && r.search("amt_gross") && !r.search("amt_netx"));
        let r = Regex::new(r"\d{2,}").unwrap();
        assert!(r.search("a12") && !r.search("a1b"));
        let r = Regex::new("^[^a-c]x+").unwrap();
        assert!(r.search("dxx") && !r.search("axx"));
        let r = Regex::new(r"(?P<method>\w+) (?P<path>/\S*)").unwrap();
        assert_eq!(r.groups().0, vec![Some("method".to_string()), Some("path".to_string())]);
        assert!(Regex::new("(?=a)").is_none());
    }
}
