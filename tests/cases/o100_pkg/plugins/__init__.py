"""A registry filled by a decorator; importing the submodules registers their classes."""
REGISTRY = {}


def register(name, **opts):
    def deco(cls):
        REGISTRY[name] = {"cls": cls, "opts": dict(opts)}
        cls.plugin_name = name
        return cls

    return deco


from . import csv_in, json_in  # noqa: E402  (registration side effects)

__all__ = ["REGISTRY", "register", "csv_in", "json_in"]
