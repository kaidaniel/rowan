# Vision Transformer (Dosovitskiy et al. 2021, "An Image is Worth 16x16 Words: Transformers
# for Image Recognition at Scale"), written from scratch in the style of timm's
# VisionTransformer: PatchEmbedding as Conv2d(kernel = stride = patch) then
# flatten(2).transpose(1, 2) (timm asserts the input size), a learned [CLS] token
# (nn.Parameter, expanded over the batch and concatenated in front), a learned positional
# embedding Parameter, pre-norm encoder blocks with nn.MultiheadAttention(batch_first=True)
# and an MLP, and a linear head on the [CLS] token x[:, 0].
# Checks: the conv size formula with kernel = stride = a symbolic patch size
# (`(S - p)//p + 1` normalises to `S//p`), facts from the size assert, the patch count
# `(img_size//patch_size)**2` meeting the positional embedding, expand / cat of the class
# token, MultiheadAttention on a query of unknown rank (2-D or 3-D) and its None weights,
# requirements of the layer calls, concrete shapes in the driver, and a positional embedding
# added to the tokens of a larger image (a broadcast mismatch).
import torch
import torch.nn as nn


class PatchEmbedding(nn.Module):
    def __init__(self, img_size, patch_size, in_ch, embed_dim):
        super().__init__()
        self.img_size = img_size
        self.patch_size = patch_size
        self.in_ch = in_ch
        self.embed_dim = embed_dim
        self.proj = nn.Conv2d(in_ch, embed_dim, kernel_size=patch_size, stride=patch_size)

    def forward(self, x):
        B, C, H, W = x.shape  #@dim H: x.shape[2]
        assert H == self.img_size and W == self.img_size, f"input {H}x{W} is not {self.img_size}"
        #@dim H: self.img_size; W: self.img_size
        h = self.proj(x)
        #@shape h: [x.shape[0], self.embed_dim, self.img_size//self.patch_size, self.img_size//self.patch_size]
        h = h.flatten(2)  #@shape h: [x.shape[0], self.embed_dim, (self.img_size//self.patch_size)**2]
        h = h.transpose(1, 2)  #@shape h: [x.shape[0], (self.img_size//self.patch_size)**2, self.embed_dim]
        return h


class MLP(nn.Module):
    def __init__(self, dim, hidden_dim, dropout):
        super().__init__()
        self.dim = dim
        self.hidden_dim = hidden_dim
        self.fc1 = nn.Linear(dim, hidden_dim)
        self.act = nn.GELU()
        self.fc2 = nn.Linear(hidden_dim, dim)
        self.drop = nn.Dropout(dropout)

    def forward(self, x):
        h = self.drop(self.act(self.fc1(x)))  #@shape h: [*x.shape[:-1], self.hidden_dim]
        #@requires x.shape[-1] == self.dim
        y = self.drop(self.fc2(h))  #@shape y: [*x.shape[:-1], self.dim]
        #@requires none
        return y


class EncoderBlock(nn.Module):
    """Pre-norm transformer encoder block: x + MSA(LN(x)), then x + MLP(LN(x))."""

    def __init__(self, embed_dim, num_heads, mlp_ratio, dropout):
        super().__init__()
        self.embed_dim = embed_dim
        self.norm1 = nn.LayerNorm(embed_dim)
        self.attn = nn.MultiheadAttention(embed_dim, num_heads, dropout=dropout, batch_first=True)
        self.norm2 = nn.LayerNorm(embed_dim)
        self.mlp = MLP(embed_dim, embed_dim * mlp_ratio, dropout)

    def forward(self, x):
        h = self.norm1(x)  #@shape h: [*x.shape[:-1], self.embed_dim]
        #@requires x.shape[-1] == self.embed_dim
        a, w = self.attn(h, h, h, need_weights=False)  #@shape a: [*x.shape[:-1], self.embed_dim]; w: none
        #@requires x.dim() in (2, 3)
        x1 = x + a  #@shape x1: [*x.shape[:-1], self.embed_dim]
        y = x1 + self.mlp(self.norm2(x1))  #@shape y: [*x.shape[:-1], self.embed_dim]
        #@requires none
        return y


class ViT(nn.Module):
    def __init__(self, img_size, patch_size, in_ch, num_classes, embed_dim, depth, num_heads, mlp_ratio, dropout=0.0):
        super().__init__()
        self.img_size = img_size
        self.patch_size = patch_size
        self.in_ch = in_ch
        self.num_classes = num_classes
        self.embed_dim = embed_dim
        self.patch_embed = PatchEmbedding(img_size, patch_size, in_ch, embed_dim)
        num_patches = (img_size // patch_size) ** 2
        self.cls_token = nn.Parameter(torch.zeros(1, 1, embed_dim))
        self.pos_embed = nn.Parameter(torch.randn(1, num_patches + 1, embed_dim) * 0.02)
        self.pos_drop = nn.Dropout(dropout)
        self.blocks = nn.ModuleList([EncoderBlock(embed_dim, num_heads, mlp_ratio, dropout) for _ in range(depth)])
        self.norm = nn.LayerNorm(embed_dim)
        self.head = nn.Linear(embed_dim, num_classes)

    def forward(self, img):
        x = self.patch_embed(img)  #@shape x: [img.shape[0], (self.img_size//self.patch_size)**2, self.embed_dim]
        #@requires img.dim() == 4; img.shape[1] == self.in_ch
        #@shape@L126 x: [2, 16, 64]
        B = x.shape[0]  #@dim B: img.shape[0]
        cls = self.cls_token.expand(B, -1, -1)  #@shape cls: [img.shape[0], 1, self.embed_dim]
        x = torch.cat([cls, x], dim=1)  #@shape x: [img.shape[0], (self.img_size//self.patch_size)**2 + 1, self.embed_dim]
        #@requires none
        x = self.pos_drop(x + self.pos_embed)  #@shape x: [img.shape[0], (self.img_size//self.patch_size)**2 + 1, self.embed_dim]
        #@requires none
        #@shape@L126 x: [2, 17, 64]
        for blk in self.blocks:
            x = blk(x)  #@shape x: [img.shape[0], (self.img_size//self.patch_size)**2 + 1, self.embed_dim]
            #@requires none
        x = self.norm(x)
        cls_out = x[:, 0]  #@shape cls_out: [img.shape[0], self.embed_dim]
        logits = self.head(cls_out)  #@shape logits: [img.shape[0], self.num_classes]
        #@requires none
        return logits


def pos_embed_for_larger_image(model):
    # the patch grid of a 48x48 image is 6x6 = 36 tokens; the embedding was learned for 4x4
    img = torch.randn(1, 3, 48, 48)
    tokens = model.patch_embed.proj(img).flatten(2).transpose(1, 2)  #@shape@L133 tokens: [1, 36, 64]
    return tokens + model.pos_embed[:, 1:]  #@shape-error


if __name__ == "__main__":
    model = ViT(img_size=32, patch_size=8, in_ch=3, num_classes=10, embed_dim=64, depth=2, num_heads=4, mlp_ratio=4)
    img = torch.randn(2, 3, 32, 32)
    logits = model(img)  #@shape logits: [2, 10]
    patches = model.patch_embed(img)  #@shape patches: [2, 16, 64]
    # one block on its own, on an unbatched sequence (MultiheadAttention takes 2-D input too)
    blk = EncoderBlock(embed_dim=16, num_heads=2, mlp_ratio=2, dropout=0.1)
    seq = blk(torch.randn(5, 16))  #@shape seq: [5, 16]
    mlp_out = MLP(16, 48, 0.0)(torch.randn(2, 3, 4, 16))  #@shape mlp_out: [2, 3, 4, 16]
    try:
        pos_embed_for_larger_image(model)
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass
