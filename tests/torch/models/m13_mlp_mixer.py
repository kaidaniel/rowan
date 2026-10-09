# MLP-Mixer (Tolstikhin et al., "MLP-Mixer: An all-MLP Architecture for Vision", 2021).
# Patch embedding by a Conv2d with kernel = stride = patch size, then blocks of a token-mixing
# MLP (applied on the transposed [B, C, N] tensor) and a channel-mixing MLP, each behind a
# LayerNorm with a residual, then a LayerNorm, global average pooling over the tokens
# (mean(dim=1)) and a linear head.
# Checked: the MLP on a rest of unknown rank, the block on [B, N, C] after an assert that fixes
# the token count, the conv patch formula, flatten/transpose to tokens, pooling and head, the
# requirements on the layer calls; concrete shapes under the driver; a wrong channel count.
import torch
import torch.nn as nn
import torch.nn.functional as F


class MlpBlock(nn.Module):
    """Linear -> GELU -> Linear on the last dim."""

    def __init__(self, dim, hidden_dim, dropout=0.0):
        super().__init__()
        self.dim = dim
        self.hidden_dim = hidden_dim
        self.fc1 = nn.Linear(dim, hidden_dim)
        self.fc2 = nn.Linear(hidden_dim, dim)
        self.drop = nn.Dropout(dropout)

    def forward(self, x):
        h = self.fc1(x)  #@shape h: [*x.shape[:-1], self.hidden_dim]
        #@requires x.shape[-1] == self.dim
        a = self.drop(F.gelu(h))  #@shape a: [*x.shape[:-1], self.hidden_dim]
        o = self.fc2(a)  #@shape o: [*x.shape[:-1], self.dim]
        #@requires none
        return self.drop(o)


class MixerBlock(nn.Module):
    def __init__(self, num_patches, dim, token_hidden, channel_hidden, dropout=0.0):
        super().__init__()
        self.num_patches = num_patches
        self.dim = dim
        self.norm1 = nn.LayerNorm(dim)
        self.token_mlp = MlpBlock(num_patches, token_hidden, dropout)
        self.norm2 = nn.LayerNorm(dim)
        self.channel_mlp = MlpBlock(dim, channel_hidden, dropout)

    def forward(self, x):
        B, N, C = x.shape  #@dim B: x.shape[0]; N: x.shape[1]; C: x.shape[2]
        #@requires x.dim() == 3
        assert N == self.num_patches, "token count must match the token-mixing MLP"
        y = self.norm1(x)  #@shape y: [x.shape[0], self.num_patches, self.dim]
        #@requires x.shape[2] == self.dim
        #@shape@L111 y: [2, 16, 24]
        yt = y.transpose(1, 2)  #@shape yt: [x.shape[0], self.dim, self.num_patches]
        t = self.token_mlp(yt)  #@shape t: [x.shape[0], self.dim, self.num_patches]
        x1 = x + t.transpose(1, 2)  #@shape x1: [x.shape[0], self.num_patches, self.dim]
        z = self.norm2(x1)  #@shape z: [x.shape[0], self.num_patches, self.dim]
        #@requires none
        c = self.channel_mlp(z)  #@shape c: [x.shape[0], self.num_patches, self.dim]
        out = x1 + c  #@shape out: [x.shape[0], self.num_patches, self.dim]
        #@shape@L111 out: [2, 16, 24]
        return out


class MlpMixer(nn.Module):
    def __init__(self, image_size, patch_size, in_channels, dim, depth, token_hidden, channel_hidden, num_classes, dropout=0.0):
        super().__init__()
        assert image_size % patch_size == 0, "image size must be divisible by the patch size"
        self.patch_size = patch_size
        self.in_channels = in_channels
        self.dim = dim
        self.num_classes = num_classes
        self.num_patches = (image_size // patch_size) ** 2
        self.stem = nn.Conv2d(in_channels, dim, kernel_size=patch_size, stride=patch_size)
        self.blocks = nn.ModuleList(
            [MixerBlock(self.num_patches, dim, token_hidden, channel_hidden, dropout) for _ in range(depth)]
        )
        self.norm = nn.LayerNorm(dim)
        self.head = nn.Linear(dim, num_classes)

    def embed(self, images):
        B, C, H, W = images.shape  #@dim C: images.shape[1]; H: images.shape[2]
        #@requires images.dim() == 4
        p = self.stem(images)  #@shape p: [images.shape[0], self.dim, Hp, Wp] where Hp=(images.shape[2] - self.patch_size)//self.patch_size + 1, Wp=(images.shape[3] - self.patch_size)//self.patch_size + 1
        #@requires images.shape[1] == self.in_channels
        #@shape@L111 p: [2, 24, 4, 4]
        flat = p.flatten(2)  #@shape flat: [images.shape[0], self.dim, Hp*Wp] where Hp=(images.shape[2] - self.patch_size)//self.patch_size + 1, Wp=(images.shape[3] - self.patch_size)//self.patch_size + 1
        tokens = flat.transpose(1, 2)  #@shape tokens: [images.shape[0], Hp*Wp, self.dim] where Hp=(images.shape[2] - self.patch_size)//self.patch_size + 1, Wp=(images.shape[3] - self.patch_size)//self.patch_size + 1
        return tokens

    def forward(self, images):
        x = self.embed(images)  #@shape x: [images.shape[0], Hp*Wp, self.dim] where Hp=(images.shape[2] - self.patch_size)//self.patch_size + 1, Wp=(images.shape[3] - self.patch_size)//self.patch_size + 1
        for blk in self.blocks:
            x = blk(x)
        # the token count joins Hp*Wp (no block ran) with self.num_patches (a block ran): `?`,
        # unless the engine relates them; either is accepted
        h = self.norm(x)  #@shape h: [images.shape[0], _, self.dim]
        #@requires none
        #@shape@L111 h: [2, 16, 24]
        pooled = h.mean(dim=1)  #@shape pooled: [images.shape[0], self.dim]
        logits = self.head(pooled)  #@shape logits: [images.shape[0], self.num_classes]
        #@requires none
        return logits


def wrong_channels(model):
    gray = torch.randn(2, 1, 32, 32)
    return model.stem(gray)  #@shape-error Conv2d(3 -> 24) given 1 channel


if __name__ == "__main__":
    model = MlpMixer(image_size=32, patch_size=8, in_channels=3, dim=24, depth=2, token_hidden=12, channel_hidden=48, num_classes=10)
    images = torch.randn(2, 3, 32, 32)  #@shape images: [2, 3, 32, 32]
    logits = model(images)  #@shape logits: [2, 10]
    tokens = model.embed(images)  #@shape tokens: [2, 16, 24]
    mixed = model.blocks[0](tokens)  #@shape mixed: [2, 16, 24]
    mlp_out = model.blocks[0].channel_mlp(tokens)  #@shape mlp_out: [2, 16, 24]
    probs = F.softmax(logits, dim=-1)  #@shape probs: [2, 10]
    try:
        wrong_channels(model)
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass
