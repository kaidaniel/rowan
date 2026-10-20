//! The other models of the standard library: copies, `namedtuple` / `SimpleNamespace`,
//! `operator.attrgetter` / `itemgetter`, `globals` / `locals`, imports by name, `exec`,
//! dataclass helpers (`astuple`, `replace`, `make_dataclass`), `defaultdict`, `DictReader`,
//! argparse's entry, `functools.partial` / `lru_cache`, `itertools.chain`, logging,
//! `os.getenv`, `typing.get_type_hints`, decimal / fractions / datetime constructors, and the
//! library classes analysed from Python source (`collections.UserDict`, `urllib.parse`).

use super::*;

/// Library classes written in Python that are analysed as such (generated once): their
/// methods run as user code (`collections.UserDict`: `self.data`, `__missing__`, …).
pub fn generated_class(an: &mut Analyzer, st: &mut State, qual: &str) -> Option<Value> {
    let (modname, cname, src) = match qual {
        "collections.UserDict" => ("<collections.UserDict>", "UserDict", USERDICT_SRC),
        _ => match qual.strip_prefix("urllib.parse.") {
            Some(n) if URLLIB_NAMES.contains(&n) => ("<urllib.parse>", n, URLLIB_SRC),
            _ => return None,
        },
    };
    let moid = match an.module(&modname) {
        Some(m) => m,
        None => {
            let (idx, tree) = crate::frontend::parse(modname, None, src.to_string()).ok()?;
            let m = an.make_module(modname, None, idx, tree);
            an.run_module(m, st);
            m
        }
    };
    if st.slot_opt(moid, Ns::Attr, key_str_lit(cname)).is_none() {
        an.run_module(moid, st);
    }
    let v = st.slot(moid, Ns::Attr, key_str_lit(cname));
    if cname == "UserDict" {
        if let Some(c) = v.single() {
            an.m.userdict_classes.insert(c);
        }
    }
    Some(v)
}

/// `urllib.parse` functions analysed as Python (CPython's, for str inputs; anything else is an
/// unknown call). Quoting is `models::pytext` (through `_up`).
pub(super) const URLLIB_NAMES: &[&str] = &["urlparse", "urlsplit", "urlunparse", "urlunsplit", "urljoin", "urldefrag", "parse_qs", "parse_qsl", "urlencode", "ParseResult", "SplitResult", "DefragResult"];

pub(super) const URLLIB_SRC: &str = r#"
import urllib.parse as _up
import _rowan_opaque
from typing import NamedTuple

uses_relative = ['', 'ftp', 'http', 'gopher', 'nntp', 'imap', 'wais', 'file', 'https', 'shttp', 'mms', 'prospero', 'rtsp', 'rtsps', 'rtspu', 'sftp', 'svn', 'svn+ssh', 'ws', 'wss']
uses_netloc = ['', 'ftp', 'http', 'gopher', 'nntp', 'telnet', 'imap', 'wais', 'file', 'mms', 'https', 'shttp', 'snews', 'prospero', 'rtsp', 'rtsps', 'rtspu', 'rsync', 'svn', 'svn+ssh', 'sftp', 'nfs', 'git', 'git+ssh', 'ws', 'wss', 'itms-services']
uses_params = ['', 'ftp', 'hdl', 'prospero', 'http', 'imap', 'https', 'shttp', 'rtsp', 'rtsps', 'rtspu', 'sip', 'sips', 'mms', 'sftp', 'tel']
scheme_chars = 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789+-.'
_WS = '\x00\x01\x02\x03\x04\x05\x06\x07\x08\t\n\x0b\x0c\r\x0e\x0f\x10\x11\x12\x13\x14\x15\x16\x17\x18\x19\x1a\x1b\x1c\x1d\x1e\x1f '


def _userinfo(netloc):
    userinfo, have_info, hostinfo = netloc.rpartition('@')
    if have_info:
        username, have_password, password = userinfo.partition(':')
        if not have_password:
            password = None
    else:
        username = password = None
    return username, password


def _hostinfo(netloc):
    _, _, hostinfo = netloc.rpartition('@')
    _, have_open_br, bracketed = hostinfo.partition('[')
    if have_open_br:
        hostname, _, port = bracketed.partition(']')
        _, _, port = port.partition(':')
    else:
        hostname, _, port = hostinfo.partition(':')
    if not port:
        port = None
    return hostname, port


def _hostname(netloc):
    hostname = _hostinfo(netloc)[0]
    if not hostname:
        return None
    hostname, percent, zone = hostname.partition('%')
    return hostname.lower() + percent + zone


def _port(netloc):
    port = _hostinfo(netloc)[1]
    if port is not None:
        if port.isdigit() and port.isascii():
            port = int(port)
        else:
            raise ValueError("Port could not be cast to integer value")
        if not (0 <= port <= 65535):
            raise ValueError("Port out of range 0-65535")
    return port


class SplitResult(NamedTuple):
    scheme: str
    netloc: str
    path: str
    query: str
    fragment: str

    @property
    def username(self):
        return _userinfo(self.netloc)[0]

    @property
    def password(self):
        return _userinfo(self.netloc)[1]

    @property
    def hostname(self):
        return _hostname(self.netloc)

    @property
    def port(self):
        return _port(self.netloc)

    def geturl(self):
        return urlunsplit(self)


class ParseResult(NamedTuple):
    scheme: str
    netloc: str
    path: str
    params: str
    query: str
    fragment: str

    @property
    def username(self):
        return _userinfo(self.netloc)[0]

    @property
    def password(self):
        return _userinfo(self.netloc)[1]

    @property
    def hostname(self):
        return _hostname(self.netloc)

    @property
    def port(self):
        return _port(self.netloc)

    def geturl(self):
        return urlunparse(self)


class DefragResult(NamedTuple):
    url: str
    fragment: str

    def geturl(self):
        if self.fragment:
            return self.url + '#' + self.fragment
        return self.url


def _splitnetloc(url, start=0):
    delim = len(url)
    for c in '/?#':
        wdelim = url.find(c, start)
        if wdelim >= 0:
            delim = min(delim, wdelim)
    return url[start:delim], url[delim:]


def _urlsplit(url, scheme=None, allow_fragments=True):
    url = url.lstrip(_WS)
    url = url.replace('\t', '').replace('\r', '').replace('\n', '')
    if scheme is not None:
        scheme = scheme.strip(_WS)
        scheme = scheme.replace('\t', '').replace('\r', '').replace('\n', '')
    netloc = query = fragment = None
    i = url.find(':')
    if i > 0 and url[0].isascii() and url[0].isalpha():
        ok = True
        for c in url[:i]:
            if c not in scheme_chars:
                ok = False
                break
        if ok:
            scheme, url = url[:i].lower(), url[i+1:]
    if url[:2] == '//':
        netloc, url = _splitnetloc(url, 2)
        if '[' in netloc or ']' in netloc:
            return _rowan_opaque.urlsplit(url)
        if not netloc.isascii():
            return _rowan_opaque.urlsplit(url)
    if allow_fragments and '#' in url:
        url, fragment = url.split('#', 1)
    if '?' in url:
        url, query = url.split('?', 1)
    return (scheme, netloc, url, query, fragment)


def _splitparams(url):
    if '/' in url:
        i = url.find(';', url.rfind('/'))
        if i < 0:
            return url, None
    else:
        i = url.find(';')
    return url[:i], url[i+1:]


def urlsplit(url, scheme='', allow_fragments=True):
    if not isinstance(url, str) or not isinstance(scheme, str):
        return _rowan_opaque.urlsplit(url, scheme)
    scheme, netloc, url, query, fragment = _urlsplit(url, scheme, allow_fragments)
    return SplitResult(scheme or '', netloc or '', url, query or '', fragment or '')


def urlparse(url, scheme='', allow_fragments=True):
    if not isinstance(url, str) or not isinstance(scheme, str):
        return _rowan_opaque.urlparse(url, scheme)
    scheme, netloc, url, query, fragment = _urlsplit(url, scheme, allow_fragments)
    if (scheme or '') in uses_params and ';' in url:
        url, params = _splitparams(url)
    else:
        params = None
    return ParseResult(scheme or '', netloc or '', url, params or '', query or '', fragment or '')


def _urlunsplit(scheme, netloc, url, query, fragment):
    if netloc is not None:
        if url and url[:1] != '/':
            url = '/' + url
        url = '//' + netloc + url
    elif url[:2] == '//':
        url = '//' + url
    if scheme:
        url = scheme + ':' + url
    if query is not None:
        url = url + '?' + query
    if fragment is not None:
        url = url + '#' + fragment
    return url


def urlunsplit(components):
    scheme, netloc, url, query, fragment = components
    if not netloc:
        if scheme and scheme in uses_netloc and (not url or url[:1] == '/'):
            netloc = ''
        else:
            netloc = None
    return _urlunsplit(scheme or None, netloc, url, query or None, fragment or None)


def urlunparse(components):
    scheme, netloc, url, params, query, fragment = components
    if not netloc:
        if scheme and scheme in uses_netloc and (not url or url[:1] == '/'):
            netloc = ''
        else:
            netloc = None
    if params:
        url = "%s;%s" % (url, params)
    return _urlunsplit(scheme or None, netloc, url, query or None, fragment or None)


def urljoin(base, url, allow_fragments=True):
    if not base:
        return url
    if not url:
        return base
    if not isinstance(base, str) or not isinstance(url, str):
        return _rowan_opaque.urljoin(base, url)
    bscheme, bnetloc, bpath, bquery, bfragment = _urlsplit(base, None, allow_fragments)
    scheme, netloc, path, query, fragment = _urlsplit(url, None, allow_fragments)
    if scheme is None:
        scheme = bscheme
    if scheme != bscheme or (scheme and scheme not in uses_relative):
        return url
    if not scheme or scheme in uses_netloc:
        if netloc:
            return _urlunsplit(scheme, netloc, path, query, fragment)
        netloc = bnetloc
    if not path:
        path = bpath
        if query is None:
            query = bquery
            if fragment is None:
                fragment = bfragment
        return _urlunsplit(scheme, netloc, path, query, fragment)
    base_parts = bpath.split('/')
    if base_parts[-1] != '':
        del base_parts[-1]
    if path[:1] == '/':
        segments = path.split('/')
    else:
        segments = base_parts + path.split('/')
        segments = segments[:1] + [s for s in segments[1:-1] if s] + segments[-1:]
    resolved_path = []
    for seg in segments:
        if seg == '..':
            if resolved_path:
                resolved_path.pop()
        elif seg == '.':
            continue
        else:
            resolved_path.append(seg)
    if segments[-1] in ('.', '..'):
        resolved_path.append('')
    return _urlunsplit(scheme, netloc, '/'.join(resolved_path) or '/', query, fragment)


def urldefrag(url):
    if not isinstance(url, str):
        return _rowan_opaque.urldefrag(url)
    if '#' in url:
        s, n, p, q, frag = _urlsplit(url)
        defrag = _urlunsplit(s, n, p, q, None)
    else:
        frag = ''
        defrag = url
    return DefragResult(defrag, frag or '')


def parse_qsl(qs, keep_blank_values=False, strict_parsing=False, encoding='utf-8', errors='replace', max_num_fields=None, separator='&'):
    if not isinstance(qs, str) or not isinstance(separator, str) or not separator or strict_parsing or max_num_fields is not None:
        return _rowan_opaque.parse_qsl(qs)
    r = []
    if not qs:
        return r
    for name_value in qs.split(separator):
        if name_value:
            name, has_eq, value = name_value.partition('=')
            if value or keep_blank_values:
                name = _up.unquote_plus(name, encoding=encoding, errors=errors)
                value = _up.unquote_plus(value, encoding=encoding, errors=errors)
                r.append((name, value))
    return r


def parse_qs(qs, keep_blank_values=False, strict_parsing=False, encoding='utf-8', errors='replace', max_num_fields=None, separator='&'):
    parsed_result = {}
    pairs = parse_qsl(qs, keep_blank_values, strict_parsing, encoding, errors, max_num_fields, separator)
    for name, value in pairs:
        if name in parsed_result:
            parsed_result[name].append(value)
        else:
            parsed_result[name] = [value]
    return parsed_result


def urlencode(query, doseq=False, safe='', encoding=None, errors=None):
    if hasattr(query, "items"):
        query = query.items()
    l = []
    for k, v in query:
        k = _up.quote_plus(k if isinstance(k, str) else str(k), safe)
        if not doseq or isinstance(v, (str, int, float, bool)) or v is None:
            l.append(k + '=' + _up.quote_plus(v if isinstance(v, str) else str(v), safe))
        else:
            for elt in v:
                l.append(k + '=' + _up.quote_plus(elt if isinstance(elt, str) else str(elt), safe))
    return '&'.join(l)
"#;

pub(super) const USERDICT_SRC: &str = r#"
class UserDict:
    def __init__(self, dict=None, /, **kwargs):
        self.data = {}
        if dict is not None:
            self.update(dict)
        if kwargs:
            self.update(kwargs)

    def __len__(self):
        return len(self.data)

    def __getitem__(self, key):
        if key in self.data:
            return self.data[key]
        if hasattr(self.__class__, "__missing__"):
            return self.__class__.__missing__(self, key)
        raise KeyError(key)

    def __setitem__(self, key, item):
        self.data[key] = item

    def __delitem__(self, key):
        del self.data[key]

    def __iter__(self):
        return iter(self.data)

    def __contains__(self, key):
        return key in self.data

    def update(self, other=(), /, **kwds):
        for key in other:
            self[key] = other[key]
        for key in kwds:
            self[key] = kwds[key]

    def get(self, key, default=None):
        if key in self:
            return self[key]
        return default

    def keys(self):
        return self.data.keys()

    def values(self):
        return self.data.values()

    def items(self):
        return self.data.items()

    def pop(self, key, *default):
        return self.data.pop(key, *default)

    def setdefault(self, key, default=None):
        if key not in self:
            self[key] = default
        return self[key]

    def copy(self):
        return dict(self.data)
"#;

pub(super) fn m_copy(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(a) = c.args.first() else { return Value::data("copy") };
    let deep = c.qual.ends_with("deepcopy");
    let mut outs = Vec::new();
    for o in a.pts.iter() {
        // (an object from input is a new object too: stores into the copy do not reach it)
        if matches!(an.heap.kind(o), Kind::Dict | Kind::List | Kind::Tuple | Kind::Set | Kind::Instance | Kind::Frame) || (an.heap.kind(o) == Kind::Unknown) {
            outs.push(Value::obj(an.copy_object(o, &c.node, st, deep, &mut IndexMap::new())));
        } else {
            outs.push(Value::obj(o));
        }
    }
    if outs.is_empty() {
        a.clone()
    } else {
        join_values(outs)
    }
}

/// `namedtuple("P", ["x", "y"])` / `NamedTuple("P", [("x", int)])` with known names: the
/// class an equivalent `class P(NamedTuple)` (with `_asdict`, `_replace`, `_fields`) defines,
/// analysed as a small generated module.
pub(super) fn m_namedtuple(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fallback = |an: &mut Analyzer, st: &mut State, c: &Call| an.unknown_call(&c.qual, None, c.arguments(), st, &c.node);
    let Some(tname) = c.args.first().and_then(|a| a.kv.as_lit()).and_then(intern::key_as_str) else { return fallback(an, st, c) };
    let Some(spec) = c.args.get(1).cloned().or_else(|| c.kw("field_names")).or_else(|| c.kw("fields")) else { return fallback(an, st, c) };
    let ident = |s: &str| !s.is_empty() && s.chars().all(|ch| ch.is_alphanumeric() || ch == '_') && !s.starts_with(|ch: char| ch.is_ascii_digit());
    let fields: Option<Vec<String>> = if let Some(s0) = spec.kv.as_lit().filter(|_| spec.pts.is_empty()).and_then(intern::key_as_str) {
        Some(s0.replace(',', " ").split_whitespace().map(|x| x.to_string()).collect())
    } else {
        an.iterate(&spec, &c.node, st).items.and_then(|items| {
            items
                .iter()
                .map(|it| {
                    // a name, or a (name, type) pair
                    let v = if it.pts.is_empty() { it.clone() } else { an.unpack(it, 2, st, &c.node).remove(0) };
                    v.kv.as_lit().filter(|_| v.pts.is_empty()).and_then(intern::key_as_str).map(|s| s.to_string())
                })
                .collect()
        })
    };
    let Some(fields) = fields.filter(|fs| !fs.is_empty() && fs.iter().all(|f| ident(f)) && ident(&tname)) else { return fallback(an, st, c) };
    // the defaults (of the last fields), as module globals the generated class refers to
    let dvals: Vec<Value> = c.kw("defaults").map(|d| an.iterate(&d, &c.node, st).items.unwrap_or_default()).unwrap_or_default();
    let ndefaults = dvals.len();
    let modname = format!("<namedtuple {} {}>", tname, c.site);
    if let Some(moid) = an.module(&modname) {
        return st.slot(moid, Ns::Attr, key_str_lit(&tname));
    }
    let mut src = format!("from typing import NamedTuple\n\n\nclass {}(NamedTuple):\n", tname);
    let first_default = fields.len().saturating_sub(ndefaults);
    for (i, f) in fields.iter().enumerate() {
        let dflt = if i >= first_default { format!(" = _rowan_default_{}", i - first_default) } else { String::new() };
        src.push_str(&format!("    {}: \"_field\"{}\n", f, dflt));
    }
    let tuple_src = fields.iter().map(|f| format!("{:?}", f)).collect::<Vec<_>>().join(", ");
    src.push_str(&format!("    _fields = ({},)\n", tuple_src));
    let fd = fields.iter().enumerate().filter(|(i, _)| *i >= first_default).map(|(i, f)| format!("{:?}: _rowan_default_{}", f, i - first_default)).collect::<Vec<_>>().join(", ");
    src.push_str(&format!("    _field_defaults = {{{}}}\n\n", fd));
    let items = fields.iter().map(|f| format!("{:?}: self.{}", f, f)).collect::<Vec<_>>().join(", ");
    src.push_str(&format!("    def _asdict(self):\n        return {{{}}}\n\n", items));
    let args = fields.iter().map(|f| format!("kw.get({:?}, self.{})", f, f)).collect::<Vec<_>>().join(", ");
    src.push_str(&format!("    def _replace(self, **kw):\n        for k in kw:\n            if k not in {:?}:\n                raise ValueError(k)\n        return {}({})\n", fields, tname, args).replace("[", "(").replace("]", ",)"));
    let Ok((idx, tree)) = crate::frontend::parse(&modname, None, src) else { return fallback(an, st, c) };
    let moid = an.make_module(&modname, None, idx, tree);
    for (i, v) in dvals.iter().enumerate() {
        st.set_slot(moid, Ns::Attr, key_str_lit(&format!("_rowan_default_{}", i)), v.clone(), true);
    }
    an.run_module(moid, st);
    st.slot(moid, Ns::Attr, key_str_lit(&tname))
}

/// `p._asdict()` / `p._replace(**kw)` on an instance of a class-syntax NamedTuple.
/// `itemgetter(k, ...)` / `attrgetter("a", ...)` with known keys: the function they stand for,
/// analysed as a small generated module (`def g(obj): return obj[k]`).
pub(super) fn m_getter(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let item = c.qual.ends_with("itemgetter");
    let keys: Option<Vec<KeyId>> = c.args.iter().map(|a| a.kv.as_lit().filter(|_| a.pts.is_empty())).collect();
    let Some(keys) = keys.filter(|k| !k.is_empty() && c.star_args.is_empty()) else {
        return an.unknown_call(&c.qual, None, c.arguments(), st, &c.node);
    };
    let mut parts: Vec<String> = Vec::new();
    for k in &keys {
        if item {
            parts.push(format!("obj[{}]", intern::key_repr(*k)));
        } else {
            let Some(a) = intern::key_as_str(*k).filter(|a| a.split('.').all(|p| !p.is_empty() && p.chars().all(|ch| ch.is_alphanumeric() || ch == '_'))) else {
                return an.unknown_call(&c.qual, None, c.arguments(), st, &c.node);
            };
            parts.push(format!("obj.{}", a));
        }
    }
    let body = if parts.len() == 1 { parts[0].clone() } else { format!("({},)", parts.join(", ")) };
    let modname = format!("<{} {}>", if item { "itemgetter" } else { "attrgetter" }, body);
    let moid = match an.module(&modname) {
        Some(m) => m,
        None => {
            let src = format!("def getter(obj):\n    return {}\n", body);
            let Ok((idx, tree)) = crate::frontend::parse(&modname, None, src) else { return Value::data("getter") };
            let m = an.make_module(&modname, None, idx, tree);
            an.run_module(m, st);
            m
        }
    };
    // (made in another state — a summary's —: its function is bound in this one too)
    if st.slot_opt(moid, Ns::Attr, key_str_lit("getter")).is_none() {
        an.run_module(moid, st);
    }
    st.slot(moid, Ns::Attr, key_str_lit("getter"))
}

/// `asyncio.gather(a, b, *rest)`: the list of their results, in order.
/// `re.match(pattern, s)` with a known pattern: a match object whose `groupdict()` has the
/// pattern's named groups (or None: whether it matches is not decided here).
/// `SimpleNamespace(a=1, **d)`: attributes from the keywords and the mapping (reading one of the
/// mapping's is a read of the mapping).
pub(super) fn m_simplenamespace(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let oid = an.alloc_at(Kind::Namespace, &c.node, st, &format!("namespace@L{}", c.line), Payload::None, Some(Row::new()), Ns::Attr, None, None);
    for sk in &c.star_kwargs {
        an.copy_into(oid, Ns::Attr, sk, st, true, &KeySet::new());
    }
    for (k, v) in c.kwargs.iter() {
        an.store_key(oid, Ns::Attr, &Value::lit(key_str_lit(&intern::str_get(*k))), v, true, &c.node, st);
    }
    Value::obj(oid)
}

/// `exec(code)` / `eval(code)`: a literal code string is run here, in this frame; anything
/// else may change every record this frame reaches (and gives an unknown value).
/// `globals()`: the module's names as a dict (stores through it bind names).
pub(super) fn m_globals(an: &mut Analyzer, st: &mut State, _c: &Call) -> Value {
    let m = an.frame().module;
    an.force_module(m, st);
    let vid = an.heap.alloc_raw(Kind::View, &format!("view:attrs:{}", m), "", true, "globals()", Payload::View(ViewKind::Attrs, Value::obj(m)));
    an.heap.obj_mut(vid).payload = Payload::View(ViewKind::Attrs, Value::obj(m));
    Value::obj(vid)
}

/// `locals()`: at module level the module's names (as `globals()`); in a function a snapshot
/// whose changes go nowhere.
pub(super) fn m_locals(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    if an.frame().kind == crate::interp::FrameKind::Module {
        return m_globals(an, st, c);
    }
    Value::data(&format!("L{}", c.line))
}

/// `__import__("os")` / `importlib.import_module("os")`: the module (by a known name), as an
/// import statement gives it (`__import__("a.b")` gives `a`).
pub(super) fn m_import(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(name) = c.args.first().and_then(|a| a.kv.as_lit().filter(|_| a.pts.is_empty())).and_then(intern::key_as_str) else {
        return an.unknown_call(&c.qual, None, c.arguments(), st, &c.node);
    };
    let m = an.import_module(&name, &c.node, st);
    match name.split_once('.') {
        Some((top, _)) if c.qual == "builtins.__import__" => an.import_module(top, &c.node, st),
        _ => m,
    }
}

/// `exit()` / `quit()` / `sys.exit()`: SystemExit.
pub(super) fn m_exit(an: &mut Analyzer, st: &mut State, _c: &Call) -> Value {
    an.raise_here("SystemExit", st);
    Value::new()
}

pub(super) fn m_exec(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let is_eval = c.qual.ends_with("eval");
    let code = c.args.first().and_then(|a| a.kv.as_lit().filter(|_| a.pts.is_empty())).and_then(intern::key_as_str);
    if let (Some(code), 1) = (code, c.args.len()) {
        if is_eval {
            if let Some(e) = crate::frontend::parse_expression(&code) {
                return an.eval(e, st);
            }
        } else if let Ok((_, tree)) = crate::frontend::parse(&format!("<exec L{}>", c.line), None, code.to_string()) {
            an.exec_block(&tree.body, st);
            st.reachable = true;
            return an.none_v.clone();
        }
    }
    // unknown code: everything reachable from the frame's names (and the module's) may change
    let mut reach: Vec<Value> = st.env.values().cloned().collect();
    let module = an.frame().module;
    reach.push(Value::obj(module));
    reach.extend(c.args.iter().cloned());
    an.unknown_call(&c.qual, None, Args::new(&reach, &c.kwargs, &c.star_args, &c.star_kwargs), st, &c.node)
}

pub fn re_match_generated(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(pat) = c.args.first().and_then(|a| a.kv.as_lit().filter(|_| a.pts.is_empty())).and_then(intern::key_as_str) else {
        return an.unknown_call(&c.qual, None, c.arguments(), st, &c.node);
    };
    let mut names: Vec<String> = Vec::new();
    let mut rest: &str = &pat;
    while let Some(i) = rest.find("(?P<") {
        let after = &rest[i + 4..];
        let Some(j) = after.find('>') else { break };
        let n = &after[..j];
        if !n.is_empty() && n.chars().all(|ch| ch.is_alphanumeric() || ch == '_') && !names.iter().any(|x| x == n) {
            names.push(n.to_string());
        }
        rest = &after[j..];
    }
    let ngroups = pat.matches('(').count().saturating_sub(pat.matches("(?").count()) + names.len();
    let modname = format!("<re.Match {:?}>", &*pat);
    let moid = match an.module(&modname) {
        Some(m) => m,
        None => {
            // (an unknown text's groups: unknown strings, shown as `?`, not the helper's name)
            let items = names.iter().map(|n| format!("{:?}: str(_GROUP)", n)).collect::<Vec<_>>().join(", ");
            let groups = (0..ngroups).map(|_| "str(_GROUP)").collect::<Vec<_>>().join(", ");
            let src = format!(
                "class Match:\n    def groupdict(self, default=None):\n        return {{{}}}\n\n    def group(self, *names):\n        return str(_GROUP)\n\n    def groups(self, default=None):\n        return ({}{})\n\n    def start(self, g=0):\n        return int(_POS)\n\n    def end(self, g=0):\n        return int(_POS)\n\n    def span(self, g=0):\n        return (int(_POS), int(_POS))\n\n\ndef matched():\n    return Match()\n",
                items,
                groups,
                if ngroups == 1 { "," } else { "" }
            );
            let Ok((idx, tree)) = crate::frontend::parse(&modname, None, src) else { return Value::data("re") };
            let m = an.make_module(&modname, None, idx, tree);
            an.run_module(m, st);
            m
        }
    };
    if st.slot_opt(moid, Ns::Attr, key_str_lit("matched")).is_none() {
        an.run_module(moid, st);
    }
    let f = st.slot(moid, Ns::Attr, key_str_lit("matched"));
    let m = an.call_value(&f, &c.node, None, st, Some(vec![]));
    m.join(&an.none_v.clone())
}

pub(super) fn m_gather(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let mut vals: Option<Vec<Value>> = Some(c.args.clone());
    for sa in &c.star_args {
        match (an.iterate(sa, &c.node, st).items, vals.as_mut()) {
            (Some(its), Some(vs)) => vs.extend(its),
            _ => vals = None,
        }
    }
    match vals {
        Some(vs) => an.alloc_positional(Kind::List, &c.node, Some(vs), Value::new(), DATA, st),
        None => {
            let e = join_values(c.args.iter().cloned().chain(c.star_args.iter().map(|sa| an.elem_of(sa, &c.node, st))).collect::<Vec<_>>());
            an.alloc_positional(Kind::List, &c.node, None, e, DATA, st)
        }
    }
}

pub(super) fn m_nt_method(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(recv) = c.recv.clone() else { return Value::data("namedtuple") };
    let mut outs: Vec<Value> = Vec::new();
    for o in recv.pts.iter() {
        let cls = an.heap.instance_class(o);
        let Some(cls) = cls.filter(|cl| crate::models::dataclasses::is_named_tuple_class(an, *cl)) else { continue };
        let fields: Vec<String> = crate::models::dataclasses::dataclass_fields_mro(an, cls).into_iter().map(|(f, _)| f).collect();
        if c.qual.ends_with("_asdict") {
            let d = an.alloc_simple(Kind::Dict, &c.node, st, &format!("dict@L{}", c.line));
            for f in &fields {
                let v = st.slot(o, Ns::Attr, key_str_lit(f));
                an.store_key(d, Ns::Item, &Value::lit(key_str_lit(f)), &v, true, &c.node, st);
            }
            outs.push(Value::obj(d));
        } else {
            if c.kwargs.keys().any(|k| !fields.iter().any(|f| *f == *intern::str_get(*k))) {
                // `_replace` with an unknown field name: TypeError (ValueError before 3.14)
                an.raise_here("TypeError", st);
                continue;
            }
            let n = an.copy_object(o, &c.node, st, false, &mut IndexMap::new());
            for (k, v) in c.kwargs.iter() {
                let name = intern::str_get(*k);
                if fields.iter().any(|f| *f == *name) {
                    an.store_key(n, Ns::Attr, &Value::lit(key_str_lit(&name)), v, true, &c.node, st);
                }
            }
            outs.push(Value::obj(n));
        }
    }
    if outs.is_empty() {
        return Value::data("namedtuple");
    }
    join_values(outs)
}

/// `dataclasses.astuple(obj)`: the fields' values in order (nested dataclasses as tuples).
pub(super) fn m_astuple(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    fn conv(an: &mut Analyzer, st: &mut State, c: &Call, v: &Value, depth: u32) -> Value {
        let mut out = Value::with_kv(crate::domain::ObjSet::new(), v.kv.clone());
        for o in v.pts.iter() {
            let cls = match an.heap.kind(o) {
                Kind::Instance => an.heap.instance_class(o),
                _ => None,
            };
            let Some(cls) = cls.filter(|cl| !crate::models::dataclasses::dataclass_fields_mro(an, *cl).is_empty()).filter(|_| depth < 4) else {
                out = out.join(&Value::obj(o));
                continue;
            };
            let fields: Vec<String> = crate::models::dataclasses::dataclass_fields_mro(an, cls).into_iter().map(|(f, _)| f).collect();
            let mut vals = Vec::new();
            for f in fields {
                let x = st.slot(o, Ns::Attr, key_str_lit(&f));
                vals.push(conv(an, st, c, &x, depth + 1));
            }
            an.frame_mut().unroll.push(9200 + depth as usize);
            let t = an.alloc_positional(Kind::Tuple, &c.node, Some(vals), Value::new(), DATA, st);
            an.frame_mut().unroll.pop();
            out = out.join(&t);
        }
        out
    }
    let Some(a) = c.args.first().cloned() else { return Value::data("astuple") };
    conv(an, st, c, &a, 0)
}

/// `make_dataclass(name, fields)`: a generated `@dataclass` class (`name`, `(name, type)`,
/// `(name, type, field(default=…))` with literal defaults read from the call's text).
pub(super) fn m_make_dataclass(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fallback = |an: &mut Analyzer, st: &mut State, c: &Call| an.unknown_call(&c.qual, None, c.arguments(), st, &c.node);
    let Some(tname) = c.args.first().and_then(|a| a.kv.as_lit()).and_then(intern::key_as_str) else { return fallback(an, st, c) };
    let Some(ast_call) = c.call_ast else { return fallback(an, st, c) };
    let mut fields: Vec<(String, Option<&'static ast::Expr>)> = Vec::new();
    let elts: &'static [ast::Expr] = match &ast_call.arguments.args[1] {
        ast::Expr::List(l) => &l.elts,
        _ => return fallback(an, st, c),
    };
    for e in elts {
        match e {
            ast::Expr::StringLiteral(s0) => fields.push((s0.value.to_str().to_string(), None)),
            ast::Expr::Tuple(t) => {
                let Some(ast::Expr::StringLiteral(n)) = t.elts.first() else { return fallback(an, st, c) };
                let d: Option<&'static ast::Expr> = match t.elts.get(2) {
                    Some(ast::Expr::Call(fc)) if crate::syntax::dotted_last(&fc.func) == "field" => fc.arguments.keywords.iter().find(|k| k.arg.as_ref().map(|x| x.as_str() == "default").unwrap_or(false)).map(|k| &k.value),
                    Some(other) => Some(other),
                    None => None,
                };
                fields.push((n.value.to_str().to_string(), d));
            }
            _ => return fallback(an, st, c),
        }
    }
    let modname = format!("<make_dataclass {} {}>", tname, c.site);
    if let Some(moid) = an.module(&modname) {
        return st.slot(moid, Ns::Attr, key_str_lit(&tname));
    }
    let mut src = format!("from dataclasses import dataclass\n\n\n@dataclass\nclass {}:\n", tname);
    let mut dvals: Vec<Value> = Vec::new();
    for (f, d) in &fields {
        match d {
            Some(e) => {
                src.push_str(&format!("    {}: \"_field\" = _rowan_default_{}\n", f, dvals.len()));
                dvals.push(an.eval(e, st));
            }
            None => src.push_str(&format!("    {}: \"_field\"\n", f)),
        }
    }
    if fields.is_empty() {
        src.push_str("    pass\n");
    }
    let Ok((idx, tree)) = crate::frontend::parse(&modname, None, src) else { return fallback(an, st, c) };
    let moid = an.make_module(&modname, None, idx, tree);
    for (i, v) in dvals.iter().enumerate() {
        st.set_slot(moid, Ns::Attr, key_str_lit(&format!("_rowan_default_{}", i)), v.clone(), true);
    }
    an.run_module(moid, st);
    st.slot(moid, Ns::Attr, key_str_lit(&tname))
}

pub(super) fn m_dc_replace(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(a) = c.args.first() else { return Value::data("replace") };
    let mut outs = Vec::new();
    for o in a.pts.iter() {
        // a new instance through `__init__`: the init fields from the object (the changes
        // win); non-init fields and attributes added later are not carried over
        let cls = match an.heap.kind(o) {
            Kind::Instance => an.heap.instance_class(o),
            _ => None,
        };
        if let Some(cls) = cls.filter(|cl| an.heap.class_info_opt(*cl).map(|ci| ci.has_fields() && !ci.lib.named_tuple).unwrap_or(false)) {
            let specs = crate::models::dataclasses::dc_specs(an, cls);
            let mut kw: IndexMap<intern::StrId, Value> = IndexMap::new();
            for sp in specs.iter().filter(|sp| sp.init && !sp.initvar) {
                let v = st.slot(o, Ns::Attr, key_str_lit(&sp.name));
                kw.insert(intern::s(&sp.name), v);
            }
            for (k, v) in c.kwargs.iter() {
                kw.insert(*k, v.clone());
            }
            an.frame_mut().unroll.push(9100);
            outs.push(an.construct(cls, Args::kw(&kw), st, &c.node));
            an.frame_mut().unroll.pop();
            continue;
        }
        if an.heap.kind(o) == Kind::Instance {
            let nid = an.copy_object(o, &c.node, st, false, &mut IndexMap::new());
            for (k, v) in c.kwargs.iter() {
                an.store_key(nid, Ns::Attr, &Value::lit(intern::key_of_str(*k)), v, true, &c.node, st);
            }
            outs.push(Value::obj(nid));
        }
    }
    if outs.is_empty() {
        a.clone()
    } else {
        join_values(outs)
    }
}

pub(super) fn m_first_arg(_an: &mut Analyzer, _st: &mut State, c: &Call) -> Value {
    c.args.first().cloned().unwrap_or_else(|| Value::data(&format!("L{}", c.line)))
}

pub(super) fn m_passthrough(_an: &mut Analyzer, _st: &mut State, c: &Call) -> Value {
    if c.qual == "typing.cast" && c.args.len() >= 2 {
        return c.args[1].clone();
    }
    Value::data(&format!("L{}", c.line))
}

pub(super) fn m_defaultdict(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = last(&c.qual).to_string();
    if name == "Counter" {
        return counter::construct(an, st, c);
    }
    let oid = an.alloc_simple(Kind::Dict, &c.node, st, &format!("{}@L{}", name, c.line));
    let src_arg: Option<Value>;
    if name == "defaultdict" || name == "Counter" {
        if name == "Counter" {
            if c.args.is_empty() {
                // an empty Counter is a closed dict whose missing keys read as 0
                st.set_slot(oid, Ns::Item, STAR, Value::lit(intern::key_int(0)), false);
            } else {
                st.set_slot(oid, Ns::Item, STAR, Value::lit(intern::key_int(0)), false);
                // known items with literal values: those are the keys (counts unknown)
                let it = if an.record_objects(&c.args[0]).iter().all(|o| an.heap.kind(*o) != Kind::Dict) { an.iterate(&c.args[0], &c.node, st).items } else { None };
                let lits: Option<Vec<KeyId>> = it.and_then(|items| items.iter().map(|v| if v.pts.is_empty() { v.kv.as_lit() } else { None }).collect());
                match lits {
                    Some(ks) => {
                        let mut r = st.row(oid, Ns::Item);
                        for k in ks {
                            r = r.store(k, true);
                            st.set_slot(oid, Ns::Item, k, Value::data("count"), true);
                        }
                        st.set_row(oid, Ns::Item, r);
                    }
                    None => {
                        let r = st.row(oid, Ns::Item).data_store(Source::new(&format!("{}@L{}", name, c.line), SourceKind::Data));
                        st.set_row(oid, Ns::Item, r);
                    }
                }
            }
        }
        if name == "defaultdict" && !c.args.is_empty() {
            if !c.args[0].pts.is_empty() {
                // the factory runs per inserted key (see `load_item`); no shared default
                an.m.dd_factories.insert(oid, c.args[0].clone());
            } else if !c.args[0].pts.is_empty() {
                let dflt = an.call_value(&c.args[0], &c.node, None, st, Some(vec![]));
                st.set_slot(oid, Ns::Item, STAR, Value::with_kv(dflt.pts, DATA), false);
            } else {
                st.set_slot(oid, Ns::Item, STAR, Value::data("default"), false);
            }
        }
        src_arg = if name == "defaultdict" { c.args.get(1).cloned() } else { c.args.first().cloned() };
    } else if name == "ChainMap" {
        // the maps it looks through (writes go to the first; `.maps`, `new_child`)
        let maps: Vec<Value> = if c.args.is_empty() {
            let d = an.alloc_at(Kind::Dict, &c.node, st, &format!("dict@L{}", c.line), Payload::None, Some(Row::new()), Ns::Item, None, Some("chainmap0"));
            st.set_row(d, Ns::Item, Row::new());
            vec![Value::obj(d)]
        } else {
            c.args.clone()
        };
        an.m.chainmaps.insert(oid, maps.clone());
        // (built from parameters: the maps are known per call site)
        // every map, the first one winning: copied from the last to the first
        for sa in maps.iter().rev() {
            if !an.record_objects(sa).is_empty() {
                an.copy_into(oid, Ns::Item, sa, st, true, &KeySet::new());
            } else {
                let r = st.row(oid, Ns::Item).data_store(Source::new(&format!("{}@L{}", name, c.line), SourceKind::Data));
                st.set_row(oid, Ns::Item, r);
            }
        }
        src_arg = None;
    } else {
        src_arg = c.args.first().cloned();
    }
    if let Some(sa) = src_arg {
        if update_from_instances(an, st, oid, &sa, true, c) {
        } else if !an.record_objects(&sa).is_empty() {
            an.copy_into(oid, Ns::Item, &sa, st, true, &KeySet::new());
        } else if name != "Counter" {
            let r = st.row(oid, Ns::Item).data_store(Source::new(&format!("{}@L{}", name, c.line), SourceKind::Data));
            st.set_row(oid, Ns::Item, r);
        }
    }
    for (k, v) in c.kwargs.iter() {
        an.store_key(oid, Ns::Item, &Value::lit(intern::key_of_str(*k)), v, true, &c.node, st);
    }
    Value::obj(oid)
}

pub(super) fn m_dictreader(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fields = c.kw("fieldnames").and_then(|f| an.lit_keys(&f, st));
    let row = match fields {
        Some(f) => Row::closed_with(f),
        None => Row::with_source(Source::new(&format!("csv.DictReader@L{}", c.line), SourceKind::External)),
    };
    let rid = an.alloc_at(Kind::Dict, &c.node, st, &format!("DictReader row@L{}", c.line), Payload::None, Some(row), Ns::Item, Some(false), None);
    an.heap.obj_mut(rid).iter_elem = true;
    let ctx = an.frame().ctx.clone();
    let vid = an.heap.alloc_raw(Kind::View, &format!("view:elems:{}", c.site), &ctx, true, "DictReader", Payload::View(ViewKind::Elems, Value::obj(rid)));
    an.heap.obj_mut(vid).payload = Payload::View(ViewKind::Elems, Value::obj(rid));
    Value::obj(vid)
}

pub(super) fn m_argparser(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let oid = an.alloc_simple(Kind::Parser, &c.node, st, &format!("ArgumentParser@L{}", c.line));
    st.set_row(oid, Ns::Attr, Row::new());
    Value::obj(oid)
}

pub(super) fn m_parser_method(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = last(&c.qual);
    let recv = c.recv.clone().unwrap_or_default();
    let parsers: Vec<u32> = recv.pts.iter().filter(|o| an.heap.kind(*o) == Kind::Parser).collect();
    match name {
        "add_argument" => {
            let mut dest: Option<String> = None;
            if let Some(dv) = c.kw("dest") {
                if let Some(k) = dv.kv.as_lit() {
                    dest = Some(intern::key_py_str(k));
                }
            } else {
                for a in &c.args {
                    if let Some(k) = a.kv.as_lit() {
                        if let Some(sk) = intern::key_as_str(k) {
                            if let Some(rest) = sk.strip_prefix("--") {
                                dest = Some(rest.replace('-', "_"));
                                break;
                            }
                            if !sk.starts_with('-') {
                                dest = Some(sk.replace('-', "_"));
                                break;
                            }
                            if dest.is_none() {
                                dest = Some(sk.trim_start_matches('-').replace('-', "_"));
                            }
                        }
                    }
                }
            }
            for p in parsers {
                match &dest {
                    Some(d) => {
                        let dk = key_str_lit(d);
                        let single = an.heap.obj(p).single;
                        let r = st.row(p, Ns::Attr).store(dk, single);
                        st.set_row(p, Ns::Attr, r);
                        st.set_slot(p, Ns::Attr, dk, Value::data(&format!("arg {}", d)), true);
                    }
                    None => {
                        let r = st.row(p, Ns::Attr).data_store(Source::new(&format!("add_argument@L{}", c.line), SourceKind::Data));
                        st.set_row(p, Ns::Attr, r);
                    }
                }
            }
            Value::data("action")
        }
        "parse_args" | "parse_known_args" | "parse_intermixed_args" => {
            let nid = an.alloc_at(Kind::Namespace, &c.node, st, &format!("args@L{}", c.line), Payload::None, None, Ns::Attr, None, None);
            for p in parsers {
                let r = st.row(nid, Ns::Attr).copy_from(&st.row(p, Ns::Attr), true, &KeySet::new(), intern::EMPTY_LENS);
                st.set_row(nid, Ns::Attr, r);
                for (k, v) in st.obj_slots(p, Ns::Attr) {
                    st.set_slot(nid, Ns::Attr, k, v, false);
                }
            }
            if name == "parse_known_args" {
                let tid = an.alloc_simple(Kind::Tuple, &c.node, st, "parse_known_args result");
                an.heap.set_llen(tid, st, Some(2));
                st.set_slot(tid, Ns::Pos, intern::key_int(0), Value::obj(nid), true);
                st.set_slot(tid, Ns::Pos, intern::key_int(1), Value::data("rest"), true);
                return Value::obj(tid);
            }
            Value::obj(nid)
        }
        "add_subparsers" | "add_argument_group" | "add_mutually_exclusive_group" => recv,
        _ => Value::data(&format!("L{}", c.line)),
    }
}

/// `f.cache_info()` (hits, misses, maxsize, currsize) / `f.cache_clear()` of an lru_cache'd f.
pub(super) fn m_memo(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(f) = c.recv.as_ref().and_then(|r| r.single()) else { return Value::data("cache") };
    let memo = an.heap.alloc_raw(Kind::Dict, &format!("memo:{}", f), "", true, "lru_cache", Payload::None);
    let zero = Value::lit(intern::key_int(0));
    if c.qual.ends_with("cache_clear") {
        st.set_row(memo, Ns::Item, Row::new());
        st.drop_obj_ns(memo, Ns::Item);
        st.set_slot(memo, Ns::Attr, key_str_lit("hits"), zero.clone(), true);
        st.set_slot(memo, Ns::Attr, key_str_lit("misses"), zero, true);
        return an.none_v.clone();
    }
    let oid = an.alloc_at(Kind::Namespace, &c.node, st, "CacheInfo", Payload::None, Some(Row::new()), Ns::Attr, None, Some("cacheinfo"));
    let r = st.row(memo, Ns::Item);
    let size = if r.closed() && r.present == r.definite { Value::lit(intern::key_int(r.present.len() as i64)) } else { Value::data("currsize") };
    let mut row = Row::new();
    for (k, v) in [
        ("hits", st.slot_opt(memo, Ns::Attr, key_str_lit("hits")).unwrap_or_else(|| zero.clone())),
        ("misses", st.slot_opt(memo, Ns::Attr, key_str_lit("misses")).unwrap_or_else(|| zero.clone())),
        ("maxsize", Value::data("maxsize")),
        ("currsize", size),
    ] {
        let kk = key_str_lit(k);
        row = row.store(kk, true);
        st.set_slot(oid, Ns::Attr, kk, v, true);
    }
    st.set_row(oid, Ns::Attr, row);
    Value::obj(oid)
}

pub(super) fn m_partial(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(f) = c.args.first() else { return Value::data("partial") };
    let mut func = f.clone();
    let mut args: Vec<Value> = c.args[1..].to_vec();
    let mut kw = c.kwargs.clone();
    let mut star: Vec<Value> = c.star_kwargs.clone();
    // `partial(partial(f, a, k=1), b, k=2)` is `partial(f, a, b, k=2)`
    if let Some(inner) = f.single().filter(|o| an.heap.kind(*o) == Kind::Partial) {
        if let Payload::Partial(f0, a0, k0) = an.heap.obj(inner).payload.clone() {
            func = f0;
            let mut a = a0;
            a.extend(args);
            args = a;
            let mut k = k0;
            for (kk, v) in kw.iter() {
                k.insert(*kk, v.clone());
            }
            kw = k;
            let mut s0 = an.m.partial_star.get(&inner).cloned().unwrap_or_default();
            s0.extend(star);
            star = s0;
        }
    }
    // `**opts`: its literal keys become bound keywords; an open mapping is passed on as it is
    let mut rest_star: Vec<Value> = Vec::new();
    for sk in &star {
        let recs = an.record_objects(sk);
        let closed = recs.len() == 1 && { let r = st.row(recs[0], Ns::Item); r.closed() && r.present == r.definite && r.present.iter().all(intern::key_is_str) };
        if closed {
            let d = recs[0];
            for k in st.row(d, Ns::Item).ordered_keys().unwrap_or_else(|| st.row(d, Ns::Item).present.iter().collect()) {
                if let Some(name) = intern::key_as_str(k) {
                    kw.insert(intern::s(&name), st.slot(d, Ns::Item, k));
                }
            }
        } else {
            rest_star.push(sk.clone());
        }
    }
    let payload = Payload::Partial(func, args, kw);
    let oid = an.alloc_at(Kind::Partial, &c.node, st, &format!("partial@L{}", c.line), payload.clone(), None, Ns::Item, None, None);
    an.heap.obj_mut(oid).payload = payload;
    if !rest_star.is_empty() {
        an.m.partial_star.insert(oid, rest_star);
    } else {
        an.m.partial_star.remove(&oid);
    }
    Value::obj(oid)
}

pub(super) fn m_chain(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let oid = an.alloc_simple(Kind::List, &c.node, st, &format!("chain@L{}", c.line));
    an.heap.set_llen(oid, st, None);
    let mut acc = Value::new();
    let mut kv: Option<KeyVal> = None;
    for a in &c.args {
        let it = an.iterate(a, &c.node, st);
        let Elem::One(elem) = it.elem else { continue };
        acc = acc.join(&Value::with_kv(elem.pts.clone(), DATA));
        kv = Some(match kv {
            None => elem.kv.clone(),
            Some(k) => kv_join(&k, &elem.kv),
        });
    }
    st.set_slot(oid, Ns::Elem, STAR, acc, true);
    st.elems.insert(oid, kv.unwrap_or(DATA));
    Value::obj(oid)
}

pub fn partial_call(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    m_partial_call(an, st, c)
}

pub(super) fn m_partial_call(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let recv = c.recv.clone().unwrap_or_default();
    let mut outs = Vec::new();
    for x in recv.pts.iter() {
        let Payload::Partial(f, bound_args, bound_kw) = an.heap.obj(x).payload.clone() else { continue };
        let mut kw = bound_kw.clone();
        for (k, v) in c.kwargs.iter() {
            kw.insert(*k, v.clone());
        }
        let mut fake_args = bound_args.clone();
        fake_args.extend(c.args.iter().cloned());
        // (bound `**opts` of unknown keys: passed on before the call's own)
        let mut star = an.m.partial_star.get(&x).cloned().unwrap_or_default();
        star.extend(c.star_kwargs.iter().cloned());
        for fo in f.pts.iter() {
            outs.push(an.call_one(fo, Args::new(&fake_args, &kw, &c.star_args, &star), st, &c.node, c.call_ast));
        }
    }
    if outs.is_empty() {
        Value::data("partial")
    } else {
        join_values(outs)
    }
}

pub(super) fn m_get_logger(an: &mut Analyzer, _st: &mut State, _c: &Call) -> Value {
    Value::obj(an.heap.alloc_raw(Kind::External, "ext:logging.Logger", "", true, "logging.Logger", Payload::Str("logging.Logger".into())))
}

pub(super) fn m_getenv(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let env = external_record(an, "os.environ", st).unwrap();
    let call = Call { qual: "dict.get".into(), recv: Some(Value::obj(env)), args: c.args.clone(), kwargs: c.kwargs.clone(), star_args: c.star_args.clone(), star_kwargs: c.star_kwargs.clone(), node: c.node, call_ast: c.call_ast, site: c.site.clone(), line: c.line };
    m_get(an, st, &call)
}

/// `range(...)` with known int bounds: a list whose items are known when it is short; its
/// bounds decide membership at any length.
/// The names a class (its MRO for `get_type_hints`) or a function annotates, in order.
pub fn annotated_names(an: &Analyzer, o: crate::domain::ObjId, mro: bool) -> Option<Vec<String>> {
    match an.heap.kind(o) {
        Kind::Class => {
            let mut out: Vec<String> = Vec::new();
            let classes: Vec<crate::domain::ObjId> = if mro { an.heap.mro(o).iter().rev().copied().collect() } else { vec![o] };
            let mut any = false;
            for c in classes {
                let Some(ci) = an.heap.class_info_opt(c) else { continue };
                any = true;
                for stmt in &ci.node.body {
                    if let ruff_python_ast::Stmt::AnnAssign(a) = stmt {
                        if let ruff_python_ast::Expr::Name(n) = &*a.target {
                            let n = n.id.to_string();
                            if !out.contains(&n) {
                                out.push(n);
                            }
                        }
                    }
                }
            }
            any.then_some(out)
        }
        Kind::Function => {
            let fi = an.heap.func_info(o);
            let crate::interp::FuncNode::Def(f) = fi.node else { return Some(vec![]) };
            let p = &f.parameters;
            let mut out: Vec<String> = Vec::new();
            for a in p.posonlyargs.iter().chain(p.args.iter()).chain(p.kwonlyargs.iter()) {
                if a.parameter.annotation.is_some() {
                    out.push(a.parameter.name.id.to_string());
                }
            }
            for a in p.vararg.iter().chain(p.kwarg.iter()) {
                if a.annotation.is_some() {
                    out.push(a.name.id.to_string());
                }
            }
            if f.returns.is_some() {
                out.push("return".into());
            }
            Some(out)
        }
        _ => None,
    }
}

/// A closed dict of the given keys, each holding unknown data.
pub fn names_dict(an: &mut Analyzer, st: &mut State, node: &crate::frontend::Node, names: &[String], vals: Option<Vec<Value>>, what: &str) -> Value {
    let did = an.alloc_at(Kind::Dict, node, st, &format!("{}@L{}", what, node.line), Payload::None, Some(Row::new()), Ns::Item, None, None);
    let mut row = Row::new();
    for (i, n) in names.iter().enumerate() {
        let k = key_str_lit(n);
        row = row.store(k, true);
        let v = vals.as_ref().and_then(|vs| vs.get(i).cloned()).unwrap_or_else(|| Value::data(what));
        st.set_slot(did, Ns::Item, k, v, true);
    }
    st.set_row(did, Ns::Item, row);
    Value::obj(did)
}

pub(super) fn m_type_hints(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(o) = c.args.first().and_then(|v| v.single()) else { return Value::data("hints") };
    let mro = !c.qual.starts_with("inspect.");
    match annotated_names(an, o, mro) {
        Some(names) => names_dict(an, st, &c.node, &names, None, "hints"),
        None => Value::data("hints"),
    }
}

/// `MappingProxyType(d)`: a live read-only view of d.
pub(super) fn m_mappingproxy(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(d) = c.args.first().cloned() else { return Value::data("proxy") };
    if d.pts.is_empty() || !d.pts.iter().all(|o| matches!(an.heap.kind(o), Kind::Dict | Kind::Unknown)) {
        return an.unknown_call(&c.qual, None, c.arguments(), st, &c.node);
    }
    let _ = st;
    let vid = an.heap.alloc_raw(Kind::View, &format!("view:proxy:{}:{}", c.site, d.pts.iter().map(|o| o.to_string()).collect::<Vec<_>>().join(",")), "", true, "mappingproxy", Payload::View(ViewKind::Proxy, d.clone()));
    an.heap.obj_mut(vid).payload = Payload::View(ViewKind::Proxy, d);
    Value::obj(vid)
}

/// `Decimal(x)` / `Fraction(a, b)` of known values: a literal (models::pynum).
pub(super) fn m_number_new(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let lit = |v: &Value| v.kv.as_lit().filter(|_| v.pts.is_empty());
    let dec = c.qual.starts_with("decimal.");
    let a = c.arg(0, Some(if dec { "value" } else { "numerator" }));
    let b = if dec { None } else { c.arg(1, Some("denominator")) };
    let known = a.as_ref().map(|v| lit(v).is_some()).unwrap_or(true) && b.as_ref().map(|v| lit(v).is_some()).unwrap_or(true) && (!dec || c.args.len() <= 1);
    if known {
        let (ka, kb) = (a.as_ref().and_then(lit), b.as_ref().and_then(lit));
        let r = if dec { pynum::decimal_new(ka) } else { pynum::fraction_new(ka, kb) };
        if let Some(k) = r {
            return Value::lit(k);
        }
        // `Decimal("x")` / `Fraction(1, 0)`: raises, or a value not simulated
        let bad_text = ka.and_then(intern::key_as_str).is_some_and(|s| !s.trim().to_lowercase().contains("inf") && !s.trim().to_lowercase().contains("nan"));
        if bad_text || (!dec && kb.and_then(intern::key_as_int) == Some(0)) {
            an.raise_here(if dec { "decimal.InvalidOperation" } else { "ZeroDivisionError" }, st);
            return Value::new();
        }
    }
    Value::data(if dec { "Decimal" } else { "Fraction" })
}

/// `date(…)` / `datetime(…)` / `time(…)` / `timedelta(…)` / `timezone(…)` of known values.
pub(super) fn m_chrono_new(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let class = last(&c.qual).to_string();
    let lit = |v: &Value| v.kv.as_lit().filter(|_| v.pts.is_empty());
    let args: Option<Vec<KeyId>> = c.args.iter().map(lit).collect();
    let kwargs: Option<Vec<(String, KeyId)>> = c.kwargs.iter().map(|(k, v)| lit(v).map(|x| (intern::str_get(*k).to_string(), x))).collect();
    if let (Some(args), Some(kwargs), true) = (args, kwargs, c.star_args.is_empty() && c.star_kwargs.is_empty()) {
        let kw: Vec<(&str, KeyId)> = kwargs.iter().map(|(k, v)| (k.as_str(), *v)).collect();
        match chrono::construct(&class, &args, &kw) {
            Some(Ok(k)) => return Value::lit(k),
            Some(Err(())) => {
                an.raise_here(if class == "timedelta" { "OverflowError" } else { "ValueError" }, st);
                return Value::new();
            }
            None => {}
        }
    }
    Value::data(&class)
}

/// `datetime.fromisoformat(…)`, `datetime.strptime(…)`, `date.fromordinal(…)`, `datetime.combine(…)`.
pub(super) fn m_chrono_class(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let mut parts = c.qual.rsplit('.');
    let name = parts.next().unwrap_or("").to_string();
    let class = parts.next().unwrap_or("").to_string();
    let lit = |v: &Value| v.kv.as_lit().filter(|_| v.pts.is_empty());
    if let Some(args) = c.args.iter().map(lit).collect::<Option<Vec<KeyId>>>().filter(|_| c.kwargs.is_empty()) {
        match chrono::class_method(&class, &name, &args) {
            Some(Ok(k)) => return Value::lit(k),
            Some(Err(())) => {
                an.raise_here("ValueError", st);
                return Value::new();
            }
            None => {}
        }
    }
    Value::data(if class == "date" { "date" } else { "datetime" })
}

/// The clock: unknown.
pub(super) fn m_chrono_clock(_an: &mut Analyzer, _st: &mut State, c: &Call) -> Value {
    Value::data(if c.qual.starts_with("datetime.date.") { "date" } else { "datetime" })
}

/// A changed decimal context: Decimal arithmetic is no longer simulated.
pub(super) fn m_decimal_context(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    pynum::set_opaque(true);
    an.unknown_call(&c.qual, None, c.arguments(), st, &c.node)
}

/// A Decimal / Fraction method (`d.quantize(…)`, `f.limit_denominator(…)`).
pub(super) fn m_num_method(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = last(&c.qual).to_string();
    let lit = |v: &Value| v.kv.as_lit().filter(|_| v.pts.is_empty());
    let recv = c.recv.clone().unwrap_or_default();
    let args: Option<Vec<KeyId>> = c.args.iter().map(lit).collect();
    let kwargs: Option<Vec<(String, KeyId)>> = c.kwargs.iter().map(|(k, v)| lit(v).map(|x| (intern::str_get(*k).to_string(), x))).collect();
    let (Some(args), Some(kwargs)) = (args, kwargs) else {
        return Value::data("number");
    };
    let kw: Vec<(&str, KeyId)> = kwargs.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    let mut outs = Vec::new();
    for r in crate::domain::kv_literals(&recv.kv).iter() {
        let res = if chrono::is_chrono_key(r) { chrono::method(r, &name, &args, &kw) } else { pynum::method(r, &name, &args, &kw) };
        match res {
            Some(k) => outs.push(pure_result(an, st, c, k)),
            None => return Value::data("number"),
        }
    }
    if outs.is_empty() {
        return Value::data("number");
    }
    join_values(outs)
}
