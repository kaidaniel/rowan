//! What the interpreter needs to know of the builtins' and the libraries' functions and
//! methods without running a model: which raise only on arguments of a wrong type, which have
//! no effect, which report their own raises, which take an iterable's items at once.

pub const SAFE_METHODS: &[&str] = &["dict.pop", "list.pop", 
    "dict.get", "dict.keys", "dict.values", "dict.items", "dict.setdefault", "dict.copy", "list.append", "str.upper", "str.lower",
    "str.strip", "str.title", "str.format", "str.join",
];

/// Builtin container / str methods that raise only on arguments of the wrong type (not on
/// the values): their calls are not "may raise anything".
pub fn safe_method(q: &str) -> bool {
    if SAFE_METHODS.contains(&q) {
        return true;
    }
    let Some((recv, name)) = q.split_once('.') else { return false };
    match recv {
        "str" => !matches!(name, "index" | "rindex" | "format" | "format_map" | "encode" | "decode"),
        "list" => matches!(name, "append" | "extend" | "insert" | "sort" | "reverse" | "copy" | "count" | "clear"),
        "dict" => matches!(name, "get" | "keys" | "values" | "items" | "setdefault" | "copy" | "update" | "clear" | "fromkeys"),
        "set" | "frozenset" => matches!(name, "add" | "update" | "union" | "intersection" | "difference" | "symmetric_difference" | "copy" | "clear" | "discard" | "issubset" | "issuperset" | "isdisjoint" | "intersection_update" | "difference_update" | "symmetric_difference_update"),
        "tuple" => name == "count",
        // frame methods that cannot fail on a frame (reset_index reports its own ValueError)
        // (a string column's accessor: these do not fail on its values)
        "Series" => name.strip_prefix("str.").is_some_and(|m| matches!(m, "slice" | "lower" | "upper" | "strip" | "lstrip" | "rstrip" | "len" | "startswith" | "endswith" | "title" | "capitalize" | "casefold" | "swapcase" | "zfill" | "get")),
        "DataFrame" => matches!(name, "copy" | "head" | "tail" | "to_dict" | "isna" | "notna" | "isnull" | "notnull" | "nunique" | "count" | "keys" | "items" | "iterrows" | "itertuples" | "to_numpy" | "to_records" | "reset_index" | "memory_usage" | "info"),
        _ => false,
    }
}
pub const STR_METHOD_NAMES: &[&str] = &[
    "strip", "lower", "upper", "title", "split", "replace", "startswith", "endswith", "format", "join", "lstrip", "rstrip", "encode",
    "decode", "isdigit", "isalpha", "zfill", "casefold", "capitalize", "partition", "rsplit", "splitlines", "find", "count", "ljust",
    "rjust", "center", "rpartition", "rfind", "rindex", "isascii", "isalnum", "isspace", "islower", "isupper", "istitle", "isnumeric",
    "isdecimal", "isidentifier", "isprintable", "removeprefix", "removesuffix", "swapcase", "expandtabs", "format_map", "translate",
];
pub const SCALAR_EXTRA_NAMES: &[&str] = &[
    "append", "extend", "sort", "index", "insert", "remove", "isoformat", "strftime", "timestamp", "date", "time",
    "total_seconds", "to_bytes", "bit_length", "as_integer_ratio", "is_integer", "hex", "conjugate", "real", "imag", "tolist",
    "item", "copy", "clear", "reverse",
];
pub fn is_scalar_method(name: &str) -> bool {
    STR_METHOD_NAMES.contains(&name) || SCALAR_EXTRA_NAMES.contains(&name)
}
pub const NOEFFECT_METHOD_NAMES: &[&str] = &[
    "info", "debug", "warning", "warn", "error", "critical", "exception", "log", "write", "flush", "close", "print", "assert_called",
    "assert_called_with", "equals",
];
pub const DICT_METHOD_NAMES: &[&str] = &["get", "items", "keys", "values", "update", "pop", "setdefault", "copy", "popitem", "clear"];
/// Methods that make a class-less unknown a list (modelled by `list.*`).
pub const LIST_METHOD_NAMES: &[&str] = &["append", "extend", "insert", "remove", "sort", "reverse"];

pub const KNOWN_IDENTITY_DECORATORS: &[&str] = &[
    "functools.lru_cache", "functools.cache", "functools.wraps", "contextlib.contextmanager", "abc.abstractmethod", "typing.overload",
    "typing.final", "builtins.staticmethod", "builtins.classmethod", "pytest.fixture", "asyncio.coroutine", "dataclasses.dataclass",
    "typing.no_type_check", "functools.total_ordering", "typing.runtime_checkable",
    // pydantic registers these (models::pydantic reads them from the class body)
    "pydantic.field_validator", "pydantic.model_validator", "pydantic.validator", "pydantic.root_validator", "pydantic.field_serializer",
    "pydantic.model_serializer", "pydantic.computed_field", "pydantic.functional_validators.field_validator", "pydantic.dataclasses.dataclass",
];

pub const BUILTIN_NAMES: &[&str] = &[
    "dict", "list", "set", "tuple", "len", "print", "str", "int", "float", "bool", "range", "enumerate", "zip", "sorted", "min", "max", "sum",
    "any", "all", "isinstance", "type", "open", "getattr", "setattr", "hasattr", "repr", "iter", "next", "abs", "round", "map", "filter",
    "reversed", "id", "hash", "object", "Exception", "KeyError", "ValueError", "TypeError", "AttributeError", "IndexError", "LookupError",
    "RuntimeError", "property", "staticmethod", "classmethod", "super", "vars", "format", "frozenset", "bytes", "input",
    "NotImplementedError", "StopIteration", "BaseException", "callable", "chr", "ord", "divmod", "pow", "slice", "issubclass", "memoryview",
    "bytearray", "complex", "globals", "locals", "exec", "eval", "compile", "delattr", "help", "hex", "oct", "bin", "ascii", "breakpoint",
    "FileNotFoundError", "OSError", "IOError", "ZeroDivisionError", "AssertionError", "NameError", "ImportError", "ModuleNotFoundError",
    "PermissionError", "TimeoutError", "ConnectionError", "StopAsyncIteration", "GeneratorExit", "KeyboardInterrupt", "SystemExit", "Warning",
    "UserWarning", "DeprecationWarning", "__name__", "__file__", "__doc__", "NotImplemented", "Ellipsis", "None", "True", "False", "anext",
    "aiter", "__import__", "__build_class__", "__debug__", "__loader__", "__package__", "__spec__", "ArithmeticError", "BlockingIOError",
    "BrokenPipeError", "BufferError", "BytesWarning", "ChildProcessError", "ConnectionAbortedError", "ConnectionRefusedError",
    "ConnectionResetError", "EOFError", "EnvironmentError", "FloatingPointError", "FutureWarning", "ImportWarning", "IndentationError",
    "InterruptedError", "IsADirectoryError", "MemoryError", "NotADirectoryError", "OverflowError", "PendingDeprecationWarning",
    "ProcessLookupError", "RecursionError", "ReferenceError", "ResourceWarning", "RuntimeWarning", "SyntaxError", "SyntaxWarning",
    "SystemError", "TabError", "UnboundLocalError", "UnicodeDecodeError", "UnicodeEncodeError", "UnicodeError", "UnicodeTranslateError",
    "UnicodeWarning", "copyright", "credits", "exit", "license", "quit", "dir", "FileExistsError", "ExceptionGroup", "BaseExceptionGroup",
    "EncodingWarning", "PythonFinalizationError",
];

pub fn is_builtin_name(n: &str) -> bool {
    BUILTIN_NAMES.contains(&n)
}

/// Models that say themselves where they raise (raise_here / may_raise_here): their calls are
/// not "may raise anything".
/// Library constructors whose models report their raises.
pub const SELF_REPORTING_EXTERNALS: &[&str] = &[
    // (simulated exactly: an exception is raised by the code they run, `__post_init__` …)
    "builtins.next", "configparser.ConfigParser", "configparser.RawConfigParser", "dataclasses.replace", "dataclasses.asdict", "dataclasses.astuple", "dataclasses.fields", "dataclasses.is_dataclass", "json.loads", "builtins.setattr",
    "os.path.join", "os.path.basename", "os.path.dirname", "os.path.split", "os.path.splitext", "os.path.normpath", "os.path.isabs",
    "pathlib.PurePosixPath", "pathlib.PurePath", "pathlib.Path", "pathlib.PosixPath",
    "pandas.DataFrame", "io.StringIO", "csv.writer", "csv.DictWriter", "csv.reader", "csv.DictReader", "argparse.ArgumentParser", "threading.Thread",
    "concurrent.futures.ThreadPoolExecutor", "concurrent.futures.ProcessPoolExecutor", "concurrent.futures.as_completed", "builtins.slice", "contextlib.ExitStack", "contextlib.redirect_stdout", "contextlib.redirect_stderr", "contextlib.closing", "contextlib.nullcontext", "itertools.count", "itertools.cycle", "itertools.repeat",
    "itertools.islice", "itertools.chain", "itertools.chain.from_iterable", "itertools.product", "itertools.permutations",
    "itertools.combinations", "itertools.combinations_with_replacement", "itertools.zip_longest", "itertools.tee", "itertools.pairwise",
    "itertools.batched", "itertools.compress", "heapq.heappush", "heapq.heapify", "heapq.nlargest", "heapq.nsmallest", "heapq.merge",
    "bisect.bisect", "bisect.bisect_left", "bisect.bisect_right", "bisect.insort", "bisect.insort_left", "bisect.insort_right",
    "operator.methodcaller", "operator.itemgetter", "operator.attrgetter",
];

pub const SELF_REPORTING: &[&str] = &["excinit", "str.format", "cfgparser.", "cfgsection.", "pydantic.", "fieldinfo.", "pydverr.", "typeadapter.", "fastapi.client.", "fastapi.response.", "fastapi.request.", "stdobj.", "fastapi.bg.", "fastapi.app.", "fastapi.router.", "fastapi.state."];

/// Callees that take all of an iterable's items at once (no program code runs between them).
pub const BULK_CONSUMERS: &[&str] = &[
    "builtins.list", "builtins.tuple", "builtins.set", "builtins.frozenset", "builtins.dict", "builtins.sorted", "builtins.sum", "builtins.min", "builtins.max", "builtins.any", "builtins.all",
    "str.join", "list.extend", "set.update", "dict.update", "collections.Counter", "collections.deque", "collections.OrderedDict",
];

pub const SAFE_BUILTINS: &[&str] = &["len", "str", "repr", "bool", "int", "float", "sorted", "reversed", "list", "dict", "set", "tuple", "frozenset", "min", "max", "sum", "abs", "round", "isinstance", "issubclass", "hasattr", "enumerate", "zip", "range", "print", "id", "hash", "type", "callable", "any", "all", "map", "filter", "format", "iter", "super"];
/// Method names that neither change their receiver nor keep their arguments.
pub const PURE_METHODS: &[&str] = &[
    "get", "keys", "values", "items", "copy", "count", "index", "startswith", "endswith", "format", "join", "split", "rsplit", "strip", "lstrip", "rstrip", "lower",
    "upper", "replace", "find", "rfind", "encode", "decode", "isdigit", "isalpha", "isspace", "splitlines", "partition", "rpartition", "casefold", "title",
];
/// Builtins that neither change nor keep their arguments.
pub const PURE_BUILTINS: &[&str] = &["len", "isinstance", "issubclass", "print", "str", "repr", "int", "float", "bool", "hash", "id", "type", "hasattr", "abs", "round", "format"];

/// Library functions that only read their arguments: they change none of them, keep none of
/// them once they return and give back none of their objects (a new value: text, numbers, a
/// new container of texts), and run none of the program's code but what they are given as
/// code (`re.sub`'s `repl`, `json.dumps`'s `default`) and the dunders reading calls. A
/// qualified name ending in `.` stands for every function of that module.
const READ_ONLY_LIBRARY: &[&str] = &[
    "json.dumps", "json.loads", "os.path.", "posixpath.", "ntpath.", "os.fspath", "re.match", "re.search", "re.fullmatch", "re.findall",
    "re.finditer", "re.sub", "re.subn", "re.split", "re.escape", "re.compile", "math.", "warnings.warn", "inspect.isclass", "inspect.isfunction",
    "inspect.ismethod", "inspect.iscoroutinefunction", "inspect.isgeneratorfunction", "inspect.cleandoc", "textwrap.dedent", "textwrap.indent",
    "textwrap.wrap", "textwrap.fill", "textwrap.shorten", "shlex.quote", "shlex.split", "shlex.join", "fnmatch.", "unicodedata.", "string.capwords",
    "keyword.iskeyword", "base64.", "binascii.", "struct.pack", "struct.unpack", "struct.calcsize", "html.escape", "html.unescape",
    "urllib.parse.quote", "urllib.parse.unquote", "urllib.parse.urlparse", "urllib.parse.urlsplit", "urllib.parse.urlencode", "urllib.parse.urljoin",
    "urllib.parse.parse_qs", "urllib.parse.parse_qsl", "time.sleep", "time.strftime", "sys.intern", "logging.getLogger",
];

/// Does library function `qual` only read its arguments (`READ_ONLY_LIBRARY`)?
pub fn read_only_library(qual: &str) -> bool {
    READ_ONLY_LIBRARY.iter().any(|p| if p.ends_with('.') { qual.starts_with(p) } else { qual == *p })
}
