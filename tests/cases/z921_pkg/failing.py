"""For z921_reimport.py: a module whose code raises unless Z921_OK is set."""
import os

X = 1
if not os.environ.get("Z921_OK"):
    raise ImportError("not configured")
Y = 2
