"""An instance attribute stored on some paths only, over a class attribute of the same name:
on the other paths the read gives the class's value, and it cannot fail. The read took the
instance's value alone and warned it was possibly missing."""
import sys


class Settings:
    retries = 3


s = Settings()
if len(sys.argv) > 1:
    s.retries = 5
r = s.retries                                    #@ ok retries
#@value r: exactly={3, 5}
