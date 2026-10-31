# typer's style, `config: Path = typer.Option(...)`: the default is no Path but a marker the
# framework replaces before every call it makes. In a run for any call the parameter is as
# annotated (rowan's claims hold relative to annotations), so not that marker: reading a Path
# method of it is no missing attribute. A default the annotation allows stays (None, an
# instance of the annotated class, anything under `Any`).
from pathlib import Path
from typing import Any, Optional


class OptionInfo:
    def __init__(self, default):
        self.default = default


def option(default) -> Any:
    return OptionInfo(default)


def run(config: Path = option(Path("~/.cfg")), out: Optional[Path] = None):
    full = config.expanduser()        #@ input expanduser ; sound expanduser
    where = out                       #@general where: includes={None}
    return full, where


def marked(marker: OptionInfo = option(1)):
    v = marker.default                #@general v: shows={1}
    return v


def loose(marker: Any = option(2)):
    v = marker.default                #@general v: shows={2}
    return v
