# DCGAN (Radford, Metz, Chintala 2016, "Unsupervised Representation Learning with Deep
# Convolutional Generative Adversarial Networks"), as in the PyTorch DCGAN tutorial:
# a Generator of ConvTranspose2d layers from z [N, nz, 1, 1] up to [N, nc, 64, 64]
# (4x4 stride 1 projection, then 4x4 stride 2 padding 1 doubling the size), and a
# Discriminator of strided convs down to [N, 1, 1, 1], flattened with view(-1).
# Checks: the transposed-conv formula on symbolic sizes (`(H-1)*2 - 2 + 3 + 1 = 2H`), the
# conv formula with stride 2 (`(H - 2)//2 + 1` normalises to `H//2`), BatchNorm2d fixing the
# rank, channel widths as multiples of ngf / ndf, view(-1) as the product of the dims,
# requirements of the first layers, one training step in the driver with BCELoss, and a
# conditional label map concatenated at the wrong resolution (a cat mismatch).
import torch
import torch.nn as nn
import torch.nn.functional as F


def weights_init(m):
    # custom weights initialization called on netG and netD (from the tutorial)
    classname = m.__class__.__name__
    if classname.find("Conv") != -1:
        nn.init.normal_(m.weight.data, 0.0, 0.02)
    elif classname.find("BatchNorm") != -1:
        nn.init.normal_(m.weight.data, 1.0, 0.02)
        nn.init.constant_(m.bias.data, 0)


class Generator(nn.Module):
    def __init__(self, nz, ngf, nc):
        super().__init__()
        self.nz = nz
        self.ngf = ngf
        self.nc = nc
        # input is Z, going into a convolution
        self.project = nn.ConvTranspose2d(nz, ngf * 8, 4, 1, 0, bias=False)
        self.bn0 = nn.BatchNorm2d(ngf * 8)
        # state size. (ngf*8) x 4 x 4
        self.up1 = nn.ConvTranspose2d(ngf * 8, ngf * 4, 4, 2, 1, bias=False)
        self.bn1 = nn.BatchNorm2d(ngf * 4)
        # state size. (ngf*4) x 8 x 8
        self.up2 = nn.ConvTranspose2d(ngf * 4, ngf * 2, 4, 2, 1, bias=False)
        self.bn2 = nn.BatchNorm2d(ngf * 2)
        # state size. (ngf*2) x 16 x 16
        self.up3 = nn.ConvTranspose2d(ngf * 2, ngf, 4, 2, 1, bias=False)
        self.bn3 = nn.BatchNorm2d(ngf)
        # state size. ngf x 32 x 32
        self.to_img = nn.ConvTranspose2d(ngf, nc, 4, 2, 1, bias=False)
        # state size. nc x 64 x 64

    def forward(self, z):
        h = self.project(z)  #@shape h: [*z.shape[:-3], 8*self.ngf, z.shape[-2] + 3, z.shape[-1] + 3]
        #@requires z.dim() in (3, 4); z.shape[-3] == self.nz
        #@shape@L124 h: [4, 64, 4, 4]
        h = F.relu(self.bn0(h))  #@shape h: [z.shape[0], 8*self.ngf, z.shape[2] + 3, z.shape[3] + 3]
        h = F.relu(self.bn1(self.up1(h)))  #@shape h: [z.shape[0], 4*self.ngf, 2*z.shape[2] + 6, 2*z.shape[3] + 6]
        #@requires none
        h = F.relu(self.bn2(self.up2(h)))  #@shape h: [z.shape[0], 2*self.ngf, 4*z.shape[2] + 12, 4*z.shape[3] + 12]
        h = F.relu(self.bn3(self.up3(h)))  #@shape h: [z.shape[0], self.ngf, 8*z.shape[2] + 24, 8*z.shape[3] + 24]
        #@shape@L124 h: [4, 8, 32, 32]
        img = torch.tanh(self.to_img(h))  #@shape img: [z.shape[0], self.nc, 16*z.shape[2] + 48, 16*z.shape[3] + 48]
        #@requires none
        return img


class Discriminator(nn.Module):
    def __init__(self, nc, ndf):
        super().__init__()
        self.nc = nc
        self.ndf = ndf
        # input is (nc) x 64 x 64
        self.conv1 = nn.Conv2d(nc, ndf, 4, 2, 1, bias=False)
        # state size. (ndf) x 32 x 32
        self.conv2 = nn.Conv2d(ndf, ndf * 2, 4, 2, 1, bias=False)
        self.bn2 = nn.BatchNorm2d(ndf * 2)
        # state size. (ndf*2) x 16 x 16
        self.conv3 = nn.Conv2d(ndf * 2, ndf * 4, 4, 2, 1, bias=False)
        self.bn3 = nn.BatchNorm2d(ndf * 4)
        # state size. (ndf*4) x 8 x 8
        self.conv4 = nn.Conv2d(ndf * 4, ndf * 8, 4, 2, 1, bias=False)
        self.bn4 = nn.BatchNorm2d(ndf * 8)
        # state size. (ndf*8) x 4 x 4
        self.conv5 = nn.Conv2d(ndf * 8, 1, 4, 1, 0, bias=False)

    def forward(self, x):
        h = F.leaky_relu(self.conv1(x), 0.2)  #@shape h: [*x.shape[:-3], self.ndf, x.shape[-2]//2, x.shape[-1]//2]
        #@requires x.dim() in (3, 4); x.shape[-3] == self.nc
        h = F.leaky_relu(self.bn2(self.conv2(h)), 0.2)  #@shape h: [x.shape[0], 2*self.ndf, x.shape[2]//4, x.shape[3]//4]
        #@requires x.dim() == 4
        h = F.leaky_relu(self.bn3(self.conv3(h)), 0.2)  #@shape h: [x.shape[0], 4*self.ndf, x.shape[2]//8, x.shape[3]//8]
        #@requires none
        h = F.leaky_relu(self.bn4(self.conv4(h)), 0.2)  #@shape h: [x.shape[0], 8*self.ndf, x.shape[2]//16, x.shape[3]//16]
        #@shape@L125 h: [4, 64, 4, 4]
        h = torch.sigmoid(self.conv5(h))  #@shape h: [x.shape[0], 1, x.shape[2]//16 - 3, x.shape[3]//16 - 3]
        out = h.view(-1)  #@shape out: [x.shape[0]*(x.shape[2]//16 - 3)*(x.shape[3]//16 - 3)]
        #@shape@L125 out: [4]
        return out


def conditioned_input(netD, n_classes):
    # conditional-GAN wiring mistake: the one-hot label maps are made at 32x32 while the
    # images are 64x64, so they cannot be stacked onto the image channels
    imgs = torch.randn(2, 3, 64, 64)
    labels = torch.tensor([1, 4])
    onehot = F.one_hot(labels, n_classes).float()  #@shape@L142 onehot: [2, 5]
    maps = onehot[:, :, None, None].expand(-1, -1, 32, 32)  #@shape@L142 maps: [2, 5, 32, 32]
    x = torch.cat([imgs, maps], dim=1)  #@shape-error
    return netD(x)


if __name__ == "__main__":
    nz, ngf, ndf, nc = 16, 8, 8, 3
    netG = Generator(nz, ngf, nc)
    netG.apply(weights_init)
    netD = Discriminator(nc, ndf)
    netD.apply(weights_init)
    criterion = nn.BCELoss()
    optD = torch.optim.Adam(netD.parameters(), lr=2e-4, betas=(0.5, 0.999))
    optG = torch.optim.Adam(netG.parameters(), lr=2e-4, betas=(0.5, 0.999))
    real = torch.rand(4, nc, 64, 64) * 2 - 1
    real_label, fake_label = 1.0, 0.0

    # (1) update D: maximize log(D(x)) + log(1 - D(G(z)))
    netD.zero_grad()
    label = torch.full((4,), real_label)  #@shape label: [4]
    noise = torch.randn(4, nz, 1, 1)
    fake = netG(noise)  #@shape fake: [4, 3, 64, 64]
    pred_real = netD(real)  #@shape pred_real: [4]
    errD_real = criterion(pred_real, label)  #@shape errD_real: []
    errD_real.backward()
    label.fill_(fake_label)
    pred_fake = netD(fake.detach())  #@shape pred_fake: [4]
    errD_fake = criterion(pred_fake, label)  #@shape errD_fake: []
    errD_fake.backward()
    errD = errD_real + errD_fake  #@shape errD: []
    optD.step()
    # (2) update G: maximize log(D(G(z)))
    netG.zero_grad()
    errG = criterion(netD(fake), label.fill_(real_label))  #@shape errG: []
    errG.backward()
    optG.step()
    # a 2x2 noise map gives a 2x larger image: 16*2 + 48 = 80
    big = netG(torch.randn(1, nz, 2, 2))  #@shape big: [1, 3, 80, 80]
    try:
        conditioned_input(Discriminator(nc + 5, ndf), 5)
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass
