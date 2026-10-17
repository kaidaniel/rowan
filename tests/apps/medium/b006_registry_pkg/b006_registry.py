"""The main file asks the registry for one model by name: `arch("m007.tag")` is "m007" (the
helper's tuple, made in more contexts by the imported module's registrations than a site keeps
apart, was the summary of all of them: any name, so any of the 260 models). A call the main
file makes keeps its own objects past that bound, up to a few more per site.

Expectations were written from the program text before running the engine."""
from reg.models import REGISTRY, arch

a = arch("m007.tag")
#@value a: exactly={"m007"}
f = REGISTRY[a]
r = f()
#@value r: exactly={7}
