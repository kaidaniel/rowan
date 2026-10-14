"""Driver for the pip benchmark: pip's commands through its CLI entry point, then the pieces a
requirements tool uses directly (requirement lines, markers, versions, wheel tags, links)."""

import io
import json
import sys
from contextlib import redirect_stdout

from pip._internal.cli.main import main
from pip._internal.commands import commands_dict, create_command
from pip._internal.models.link import Link
from pip._internal.models.wheel import Wheel
from pip._internal.req.constructors import install_req_from_line
from pip._internal.req.req_file import break_args_options, preprocess
from pip._internal.utils.compatibility_tags import get_supported
from pip._internal.utils.misc import format_size, splitext
from pip._internal.utils.hashes import Hashes
from pip._vendor.packaging.markers import Marker
from pip._vendor.packaging.requirements import Requirement
from pip._vendor.packaging.specifiers import SpecifierSet
from pip._vendor.packaging.utils import canonicalize_name, parse_wheel_filename
from pip._vendor.packaging.version import Version

# the commands rowan follows statically (pip imports them by name with importlib)
from pip._internal.commands import (  # noqa: F401
    cache, check, completion, configuration, debug, download, freeze, hash, help, index,
    inspect, install, list, lock, search, show, uninstall, wheel,
)

REQUIREMENTS = """\
# pinned for the service
requests[socks]>=2.31,<3 ; python_version >= "3.8"
rich==13.9.4 \\
    --hash=sha256:6049d5e6ec054bf2779ab3358186963bac2ea89175919d699e378b99738c2a90
Django~=5.0
numpy>=1.26; platform_machine != "armv7l"
-e git+https://github.com/pallets/click.git@8.1.7#egg=click
./vendor/tool-1.0-py3-none-any.whl
"""


def run(args):
    out = io.StringIO()
    with redirect_stdout(out):
        try:
            code = main(args)
        except SystemExit as e:
            code = e.code
    return code, out.getvalue()


results = {}
for args in (
    ["--version"],
    ["help", "install"],
    ["list", "--format=json"],
    ["show", "pip"],
    ["freeze", "--all"],
    ["check"],
    ["config", "list"],
    ["debug"],
    ["hash", __file__],
    ["install", "--dry-run", "--no-index", "--find-links", ".", "--report", "-", "tool"],
    ["download", "--no-index", "--no-deps", "-d", "/tmp/pip-bench", "tool"],
    ["uninstall", "-y", "not-installed-anywhere"],
):
    results[" ".join(args)] = run(args)

cmd = create_command("install", isolated=True)
options, rest = cmd.parse_args(["--no-index", "--upgrade", "-r", "requirements.txt"])

pins = {}
for line_number, line in preprocess(REQUIREMENTS):
    req_text, opts = break_args_options(line)
    if not req_text or req_text.startswith("-e"):
        continue
    ireq = install_req_from_line(req_text, line_source=f"requirements.txt (line {line_number})")
    name = canonicalize_name(ireq.name) if ireq.name else req_text
    pins[name] = {
        "specifier": str(ireq.specifier),
        "markers": str(ireq.markers) if ireq.markers else None,
        "applies": ireq.match_markers(),
        "link": ireq.link.url if ireq.link else None,
        "options": opts,
    }

req = Requirement('requests[socks]>=2.31,<3 ; python_version >= "3.8"')
spec = SpecifierSet(">=2.31,<3")
candidates = [Version(v) for v in ("2.30.0", "2.31.0", "2.32.3", "3.0.0a1", "3.0.0")]
allowed = sorted(spec.filter(candidates))
best = max(allowed)
env_ok = Marker('python_version >= "3.8" and os_name == "posix"').evaluate()

tags = get_supported()
wheel = Wheel("rich-13.9.4-py3-none-any.whl")
supported = wheel.supported(tags)
name, version, build, wheel_tags = parse_wheel_filename("numpy-2.1.0-cp314-cp314-macosx_14_0_arm64.whl")
link = Link("https://files.pythonhosted.org/packages/rich-13.9.4-py3-none-any.whl#sha256=" + "0" * 64)
base, ext = splitext(link.filename)
hashes = Hashes({"sha256": [link.hash or ""]})

summary = {
    "commands": sorted(commands_dict),
    "exit_codes": {k: v[0] for k, v in results.items()},
    "pins": pins,
    "best": str(best),
    "extras": sorted(req.extras),
    "env_ok": env_ok,
    "wheel": [wheel.name, wheel.version, supported, wheel.support_index_min(tags) if supported else None],
    "numpy": [str(name), str(version), sorted(str(t) for t in wheel_tags)],
    "link": [base, ext, link.hash_name, link.is_wheel, hashes.is_hash_allowed("sha256", link.hash or "")],
    "size": format_size(123456789),
}
json.dump(summary, sys.stdout, indent=1, default=str)
