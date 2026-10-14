# Implicit namespace packages (PEP 420): `nsapp/` and `nsapp/inner/` have no __init__.py, and
# Python still imports their modules; rowan analyses them like those of a regular package.
from nsapp.config import load
from nsapp.inner.names import label
import nsapp.config

cfg = load({"debug": True})
#@row cfg: present={"host", "port", "debug"} definite={"host", "port", "debug"} closed
text = label(cfg)
#@value text: exactly={"localhost:8080"}
port = nsapp.config.DEFAULTS["port"]  #@ ok port
#@value port: exactly={8080}
user = cfg["user"]  #@ missing user
