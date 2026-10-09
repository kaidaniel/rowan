# U-Net (Ronneberger, Fischer, Brox 2015, "U-Net: Convolutional Networks for Biomedical
# Image Segmentation"), laid out like the widely used milesial/Pytorch-UNet: DoubleConv
# (conv-BN-ReLU twice), Down (MaxPool2d(2) then DoubleConv), Up (ConvTranspose2d(2, stride 2)
# halving the channels, the skip connection padded to size and concatenated along the
# channels, DoubleConv), OutConv (1x1 conv). padding=1 keeps H and W through every conv.
# Checks: the conv / pool / transposed-conv size formulas on symbolic dims (`(H - 2)//2 + 1`
# normalises to `H//2`, nested floors `H//2//2` to `H//4`), BatchNorm2d fixing the rank to 4,
# the F.pad arithmetic that makes the upsampled map exactly the skip's size, channel
# bookkeeping through torch.cat, requirements of the layer calls, concrete shapes in the
# driver, and a skip connection taken from the wrong level (a cat mismatch).
import torch
import torch.nn as nn
import torch.nn.functional as F


class DoubleConv(nn.Module):
    """(conv 3x3, padding 1 => BN => ReLU) * 2"""

    def __init__(self, in_ch, out_ch):
        super().__init__()
        self.in_ch = in_ch
        self.out_ch = out_ch
        self.conv1 = nn.Conv2d(in_ch, out_ch, kernel_size=3, padding=1, bias=False)
        self.bn1 = nn.BatchNorm2d(out_ch)
        self.conv2 = nn.Conv2d(out_ch, out_ch, kernel_size=3, padding=1, bias=False)
        self.bn2 = nn.BatchNorm2d(out_ch)
        self.relu = nn.ReLU(inplace=True)

    def forward(self, x):
        h = self.conv1(x)  #@shape h: [*x.shape[:-3], self.out_ch, x.shape[-2], x.shape[-1]]
        # BatchNorm2d takes 4-D input only: from here on x is known to be NCHW
        h = self.relu(self.bn1(h))  #@shape h: [x.shape[0], self.out_ch, x.shape[2], x.shape[3]]
        h = self.relu(self.bn2(self.conv2(h)))  #@shape h: [x.shape[0], self.out_ch, x.shape[2], x.shape[3]]
        return h


class Down(nn.Module):
    """Downscaling with maxpool then double conv"""

    def __init__(self, in_ch, out_ch):
        super().__init__()
        self.out_ch = out_ch
        self.pool = nn.MaxPool2d(2)
        self.conv = DoubleConv(in_ch, out_ch)

    def forward(self, x):
        p = self.pool(x)  #@shape p: [*x.shape[:-2], x.shape[-2]//2, x.shape[-1]//2]
        #@requires x.dim() in (3, 4)
        y = self.conv(p)  #@shape y: [x.shape[0], self.out_ch, x.shape[2]//2, x.shape[3]//2]
        return y


class Up(nn.Module):
    """Upscaling (transposed conv halving the channels), pad, concat the skip, double conv"""

    def __init__(self, in_ch, out_ch):
        super().__init__()
        self.in_ch = in_ch
        self.out_ch = out_ch
        self.up = nn.ConvTranspose2d(in_ch, in_ch // 2, kernel_size=2, stride=2)
        self.conv = DoubleConv(in_ch, out_ch)

    def forward(self, x1, x2):
        # x1: the coarser map from below, x2: the skip connection from the contracting path
        assert x1.dim() == 4 and x2.dim() == 4, "Up expects NCHW maps"
        u = self.up(x1)  #@shape u: [x1.shape[0], self.in_ch // 2, 2*x1.shape[2], 2*x1.shape[3]]
        #@requires x1.shape[1] == self.in_ch
        # odd sizes on the way down leave u one pixel short: pad it to the skip's size
        diff_y = x2.size(2) - u.size(2)  #@dim diff_y: x2.shape[2] - 2*x1.shape[2]
        diff_x = x2.size(3) - u.size(3)  #@dim diff_x: x2.shape[3] - 2*x1.shape[3]
        u = F.pad(u, [diff_x // 2, diff_x - diff_x // 2, diff_y // 2, diff_y - diff_y // 2])
        #@shape u: [x1.shape[0], self.in_ch // 2, x2.shape[2], x2.shape[3]]
        #@requires none
        x = torch.cat([x2, u], dim=1)  #@shape x: [x1.shape[0], x2.shape[1] + self.in_ch // 2, x2.shape[2], x2.shape[3]]
        #@requires x1.shape[0] == x2.shape[0]
        y = self.conv(x)  #@shape y: [x1.shape[0], self.out_ch, x2.shape[2], x2.shape[3]]
        return y


class OutConv(nn.Module):
    def __init__(self, in_ch, out_ch):
        super().__init__()
        self.in_ch = in_ch
        self.out_ch = out_ch
        self.conv = nn.Conv2d(in_ch, out_ch, kernel_size=1)

    def forward(self, x):
        y = self.conv(x)  #@shape y: [*x.shape[:-3], self.out_ch, x.shape[-2], x.shape[-1]]
        #@requires x.dim() in (3, 4); x.shape[-3] == self.in_ch
        return y


class UNet(nn.Module):
    def __init__(self, n_channels, n_classes):
        super().__init__()
        self.n_channels = n_channels
        self.n_classes = n_classes
        self.inc = DoubleConv(n_channels, 64)
        self.down1 = Down(64, 128)
        self.down2 = Down(128, 256)
        self.down3 = Down(256, 512)
        self.down4 = Down(512, 1024)
        self.up1 = Up(1024, 512)
        self.up2 = Up(512, 256)
        self.up3 = Up(256, 128)
        self.up4 = Up(128, 64)
        self.outc = OutConv(64, n_classes)

    def forward(self, x):
        B, C, H, W = x.shape  #@dim B: x.shape[0]; H: x.shape[2]; W: x.shape[3]
        #@requires x.dim() == 4
        x1 = self.inc(x)  #@shape x1: [x.shape[0], 64, x.shape[2], x.shape[3]]
        #@requires x.shape[1] == self.n_channels
        x2 = self.down1(x1)  #@shape x2: [x.shape[0], 128, x.shape[2]//2, x.shape[3]//2]
        #@requires none
        x3 = self.down2(x2)  #@shape x3: [x.shape[0], 256, x.shape[2]//4, x.shape[3]//4]
        x4 = self.down3(x3)  #@shape x4: [x.shape[0], 512, x.shape[2]//8, x.shape[3]//8]
        x5 = self.down4(x4)  #@shape x5: [x.shape[0], 1024, x.shape[2]//16, x.shape[3]//16]
        #@shape@L144 x5: [2, 1024, 2, 2]
        y = self.up1(x5, x4)  #@shape y: [x.shape[0], 512, x.shape[2]//8, x.shape[3]//8]
        #@requires none
        y = self.up2(y, x3)  #@shape y: [x.shape[0], 256, x.shape[2]//4, x.shape[3]//4]
        y = self.up3(y, x2)  #@shape y: [x.shape[0], 128, x.shape[2]//2, x.shape[3]//2]
        y = self.up4(y, x1)  #@shape y: [x.shape[0], 64, x.shape[2], x.shape[3]]
        #@shape@L144 y: [2, 64, 32, 32]
        logits = self.outc(y)  #@shape logits: [x.shape[0], self.n_classes, x.shape[2], x.shape[3]]
        #@requires none
        return logits


def wrong_skip(x):
    # wiring mistake: the 2x-upsampled map of level 2 is joined with level 2 itself
    # instead of level 1, so the spatial sizes differ by a factor of two
    model = UNet(3, 2)
    x1 = model.inc(x)  #@shape@L151 x1: [1, 64, 16, 16]
    x2 = model.down1(x1)  #@shape@L151 x2: [1, 128, 8, 8]
    u = model.up4.up(x2)  #@shape@L151 u: [1, 64, 16, 16]
    return torch.cat([x2, u], dim=1)  #@shape-error


if __name__ == "__main__":
    model = UNet(n_channels=3, n_classes=2)
    img = torch.randn(2, 3, 32, 32)
    out = model(img)  #@shape out: [2, 2, 32, 32]
    # a block on its own, with an odd-sized skip connection (the pad makes up the pixel)
    up = Up(16, 8)
    z = up(torch.randn(1, 16, 3, 3), torch.randn(1, 8, 7, 7))  #@shape z: [1, 8, 7, 7]
    d = Down(8, 16)(torch.randn(1, 8, 9, 9))  #@shape d: [1, 16, 4, 4]
    o = OutConv(8, 5)(torch.randn(8, 6, 6))  #@shape o: [5, 6, 6]
    try:
        wrong_skip(torch.randn(1, 3, 16, 16))
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass
