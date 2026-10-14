# The owner's pytorch-playground `dimensions.py`, unchanged but for this header and the
# directives: `expand` is a view of the same 8 numbers with stride 0 along the new rows.
import torch

y = torch.randn(1, 8)
#@shape y: [1, 8]
ye = y.expand(4, 8)                 # what broadcasting does internally
#@shape ye: [4, 8]
print(ye.shape, ye.stride())        # (4, 8), stride (0, 1) — row step is 0
print(ye.data_ptr() == y.data_ptr())   # True: same 8 numbers
