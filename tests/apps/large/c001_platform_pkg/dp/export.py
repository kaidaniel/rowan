"""Export layer: exporter plugins (JSON with sorted keys, CSV through csv.DictWriter) and
string formatting of a report."""
import csv
import io
import json

from dp.registry import Plugin, register


class Exporter(Plugin):
    """Base of the exporters."""

    def render(self, payload, cfg):
        raise NotImplementedError


@register("exporter", "json")
class JsonExporter(Exporter):
    """A JSON document."""

    def render(self, payload, cfg):
        return json.dumps(payload, sort_keys=cfg.sort_keys, indent=cfg.indent or None)


@register("exporter", "csv")
class CsvExporter(Exporter):
    """The payload's rows as CSV with a header line."""

    def render(self, payload, cfg):
        rows = payload["rows"]
        fields = self.option("fields") or sorted({k for r in rows for k in r})
        buf = io.StringIO()
        w = csv.DictWriter(buf, fieldnames=fields, extrasaction="ignore", lineterminator="\n")
        w.writeheader()
        for r in rows:
            w.writerow(r)
        return buf.getvalue()


def summary_line(report):
    return "{name}: {n_events} events, {n_rejected} rejected, total {total:.2f} {currency}".format(**report)


def export_payload(report):
    payload = {"name": report["name"], "rows": report["users"], "by_region": report["by_region"]}
    if report["errors"]:
        payload["errors"] = report["errors"]
    return payload


def export(report, settings, registry, **options):
    cfg = settings.export
    exporter = registry.create("exporter", cfg.format, **options)
    text = exporter.render(export_payload(report), cfg)
    return {"format": cfg.format, "exporter": type(exporter).__name__, "chars": len(text), "text": text}
