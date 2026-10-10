"""A small FastAPI app: run the pipeline on posted inputs, read runs back, export them."""
from typing import Optional

from fastapi import Body, Depends, FastAPI, HTTPException
from pydantic import BaseModel, ValidationError

from dp.pipeline import Pipeline
from dp.registry import REGISTRY, RegistryError
from dp.settings import ExportConfig, try_settings

app = FastAPI(title="dp")
STATE = {"runs": {}, "profiles": [], "calls": 0}


class RunRequest(BaseModel):
    config: str = ""
    inputs: dict[str, str] = {}
    transforms: list[str] = ["threshold"]


class RunSummary(BaseModel):
    name: str
    events: int
    rejected: int
    kept: int
    sources: list[str]


def runs_store():
    STATE["calls"] += 1
    return STATE["runs"]


@app.get("/plugins")
def list_plugins():
    return {kind: REGISTRY.names(kind) for kind in sorted(REGISTRY.kinds())}


@app.get("/plugins/{kind}")
def describe_plugins(kind: str):
    if kind not in REGISTRY.kinds():
        raise HTTPException(status_code=404, detail=f"no plugin kind {kind}")
    return REGISTRY.describe(kind)


@app.post("/runs/{name}", response_model=RunSummary)
def create_run(name: str, req: RunRequest, runs: dict = Depends(runs_store)):
    settings, errors = try_settings(req.config, {})
    if settings is None:
        raise HTTPException(status_code=422, detail=[{"loc": loc, "type": typ} for loc, typ in errors])
    pipe = Pipeline(settings)
    pipe.ingest(req.inputs)
    pipe.validate()
    try:
        pipe.process(req.transforms)
    except RegistryError as e:
        raise HTTPException(status_code=400, detail=e.args[0])
    runs[name] = pipe.report(STATE["profiles"])
    return RunSummary(name=name, events=len(pipe.events), rejected=len(pipe.rejected), kept=len(pipe.frame), sources=list(pipe.sources))


@app.get("/runs/{name}")
def read_run(name: str, fields: Optional[str] = None, runs: dict = Depends(runs_store)):
    if name not in runs:
        raise HTTPException(status_code=404, detail=f"no run named {name}")
    rep = runs[name]
    if fields:
        return {k: rep[k] for k in fields.split(",") if k in rep}
    return {k: v for k, v in rep.items() if k != "users"}


@app.post("/runs/{name}/export")
def export_run(name: str, payload: dict = Body(...), runs: dict = Depends(runs_store)):
    if name not in runs:
        raise HTTPException(status_code=404, detail=f"no run named {name}")
    rep = runs[name]
    fmt = payload["format"]
    try:
        cfg = ExportConfig(format=fmt, indent=payload.get("indent", 2), sort_keys=payload.get("sort_keys", True))
    except ValidationError as e:
        raise HTTPException(status_code=422, detail=[err["type"] for err in e.errors()])
    exporter = REGISTRY.create("exporter", cfg.format)
    text = exporter.render({"name": rep["name"], "rows": rep["users"], "total": rep["total"]}, cfg)
    return {"name": name, "format": cfg.format, "lines": text.count("\n") + 1, "text": text}
