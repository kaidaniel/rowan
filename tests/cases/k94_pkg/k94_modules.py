"""Cross-module flows: classes defined in one module, instantiated in another, mutated
through a third; a module-level registry written by a constructor; a constant used as a
key; subclassing across modules; `import x`, `import x as y`, `from x import y`.

Expectations were written from the program text before running the engine.  The module
boundaries themselves work (imports in three styles, a registry written by a constructor in
another module, a constant from a third module used as a key, subclassing across modules);
the `#@todo` checks fail for the reason documented for `j93`: `item` inside `Store.add` has no
class, so `item.label()` dispatches by name across Item / Bundle / Special and the three adds
join (`ROWAN_ON=sigsum`, summaries specialised by argument classes, is the experimental fix)."""
import service
import store
from models import Item, KIND_KEY

s = service.seed("main")
_ = 0                                     #@row s: present={name,items,by_kind,log} definite={name,items,by_kind,log} closed
items = s.items                           #@todo row items: present={a,b,c} definite={a,b,c} closed
a = items["a"]                            #@todo row a: present={sku,price,tags} definite={sku,price,tags} closed
b = items["b"]                            #@todo row b: present={sku,price,tags,items} definite={sku,price,tags,items} closed
c = items["c"]                            #@todo row c: present={sku,price,tags} definite={sku,price,tags} closed
by_kind = s.by_kind                       #@row by_kind: present={item,bundle} definite={item,bundle} closed
log0 = s.log[0]                           #@row log0: present={op,sku,kind} definite={op,sku,kind} closed
rep = service.report("main")              #@row rep: present={store,labels,first_log} definite={store,labels,first_log} closed
summ = rep["store"]                       #@row summ: present={name,n,kinds} definite={name,n,kinds} closed
labels = rep["labels"]                    #@todo row labels: present={a,b,c} definite={a,b,c} closed
lab_b = labels["b"]                       #@todo row lab_b: present={sku,kind,count} definite={sku,kind,count} closed
lab_c = labels["c"]                       #@todo row lab_c: present={sku,kind,special} definite={sku,kind,special} closed
registry = store.STORES                   #@row registry: present={main} definite={main} closed
same = service.open_store("main")         #@row same: present={name,items,by_kind,log} definite={name,items,by_kind,log} closed
other = store.open_store("other")         #@row other: present={name,items,by_kind,log} definite={name,items,by_kind,log} closed
_ = 0                                     #@row registry: present={main,other} definite={main,other} closed
direct = Item("d", 5.0).label()           #@row direct: present={sku,kind} definite={sku,kind} closed
direct[KIND_KEY]                          #@ ok kind
labels["a"]["count"]                      #@todo missing count
lab_b["special"]                          #@todo missing special
