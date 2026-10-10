"""Plugin registry: handler classes register themselves under (kind, name) with a decorator
and are looked up by the names the configuration gives."""


class RegistryError(KeyError):
    """No plugin of that kind and name."""


class Registry:
    def __init__(self):
        self._plugins = {}  # kind -> {name: cls}
        self._aliases = {}  # (kind, alias) -> name
        self.history = []

    def register(self, kind, name, *, aliases=()):
        def deco(cls):
            bucket = self._plugins.setdefault(kind, {})
            if name in bucket:
                raise ValueError(f"duplicate {kind} plugin {name!r}")
            bucket[name] = cls
            cls.plugin_kind = kind
            cls.plugin_name = name
            for alias in aliases:
                self._aliases[(kind, alias)] = name
            self.history.append(f"{kind}:{name}")
            return cls

        return deco

    def canonical(self, kind, name):
        return self._aliases.get((kind, name), name)

    def resolve(self, kind, name):
        name = self.canonical(kind, name)
        try:
            return self._plugins[kind][name]
        except KeyError:
            raise RegistryError(f"no {kind} plugin named {name}") from None

    def create(self, kind, name, **options):
        cls = self.resolve(kind, name)
        return cls(**options)

    def has(self, kind, name):
        return self.canonical(kind, name) in self._plugins.get(kind, {})

    def names(self, kind):
        return sorted(self._plugins.get(kind, {}))

    def kinds(self):
        return {kind: len(bucket) for kind, bucket in self._plugins.items()}

    def describe(self, kind):
        out = []
        for name, cls in sorted(self._plugins.get(kind, {}).items()):
            doc = (cls.__doc__ or "").strip()
            out.append({"name": name, "cls": cls.__name__, "doc": doc.splitlines()[0] if doc else ""})
        return out


REGISTRY = Registry()
register = REGISTRY.register


class Plugin:
    """Base of every plugin: keeps its options."""

    plugin_kind = "?"
    plugin_name = "?"

    def __init__(self, **options):
        self.options = dict(options)

    def option(self, key, default=None):
        return self.options.get(key, default)

    def info(self):
        return {"kind": self.plugin_kind, "name": self.plugin_name, "options": sorted(self.options)}
