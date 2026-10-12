# Plain Python numbers in a torch program are Python's: floor division and modulo round toward
# negative infinity (7 // -2 is -4, 7 % -2 is -1; they were -3 and 1), and a modulo by 0.0
# raises ZeroDivisionError (it gave nan). Expectations written from the program text.
import torch


def mod(a, b):
    try:
        return a % b
    except ZeroDivisionError:
        return "zero"


q = 7 // -2                    #@value q: exactly={-4}
r = 7 % -2                     #@value r: exactly={-1}
s = -7 // 2                    #@value s: exactly={-4}
big = 2 ** 62 * 4              #@todo value big: exactly={18446744073709551616} (ints past 64 bits are unknown to rowan)
z = mod(5, 0.0)                #@value z: includes={"zero"}
x = torch.zeros(q * -1, 3)     #@shape x: [4, 3]
