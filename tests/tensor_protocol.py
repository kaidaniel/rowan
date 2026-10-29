"""The tensor-shape models (`models/torch.rs`, `shape.rs`: PyTorch and numpy), checked
exhaustively against real torch and numpy in the style of `lib_protocol.py`:

- an input tensor or array (`INPUTS` per library): literal shapes (a matrix, a vector, three
  dims, a dim of one, an empty dim), one of two joined by a branch, and a dim of unknown size
  (an environment variable's int: its run's value fixed);
- one operation or a short chain of them on it (`OPS`): elementwise with itself and with a
  broadcast operand, matmul, transposes, `view` / `reshape` (also with sizes that do not fit),
  reductions with and without `keepdim`, `unsqueeze` / `squeeze`, `cat` / `stack`, indexing,
  `flatten`, `expand`, a Linear layer, `size` / `dim` / `numel` / `len`, softmax, `argmax`,
  `einsum`, `chunk`, `repeat`;
- in a `try` naming the class of what it raises (a shape mismatch: RuntimeError or
  ValueError, `Exception!` here).

Observation point: the result's shape as a tuple of ints (`ENC`), a scalar, or the class it
raised.

    uv run --no-project --with torch --with numpy python tests/tensor_protocol.py [--rowan N | --all] [--seed S] [-v]
"""
from __future__ import annotations

import itertools

from lib_protocol import ENC_HELPERS, TAIL_ENC, program
from protocol_lib import cli

NAMES = ["O"]

HELPERS = [
    "def SH(t):", "    return tuple(int(d) for d in t.shape)",
    *ENC_HELPERS,
]

LIBS = {
    "torch": {
        "imports": ["import torch", "import torch.nn.functional as F", "from torch import nn"],
        "inputs": {
            "mat": "torch.zeros(2, 3)", "vec": "torch.ones(4)", "cube": "torch.zeros(2, 3, 4)", "row": "torch.zeros(1, 3)",
            "empty": "torch.zeros(0, 3)", "joined": "torch.zeros(2, 3) if c1 else torch.zeros(3, 3)",
            "unknown": "torch.zeros(int(os.environ.get('RT{N}', '5')), 3)",
        },
        "ops": {
            "add": "SH(v + v)", "bcast": "SH(v + torch.ones(3))", "bcast_bad": "SH(v + torch.ones(5))", "matmul": "SH(v @ torch.ones(3, 5))",
            "matmul_self": "SH(v @ v)", "T": "SH(v.T)", "view_flat": "SH(v.view(-1))", "view3": "SH(v.view(3, -1))", "view_bad": "SH(v.view(4, 4))",
            "reshape": "SH(v.reshape(-1, 1))", "sum0": "SH(v.sum(0))", "sum_keep": "SH(v.sum(dim=-1, keepdim=True))", "mean": "SH(v.mean(-1))",
            "unsq": "SH(v.unsqueeze(0))", "unsq_end": "SH(v.unsqueeze(-1))", "squeeze": "SH(v.squeeze())", "squeeze0": "SH(v.squeeze(0))",
            "transpose": "SH(v.transpose(0, 1))", "permute": "SH(v.permute(1, 0))", "cat0": "SH(torch.cat([v, v], 0))", "cat1": "SH(torch.cat([v, v], 1))",
            "stack": "SH(torch.stack([v, v]))", "idx0": "SH(v[0])", "col": "SH(v[:, 0])", "none_idx": "SH(v[..., None])", "flatten": "SH(v.flatten())",
            "expand": "SH(v.expand(2, -1, -1))", "linear": "SH(nn.Linear(3, 4)(v))", "size0": "v.size(0)", "dim": "v.dim()", "numel": "v.numel()",
            "len": "len(v)", "last": "v.shape[-1]", "softmax": "SH(F.softmax(v, dim=-1))", "argmax": "SH(v.argmax(1))",
            "einsum": "SH(torch.einsum('ij,jk->ik', v, torch.ones(3, 2)))", "chunk": "len(v.chunk(2, 0))", "repeat": "SH(v.repeat(2, 1))",
            "max_vals": "SH(v.max(dim=0).values)", "slice": "SH(v[1:])", "relu": "SH(F.relu(v))", "zeros_like": "SH(torch.zeros_like(v))",
            "bmm": "SH(torch.bmm(v, v.transpose(1, 2)))", "masked": "SH(v[v > 0])", "where": "SH(torch.where(v > 0, v, torch.zeros(3)))",
        },
        "helpers": HELPERS,
    },
    "numpy": {
        "imports": ["import numpy as np"],
        "inputs": {
            "mat": "np.zeros((2, 3))", "vec": "np.ones(4)", "cube": "np.zeros((2, 3, 4))", "row": "np.zeros((1, 3))",
            "empty": "np.zeros((0, 3))", "joined": "np.zeros((2, 3)) if c1 else np.zeros((3, 3))",
            "unknown": "np.zeros((int(os.environ.get('RN{N}', '5')), 3))",
        },
        "ops": {
            "add": "SH(v + v)", "bcast": "SH(v + np.ones(3))", "bcast_bad": "SH(v + np.ones(5))", "matmul": "SH(v @ np.ones((3, 5)))",
            "matmul_self": "SH(v @ v)", "T": "SH(v.T)", "reshape_flat": "SH(v.reshape(-1))", "reshape3": "SH(v.reshape(3, -1))",
            "reshape_bad": "SH(v.reshape(4, 4))", "sum0": "SH(v.sum(axis=0))", "sum_keep": "SH(v.sum(axis=-1, keepdims=True))",
            "mean": "SH(v.mean(-1))", "expand_dims": "SH(np.expand_dims(v, 0))", "squeeze": "SH(v.squeeze())", "transpose": "SH(v.transpose())",
            "concat0": "SH(np.concatenate([v, v], axis=0))", "concat1": "SH(np.concatenate([v, v], axis=1))", "stack": "SH(np.stack([v, v]))",
            "idx0": "SH(v[0])", "col": "SH(v[:, 0])", "none_idx": "SH(v[..., None])", "flatten": "SH(v.flatten())", "ndim": "v.ndim",
            "size": "v.size", "len": "len(v)", "last": "v.shape[-1]", "argmax": "SH(v.argmax(axis=1))", "dot": "SH(np.dot(v, np.ones(3)))",
            "einsum": "SH(np.einsum('ij,jk->ik', v, np.ones((3, 2))))", "tile": "SH(np.tile(v, (2, 1)))", "slice": "SH(v[1:])",
            "where": "SH(np.where(v > 0, v, 0))", "zeros_like": "SH(np.zeros_like(v))", "vstack": "SH(np.vstack([v, v]))", "ravel": "SH(v.ravel())",
        },
        "helpers": HELPERS,
    },
}


def programs():
    for name, lib in LIBS.items():
        for (ik, iv), (ok, ov) in itertools.product(lib["inputs"].items(), lib["ops"].items()):
            p = program(lib, iv, ov, f"{name} {ik} {ok}", TAIL_ENC)
            # (rowan's tensors are numpy's while torch is not imported: a batch of one library)
            p.group = name
            yield p


if __name__ == "__main__":
    cli(programs, NAMES)
