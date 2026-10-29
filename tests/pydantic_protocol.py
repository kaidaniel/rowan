"""The pydantic model (`models/pydantic.rs`), checked exhaustively against real pydantic in the
style of `lib_protocol.py`:

- an input dict (`INPUTS`): fields of the right type, of a type pydantic coerces (`'2'` for
  an int), of a wrong type, missing, None, extra keys, a value a validator rejects; one of
  two joined by a branch; an unknown (`json.loads` of an environment variable: its run's
  value fixed);
- one construction or validation of a model (`MODELS`: defaults, Optional, a list field, extra
  forbid / allow, a field validator, a model validator, nested models, an alias) and a read
  of what it gives (`OPS`): a field, `model_dump` (with exclude / include / unset / none),
  `model_dump_json`, `model_fields_set`, `model_copy`, `model_fields`, equality, `hasattr`,
  `getattr` with a default, `model_validate_json`, the errors' locations and types;
- in a `try` naming the class of what it raises (a ValidationError is a ValueError).

Observation point: the result (a scalar, a container's items, `lib_protocol.ENC`) or the class
it raised.

    uv run --no-project --with pydantic python tests/pydantic_protocol.py [--rowan N | --all] [--seed S] [-v]
"""
from __future__ import annotations

import itertools

from lib_protocol import ENC_HELPERS, TAIL_ENC, program
from protocol_lib import cli, typing_shim

NAMES = ["O"]

LIB = {
    "imports": [
        "import json", "from typing import Literal, Optional, Union",
        "from pydantic import BaseModel, ConfigDict, Field, ValidationError, field_validator, model_validator",
    ],
    "inputs": {
        "ok": "{'a': 1}", "coerce": "{'a': '2', 'b': 'y'}", "bad": "{'a': 'x'}", "empty": "{}", "extra": "{'a': 1, 'zz': 3}",
        "neg": "{'a': -1}", "float": "{'a': 1.5}", "none": "{'a': None}", "full": "{'a': 4, 'b': 'q', 'c': 2, 'tags': ['t']}",
        "badlist": "{'a': 1, 'tags': 'abc'}", "joined": "{'a': 1} if c1 else {'a': 'q'}",
        "unknown": "json.loads(os.environ.get('RY{N}', '{\"a\": 3}'))",
    },
    "ops": {
        "a": "M(**v).a", "b": "M(**v).b", "c": "M(**v).c", "tags": "M(**v).tags", "atype": "type(M(**v).a).__name__",
        "dump": "M.model_validate(v).model_dump()", "dump_unset": "M(**v).model_dump(exclude_unset=True)",
        "dump_none": "M(**v).model_dump(exclude_none=True)", "dump_include": "M(**v).model_dump(include={'a'})",
        "dump_exclude": "M(**v).model_dump(exclude={'tags', 'c'})", "json": "M(**v).model_dump_json()",
        "fields_set": "sorted(M.model_validate(v).model_fields_set)", "copy": "M(**v).model_copy(update={'b': 'z'}).b",
        "fields": "list(M.model_fields)", "eq": "M(**v) == M(a=1)", "hasattr": "hasattr(M(**v), 'zz')",
        "strict": "Strict(**v).a", "allow": "Allow(**v).model_dump()", "allow_get": "getattr(Allow(**v), 'zz', None)",
        "validator": "V(**v).a", "nested": "Nested(m=v).m.a", "nested_list": "Nested.model_validate({'m': v, 'items': [v, {'a': 7}]}).items[1].a",
        "nested_dump": "Nested(m=v).model_dump()", "alias": "Alias.model_validate({'userId': v.get('a')}).user_id",
        "alias_name": "Alias(user_id=1).user_id", "alias_dump": "Alias.model_validate({'userId': 5}).model_dump(by_alias=True)",
        "model_validator": "MV(lo=v.get('a', 0), hi=1).lo", "validate_json": "M.model_validate_json(json.dumps(v)).a",
        "errors": "ERRS(v)", "error_type": "ERRTYPE(v)", "setattr": "SETA(v)", "construct": "M.model_construct(**v).b",
        "dict_v1": "M(**v).dict()", "default_list": "DEFLIST(v)", "frozen": "FROZ(v)",
        "gt": "C(x=v.get('a', 1)).x", "minlen": "C(x=1, s=v.get('b', 'zz')).s", "bool": "C(x=1, b=v.get('a', False)).b",
        "str_from_int": "M(a=1, b=v.get('a')).b", "int_list": "C(x=1, lst=v.get('tags', [])).lst", "factory": "len(C(x=1).lst)",
        "c_errors": "CERRS(v)", "from_attrs": "FA(v)", "opt_none": "M(a=1, c=v.get('c')).c", "json_errors": "JERR(v)",
        "union": "U(x=v.get('a')).x", "union_type": "type(U(x=v.get('a')).x).__name__", "literal": "U(x=1, lit=v.get('b', 'a')).lit",
        "dict_field": "U(x=1, d={'k': v.get('a')}).d", "tuple_field": "U(x=1, t=[v.get('a', 0), 2]).t", "before": "BV(a=v.get('a')).a",
        "dump_json_mode": "M(**v).model_dump(mode='json')", "populate": "PA(user_id=v.get('a')).user_id", "populate_alias": "PA(userId=v.get('a')).user_id",
        "nested_loc": "NERR(v)", "deep_copy": "DEEPC(v)",
    },
    "helpers": [
        "class M(BaseModel):", "    a: int", "    b: str = 'x'", "    c: Optional[float] = None", "    tags: list[str] = []",
        "class Strict(BaseModel):", "    model_config = ConfigDict(extra='forbid')", "    a: int",
        "class Allow(BaseModel):", "    model_config = ConfigDict(extra='allow')", "    a: int",
        "class V(BaseModel):", "    a: int", "    @field_validator('a')", "    @classmethod", "    def pos(cls, x):",
        "        if x < 0:", "            raise ValueError('neg')", "        return x * 2",
        "class Nested(BaseModel):", "    m: M", "    items: list[M] = []",
        "class Alias(BaseModel):", "    user_id: int = Field(alias='userId')",
        "class MV(BaseModel):", "    lo: int", "    hi: int", "    @model_validator(mode='after')", "    def order(self):",
        "        if self.lo > self.hi:", "            raise ValueError('order')", "        return self",
        "class F(BaseModel):", "    model_config = ConfigDict(frozen=True)", "    a: int",
        "class C(BaseModel):", "    x: int = Field(gt=0)", "    s: str = Field(default='ok', min_length=2)", "    b: bool = False",
        "    lst: list[int] = Field(default_factory=list)",
        "class FromAttrs(BaseModel):", "    model_config = ConfigDict(from_attributes=True)", "    a: int",
        "class Obj:", "    def __init__(self, a):", "        self.a = a",
        "class U(BaseModel):", "    x: Union[int, str]", "    lit: Literal['a', 'b'] = 'a'", "    d: dict[str, int] = {}",
        "    t: tuple[int, int] = (0, 0)",
        "class BV(BaseModel):", "    a: int", "    @field_validator('a', mode='before')", "    @classmethod", "    def ln(cls, x):",
        "        return len(x) if isinstance(x, str) else x",
        "class PA(BaseModel):", "    model_config = ConfigDict(populate_by_name=True)", "    user_id: int = Field(alias='userId')",
        "def NERR(v):", "    try:", "        Nested(m=v, items=[v])", "    except ValidationError as e:",
        "        return ['.'.join(str(p) for p in x['loc']) for x in e.errors()]", "    return 'ok'",
        "def DEEPC(v):", "    m = M(**v)", "    c = m.model_copy(deep=True)", "    c.tags.append('z')", "    return [len(m.tags), len(c.tags)]",
        "def CERRS(v):", "    try:", "        C(x=v.get('a', 0), s=v.get('b', ''))", "    except ValidationError as e:",
        "        return [e2['type'] for e2 in e.errors()]", "    return 'ok'",
        "def FA(v):", "    return FromAttrs.model_validate(Obj(v.get('a'))).a",
        "def JERR(v):", "    try:", "        M.model_validate_json(json.dumps(v) + 'x')", "    except ValidationError as e:",
        "        return e.errors()[0]['type']", "    return 'ok'",
        "def ERRS(v):", "    try:", "        M(**v)", "    except ValidationError as e:", "        return [str(x['loc'][0]) for x in e.errors()]",
        "    return 'ok'",
        "def ERRTYPE(v):", "    try:", "        M(**v)", "    except ValidationError as e:", "        return e.errors()[0]['type']", "    return 'ok'",
        "def SETA(v):", "    m = M(**v)", "    m.b = 'set'", "    m.zz = 1", "    return m.b",
        "def DEFLIST(v):", "    m1 = M(**v)", "    m1.tags.append('q')", "    return len(M(a=1).tags)",
        "def FROZ(v):", "    f = F(**v)", "    f.a = 2", "    return f.a",
        *ENC_HELPERS,
    ],
}


def programs():
    for (ik, iv), (ok, ov) in itertools.product(LIB["inputs"].items(), LIB["ops"].items()):
        yield program(LIB, iv, ov, f"pydantic {ik} {ok}", TAIL_ENC)


if __name__ == "__main__":
    typing_shim()
    cli(programs, NAMES)
