"""dp: a small plugin-based data platform.

Importing the package registers the built-in plugins (sources, transforms, exporters); the
settings layer, the pipeline and the HTTP API are importable from here."""
from dp.registry import REGISTRY, Plugin, Registry, RegistryError, register
from dp import ingest, processing, export  # noqa: F401  (registration side effects)
from dp.settings import ExportConfig, PlatformSettings, build_settings, try_settings
from dp.pipeline import Pipeline, run_pipeline

VERSION = {"major": 0, "minor": 3, "label": "dp-0.3"}

__all__ = [
    "REGISTRY",
    "Plugin",
    "Registry",
    "RegistryError",
    "register",
    "ExportConfig",
    "PlatformSettings",
    "build_settings",
    "try_settings",
    "Pipeline",
    "run_pipeline",
    "VERSION",
]
