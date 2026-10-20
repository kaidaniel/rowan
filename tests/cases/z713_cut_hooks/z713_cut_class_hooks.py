"""A class whose creation runs an installed package's code (a base's `__init_subclass__`) that
does not complete (past the entry's allowance its calls do not run) may have been given
attributes of any name, as a class with a base rowan does not see may: following the package
is never worse than not following it. Before, the hook's `_map(cls)` did not run and its
`setattr(cls, name, …)` by a computed name was ignored, so `hasattr(type(self), "total")` in
`Base.__init__` was decided False and `Order(total=1.5)` always raised: nothing after it ran.

Expectations were written from the program text before running the engine."""
#@argv --follow hooklib --site-packages site
#@limits budget=64
from hooklib import Base


class Order(Base):
    total: float


o = Order(total=1.5)
t = o.total                                  #@value t: includes={1.5}
done = True                                  #@value done: exactly={True}
