import helpers
from helpers import DEFAULTS

n = helpers.normalize({"Name": "k", "AGE": 3})  #@row n: present={name,age,normalized} definite={name,age,normalized} closed
n["name"]                                       #@ ok name
n["Name"]                                       #@ missing Name
DEFAULTS["timeout"]                             #@ ok timeout
DEFAULTS["timeouts"]                            #@ missing timeouts
helpers.DEFAULTS["retries"]                     #@ ok retries
