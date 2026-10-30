"""A metaclass's `__new__` gets the class body's namespace as a dict: it may take entries out
before `super().__new__` makes the class from it (they are then no class attributes), and set
attributes on the class it made. Its `__init__` then runs on that class, and may call the
metaclass's own methods through it (`cls.collect_hooks()`: a class's attribute it lacks is
its metaclass's). Here the declared fields move from the class into `_fields`, and `_hooks`
names the methods marked as hooks, which `run_hooks` calls by name. Python runs exactly this
when each class statement completes, so the reads below have one answer each.

Expectations were written from the program text before running the engine."""


class Field:
    def __init__(self, kind):
        self.kind = kind


def hook(fn):
    fn.is_hook = True
    return fn


class Meta(type):
    def __new__(mcs, name, bases, ns):
        fields = {}
        for key, value in list(ns.items()):
            if isinstance(value, Field):
                fields[key] = value.kind
                del ns[key]
        cls = super().__new__(mcs, name, bases, ns)
        inherited = {}
        for b in bases:
            inherited.update(getattr(b, "_fields", {}))
        inherited.update(fields)
        cls._fields = inherited
        return cls

    def __init__(cls, name, bases, ns):
        super().__init__(name, bases, ns)
        cls._hooks = cls.collect_hooks(ns)

    def collect_hooks(cls, ns):
        found = []
        for key, value in ns.items():
            if getattr(value, "is_hook", False):
                found.append(key)
        return found


class Base(metaclass=Meta):
    id = Field("int")

    def run_hooks(self, x):
        for name in self._hooks:
            x = getattr(self, name)(x)
        return x


class User(Base):
    name = Field("str")

    @hook
    def strip(self, x):
        return x.strip()


base_fields = list(Base._fields.items())
n_base = len(base_fields)
#@value n_base: exactly={1}
user_kinds = ",".join(k + ":" + v for k, v in User._fields.items())
#@value user_kinds: exactly={"id:int,name:str"}
n_hooks = len(Base._hooks)
#@value n_hooks: exactly={0}
user_hooks = ",".join(User._hooks)
#@value user_hooks: exactly={"strip"}
has_name = hasattr(User, "name")
#@value has_name: exactly={False}
out = User().run_hooks("  ada ")
#@value out: exactly={"ada"}
plain = Base().run_hooks(" x ")
#@value plain: exactly={" x "}
