"""A real package: `shop/__init__.py` re-exporting from its modules, a subpackage with its own
`__init__.py`, relative imports of one and two levels, `import a.b.c as x`, `from a.b import
sub`, and a submodule reached as an attribute of its package.  The entry sits next to the
package, as a script run from the project root would.

Expectations were written from the program text before running the engine."""
import shop
from shop import Cart, Item
from shop.pricing import quote, summarise
import shop.util.money as money
from shop.util import cents, CURRENCY

cart = Cart("ann")
cart.add(Item("a", 2.5), 2).add(Item("b", 1.0), 1)
_ = 0                                     #@row cart: present={owner,lines} definite={owner,lines} closed
line0 = cart.lines[0]                     #@row line0: present={item,qty} definite={item,qty} closed
item0 = line0["item"]                     #@row item0: present={sku,price} definite={sku,price} closed
q = quote(cart)                           #@row q: present={owner,lines,total,currency} definite={owner,lines,total,currency} closed
qline = q["lines"][1]                     #@row qline: present={sku,qty,cents} definite={sku,qty,cents} closed
summ = summarise(q)                       #@row summ: present={owner,n,total_str} definite={owner,n,total_str} closed
ver = shop.VERSION                        #@row ver: present={major,minor} definite={major,minor} closed
info = money.INFO                         #@row info: present={unit,symbol} definite={unit,symbol} closed
same_info = shop.util.money.INFO          #@row same_info: present={unit,symbol} definite={unit,symbol} closed
cur = CURRENCY                            #@row cur: present={code,minor} definite={code,minor} closed
other = shop.pricing.empty_cart("bob")    #@row other: present={owner,lines} definite={owner,lines} closed
# The owner is "ann", so the vip discount branch is dead.
q["discount"]                             #@ missing discount
ver["patch"]                              #@ missing patch
shop.pricing.CURRENCY["code"]             #@ ok code
cart.owner                                #@ ok owner
other.lines                               #@ ok lines
