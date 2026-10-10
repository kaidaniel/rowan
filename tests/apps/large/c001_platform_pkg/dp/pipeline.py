"""The pipeline: sources from the settings, ingestion, validation, frame transforms and the
report that the export layer and the API read."""
from dp import aggregate, processing
from dp.export import export, summary_line
from dp.registry import REGISTRY
from dp.validation import error_summary, validate_records


class Pipeline:
    def __init__(self, settings, registry=REGISTRY):
        self.settings = settings
        self.registry = registry
        self.sources = {name: registry.create("source", name) for name in settings.sources}
        self.batches = {}
        self.events = []
        self.rejected = []
        self.log = []

    def ingest(self, inputs):
        for name, src in self.sources.items():
            recs = src.load(inputs.get(name, ""))
            self.batches[name] = recs[: self.settings.batch_size]
            self.log.append(f"ingest {name}: {len(recs)} records, {len(src.errors)} errors")
        return sum(len(v) for v in self.batches.values())

    def validate(self):
        records = [rec for name in self.batches for rec in self.batches[name]]
        self.events, self.rejected = validate_records(records, strict=self.settings.strict)
        self.log.append(f"validate: {len(self.events)} ok, {len(self.rejected)} rejected")
        return len(self.events)

    def process(self, transforms=("threshold",)):
        df = processing.events_frame(self.events)
        for name in transforms:
            df = self.registry.create("transform", name).apply(df, self.settings)
        self.frame = df
        self.log.append(f"process: {len(df)} rows after {len(transforms)} transforms")
        return df

    def source_stats(self):
        return [src.stats() for src in self.sources.values()]

    def report(self, profiles):
        s = self.settings
        df = self.frame
        users = processing.with_profiles(processing.by_user(df), profiles)
        rep = {
            "name": s.name,
            "currency": s.currency,
            "n_events": len(self.events),
            "n_rejected": len(self.rejected),
            "n_kept": len(df),
            "total": round(float(df["amount"].sum()), 2),
            "users": users.to_dict("records"),
            "by_region": aggregate.totals_by(self.events, "region"),
            "by_day": aggregate.runs_by_day(self.events),
            "top": aggregate.top_users(self.events),
            "pivot": processing.pivot_dict(processing.pivot_region_source(df)),
            "errors": error_summary(self.rejected),
        }
        if s.tags:
            rep["tags"] = dict(s.tags)
        return rep


def run_pipeline(settings, inputs, profiles, transforms=("threshold",)):
    pipe = Pipeline(settings)
    pipe.ingest(inputs)
    pipe.validate()
    pipe.process(transforms)
    rep = pipe.report(profiles)
    out = export(rep, settings, pipe.registry)
    return pipe, rep, out, summary_line(rep)
