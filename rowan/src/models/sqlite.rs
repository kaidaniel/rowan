//! `sqlite3`: a connection simulates the SQL it is given when the statement is simple
//! (CREATE TABLE, INSERT … VALUES, SELECT cols FROM t [WHERE …] [ORDER BY …] [LIMIT …],
//! UPDATE, DELETE, DROP). The columns a SELECT names are the keys of its rows whatever the
//! data (`sqlite3.Row` rows are closed dicts of them, tuples otherwise); the rows themselves
//! are known for an in-memory database filled with known values outside branches and loops.
//! A database file, `SELECT *` from a table the program did not create, unknown parameters
//! or unsupported SQL leave the rows (or their columns) unknown: the input's schema.

use indexmap::IndexMap;

use crate::domain::{join_values, KeyVal, Ns, Row, Value, DATA};
use crate::frontend::Node;
use crate::intern::{self, key_str_lit, KeyId, KeySet};
use crate::interp::{Analyzer, Call, Payload};
use crate::source::{Source, SourceKind};
use crate::state::{Kind, State};

/// A table of the simulated database: its columns, its rows when known.
#[derive(Clone, Debug)]
pub struct Table {
    pub cols: Vec<(String, String)>,
    /// None: unknown contents
    pub rows: Option<Vec<Vec<Option<KeyId>>>>,
    pub next_rowid: i64,
    /// the INTEGER PRIMARY KEY column (an alias of the rowid)
    pub pk: Option<usize>,
}

/// A connection's simulated database.
#[derive(Clone, Debug, Default)]
pub struct Db {
    /// an in-memory database: every table is the program's
    pub memory: bool,
    pub tables: IndexMap<String, Table>,
    pub line: u32,
}

/// A cursor's result: column names and rows (None: unknown count / values).
#[derive(Clone, Debug, Default)]
pub struct Rs {
    pub cols: Option<Vec<String>>,
    pub rows: Option<Vec<Vec<Option<KeyId>>>>,
    /// the source an unknown result's values come from
    pub source: String,
    /// rows an INSERT / UPDATE / DELETE changed, when known
    pub rowcount: Option<i64>,
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Id(String),
    Str(String),
    Num(String),
    P(String),
    Param(Option<String>),
}

fn lex(sql: &str) -> Option<Vec<Tok>> {
    let cs: Vec<char> = sql.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < cs.len() {
        let c = cs[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c == '-' && cs.get(i + 1) == Some(&'-') {
            while i < cs.len() && cs[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            let st = i;
            while i < cs.len() && (cs[i].is_alphanumeric() || cs[i] == '_' || cs[i] == '$') {
                i += 1;
            }
            out.push(Tok::Id(cs[st..i].iter().collect()));
            continue;
        }
        if c.is_ascii_digit() || (c == '.' && cs.get(i + 1).map(|d| d.is_ascii_digit()).unwrap_or(false)) {
            let st = i;
            while i < cs.len() && (cs[i].is_ascii_digit() || cs[i] == '.' || cs[i] == 'e' || cs[i] == 'E') {
                i += 1;
            }
            out.push(Tok::Num(cs[st..i].iter().collect()));
            continue;
        }
        match c {
            '\'' => {
                let mut s = String::new();
                i += 1;
                loop {
                    let ch = *cs.get(i)?;
                    i += 1;
                    if ch == '\'' {
                        if cs.get(i) == Some(&'\'') {
                            s.push('\'');
                            i += 1;
                            continue;
                        }
                        break;
                    }
                    s.push(ch);
                }
                out.push(Tok::Str(s));
            }
            '"' | '`' | '[' => {
                let close = if c == '[' { ']' } else { c };
                let mut s = String::new();
                i += 1;
                loop {
                    let ch = *cs.get(i)?;
                    i += 1;
                    if ch == close {
                        break;
                    }
                    s.push(ch);
                }
                out.push(Tok::Id(s));
            }
            '?' => {
                i += 1;
                // (a numbered parameter, `?1`: not modelled)
                if i < cs.len() && cs[i].is_ascii_digit() {
                    return None;
                }
                out.push(Tok::Param(None));
            }
            ':' | '@' | '$' if cs.get(i + 1).map(|d| d.is_alphabetic() || *d == '_').unwrap_or(false) => {
                let st = i + 1;
                i += 1;
                while i < cs.len() && (cs[i].is_alphanumeric() || cs[i] == '_') {
                    i += 1;
                }
                out.push(Tok::Param(Some(cs[st..i].iter().collect())));
            }
            _ => {
                let two: String = cs[i..(i + 2).min(cs.len())].iter().collect();
                if matches!(two.as_str(), "<=" | ">=" | "!=" | "<>" | "==" | "||") {
                    out.push(Tok::P(two));
                    i += 2;
                } else {
                    out.push(Tok::P(c.to_string()));
                    i += 1;
                }
            }
        }
    }
    Some(out)
}

/// A value in SQL: a literal key (None key = NULL), a parameter's position / name, unknown.
#[derive(Clone, Debug)]
enum SqlV {
    Lit(KeyId),
    Param(Option<String>),
    /// (UPDATE … SET only) a column of the row, `a <op> b`
    Col(String),
    Bin(Box<SqlV>, String, Box<SqlV>),
}

#[derive(Clone, Debug)]
struct Cond {
    col: String,
    op: String,
    val: Option<SqlV>,
}

#[derive(Clone, Debug)]
enum Stmt {
    Create { name: String, cols: Vec<(String, String)>, pk: Option<usize>, if_not: bool },
    Insert { table: String, cols: Option<Vec<String>>, rows: Vec<Vec<SqlV>>, or: Option<String> },
    Select { cols: Option<Vec<(String, Option<SExpr>)>>, table: String, conds: Option<Vec<Cond>>, group: Vec<String>, having: bool, order: Vec<(OrdT, bool)>, limit: Option<SqlV>, offset: Option<SqlV>, distinct: bool },
    Update { table: String, sets: Vec<(String, SqlV)>, conds: Option<Vec<Cond>> },
    Delete { table: String, conds: Option<Vec<Cond>> },
    Drop { table: String },
    Nothing,
}

struct P {
    t: Vec<Tok>,
    i: usize,
}

impl P {
    fn kw(&self, k: &str) -> bool {
        matches!(self.t.get(self.i), Some(Tok::Id(x)) if x.eq_ignore_ascii_case(k))
    }
    fn eat_kw(&mut self, k: &str) -> bool {
        if self.kw(k) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn p(&self, s: &str) -> bool {
        matches!(self.t.get(self.i), Some(Tok::P(x)) if x == s)
    }
    fn eat_p(&mut self, s: &str) -> bool {
        if self.p(s) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn ident(&mut self) -> Option<String> {
        match self.t.get(self.i)? {
            Tok::Id(x) => {
                let x = x.clone();
                self.i += 1;
                // `schema.table` / `t.col`: the last part
                if self.p(".") {
                    self.i += 1;
                    return self.ident();
                }
                Some(x)
            }
            _ => None,
        }
    }
    fn value(&mut self) -> Option<SqlV> {
        let neg = self.eat_p("-");
        let v = match self.t.get(self.i)?.clone() {
            Tok::Str(s) => SqlV::Lit(key_str_lit(&s)),
            Tok::Num(n) => {
                let n = if neg { format!("-{}", n) } else { n };
                match n.parse::<i64>() {
                    Ok(i) => SqlV::Lit(intern::key_int(i)),
                    Err(_) => SqlV::Lit(intern::key_float(n.parse::<f64>().ok()?)),
                }
            }
            Tok::Id(x) if x.eq_ignore_ascii_case("NULL") => SqlV::Lit(intern::key_none()),
            Tok::Id(x) if x.eq_ignore_ascii_case("TRUE") => SqlV::Lit(intern::key_int(1)),
            Tok::Id(x) if x.eq_ignore_ascii_case("FALSE") => SqlV::Lit(intern::key_int(0)),
            Tok::Param(n) => SqlV::Param(n),
            _ => return None,
        };
        self.i += 1;
        Some(v)
    }
    /// `SET c = <expr>`: a value, a column, or `x <op> y` of those (+ - * ||).
    fn set_operand(&mut self) -> Option<SqlV> {
        if let Some(Tok::Id(x)) = self.t.get(self.i).cloned() {
            if !["NULL", "TRUE", "FALSE"].iter().any(|k| x.eq_ignore_ascii_case(k)) {
                self.i += 1;
                return Some(SqlV::Col(x));
            }
        }
        self.value()
    }
    fn set_expr(&mut self) -> Option<SqlV> {
        let a = self.set_operand()?;
        for op in ["+", "-", "*", "||"] {
            if self.eat_p(op) {
                let b = self.set_operand()?;
                return Some(SqlV::Bin(Box::new(a), op.to_string(), Box::new(b)));
            }
        }
        Some(a)
    }
    fn end(&mut self) -> bool {
        self.eat_p(";");
        self.i >= self.t.len()
    }
    /// `a = ? AND b IS NULL …`; None: not simulated.
    fn conds(&mut self) -> Option<Vec<Cond>> {
        let mut out = Vec::new();
        loop {
            let col = self.ident()?;
            let c = if self.eat_kw("IS") {
                let not = self.eat_kw("NOT");
                if !self.eat_kw("NULL") {
                    return None;
                }
                Cond { col, op: if not { "notnull".into() } else { "null".into() }, val: None }
            } else {
                let op = match self.t.get(self.i)? {
                    Tok::P(x) if matches!(x.as_str(), "=" | "==" | "!=" | "<>" | "<" | "<=" | ">" | ">=") => x.clone(),
                    _ => return None,
                };
                self.i += 1;
                let v = self.value()?;
                Cond { col, op, val: Some(v) }
            };
            out.push(c);
            if !self.eat_kw("AND") {
                break;
            }
        }
        Some(out)
    }
}

/// The text of a result column as sqlite names it (the expression as written).
fn expr_text(toks: &[Tok]) -> String {
    let mut out = String::new();
    for (i, t) in toks.iter().enumerate() {
        let s = match t {
            Tok::Id(x) => x.clone(),
            Tok::Str(x) => format!("'{}'", x.replace('\'', "''")),
            Tok::Num(x) => x.clone(),
            Tok::P(x) => x.clone(),
            Tok::Param(None) => "?".into(),
            Tok::Param(Some(n)) => format!(":{}", n),
        };
        let glue = i > 0 && !matches!(t, Tok::P(x) if x == "(" || x == ")" || x == "," || x == ".") && !matches!(&toks[i - 1], Tok::P(x) if x == "(" || x == ".");
        if glue {
            out.push(' ');
        }
        out.push_str(&s);
    }
    out
}

fn parse(sql: &str) -> Option<Stmt> {
    let t = lex(sql)?;
    if t.is_empty() {
        return Some(Stmt::Nothing);
    }
    let mut p = P { t, i: 0 };
    if p.eat_kw("BEGIN") || p.eat_kw("COMMIT") || p.eat_kw("ROLLBACK") || p.eat_kw("END") || p.eat_kw("PRAGMA") || p.eat_kw("ANALYZE") || p.eat_kw("VACUUM") {
        return Some(Stmt::Nothing);
    }
    if p.eat_kw("CREATE") {
        if p.eat_kw("INDEX") || p.eat_kw("UNIQUE") || p.kw("VIEW") || p.kw("TRIGGER") {
            return if p.kw("VIEW") || p.kw("TRIGGER") { None } else { Some(Stmt::Nothing) };
        }
        p.eat_kw("TEMP");
        p.eat_kw("TEMPORARY");
        if !p.eat_kw("TABLE") {
            return None;
        }
        let if_not = if p.eat_kw("IF") {
            if !(p.eat_kw("NOT") && p.eat_kw("EXISTS")) {
                return None;
            }
            true
        } else {
            false
        };
        let name = p.ident()?;
        if !p.eat_p("(") {
            return None;
        }
        let mut cols = Vec::new();
        let mut pk = None;
        loop {
            if ["PRIMARY", "UNIQUE", "FOREIGN", "CHECK", "CONSTRAINT"].iter().any(|k| p.kw(k)) {
                // a table constraint: skip to its end
                let mut depth = 0;
                while p.i < p.t.len() {
                    if p.p("(") {
                        depth += 1;
                    } else if p.p(")") {
                        if depth == 0 {
                            break;
                        }
                        depth -= 1;
                    } else if p.p(",") && depth == 0 {
                        break;
                    }
                    p.i += 1;
                }
            } else {
                let c = p.ident()?;
                let mut ty = String::new();
                let mut words: Vec<String> = Vec::new();
                let mut depth = 0;
                while p.i < p.t.len() {
                    if p.p("(") {
                        depth += 1;
                    } else if p.p(")") {
                        if depth == 0 {
                            break;
                        }
                        depth -= 1;
                    } else if p.p(",") && depth == 0 {
                        break;
                    }
                    if let Some(Tok::Id(w)) = p.t.get(p.i) {
                        words.push(w.to_uppercase());
                    }
                    p.i += 1;
                }
                for w in &words {
                    if matches!(w.as_str(), "PRIMARY" | "NOT" | "NULL" | "UNIQUE" | "DEFAULT" | "CHECK" | "REFERENCES" | "COLLATE" | "AUTOINCREMENT" | "KEY" | "GENERATED" | "CONSTRAINT") {
                        break;
                    }
                    if !ty.is_empty() {
                        ty.push(' ');
                    }
                    ty.push_str(w);
                }
                let joined = words.join(" ");
                if joined.contains("PRIMARY KEY") && ty == "INTEGER" {
                    pk = Some(cols.len());
                }
                if joined.contains("DEFAULT") || joined.contains("GENERATED") {
                    return None;
                }
                cols.push((c, ty));
            }
            if p.eat_p(",") {
                continue;
            }
            if p.eat_p(")") {
                break;
            }
            return None;
        }
        p.eat_kw("WITHOUT");
        p.eat_kw("ROWID");
        p.eat_kw("STRICT");
        if !p.end() {
            return None;
        }
        return Some(Stmt::Create { name, cols, pk, if_not });
    }
    if p.eat_kw("INSERT") || p.kw("REPLACE") {
        let mut or = None;
        if p.eat_kw("REPLACE") {
            or = Some("REPLACE".to_string());
        } else if p.eat_kw("OR") {
            or = Some(p.ident()?.to_uppercase());
        }
        if !p.eat_kw("INTO") {
            return None;
        }
        let table = p.ident()?;
        let cols = if p.eat_p("(") {
            let mut cs = vec![p.ident()?];
            while p.eat_p(",") {
                cs.push(p.ident()?);
            }
            if !p.eat_p(")") {
                return None;
            }
            Some(cs)
        } else {
            None
        };
        if !p.eat_kw("VALUES") {
            return None;
        }
        let mut rows = Vec::new();
        loop {
            if !p.eat_p("(") {
                return None;
            }
            let mut vs = vec![p.value()?];
            while p.eat_p(",") {
                vs.push(p.value()?);
            }
            if !p.eat_p(")") {
                return None;
            }
            rows.push(vs);
            if !p.eat_p(",") {
                break;
            }
        }
        if !p.end() {
            return None;
        }
        return Some(Stmt::Insert { table, cols, rows, or });
    }
    if p.eat_kw("SELECT") {
        let distinct = p.eat_kw("DISTINCT");
        p.eat_kw("ALL");
        // result columns up to FROM
        let mut cols: Option<Vec<(String, Option<SExpr>)>> = Some(Vec::new());
        let mut star = false;
        loop {
            if p.eat_p("*") {
                star = true;
            } else {
                let st = p.i;
                let mut depth = 0;
                while p.i < p.t.len() {
                    if p.p("(") {
                        depth += 1;
                    } else if p.p(")") {
                        depth -= 1;
                    } else if depth == 0 && (p.p(",") || p.kw("FROM") || p.kw("AS")) {
                        break;
                    }
                    p.i += 1;
                }
                let toks: Vec<Tok> = p.t[st..p.i].to_vec();
                if toks.is_empty() {
                    return None;
                }
                // a plain column: its name (`t.col` → `col`); an expression: its text
                let plain = match &toks[..] {
                    [Tok::Id(x)] => Some(x.clone()),
                    [Tok::Id(_), Tok::P(d), Tok::Id(x)] if d == "." => Some(x.clone()),
                    _ => None,
                };
                let mut name = plain.clone().unwrap_or_else(|| expr_text(&toks));
                if p.eat_kw("AS") {
                    name = match p.t.get(p.i)? {
                        Tok::Id(x) | Tok::Str(x) => x.clone(),
                        _ => return None,
                    };
                    p.i += 1;
                } else if let Some(Tok::Id(x)) = p.t.get(p.i) {
                    if !x.eq_ignore_ascii_case("FROM") {
                        // an alias without AS
                        name = x.clone();
                        p.i += 1;
                    }
                }
                let src = parse_sexpr(&toks);
                if let Some(cs) = cols.as_mut() {
                    cs.push((name, src));
                }
            }
            if !p.eat_p(",") {
                break;
            }
        }
        if star {
            if cols.as_ref().map(|c| !c.is_empty()).unwrap_or(false) {
                return None;
            }
            cols = None;
        }
        if !p.eat_kw("FROM") {
            return None;
        }
        let table = p.ident()?;
        if let Some(Tok::Id(x)) = p.t.get(p.i) {
            if !["WHERE", "ORDER", "LIMIT", "GROUP"].iter().any(|k| x.eq_ignore_ascii_case(k)) {
                // a table alias
                p.i += 1;
            }
        }
        let conds = if p.eat_kw("WHERE") {
            let save = p.i;
            match p.conds() {
                Some(c) => Some(c),
                None => {
                    p.i = save;
                    // an unsupported filter: skip to ORDER / LIMIT, rows unknown
                    while p.i < p.t.len() && !p.kw("ORDER") && !p.kw("LIMIT") && !p.kw("GROUP") {
                        p.i += 1;
                    }
                    None
                }
            }
        } else {
            Some(Vec::new())
        };
        let mut group = Vec::new();
        if p.eat_kw("GROUP") {
            if !p.eat_kw("BY") {
                return None;
            }
            loop {
                group.push(p.ident()?);
                if !p.eat_p(",") {
                    break;
                }
            }
        }
        // (a HAVING filter: the rows are not simulated, the columns are known)
        let mut having = false;
        if p.eat_kw("HAVING") {
            having = true;
            while p.i < p.t.len() && !p.kw("ORDER") && !p.kw("LIMIT") {
                p.i += 1;
            }
        }
        let mut order = Vec::new();
        if p.eat_kw("ORDER") {
            if !p.eat_kw("BY") {
                return None;
            }
            loop {
                let term = match p.t.get(p.i)? {
                    Tok::Num(n) => {
                        let k = n.parse::<usize>().ok()?;
                        p.i += 1;
                        OrdT::Pos(k)
                    }
                    _ => OrdT::Name(p.ident()?),
                };
                let desc = if p.eat_kw("DESC") {
                    true
                } else {
                    p.eat_kw("ASC");
                    false
                };
                order.push((term, desc));
                if !p.eat_p(",") {
                    break;
                }
            }
        }
        let mut limit = None;
        let mut offset = None;
        if p.eat_kw("LIMIT") {
            limit = Some(p.value()?);
            if p.eat_kw("OFFSET") {
                offset = Some(p.value()?);
            } else if p.eat_p(",") {
                // `LIMIT offset, count`
                offset = limit.take();
                limit = Some(p.value()?);
            }
        }
        if !p.end() {
            return None;
        }
        return Some(Stmt::Select { cols, table, conds, group, having, order, limit, offset, distinct });
    }
    if p.eat_kw("UPDATE") {
        let table = p.ident()?;
        if !p.eat_kw("SET") {
            return None;
        }
        let mut sets = Vec::new();
        loop {
            let c = p.ident()?;
            if !p.eat_p("=") {
                return None;
            }
            sets.push((c, p.set_expr()?));
            if !p.eat_p(",") {
                break;
            }
        }
        let conds = if p.eat_kw("WHERE") { p.conds() } else { Some(Vec::new()) };
        if !p.end() {
            return None;
        }
        return Some(Stmt::Update { table, sets, conds });
    }
    if p.eat_kw("DELETE") {
        if !p.eat_kw("FROM") {
            return None;
        }
        let table = p.ident()?;
        let conds = if p.eat_kw("WHERE") { p.conds() } else { Some(Vec::new()) };
        if !p.end() {
            return None;
        }
        return Some(Stmt::Delete { table, conds });
    }
    if p.eat_kw("DROP") {
        if !p.eat_kw("TABLE") {
            return Some(Stmt::Nothing);
        }
        if p.eat_kw("IF") {
            p.eat_kw("EXISTS");
        }
        let table = p.ident()?;
        return Some(Stmt::Drop { table });
    }
    None
}

// --------------------------------------------------------------------- expressions

/// An ORDER BY term: a result column (or a table column) by name, or a result position.
#[derive(Clone, Debug)]
enum OrdT {
    Name(String),
    Pos(usize),
}

/// A result-column expression.
#[derive(Clone, Debug)]
enum SExpr {
    Lit(KeyId),
    Param(Option<String>),
    Col(String),
    Neg(Box<SExpr>),
    Bin(Box<SExpr>, String, Box<SExpr>),
    Func(String, Vec<SExpr>),
    /// an aggregate: name, argument (None: `*`), DISTINCT
    Agg(String, Option<Box<SExpr>>, bool),
}

const AGGS: &[&str] = &["sum", "total", "count", "avg", "min", "max"];

impl SExpr {
    fn is_agg(&self) -> bool {
        match self {
            SExpr::Agg(..) => true,
            SExpr::Neg(a) => a.is_agg(),
            SExpr::Bin(a, _, b) => a.is_agg() || b.is_agg(),
            SExpr::Func(_, xs) => xs.iter().any(|x| x.is_agg()),
            _ => false,
        }
    }
    fn has_positional(&self) -> bool {
        match self {
            SExpr::Param(None) => true,
            SExpr::Neg(a) => a.has_positional(),
            SExpr::Bin(a, _, b) => a.has_positional() || b.has_positional(),
            SExpr::Func(_, xs) => xs.iter().any(|x| x.has_positional()),
            SExpr::Agg(_, Some(a), _) => a.has_positional(),
            _ => false,
        }
    }
}

/// A result column's tokens as an expression; None when not understood.
fn parse_sexpr(toks: &[Tok]) -> Option<SExpr> {
    let mut i = 0;
    let e = sexpr(toks, &mut i, 0)?;
    (i == toks.len()).then_some(e)
}

fn sexpr(t: &[Tok], i: &mut usize, min: u8) -> Option<SExpr> {
    let prec = |op: &str| -> Option<u8> {
        match op {
            "||" => Some(3),
            "*" | "/" | "%" => Some(2),
            "+" | "-" => Some(1),
            _ => None,
        }
    };
    let mut lhs = satom(t, i)?;
    while let Some(Tok::P(op)) = t.get(*i) {
        let Some(pr) = prec(op) else { break };
        if pr < min {
            break;
        }
        let op = op.clone();
        *i += 1;
        let rhs = sexpr(t, i, pr + 1)?;
        lhs = SExpr::Bin(Box::new(lhs), op, Box::new(rhs));
    }
    Some(lhs)
}

fn satom(t: &[Tok], i: &mut usize) -> Option<SExpr> {
    let tok = t.get(*i)?.clone();
    *i += 1;
    Some(match tok {
        Tok::P(x) if x == "(" => {
            let e = sexpr(t, i, 0)?;
            match t.get(*i)? {
                Tok::P(c) if c == ")" => *i += 1,
                _ => return None,
            }
            e
        }
        Tok::P(x) if x == "-" => SExpr::Neg(Box::new(satom(t, i)?)),
        Tok::P(x) if x == "+" => satom(t, i)?,
        Tok::Num(n) => match n.parse::<i64>() {
            Ok(v) => SExpr::Lit(intern::key_int(v)),
            Err(_) => SExpr::Lit(intern::key_float(n.parse::<f64>().ok()?)),
        },
        Tok::Str(s) => SExpr::Lit(key_str_lit(&s)),
        Tok::Param(n) => SExpr::Param(n),
        Tok::Id(x) if x.eq_ignore_ascii_case("NULL") => SExpr::Lit(intern::key_none()),
        Tok::Id(x) if x.eq_ignore_ascii_case("TRUE") => SExpr::Lit(intern::key_int(1)),
        Tok::Id(x) if x.eq_ignore_ascii_case("FALSE") => SExpr::Lit(intern::key_int(0)),
        Tok::Id(f) if matches!(t.get(*i), Some(Tok::P(p)) if p == "(") => {
            *i += 1;
            let name = f.to_ascii_lowercase();
            let mut distinct = false;
            if matches!(t.get(*i), Some(Tok::Id(d)) if d.eq_ignore_ascii_case("DISTINCT")) {
                distinct = true;
                *i += 1;
            }
            let mut args: Vec<SExpr> = Vec::new();
            let mut star = false;
            if matches!(t.get(*i), Some(Tok::P(p)) if p == "*") {
                star = true;
                *i += 1;
            } else if !matches!(t.get(*i), Some(Tok::P(p)) if p == ")") {
                loop {
                    args.push(sexpr(t, i, 0)?);
                    if matches!(t.get(*i), Some(Tok::P(p)) if p == ",") {
                        *i += 1;
                        continue;
                    }
                    break;
                }
            }
            match t.get(*i)? {
                Tok::P(c) if c == ")" => *i += 1,
                _ => return None,
            }
            // `min(a, b)` / `max(a, b)` are scalar
            if AGGS.contains(&name.as_str()) && (args.len() <= 1) {
                if star && name != "count" {
                    return None;
                }
                SExpr::Agg(name, if star { None } else { Some(Box::new(args.into_iter().next()?)) }, distinct)
            } else {
                if star || distinct {
                    return None;
                }
                SExpr::Func(name, args)
            }
        }
        Tok::Id(x) => {
            // `t.col`: the column
            if matches!(t.get(*i), Some(Tok::P(d)) if d == ".") {
                *i += 1;
                match t.get(*i)? {
                    Tok::Id(c) => {
                        *i += 1;
                        SExpr::Col(c.clone())
                    }
                    _ => return None,
                }
            } else {
                SExpr::Col(x)
            }
        }
        _ => return None,
    })
}

struct Env<'a> {
    cols: &'a [(String, String)],
    ps: &'a [Value],
    named: &'a IndexMap<String, Value>,
}

fn is_float_key(k: KeyId) -> bool {
    intern::with_key(k, |kk| matches!(kk, intern::Key::Float(_)))
}

fn num_f(k: KeyId) -> Option<f64> {
    intern::with_key(k, |kk| match kk {
        intern::Key::Int(i) => Some(*i as f64),
        intern::Key::Float(b) => Some(f64::from_bits(*b)),
        _ => None,
    })
}

/// sqlite arithmetic on known values: NULL in, NULL out; integers stay integers (None on
/// overflow; `/` truncates, by zero is NULL), a real makes a real; `||` of texts / integers.
fn sql_arith(op: &str, x: KeyId, y: KeyId) -> Option<KeyId> {
    if x == intern::key_none() || y == intern::key_none() {
        return Some(intern::key_none());
    }
    if op == "||" {
        let txt = |k: KeyId| intern::key_as_str(k).map(|s| s.to_string()).or_else(|| if is_float_key(k) { None } else { intern::key_as_int(k).map(|i| i.to_string()) });
        return Some(key_str_lit(&format!("{}{}", txt(x)?, txt(y)?)));
    }
    num_f(x)?;
    num_f(y)?;
    if !is_float_key(x) && !is_float_key(y) {
        let (i, j) = (intern::key_as_int(x)?, intern::key_as_int(y)?);
        return match op {
            "+" => i.checked_add(j).map(intern::key_int),
            "-" => i.checked_sub(j).map(intern::key_int),
            "*" => i.checked_mul(j).map(intern::key_int),
            "/" | "%" if j == 0 => Some(intern::key_none()),
            "/" => i.checked_div(j).map(intern::key_int),
            "%" => i.checked_rem(j).map(intern::key_int),
            _ => None,
        };
    }
    let (p, q) = (num_f(x)?, num_f(y)?);
    let r = match op {
        "+" => p + q,
        "-" => p - q,
        "*" => p * q,
        "/" if q == 0.0 => return Some(intern::key_none()),
        "/" => p / q,
        _ => return None,
    };
    r.is_finite().then(|| intern::key_float(r))
}

/// `round(x, n)`: a real; None near a tie (sqlite's decimal rounding is not reproduced there).
fn sql_round(x: KeyId, n: i64) -> Option<KeyId> {
    if x == intern::key_none() {
        return Some(intern::key_none());
    }
    let v = num_f(x)?;
    if !(0..=10).contains(&n) || v.abs() > 1e15 {
        return None;
    }
    let m = 10f64.powi(n as i32);
    let s = v.abs() * m;
    if (s - s.floor() - 0.5).abs() < 1e-6 {
        return None;
    }
    let r = (s + 0.5).floor() / m;
    Some(intern::key_float(if v < 0.0 { -r } else { r }))
}

fn scalar_func(name: &str, args: &[Option<KeyId>]) -> Option<KeyId> {
    let none = intern::key_none();
    match (name, args) {
        ("round", [x]) => sql_round((*x)?, 0),
        ("round", [x, n]) => sql_round((*x)?, intern::key_as_int((*n)?)?),
        ("abs", [x]) => {
            let x = (*x)?;
            if x == none {
                return Some(none);
            }
            if is_float_key(x) {
                Some(intern::key_float(num_f(x)?.abs()))
            } else {
                intern::key_as_int(x)?.checked_abs().map(intern::key_int)
            }
        }
        ("upper" | "lower", [x]) => {
            let x = (*x)?;
            if x == none {
                return Some(none);
            }
            let s = intern::key_as_str(x)?;
            Some(key_str_lit(&if name == "upper" { s.to_ascii_uppercase() } else { s.to_ascii_lowercase() }))
        }
        ("length", [x]) => {
            let x = (*x)?;
            if x == none {
                return Some(none);
            }
            Some(intern::key_int(intern::key_as_str(x)?.chars().count() as i64))
        }
        ("coalesce" | "ifnull", xs) if xs.len() >= 2 => {
            for x in xs {
                let x = (*x)?;
                if x != none {
                    return Some(x);
                }
            }
            Some(none)
        }
        ("min" | "max", xs) if xs.len() >= 2 => {
            let mut best: Option<KeyId> = None;
            for x in xs {
                let x = (*x)?;
                if x == none {
                    return Some(none);
                }
                best = Some(match best {
                    None => x,
                    Some(b) => {
                        let o = sql_cmp(x, b)?;
                        if (name == "max" && o.is_gt()) || (name == "min" && o.is_lt()) { x } else { b }
                    }
                });
            }
            best
        }
        _ => None,
    }
}

fn eval_row(e: &SExpr, env: &Env, row: &[Option<KeyId>]) -> Option<KeyId> {
    match e {
        SExpr::Lit(k) => Some(*k),
        SExpr::Param(_) => param_value(env.ps, env.named, &mut 0, &match e {
            SExpr::Param(n) => SqlV::Param(n.clone()),
            _ => unreachable!(),
        })
        .and_then(|x| lit(&x)),
        SExpr::Col(c) => row[env.cols.iter().position(|(n, _)| n.eq_ignore_ascii_case(c))?],
        SExpr::Neg(a) => sql_arith("-", intern::key_int(0), eval_row(a, env, row)?),
        SExpr::Bin(a, op, b) => sql_arith(op, eval_row(a, env, row)?, eval_row(b, env, row)?),
        SExpr::Func(f, xs) => scalar_func(f, &xs.iter().map(|x| eval_row(x, env, row)).collect::<Vec<_>>()),
        SExpr::Agg(..) => None,
    }
}

/// An expression over one group: aggregates over its rows, grouped columns their key.
fn eval_group(e: &SExpr, env: &Env, gcols: &[(usize, KeyId)], rows: &[Vec<Option<KeyId>>]) -> Option<KeyId> {
    match e {
        SExpr::Col(c) => {
            let i = env.cols.iter().position(|(n, _)| n.eq_ignore_ascii_case(c))?;
            // (a bare column outside GROUP BY: sqlite picks some row)
            gcols.iter().find(|(j, _)| *j == i).map(|(_, k)| *k)
        }
        SExpr::Lit(_) | SExpr::Param(_) => eval_row(e, env, &[]),
        SExpr::Neg(a) => sql_arith("-", intern::key_int(0), eval_group(a, env, gcols, rows)?),
        SExpr::Bin(a, op, b) => sql_arith(op, eval_group(a, env, gcols, rows)?, eval_group(b, env, gcols, rows)?),
        SExpr::Func(f, xs) => scalar_func(f, &xs.iter().map(|x| eval_group(x, env, gcols, rows)).collect::<Vec<_>>()),
        SExpr::Agg(name, arg, distinct) => {
            let none = intern::key_none();
            let Some(arg) = arg else { return Some(intern::key_int(rows.len() as i64)) };
            let mut vals: Vec<KeyId> = Vec::new();
            for r in rows {
                let v = eval_row(arg, env, r)?;
                if v != none && !(*distinct && vals.contains(&v)) {
                    vals.push(v);
                }
            }
            match name.as_str() {
                "count" => Some(intern::key_int(vals.len() as i64)),
                "min" | "max" => {
                    let mut best: Option<KeyId> = None;
                    for v in vals {
                        best = Some(match best {
                            None => v,
                            Some(b) => {
                                let o = sql_cmp(v, b)?;
                                if (name == "max" && o.is_gt()) || (name == "min" && o.is_lt()) { v } else { b }
                            }
                        });
                    }
                    Some(best.unwrap_or(none))
                }
                "sum" | "total" | "avg" => {
                    if vals.iter().any(|v| num_f(*v).is_none()) {
                        return None;
                    }
                    if vals.is_empty() {
                        return Some(if name == "total" { intern::key_float(0.0) } else { none });
                    }
                    let all_int = vals.iter().all(|v| !is_float_key(*v));
                    if all_int && name == "sum" {
                        let mut s: i64 = 0;
                        for v in &vals {
                            s = s.checked_add(intern::key_as_int(*v)?)?;
                        }
                        return Some(intern::key_int(s));
                    }
                    // (sqlite ≥ 3.43: Kahan–Babuška–Neumaier summation of reals)
                    let (mut sum, mut err) = (0.0f64, 0.0f64);
                    for v in &vals {
                        let x = num_f(*v)?;
                        let t = sum + x;
                        if sum.abs() >= x.abs() {
                            err += (sum - t) + x;
                        } else {
                            err += (x - t) + sum;
                        }
                        sum = t;
                    }
                    let total = sum + err;
                    let r = if name == "avg" { total / vals.len() as f64 } else { total };
                    r.is_finite().then(|| intern::key_float(r))
                }
                _ => None,
            }
        }
    }
}

// --------------------------------------------------------------------- values

/// A SET expression with its parameters bound: a value (None: unknown), a column, `a op b`.
enum SetV {
    K(Option<KeyId>),
    Col(usize),
    Bin(Box<SetV>, String, Box<SetV>),
}

fn resolve_set(v: &SqlV, cols: &[(String, String)], ps: &[Value], named: &IndexMap<String, Value>, pos: &mut usize) -> Option<SetV> {
    Some(match v {
        SqlV::Col(c) => SetV::Col(cols.iter().position(|(n, _)| n.eq_ignore_ascii_case(c))?),
        SqlV::Bin(a, op, b) => {
            let a = resolve_set(a, cols, ps, named, pos)?;
            let b = resolve_set(b, cols, ps, named, pos)?;
            SetV::Bin(Box::new(a), op.clone(), Box::new(b))
        }
        _ => SetV::K(param_value(ps, named, pos, v).and_then(|x| lit(&x))),
    })
}

/// sqlite arithmetic on stored values: NULL in, NULL out; integers stay integers (None on
/// overflow), a real makes a real; `||` of texts; anything else is not simulated (None).
fn eval_set(v: &SetV, row: &[Option<KeyId>]) -> Option<KeyId> {
    match v {
        SetV::K(k) => *k,
        SetV::Col(i) => row[*i],
        SetV::Bin(a, op, b) => {
            let (x, y) = (eval_set(a, row)?, eval_set(b, row)?);
            if x == intern::key_none() || y == intern::key_none() {
                return Some(intern::key_none());
            }
            if op == "||" {
                return Some(key_str_lit(&format!("{}{}", intern::key_as_str(x)?, intern::key_as_str(y)?)));
            }
            let fl = |k: KeyId| intern::with_key(k, |kk| match kk {
                intern::Key::Int(i) => Some(*i as f64),
                intern::Key::Float(b) => Some(f64::from_bits(*b)),
                _ => None,
            });
            match (intern::key_as_int(x).filter(|_| fl(x).is_some()), intern::key_as_int(y).filter(|_| fl(y).is_some()), intern::with_key(x, |kk| matches!(kk, intern::Key::Float(_))) || intern::with_key(y, |kk| matches!(kk, intern::Key::Float(_)))) {
                (Some(i), Some(j), false) => match op.as_str() {
                    "+" => i.checked_add(j),
                    "-" => i.checked_sub(j),
                    _ => i.checked_mul(j),
                }
                .map(intern::key_int),
                _ => {
                    let (p, q) = (fl(x)?, fl(y)?);
                    let r = match op.as_str() {
                        "+" => p + q,
                        "-" => p - q,
                        _ => p * q,
                    };
                    r.is_finite().then(|| intern::key_float(r))
                }
            }
        }
    }
}

/// sqlite's type affinity applied to a stored value.
fn affinity(ty: &str, k: KeyId) -> KeyId {
    let t = ty.to_uppercase();
    let s = intern::key_as_str(k);
    let num = |s: &str| -> Option<KeyId> {
        let s = s.trim();
        if let Ok(i) = s.parse::<i64>() {
            return Some(intern::key_int(i));
        }
        s.parse::<f64>().ok().filter(|f| f.is_finite()).map(|f| if f.fract() == 0.0 && f.abs() < 9.0e15 { intern::key_int(f as i64) } else { intern::key_float(f) })
    };
    if t.contains("INT") {
        if let Some(s) = &s {
            if let Some(n) = num(s) {
                return n;
            }
        }
        // a float that is integral stays a float in INTEGER? sqlite stores 1.0 as 1
        if let Some(f) = intern::with_key(k, |kk| if let intern::Key::Float(b) = kk { Some(f64::from_bits(*b)) } else { None }) {
            if f.fract() == 0.0 && f.abs() < 9.0e15 {
                return intern::key_int(f as i64);
            }
        }
        return k;
    }
    if t.contains("CHAR") || t.contains("CLOB") || t.contains("TEXT") {
        if intern::key_is_number(k) && intern::with_key(k, |kk| !matches!(kk, intern::Key::Bool(_))) {
            return key_str_lit(&intern::key_py_str(k));
        }
        if intern::with_key(k, |kk| matches!(kk, intern::Key::Bool(_))) {
            return key_str_lit(if k == intern::key_bool(true) { "1" } else { "0" });
        }
        return k;
    }
    if t.contains("REAL") || t.contains("FLOA") || t.contains("DOUB") {
        if let Some(s) = &s {
            if let Some(n) = num(s) {
                return intern::key_float(intern::with_key(n, |kk| match kk {
                    intern::Key::Int(i) => *i as f64,
                    intern::Key::Float(b) => f64::from_bits(*b),
                    _ => 0.0,
                }));
            }
        }
        if let Some(i) = intern::key_as_int(k) {
            if !intern::with_key(k, |kk| matches!(kk, intern::Key::Bool(_))) {
                return intern::key_float(i as f64);
            }
        }
        return k;
    }
    if t.is_empty() || t.contains("BLOB") {
        if intern::with_key(k, |kk| matches!(kk, intern::Key::Bool(_))) {
            return intern::key_int((k == intern::key_bool(true)) as i64);
        }
        return k;
    }
    // NUMERIC
    if let Some(s) = &s {
        if let Some(n) = num(s) {
            return n;
        }
    }
    if intern::with_key(k, |kk| matches!(kk, intern::Key::Bool(_))) {
        return intern::key_int((k == intern::key_bool(true)) as i64);
    }
    k
}

/// sqlite's ordering of values: NULL < numbers < text < blobs.
fn sql_cmp(a: KeyId, b: KeyId) -> Option<std::cmp::Ordering> {
    let class = |k: KeyId| -> u8 {
        if k == intern::key_none() {
            0
        } else if intern::key_is_number(k) {
            1
        } else if intern::key_as_str(k).is_some() {
            2
        } else {
            3
        }
    };
    let (ca, cb) = (class(a), class(b));
    if ca != cb {
        return Some(ca.cmp(&cb));
    }
    match ca {
        0 => Some(std::cmp::Ordering::Equal),
        1 => crate::interp::compare::literal_order(a, b),
        2 => Some(intern::key_as_str(a)?.cmp(&intern::key_as_str(b)?)),
        _ => None,
    }
}

// --------------------------------------------------------------------- objects

/// A list / tuple of known items allocated with its own context (several at one node).
fn alloc_ctx(an: &mut Analyzer, kind: Kind, node: &Node, items: Vec<Value>, ctx: &str, st: &mut State) -> Value {
    let oid = an.alloc_at(kind, node, st, &format!("{}@L{}", kind.name(), node.line), Payload::None, None, Ns::Item, None, Some(ctx));
    let acc = join_values(items.clone());
    for (j, v) in items.iter().enumerate() {
        st.set_slot(oid, Ns::Pos, intern::key_int(j as i64), v.clone(), true);
    }
    st.set_slot(oid, Ns::Elem, intern::STAR, Value::with_kv(acc.pts.clone(), DATA), true);
    st.elems.insert(oid, if items.is_empty() { KeyVal::Lits(KeySet::new()) } else { acc.kv });
    an.heap.set_llen(oid, st, Some(items.len()));
    Value::obj(oid)
}

fn new_obj(an: &mut Analyzer, st: &mut State, node: &Node, kind: &str, label: &str, ctx: &str) -> u32 {
    let p = Payload::Method(format!("stdobj.{}", kind), Value::new());
    let oid = an.alloc_at(Kind::Namespace, node, st, label, p.clone(), Some(Row::new()), Ns::Attr, None, Some(ctx));
    an.heap.obj_mut(oid).payload = p;
    st.set_row(oid, Ns::Attr, Row::new());
    oid
}

/// `sqlite3.connect(path)`.
pub fn m_connect(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let path = c.arg(0, Some("database")).and_then(|v| v.kv.as_lit().filter(|_| v.pts.is_empty())).and_then(intern::key_as_str);
    let memory = path.as_deref() == Some(":memory:");
    let oid = new_obj(an, st, &c.node, "sqlcon", &format!("sqlite3.connect@L{}", c.line), "con");
    an.m.sql_dbs.insert(oid, Db { memory, tables: IndexMap::new(), line: c.line });
    let rf = key_str_lit("row_factory");
    st.set_row(oid, Ns::Attr, Row::new().store(rf, true));
    st.set_slot(oid, Ns::Attr, rf, an.none_v.clone(), true);
    Value::obj(oid)
}

fn param_value(params: &[Value], named: &IndexMap<String, Value>, pos: &mut usize, p: &SqlV) -> Option<Value> {
    match p {
        SqlV::Lit(k) => Some(Value::lit(*k)),
        SqlV::Param(None) => {
            let v = params.get(*pos).cloned();
            *pos += 1;
            v
        }
        SqlV::Param(Some(n)) => named.get(n).cloned(),
        SqlV::Col(_) | SqlV::Bin(..) => None,
    }
}

fn lit(v: &Value) -> Option<KeyId> {
    v.kv.as_lit().filter(|_| v.pts.is_empty())
}

/// The parameters of `execute(sql, params)`: a sequence (positional) or a mapping (named).
fn params_of(an: &mut Analyzer, st: &mut State, node: &Node, p: Option<&Value>) -> Option<(Vec<Value>, IndexMap<String, Value>)> {
    let Some(p) = p else { return Some((Vec::new(), IndexMap::new())) };
    if let Some(d) = p.single().filter(|o| an.heap.kind(*o) == Kind::Dict) {
        let row = st.row(d, Ns::Item);
        if row.open() || row.present != row.definite {
            return None;
        }
        let mut named = IndexMap::new();
        for (k, v) in an.literal_dict_items(d, st)? {
            named.insert(intern::key_as_str(k)?.to_string(), v);
        }
        return Some((Vec::new(), named));
    }
    let it = an.iterate(p, node, st);
    Some((it.items?, IndexMap::new()))
}

fn matches_conds(conds: &[Cond], cols: &[(String, String)], row: &[Option<KeyId>], params: &[Value], named: &IndexMap<String, Value>, pos: &mut usize) -> Option<bool> {
    let mut all = true;
    for c in conds {
        let i = cols.iter().position(|(n, _)| n.eq_ignore_ascii_case(&c.col))?;
        let cell = row.get(i).copied()?;
        let ok = match c.op.as_str() {
            "null" => cell? == intern::key_none(),
            "notnull" => cell? != intern::key_none(),
            op => {
                let v = lit(&param_value(params, named, pos, c.val.as_ref()?)?)?;
                let v = affinity(&cols[i].1, v);
                let x = cell?;
                if x == intern::key_none() || v == intern::key_none() {
                    false
                } else {
                    let o = sql_cmp(x, v)?;
                    use std::cmp::Ordering::*;
                    match op {
                        "=" | "==" => o == Equal,
                        "!=" | "<>" => o != Equal,
                        "<" => o == Less,
                        "<=" => o != Greater,
                        ">" => o == Greater,
                        _ => o != Less,
                    }
                }
            }
        };
        all &= ok;
    }
    Some(all)
}

/// Run one statement on the connection's database; the result set of a SELECT.
fn run(an: &mut Analyzer, con: u32, sql: &str, params: Option<(Vec<Value>, IndexMap<String, Value>)>, uncertain: bool) -> Option<Rs> {
    let mut db = an.m.sql_dbs.get(&con).cloned()?;
    let src = format!("sqlite3.connect@L{}", db.line);
    let stmt = parse(sql);
    let unknown_rs = |cols: Option<Vec<String>>, table: &str| Rs { cols, rows: None, source: format!("{}:{}", src, table), rowcount: None };
    let Some(stmt) = stmt else {
        // unsupported SQL: whatever it touched is unknown now
        let low = sql.to_lowercase();
        for (name, t) in db.tables.iter_mut() {
            if low.contains(&name.to_lowercase()) {
                t.rows = None;
            }
        }
        an.m.sql_dbs.insert(con, db);
        return Some(unknown_rs(None, "?"));
    };
    let (params, named) = match params {
        Some(p) => (Some(p.0), p.1),
        None => (None, IndexMap::new()),
    };
    let mut out = Rs::default();
    match stmt {
        Stmt::Nothing => {}
        Stmt::Create { name, cols, pk, if_not } => {
            if !(if_not && db.tables.contains_key(&name)) {
                let rows = if db.memory { Some(Vec::new()) } else { None };
                db.tables.insert(name, Table { cols, rows, next_rowid: 1, pk });
            }
        }
        Stmt::Drop { table } => {
            db.tables.shift_remove(&table);
        }
        Stmt::Insert { table, cols, rows, or } => {
            let Some(t) = db.tables.get_mut(&table) else { return Some(out) };
            let targets: Option<Vec<usize>> = match &cols {
                Some(cs) => cs.iter().map(|c| t.cols.iter().position(|(n, _)| n.eq_ignore_ascii_case(c))).collect(),
                None => Some((0..t.cols.len()).collect()),
            };
            match (targets, params.as_ref(), uncertain || or.is_some()) {
                (Some(targets), Some(ps), false) if t.rows.is_some() => {
                    for vals in &rows {
                        if vals.len() != targets.len() {
                            t.rows = None;
                            break;
                        }
                        let mut pos = 0;
                        let mut new: Vec<Option<KeyId>> = vec![Some(intern::key_none()); t.cols.len()];
                        for (j, v) in vals.iter().enumerate() {
                            let ci = targets[j];
                            new[ci] = param_value(ps, &named, &mut pos, v).and_then(|x| lit(&x)).map(|k| affinity(&t.cols[ci].1, k));
                        }
                        if let Some(pk) = t.pk {
                            match new[pk] {
                                Some(k) if k == intern::key_none() => {
                                    new[pk] = Some(intern::key_int(t.next_rowid));
                                    t.next_rowid += 1;
                                }
                                Some(k) => match intern::key_as_int(k) {
                                    Some(i) => t.next_rowid = t.next_rowid.max(i + 1),
                                    None => new[pk] = None,
                                },
                                None => {}
                            }
                        }
                        t.rows.as_mut().unwrap().push(new);
                    }
                    if t.rows.is_some() {
                        out.rowcount = Some(rows.len() as i64);
                    }
                }
                _ => t.rows = None,
            }
        }
        Stmt::Update { table, sets, conds } => {
            if let Some(t) = db.tables.get_mut(&table) {
                let ok = (|| -> Option<()> {
                    let conds = conds?;
                    let ps = params.as_ref()?;
                    let rows = t.rows.as_mut()?;
                    let mut pos0 = 0;
                    let mut set_vals: Vec<(usize, SetV)> = Vec::new();
                    for (c, v) in sets.iter() {
                        let ci = t.cols.iter().position(|(n, _)| n.eq_ignore_ascii_case(c))?;
                        set_vals.push((ci, resolve_set(v, &t.cols, ps, &named, &mut pos0)?));
                    }
                    let mut n = 0;
                    for r in rows.iter_mut() {
                        let mut pos = pos0;
                        if matches_conds(&conds, &t.cols, r, ps, &named, &mut pos)? {
                            n += 1;
                            let old = r.clone();
                            for (ci, v) in &set_vals {
                                let new = eval_set(v, &old).map(|k| affinity(&t.cols[*ci].1, k));
                                // (a statement that may not run: a changed cell is unknown)
                                r[*ci] = if uncertain && new != old[*ci] { None } else { new };
                            }
                        }
                    }
                    out.rowcount = Some(n);
                    Some(())
                })();
                if ok.is_none() {
                    t.rows = None;
                }
            }
        }
        Stmt::Delete { table, conds } => {
            if let Some(t) = db.tables.get_mut(&table) {
                let ok = (|| -> Option<()> {
                    let conds = conds?;
                    let ps = params.as_ref()?;
                    let rows = t.rows.as_ref()?;
                    let mut keep = Vec::new();
                    for r in rows.iter() {
                        let mut pos = 0;
                        if !matches_conds(&conds, &t.cols, r, ps, &named, &mut pos)? {
                            keep.push(r.clone());
                        }
                    }
                    out.rowcount = Some((rows.len() - keep.len()) as i64);
                    // (a statement that may not run and removes rows: the count is unknown)
                    if uncertain && keep.len() != rows.len() {
                        return None;
                    }
                    t.rows = Some(keep);
                    Some(())
                })();
                if ok.is_none() {
                    t.rows = None;
                }
            }
        }
        Stmt::Select { cols, table, conds, group, having, order, limit, offset, distinct } => {
            let t = db.tables.get(&table).cloned();
            // the result columns: as named, or the table's
            let names: Option<Vec<String>> = match (&cols, &t) {
                (Some(cs), _) => Some(cs.iter().map(|(n, _)| n.clone()).collect()),
                (None, Some(t)) => Some(t.cols.iter().map(|(n, _)| n.clone()).collect()),
                (None, None) => None,
            };
            out = unknown_rs(names.clone(), &table);
            let Some(t) = t else {
                an.m.sql_dbs.insert(con, db);
                return Some(out);
            };
            let rows = (|| -> Option<Vec<Vec<Option<KeyId>>>> {
                if having {
                    return None;
                }
                let conds = conds?;
                let ps = params.clone()?;
                let mut sel: Vec<Vec<Option<KeyId>>> = Vec::new();
                let mut sel_rowid: Vec<usize> = Vec::new();
                for (ri, r) in t.rows.as_ref()?.iter().enumerate() {
                    let mut pos = 0;
                    if matches_conds(&conds, &t.cols, r, &ps, &named, &mut pos)? {
                        sel.push(r.clone());
                        sel_rowid.push(ri);
                    }
                }
                // the positional parameters after the WHERE's: LIMIT / OFFSET
                let mut lpos = conds.iter().filter(|c| matches!(c.val, Some(SqlV::Param(None)))).count();
                let ex: Option<Vec<SExpr>> = match &cols {
                    None => None,
                    Some(cs) => Some(cs.iter().map(|(_, e)| e.clone()).collect::<Option<Vec<_>>>()?),
                };
                let env = Env { cols: &t.cols, ps: &ps, named: &named };
                if ex.as_ref().map(|es| es.iter().any(|e| e.has_positional())).unwrap_or(false) {
                    return None;
                }
                let grouped = !group.is_empty() || ex.as_ref().map(|es| es.iter().any(|e| e.is_agg())).unwrap_or(false);
                // result rows, each with the source row it came from (ungrouped)
                let mut res: Vec<(Vec<Option<KeyId>>, Option<Vec<Option<KeyId>>>)> = Vec::new();
                if grouped {
                    let es = ex.as_ref()?;
                    let gidx: Vec<usize> = group.iter().map(|g| t.cols.iter().position(|(n, _)| n.eq_ignore_ascii_case(g))).collect::<Option<Vec<_>>>()?;
                    let mut groups: Vec<(Vec<KeyId>, Vec<Vec<Option<KeyId>>>)> = Vec::new();
                    if gidx.is_empty() {
                        groups.push((Vec::new(), sel.clone()));
                    } else {
                        for r in &sel {
                            let key: Vec<KeyId> = gidx.iter().map(|i| r[*i]).collect::<Option<Vec<_>>>()?;
                            match groups.iter_mut().find(|(k, _)| *k == key) {
                                Some((_, rs)) => rs.push(r.clone()),
                                None => groups.push((key, vec![r.clone()])),
                            }
                        }
                        // (sqlite's GROUP BY sorter: groups in key order)
                        let mut ok = true;
                        groups.sort_by(|a, b| {
                            for (x, y) in a.0.iter().zip(b.0.iter()) {
                                match sql_cmp(*x, *y) {
                                    Some(std::cmp::Ordering::Equal) => continue,
                                    Some(o) => return o,
                                    None => {
                                        ok = false;
                                        return std::cmp::Ordering::Equal;
                                    }
                                }
                            }
                            std::cmp::Ordering::Equal
                        });
                        if !ok {
                            return None;
                        }
                    }
                    for (key, rs) in &groups {
                        let gcols: Vec<(usize, KeyId)> = gidx.iter().copied().zip(key.iter().copied()).collect();
                        let vals: Vec<Option<KeyId>> = es.iter().map(|e| eval_group(e, &env, &gcols, rs)).collect();
                        res.push((vals, None));
                    }
                } else {
                    for (ri, r) in sel.iter().enumerate() {
                        let vals: Vec<Option<KeyId>> = match &ex {
                            None => r.clone(),
                            Some(es) => es.iter().map(|e| eval_row(e, &env, r)).collect(),
                        };
                        // (the source row, then its rowid: the INTEGER PRIMARY KEY, else the
                        // insertion order, which rowids follow)
                        let mut src = r.clone();
                        src.push(match t.pk {
                            Some(pk) => r[pk],
                            None => Some(intern::key_int(sel_rowid[ri] as i64)),
                        });
                        res.push((vals, Some(src)));
                    }
                }
                if distinct {
                    let mut seen: Vec<(Vec<Option<KeyId>>, Option<Vec<Option<KeyId>>>)> = Vec::new();
                    for r in res {
                        if r.0.iter().any(|x| x.is_none()) {
                            return None;
                        }
                        if !seen.iter().any(|s| s.0 == r.0) {
                            seen.push(r);
                        }
                    }
                    res = seen;
                }
                // ORDER BY: a result column (name / position), or a column of the table
                let rnames = names.clone()?;
                for (term, desc) in order.iter().rev() {
                    let pick: Box<dyn Fn(&(Vec<Option<KeyId>>, Option<Vec<Option<KeyId>>>)) -> Option<KeyId>> = match term {
                        OrdT::Pos(k) if *k >= 1 && *k <= rnames.len() => {
                            let i = k - 1;
                            Box::new(move |r| r.0[i])
                        }
                        OrdT::Pos(_) => return None,
                        OrdT::Name(n) => match rnames.iter().position(|x| x.eq_ignore_ascii_case(n)) {
                            Some(i) => Box::new(move |r| r.0[i]),
                            None => {
                                let rowid = ["rowid", "oid", "_rowid_"].iter().any(|x| n.eq_ignore_ascii_case(x));
                                let i = match t.cols.iter().position(|(c, _)| c.eq_ignore_ascii_case(n)) {
                                    Some(i) => i,
                                    None if rowid => t.cols.len(),
                                    None => return None,
                                };
                                Box::new(move |r| r.1.as_ref().and_then(|s| s[i]))
                            }
                        },
                    };
                    let keys: Vec<KeyId> = res.iter().map(|r| pick(r)).collect::<Option<Vec<_>>>()?;
                    let mut idx: Vec<usize> = (0..res.len()).collect();
                    let mut ok = true;
                    // (stable: ties keep their order, descending too)
                    idx.sort_by(|a, b| match sql_cmp(keys[*a], keys[*b]) {
                        Some(o) => {
                            if *desc {
                                o.reverse()
                            } else {
                                o
                            }
                        }
                        None => {
                            ok = false;
                            std::cmp::Ordering::Equal
                        }
                    });
                    if !ok {
                        return None;
                    }
                    res = idx.into_iter().map(|i| res[i].clone()).collect();
                }
                let num = |v: &Option<SqlV>, lpos: &mut usize| -> Option<Option<i64>> {
                    match v {
                        None => Some(None),
                        Some(v) => {
                            let x = param_value(&ps, &named, lpos, v).and_then(|x| lit(&x))?;
                            Some(Some(intern::key_as_int(x)?))
                        }
                    }
                };
                let lim = num(&limit, &mut lpos)?;
                let off = num(&offset, &mut lpos)?;
                let proj: Vec<Vec<Option<KeyId>>> = res.into_iter().map(|r| r.0).skip(off.unwrap_or(0).max(0) as usize).collect();
                Some(match lim {
                    Some(l) if l >= 0 => proj.into_iter().take(l as usize).collect(),
                    _ => proj,
                })
            })();
            out.rows = rows;
        }
    }
    an.m.sql_dbs.insert(con, db);
    Some(out)
}

/// One result row as the row factory makes it.
fn row_value(an: &mut Analyzer, st: &mut State, node: &Node, factory: &Value, cur: u32, rs: &Rs, row: Option<&Vec<Option<KeyId>>>, i: usize) -> Value {
    let cols = rs.cols.clone();
    let vals: Option<Vec<Value>> = match (row, &cols) {
        (Some(r), _) => Some(r.iter().map(|x| x.map(Value::lit).unwrap_or_else(|| Value::data("sql"))).collect()),
        (None, Some(cs)) => Some(cs.iter().map(|_| Value::data("sql")).collect()),
        (None, None) => None,
    };
    let is_row = factory.pts.iter().any(|o| an.heap.kind(o) == Kind::External && an.heap.external_qual(o) == "sqlite3.Row");
    let is_none = factory.pts.is_empty() && factory.kv.as_lit() == Some(intern::key_none());
    if is_row {
        let did = an.alloc_at(Kind::Dict, node, st, &format!("sqlite3.Row@L{}", node.line), Payload::None, Some(Row::new()), Ns::Item, None, Some(&format!("sqlrow{}", i)));
        match (&cols, &vals) {
            (Some(cs), Some(vs)) => {
                let mut r = Row::new();
                for (c, v) in cs.iter().zip(vs.iter()) {
                    let k = key_str_lit(c);
                    if r.present.contains(k) {
                        continue;
                    }
                    r = r.store(k, true);
                    st.set_slot(did, Ns::Item, k, v.clone(), true);
                }
                st.set_row(did, Ns::Item, r);
                an.m.sql_rows.insert(did, cs.clone());
            }
            _ => {
                st.set_row(did, Ns::Item, Row::with_source(Source::new(&rs.source, SourceKind::External)));
            }
        }
        return Value::obj(did);
    }
    let tuple = match &vals {
        Some(vs) => alloc_ctx(an, Kind::Tuple, node, vs.clone(), &format!("sqltup{}", i), st),
        None => an.alloc_positional(Kind::Tuple, node, None, Value::data("sql"), DATA, st),
    };
    if is_none {
        return tuple;
    }
    // a user row factory: called with the cursor and the tuple (each row its own context)
    let mut nd = *node;
    nd.col = node.col.wrapping_add(3_000_000 + i as u32);
    an.call_value(factory, &nd, None, st, Some(vec![Value::obj(cur), tuple]))
}

fn factory_of(an: &Analyzer, st: &State, con: u32) -> Value {
    let _ = an;
    st.slot(con, Ns::Attr, key_str_lit("row_factory"))
}

/// The cursor object of an executed statement.
fn cursor_obj(an: &mut Analyzer, st: &mut State, node: &Node, con: u32, rs: Rs, existing: Option<u32>) -> Value {
    let cur = match existing {
        Some(c) => c,
        None => new_obj(an, st, node, "sqlcur", &format!("Cursor@L{}", node.line), "cur"),
    };
    let desc = match &rs.cols {
        Some(cs) => {
            let none = an.none_v.clone();
            let items: Vec<Value> = cs.iter().enumerate().map(|(i, c)| {
                let mut vs = vec![Value::lit_str(c)];
                vs.extend(std::iter::repeat_n(none.clone(), 6));
                alloc_ctx(an, Kind::Tuple, node, vs, &format!("sqldesc{}", i), st)
            }).collect();
            alloc_ctx(an, Kind::Tuple, node, items, "sqldesc", st)
        }
        None => an.alloc_positional(Kind::Tuple, node, None, Value::data("description"), DATA, st),
    };
    let set = |st: &mut State, n: &str, v: Value| {
        let k = key_str_lit(n);
        let r = st.row(cur, Ns::Attr).store(k, true);
        st.set_row(cur, Ns::Attr, r);
        st.set_slot(cur, Ns::Attr, k, v, true);
    };
    // (a row factory sees the cursor's description)
    set(st, "description", desc);
    set(st, "connection", Value::obj(con));
    let factory = factory_of(an, st, con);
    // the rows as a list (fetchall / iteration), from the cursor's position 0
    let rows: Option<Vec<Value>> = rs.rows.clone().map(|rows| rows.iter().enumerate().map(|(i, r)| row_value(an, st, node, &factory, cur, &rs, Some(r), i)).collect());
    let list = match rows {
        Some(items) => alloc_ctx(an, Kind::List, node, items, "sqlrows", st),
        None => {
            let e = row_value(an, st, node, &factory, cur, &rs, None, 0);
            an.alloc_positional(Kind::List, node, None, e, DATA, st)
        }
    };
    set(st, "\u{1}rows", list);
    set(st, "\u{1}pos", Value::lit(intern::key_int(0)));
    let rc = match (&rs.rows, rs.rowcount) {
        (_, Some(n)) => Value::lit(intern::key_int(n)),
        (Some(_), None) if rs.cols.is_some() => Value::lit(intern::key_int(-1)),
        _ => Value::data("rowcount"),
    };
    set(st, "rowcount", rc);
    an.m.sql_cursors.insert(cur, con);
    Value::obj(cur)
}

/// The rows a cursor still has (`fetchall`, iteration).
pub fn cursor_rows(an: &mut Analyzer, st: &mut State, node: &Node, cur: u32) -> Option<Value> {
    if !an.m.sql_cursors.contains_key(&cur) {
        return None;
    }
    let list = st.slot(cur, Ns::Attr, key_str_lit("\u{1}rows"));
    let pos = lit(&st.slot(cur, Ns::Attr, key_str_lit("\u{1}pos"))).and_then(intern::key_as_int);
    let lo = list.single()?;
    match (pos, an.heap.llen(lo, st)) {
        (Some(0), _) => Some(list),
        (Some(p), Some(n)) => {
            let items: Vec<Value> = (p as usize..n).map(|i| st.slot(lo, Ns::Pos, intern::key_int(i as i64))).collect();
            Some(alloc_ctx(an, Kind::List, node, items, "sqlrest", st))
        }
        _ => {
            let e = an.elem_of(&list, node, st);
            Some(an.alloc_positional(Kind::List, node, None, e, DATA, st))
        }
    }
}

fn exec_on(an: &mut Analyzer, st: &mut State, c: &Call, con: u32, existing: Option<u32>, many: bool) -> Value {
    let sql = c.arg(0, Some("sql")).and_then(|v| lit(&v)).and_then(intern::key_as_str).map(|s| s.to_string());
    let uncertain = an.cond_depth() > 0 || an.frame().loop_depth as usize > an.frame().unroll.len();
    let Some(sql) = sql else {
        // unknown SQL: every table may have changed
        if let Some(db) = an.m.sql_dbs.get_mut(&con) {
            for t in db.tables.values_mut() {
                t.rows = None;
            }
        }
        let line = an.m.sql_dbs.get(&con).map(|d| d.line).unwrap_or(0);
        let rs = Rs { cols: None, rows: None, source: format!("sqlite3.connect@L{}:?", line), rowcount: None };
        return cursor_obj(an, st, &c.node, con, rs, existing);
    };
    let pv = c.arg(1, Some(if many { "seq_of_parameters" } else { "parameters" }));
    let rs = if many {
        let seq = pv.map(|v| an.iterate(&v, &c.node, st).items);
        match seq.flatten() {
            Some(items) => {
                let mut last = Rs::default();
                let mut total: Option<i64> = Some(0);
                for item in items {
                    let ps = params_of(an, st, &c.node, Some(&item));
                    last = run(an, con, &sql, ps, uncertain).unwrap_or_default();
                    total = total.zip(last.rowcount).map(|(a, b)| a + b);
                }
                // (executemany: the rows all of them changed)
                last.rowcount = total;
                last
            }
            None => {
                // unknown parameters (a parameter, a global read in a summary: decided per
                // call site): the statement's tables lose their rows
                run(an, con, &sql, None, true).unwrap_or_default()
            }
        }
    } else {
        let ps = params_of(an, st, &c.node, pv.as_ref());
        run(an, con, &sql, ps, uncertain).unwrap_or_default()
    };
    cursor_obj(an, st, &c.node, con, rs, existing)
}

/// `con.execute(…)`, `cur.fetchone()`, … (`stdobj.sqlcon.NAME`, `stdobj.sqlcur.NAME`).
pub fn m_sql(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = c.qual.rsplit('.').next().unwrap_or("").to_string();
    let Some(o) = c.recv.as_ref().and_then(|r| r.single()) else { return Value::data("sqlite") };
    let is_con = an.m.sql_dbs.contains_key(&o);
    let con = if is_con { Some(o) } else { an.m.sql_cursors.get(&o).copied() };
    let Some(con) = con else { return Value::data("sqlite") };
    match (is_con, name.as_str()) {
        (_, "execute") => exec_on(an, st, c, con, if is_con { None } else { Some(o) }, false),
        (_, "executemany") => exec_on(an, st, c, con, if is_con { None } else { Some(o) }, true),
        (_, "executescript") => {
            if let Some(script) = c.arg(0, Some("sql_script")).and_then(|v| lit(&v)).and_then(intern::key_as_str) {
                let uncertain = an.cond_depth() > 0 || an.frame().loop_depth as usize > an.frame().unroll.len();
                for part in script.split(';') {
                    if !part.trim().is_empty() {
                        run(an, con, part, Some((Vec::new(), IndexMap::new())), uncertain);
                    }
                }
            } else if let Some(db) = an.m.sql_dbs.get_mut(&con) {
                db.tables.clear();
            }
            Value::obj(o)
        }
        (true, "cursor") => cursor_obj(an, st, &c.node, con, Rs::default(), None),
        (true, "commit" | "rollback" | "close" | "interrupt") | (false, "close") => an.none_v.clone(),
        (false, "fetchall") => cursor_rows(an, st, &c.node, o).map(|v| {
            st.set_slot(o, Ns::Attr, key_str_lit("\u{1}pos"), Value::data("pos"), true);
            v
        }).unwrap_or_else(|| Value::data("rows")),
        (false, "fetchone") => {
            let list = st.slot(o, Ns::Attr, key_str_lit("\u{1}rows"));
            let pk = key_str_lit("\u{1}pos");
            let pos = lit(&st.slot(o, Ns::Attr, pk)).and_then(intern::key_as_int);
            let lo = list.single();
            match (pos, lo.and_then(|l| an.heap.llen(l, st))) {
                (Some(p), Some(n)) => {
                    st.set_slot(o, Ns::Attr, pk, Value::lit(intern::key_int(p + 1)), true);
                    if (p as usize) < n {
                        st.slot(lo.unwrap(), Ns::Pos, intern::key_int(p))
                    } else {
                        an.none_v.clone()
                    }
                }
                _ => {
                    st.set_slot(o, Ns::Attr, pk, Value::data("pos"), true);
                    let e = an.elem_of(&list, &c.node, st);
                    e.join(&an.none_v.clone())
                }
            }
        }
        (false, "fetchmany") => {
            let e = an.elem_of(&st.slot(o, Ns::Attr, key_str_lit("\u{1}rows")), &c.node, st);
            st.set_slot(o, Ns::Attr, key_str_lit("\u{1}pos"), Value::data("pos"), true);
            an.alloc_positional(Kind::List, &c.node, None, e, DATA, st)
        }
        _ => Value::data("sqlite"),
    }
}

/// `row[i]` on an sqlite3.Row: the i-th column.
pub fn row_index(an: &Analyzer, st: &State, oid: u32, key: &Value) -> Option<Value> {
    let cols = an.m.sql_rows.get(&oid)?;
    let i = lit(key).and_then(intern::key_as_int)?;
    let i = if i < 0 { i + cols.len() as i64 } else { i };
    let c = cols.get(usize::try_from(i).ok()?)?;
    Some(st.slot(oid, Ns::Item, key_str_lit(c)))
}

pub fn register(r: &mut dyn FnMut(&[&'static str], crate::models::Model)) {
    r(&["sqlite3.connect"], m_connect);
}
