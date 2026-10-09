"""Cross-module flows: classes defined in one module, instantiated in another, mutated
through a third; a module-level registry written by a constructor; a constant used as a
key; subclassing across modules; `import x`, `import x as y`, `from x import y`.

Expectations were written from the program text before running the engine.  Nine of them
needed summaries specialised by the classes of the arguments (`Store.add` is summarised once
per class of `item`, so `item.label()` resolves to that class's method), the generated
`__init__` of a dataclass base running for subclasses (`Bundle`, `Special`), and
`self.by_kind = defaultdict(list)` giving the field a builtin class inside summaries."""
import service
import store
from models import Item, KIND_KEY

s = service.seed("main")
_ = 0                                     #@row s: present={name,items,by_kind,log} definite={name,items,by_kind,log} closed
items = s.items                           #@row items: present={a,b,c} definite={a,b,c} closed
a = items["a"]                            #@row a: present={sku,price,tags} definite={sku,price,tags} closed
b = items["b"]                            #@row b: present={sku,price,tags,items} definite={sku,price,tags,items} closed
c = items["c"]                            #@row c: present={sku,price,tags} definite={sku,price,tags} closed
by_kind = s.by_kind                       #@row by_kind: present={item,bundle} definite={item,bundle} closed
log0 = s.log[0]                           #@row log0: present={op,sku,kind} definite={op,sku,kind} closed
rep = service.report("main")              #@row rep: present={store,labels,first_log} definite={store,labels,first_log} closed
summ = rep["store"]                       #@row summ: present={name,n,kinds} definite={name,n,kinds} closed
labels = rep["labels"]                    #@row labels: present={a,b,c} definite={a,b,c} closed
lab_b = labels["b"]                       #@row lab_b: present={sku,kind,count} definite={sku,kind,count} closed
lab_c = labels["c"]                       #@row lab_c: present={sku,kind,special} definite={sku,kind,special} closed
registry = store.STORES                   #@row registry: present={main} definite={main} closed
same = service.open_store("main")         #@row same: present={name,items,by_kind,log} definite={name,items,by_kind,log} closed
other = store.open_store("other")         #@row other: present={name,items,by_kind,log} definite={name,items,by_kind,log} closed
_ = 0                                     #@row registry: present={main,other} definite={main,other} closed
direct = Item("d", 5.0).label()           #@row direct: present={sku,kind} definite={sku,kind} closed
direct[KIND_KEY]                          #@ ok kind
labels["a"]["count"]                      #@ missing count
lab_b["special"]                          #@ missing special
