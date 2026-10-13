"""Driver for the functional benchmark: an order-processing pipeline written in functional
style with the libraries side by side (parsers built from combinators, immutable records,
curried pipelines, Result / Option containers, reactive streams, lazy sequences, nested
lookups by spec, and a property checked with hypothesis)."""

import operator
from decimal import Decimal
from functools import partial, reduce

import attr
import funcy
import more_itertools as mi
import parsy
import pydash
import pyparsing as pp
import reactivex as rx
import toolz
from expression import Error, Nothing, Ok, Some, pipe
from expression.collections import Block, Map, seq
from funcparserlib.lexer import TokenSpec, make_tokenizer
from funcparserlib.parser import a, finished, many, maybe, skip, some
from functional import seq as fseq
from glom import Coalesce, T, glom
from hypothesis import given, settings
from hypothesis import strategies as st
from lark import Lark, Transformer
from pyrsistent import freeze, pmap, pvector, thaw
from reactivex import operators as ops
from returns.maybe import Maybe
from returns.pipeline import flow, is_successful
from returns.pointfree import bind, map_
from returns.result import Failure, Result, Success, safe
from sortedcontainers import SortedDict, SortedList

# --- input: one order per line, "id customer sku*qty@price,... [#tag ...]" -----------------
RAW = """\
1001 alice tea*2@3.50,cake*1@4.25 #gift
1002 bob coffee*3@2.80
1003 alice tea*1@3.50,scone*4@1.90 #rush #gift
1004 carol juice*2@3.10,cake*2@4.25
1005 bob coffee*x@2.80
1006 dave
"""


# parsy: the order line grammar
ws = parsy.regex(r"\s+")
number = parsy.regex(r"[0-9]+").map(int)
word = parsy.regex(r"[a-z]+")
price = parsy.regex(r"[0-9]+\.[0-9]{2}").map(Decimal)
item = parsy.seq(sku=word << parsy.string("*"), qty=number << parsy.string("@"), price=price)
items = item.sep_by(parsy.string(","), min=1)
tag = parsy.string("#") >> word
order_line = parsy.seq(
    id=number << ws,
    customer=word << ws,
    items=items,
    tags=(ws >> tag).many(),
)


@attr.frozen
class Item:
    sku: str
    qty: int
    price: Decimal

    @property
    def total(self) -> Decimal:
        return self.price * self.qty


@attr.frozen
class Order:
    id: int
    customer: str
    items: tuple
    tags: frozenset = frozenset()

    @property
    def total(self) -> Decimal:
        return sum((i.total for i in self.items), Decimal(0))


def parse_order(line: str) -> Result:
    try:
        d = order_line.parse(line.strip())
    except parsy.ParseError as e:
        return Failure(f"{line.split()[0]}: {e}")
    return Success(Order(d["id"], d["customer"], tuple(Item(**i) for i in d["items"]), frozenset(d["tags"])))


parsed = [parse_order(line) for line in RAW.splitlines() if line.strip()]
orders = [r.unwrap() for r in parsed if is_successful(r)]
errors = [r.failure() for r in parsed if not is_successful(r)]

# --- toolz / funcy pipelines ----------------------------------------------------------------
by_customer = toolz.groupby(operator.attrgetter("customer"), orders)
totals = toolz.valmap(lambda os: sum((o.total for o in os), Decimal(0)), by_customer)
lines = toolz.pipe(
    orders,
    toolz.curried.mapcat(lambda o: ((o.customer, i) for i in o.items)),
    toolz.curried.filter(lambda ci: ci[1].qty > 0),
    toolz.curried.map(lambda ci: {"customer": ci[0], "sku": ci[1].sku, "amount": ci[1].total}),
    list,
)
sku_amounts = toolz.reduceby("sku", lambda acc, row: acc + row["amount"], lines, Decimal(0))
merged = toolz.merge_with(sum, *({r["sku"]: r["amount"]} for r in lines))
top_skus = toolz.topk(2, sku_amounts.items(), key=toolz.second)
tagged = funcy.group_by(lambda o: "gift" in o.tags, orders)
qty_per_sku = funcy.merge_with(sum, *[{i.sku: i.qty for i in o.items} for o in orders])
rounded = funcy.walk_values(lambda v: v.quantize(Decimal("0.1")), totals)
describe = funcy.compose(str.upper, funcy.partial(str.format, "{}-{}"))
labels = [describe(o.customer, o.id) for o in orders]
first_rush = funcy.first(funcy.filter(lambda o: "rush" in o.tags, orders))

# --- more-itertools and itertools-style windows ---------------------------------------------
amounts = [o.total for o in orders]
pairs = list(mi.pairwise(amounts))
windows = [sum(w) for w in mi.windowed(amounts, 2) if None not in w]
small, large = (list(p) for p in mi.partition(lambda o: o.total > 10, orders))
batches = list(mi.chunked(orders, 2))
buckets = mi.bucket(lines, key=lambda r: r["customer"])
alice_skus = [r["sku"] for r in buckets["alice"]]
unique_skus = list(mi.unique_everseen(r["sku"] for r in lines))

# --- immutable state: pyrsistent ------------------------------------------------------------
inventory = pmap({"tea": 10, "cake": 3, "coffee": 5, "scone": 6, "juice": 2})


def reserve(stock, item):
    left = stock.get(item.sku, 0) - item.qty
    return stock.set(item.sku, left) if left >= 0 else stock


final_stock = reduce(reserve, (i for o in orders for i in o.items), inventory)
history = pvector([inventory]).append(final_stock)
nested = freeze({"orders": [attr.asdict(o, recurse=False) for o in orders], "stock": dict(final_stock)})
plain = thaw(nested)

# --- returns: railway-oriented validation ---------------------------------------------------


def require_items(order: Order) -> Result:
    return Success(order) if order.items else Failure(f"{order.id}: no items")


def within_stock(order: Order) -> Result:
    short = [i.sku for i in order.items if inventory.get(i.sku, 0) < i.qty]
    return Failure(f"{order.id}: short of {', '.join(short)}") if short else Success(order)


@safe
def discount(order: Order) -> Decimal:
    return order.total * (Decimal("0.9") if "gift" in order.tags else Decimal(1))


checked = [flow(Success(o), bind(require_items), bind(within_stock), bind(discount), map_(str)) for o in orders]
accepted = [c.unwrap() for c in checked if is_successful(c)]
maybe_vip = Maybe.from_optional(by_customer.get("alice")).map(len).value_or(0)

# --- expression: Option / Result and immutable collections ----------------------------------


def lookup(name: str):
    return Some(totals[name]) if name in totals else Nothing


def checked_total(name: str):
    return lookup(name).map(lambda t: Ok(t) if t > 5 else Error(f"{name}: too small")).default_value(Error(f"{name}: unknown"))


ledger = Map.of_seq([(c, str(t)) for c, t in totals.items()])
results = Block.of_seq(["alice", "bob", "erin"]).map(checked_total)
oks = pipe(results, seq.filter(lambda r: r.is_ok()), seq.map(lambda r: r.ok), seq.to_list)

# --- reactivex: streaming the order lines ---------------------------------------------------
running = []
batched = []
rx.from_iterable(lines).pipe(
    ops.map(lambda r: r["amount"]),
    ops.scan(operator.add, Decimal(0)),
).subscribe(on_next=running.append)
rx.from_iterable(orders).pipe(
    ops.filter(lambda o: o.total > 0),
    ops.map(lambda o: o.id),
    ops.buffer_with_count(2),
).subscribe(on_next=batched.append)
grouped_counts = []
rx.from_iterable(lines).pipe(
    ops.group_by(lambda r: r["customer"]),
    ops.flat_map(lambda g: g.pipe(ops.count(), ops.map(lambda n: (g.key, n)))),
).subscribe(on_next=grouped_counts.append)

# --- functional (PyFunctional) and pydash ---------------------------------------------------
per_sku = (
    fseq(lines)
    .map(lambda r: (r["sku"], r["amount"]))
    .reduce_by_key(operator.add)
    .sorted(key=lambda kv: kv[1], reverse=True)
    .to_list()
)
heavy = fseq(orders).filter(lambda o: len(o.items) > 1).map(lambda o: o.id).to_list()
chained = pydash.chain(lines).filter_(lambda r: r["amount"] > 5).map_("sku").uniq().sort().value()
deep = pydash.get(plain, "orders.0.customer")
picked = pydash.pick_by(dict(totals), lambda v: v > 10)

# --- glom: nested specs ---------------------------------------------------------------------
report = glom(
    plain,
    {
        "customers": ("orders", [T["customer"]], set, sorted),
        "first_items": ("orders", [("items", len)]),
        "stock": "stock",
        "missing": Coalesce("discount", default=None),
    },
)

# --- other parsers: lark, pyparsing, funcparserlib --------------------------------------------
QUERY = Lark(
    r"""
    ?start: cond ("and" cond)*
    cond: NAME OP value
    value: NUMBER | ESCAPED_STRING
    OP: ">=" | "<=" | "==" | ">" | "<"
    %import common.CNAME -> NAME
    %import common.NUMBER
    %import common.ESCAPED_STRING
    %import common.WS
    %ignore WS
    """
)


class ToPredicate(Transformer):
    OPS = {">": operator.gt, "<": operator.lt, ">=": operator.ge, "<=": operator.le, "==": operator.eq}

    def value(self, children):
        (tok,) = children
        return Decimal(tok) if tok.type == "NUMBER" else tok[1:-1]

    def cond(self, children):
        name, op, value = children
        return lambda row: self.OPS[str(op)](row[str(name)], value)

    def start(self, conds):
        return lambda row: all(c(row) for c in conds)


query = ToPredicate().transform(QUERY.parse('amount > 5 and customer == "alice"'))
matching = [r for r in lines if query(r)]

integer = pp.Word(pp.nums).set_parse_action(lambda t: int(t[0]))
operand = integer | pp.Word(pp.alphas)
arith = pp.infix_notation(operand, [(pp.one_of("* /"), 2, pp.OpAssoc.LEFT), (pp.one_of("+ -"), 2, pp.OpAssoc.LEFT)])
tree = arith.parse_string("qty * 2 + 3", parse_all=True).as_list()

tokenize = make_tokenizer([TokenSpec("space", r"\s+"), TokenSpec("num", r"\d+"), TokenSpec("op", r"[+\-]")])
tokens = [t for t in tokenize("1 + 2 - 3 + 40") if t.type != "space"]
num = some(lambda t: t.type == "num") >> (lambda t: int(t.value))
op = some(lambda t: t.type == "op") >> (lambda t: operator.add if t.value == "+" else operator.sub)
expr = num + many(op + num) + skip(finished) >> (lambda r: reduce(lambda acc, p: p[0](acc, p[1]), r[1], r[0]))
evaluated = expr.parse(tokens)
optional_sign = maybe(a(tokens[1])).parse(tokens[1:2])

# --- sorted indexes -------------------------------------------------------------------------
by_total = SortedDict({o.total: o.id for o in orders})
cheapest = by_total.peekitem(0)
ranks = SortedList(amounts)
median = ranks[len(ranks) // 2]

# --- a property, checked with hypothesis ----------------------------------------------------
found = []


@settings(max_examples=25, database=None)
@given(st.lists(st.tuples(st.sampled_from(sorted(inventory)), st.integers(min_value=0, max_value=4)), max_size=6))
def reserving_never_goes_negative(wanted):
    stock = reduce(reserve, (Item(sku, qty, Decimal(1)) for sku, qty in wanted), inventory)
    assert all(v >= 0 for v in stock.values())
    found.append(len(wanted))


reserving_never_goes_negative()

summary = {
    "orders": len(orders),
    "errors": errors,
    "totals": {k: str(v) for k, v in rounded.items()},
    "merged": len(merged),
    "top": [s for s, _ in top_skus],
    "gifts": len(tagged.get(True, [])),
    "qty": qty_per_sku,
    "labels": labels,
    "rush": first_rush.id if first_rush else None,
    "pairs": len(pairs),
    "windows": len(windows),
    "split": (len(small), len(large), len(batches)),
    "alice": alice_skus,
    "unique": unique_skus,
    "stock": dict(final_stock),
    "history": len(history),
    "accepted": accepted,
    "vip": maybe_vip,
    "ledger": dict(ledger),
    "oks": [str(x) for x in oks],
    "running": [str(x) for x in running],
    "batched": batched,
    "grouped": sorted(grouped_counts),
    "per_sku": per_sku[:2],
    "heavy": heavy,
    "chained": chained,
    "deep": deep,
    "picked": sorted(picked),
    "report": report,
    "matching": len(matching),
    "tree": tree,
    "evaluated": evaluated,
    "sign": optional_sign is not None,
    "cheapest": cheapest,
    "median": str(median),
    "examples": len(found),
    "partial": partial(operator.mul, 2)(21),
}
print(summary)
