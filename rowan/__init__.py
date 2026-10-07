"""rowan — static key-set inference for Python records (Phase-0 prototype)."""
from .interp import Analyzer  # noqa: F401
from .report import format_text, to_json  # noqa: F401

__all__ = ["Analyzer", "format_text", "to_json", "analyse_source", "analyse_file"]


def analyse_source(source: str, filename: str = "<module>", search_paths=(), **opts):
    """Analyse a program; runs in a thread with a large stack since the interpreter recurses
    as deeply as the analysed program's call nesting."""
    import sys
    import threading
    result = {}

    def work():
        sys.setrecursionlimit(max(sys.getrecursionlimit(), 200000))
        an = Analyzer(source, filename, search_paths, **opts)
        try:
            an.run()
        except BaseException as e:   # noqa: BLE001
            result["error"] = e
        result["an"] = an

    old = threading.stack_size()
    threading.stack_size(1024 * 1024 * 1024)
    try:
        t = threading.Thread(target=work)
        t.start()
        t.join()
    finally:
        threading.stack_size(old)
    if "error" in result:
        raise result["error"]
    return result["an"]


def analyse_file(path: str, **opts):
    import os
    with open(path, encoding="utf-8") as f:
        src = f.read()
    return analyse_source(src, path, search_paths=[os.path.dirname(os.path.abspath(path))], **opts)
