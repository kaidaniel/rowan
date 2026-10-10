"""Ingestion plugins: CSV text (through io.StringIO), JSON lines and a regex-parsed log
format.  Every source turns a text into plain record dicts tagged with the source name and
the record's position."""
import csv
import io
import json
import re

from dp.registry import Plugin, register

LOG_RE = re.compile(
    r"^(?P<ts>\d{4}-\d\d-\d\dT\d\d:\d\d) (?P<level>[A-Z]+) user=(?P<user>\w+) id=(?P<id>\w+)"
    r" amount=(?P<amount>-?\d+(?:\.\d+)?)(?: region=(?P<region>\w+))?$"
)


class Source(Plugin):
    """Base of the sources."""

    def __init__(self, **options):
        super().__init__(**options)
        self.errors = []
        self.seen = 0

    def records(self, text):
        raise NotImplementedError

    def load(self, text):
        out = []
        for i, rec in enumerate(self.records(text)):
            self.seen += 1
            if rec is None:
                continue
            rec["source"] = self.plugin_name
            rec["seq"] = i
            out.append(rec)
        return out

    def stats(self):
        return {"source": self.plugin_name, "seen": self.seen, "errors": len(self.errors)}


@register("source", "csv", aliases=("comma",))
class CsvSource(Source):
    """Comma separated rows under a header line."""

    def records(self, text):
        reader = csv.DictReader(io.StringIO(text), delimiter=self.option("delimiter", ","))
        for row in reader:
            if not row.get("id"):
                self.errors.append({"line": reader.line_num, "error": "no id"})
                yield None
                continue
            yield {
                "id": row["id"],
                "user": row["user"].strip(),
                "amount": row["amount"],
                "region": row.get("region") or "unknown",
                "ts": row["ts"],
            }


@register("source", "jsonl", aliases=("ndjson",))
class JsonLinesSource(Source):
    """One JSON object per line; blank lines and # comments are skipped."""

    def records(self, text):
        for n, line in enumerate(text.splitlines(), 1):
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            try:
                obj = json.loads(line)
            except json.JSONDecodeError as e:
                self.errors.append({"line": n, "error": "bad json: " + e.msg})
                yield None
                continue
            yield self.normalize(obj)

    def normalize(self, obj):
        rec = {
            "id": str(obj["id"]),
            "user": obj["user"],
            "amount": obj.get("amount", 0),
            "region": obj.get("region", "unknown"),
            "ts": obj["ts"],
        }
        if "tags" in obj:
            rec["tags"] = list(obj["tags"])
        return rec


@register("source", "log")
class LogSource(Source):
    """Application log lines: a timestamp, a level and key=value pairs."""

    levels = ("INFO", "WARN")

    def records(self, text):
        for n, line in enumerate(text.splitlines(), 1):
            m = LOG_RE.match(line.strip())
            if m is None:
                self.errors.append({"line": n, "error": "unparsed"})
                yield None
                continue
            d = m.groupdict()
            if d["level"] not in self.levels:
                yield None
                continue
            rec = {k: d[k] for k in ("id", "user", "amount", "ts")}
            rec["region"] = d["region"] or "unknown"
            rec["level"] = d["level"]
            yield rec


def parse_log_line(line):
    """One log line as its named groups (None when it does not match)."""
    m = LOG_RE.match(line.strip())
    return m.groupdict() if m else None
