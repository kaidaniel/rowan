# ResNet (He et al. 2016, "Deep Residual Learning for Image Recognition"), written from
# scratch after torchvision.models.resnet: conv3x3 / conv1x1 helpers, BasicBlock
# (expansion 1) and Bottleneck (expansion 4, stride on the 3x3 as in "ResNet v1.5"), a
# downsample shortcut (1x1 conv + BatchNorm) built by _make_layer, a 7x7/2 stem with a 3x3/2
# max pool, four stages, AdaptiveAvgPool2d((1, 1)), flatten, fc.
# Checked: the conv size formula with a symbolic stride (`(H - 1)//self.stride + 1`), the
# rank: Conv2d accepts 3-D or 4-D input, BatchNorm2d then fixes it at 4 (indices become
# non-negative), channel requirements, the stem arithmetic on an unpacked input, the head on
# a body built from an open `block` argument; concrete shapes for a BasicBlock and a
# Bottleneck network and a strided block on an odd input; one grayscale input to the stem.
import torch
import torch.nn as nn


def conv3x3(in_planes, out_planes, stride=1):
    return nn.Conv2d(in_planes, out_planes, kernel_size=3, stride=stride, padding=1, bias=False)


def conv1x1(in_planes, out_planes, stride=1):
    return nn.Conv2d(in_planes, out_planes, kernel_size=1, stride=stride, bias=False)


class BasicBlock(nn.Module):
    expansion = 1

    def __init__(self, inplanes, planes, stride, downsample):
        super().__init__()
        self.conv1 = conv3x3(inplanes, planes, stride)
        self.bn1 = nn.BatchNorm2d(planes)
        self.relu = nn.ReLU(inplace=True)
        self.conv2 = conv3x3(planes, planes)
        self.bn2 = nn.BatchNorm2d(planes)
        self.downsample = downsample
        self.stride = stride

    def forward(self, x):
        identity = x
        out = self.conv1(x)
        #@shape out: [*x.shape[:-3], planes, (x.shape[-2] - 1)//self.stride + 1, (x.shape[-1] - 1)//self.stride + 1]
        #@requires x.dim() in (3, 4); x.shape[-3] == inplanes
        out = self.bn1(out)
        #@shape out: [x.shape[0], planes, (x.shape[2] - 1)//self.stride + 1, (x.shape[3] - 1)//self.stride + 1]
        #@requires x.dim() == 4
        out = self.relu(out)
        out = self.conv2(out)
        #@shape out: [x.shape[0], planes, (x.shape[2] - 1)//self.stride + 1, (x.shape[3] - 1)//self.stride + 1]
        #@requires none
        out = self.bn2(out)
        if self.downsample is not None:
            identity = self.downsample(x)  #@shape@L169 identity: [2, 16, 8, 8]
        out += identity  #@shape@L169 out: [2, 16, 8, 8]
        out = self.relu(out)
        return out


class Bottleneck(nn.Module):
    expansion = 4

    def __init__(self, inplanes, planes, stride, downsample):
        super().__init__()
        self.conv1 = conv1x1(inplanes, planes)
        self.bn1 = nn.BatchNorm2d(planes)
        self.conv2 = conv3x3(planes, planes, stride)
        self.bn2 = nn.BatchNorm2d(planes)
        self.conv3 = conv1x1(planes, planes * self.expansion)
        self.bn3 = nn.BatchNorm2d(planes * self.expansion)
        self.relu = nn.ReLU(inplace=True)
        self.downsample = downsample
        self.stride = stride

    def forward(self, x):
        identity = x
        out = self.conv1(x)  #@shape out: [*x.shape[:-3], planes, x.shape[-2], x.shape[-1]]
        #@requires x.dim() in (3, 4); x.shape[-3] == inplanes
        out = self.bn1(out)  #@shape out: [x.shape[0], planes, x.shape[2], x.shape[3]]
        #@requires x.dim() == 4
        out = self.relu(out)
        out = self.conv2(out)
        #@shape out: [x.shape[0], planes, (x.shape[2] - 1)//self.stride + 1, (x.shape[3] - 1)//self.stride + 1]
        #@requires none
        out = self.relu(self.bn2(out))
        out = self.conv3(out)
        #@shape out: [x.shape[0], 4*planes, (x.shape[2] - 1)//self.stride + 1, (x.shape[3] - 1)//self.stride + 1]
        #@requires none
        out = self.bn3(out)
        if self.downsample is not None:
            identity = self.downsample(x)
        out += identity
        out = self.relu(out)
        return out


class ResNet(nn.Module):
    def __init__(self, block, layers, num_classes, width):
        super().__init__()
        self.inplanes = width
        self.conv1 = nn.Conv2d(3, self.inplanes, kernel_size=7, stride=2, padding=3, bias=False)
        self.bn1 = nn.BatchNorm2d(self.inplanes)
        self.relu = nn.ReLU(inplace=True)
        self.maxpool = nn.MaxPool2d(kernel_size=3, stride=2, padding=1)
        self.layer1 = self._make_layer(block, width, layers[0])
        self.layer2 = self._make_layer(block, 2 * width, layers[1], stride=2)
        self.layer3 = self._make_layer(block, 4 * width, layers[2], stride=2)
        self.layer4 = self._make_layer(block, 8 * width, layers[3], stride=2)
        self.avgpool = nn.AdaptiveAvgPool2d((1, 1))
        self.fc = nn.Linear(8 * width * block.expansion, num_classes)
        for m in self.modules():
            if isinstance(m, nn.Conv2d):
                nn.init.kaiming_normal_(m.weight, mode="fan_out", nonlinearity="relu")
            elif isinstance(m, nn.BatchNorm2d):
                nn.init.constant_(m.weight, 1)
                nn.init.constant_(m.bias, 0)

    def _make_layer(self, block, planes, blocks, stride=1):
        downsample = None
        if stride != 1 or self.inplanes != planes * block.expansion:
            downsample = nn.Sequential(
                conv1x1(self.inplanes, planes * block.expansion, stride),
                nn.BatchNorm2d(planes * block.expansion),
            )
        layers = [block(self.inplanes, planes, stride, downsample)]
        self.inplanes = planes * block.expansion
        for _ in range(1, blocks):
            layers.append(block(self.inplanes, planes, 1, None))
        return nn.Sequential(*layers)

    def forward(self, x):
        N, C, H, W = x.shape  #@dim N: x.shape[0]; C: x.shape[1]; H: x.shape[2]; W: x.shape[3]
        #@requires x.dim() == 4
        h = self.conv1(x)  #@shape h: [x.shape[0], width, (x.shape[2] - 1)//2 + 1, (x.shape[3] - 1)//2 + 1]
        #@requires x.shape[1] == 3
        #@shape@L163 h: [2, 8, 32, 32]
        h = self.relu(self.bn1(h))
        h = self.maxpool(h)  #@shape h: [x.shape[0], width, ((x.shape[2] - 1)//2)//2 + 1, ((x.shape[3] - 1)//2)//2 + 1]
        #@shape@L163 h: [2, 8, 16, 16]
        #@shape@L166 h: [2, 8, 8, 8]
        h1 = self.layer1(h)  #@shape@L163 h1: [2, 8, 16, 16]
        #@shape@L166 h1: [2, 32, 8, 8]
        h2 = self.layer2(h1)  #@shape@L163 h2: [2, 16, 8, 8]
        #@shape@L166 h2: [2, 64, 4, 4]
        h3 = self.layer3(h2)  #@shape@L163 h3: [2, 32, 4, 4]
        h4 = self.layer4(h3)  #@shape@L163 h4: [2, 64, 2, 2]
        #@shape@L166 h4: [2, 256, 1, 1]
        pooled = self.avgpool(h4)  #@shape pooled: [*_, 1, 1]
        #@shape@L163 pooled: [2, 64, 1, 1]
        flat = torch.flatten(pooled, 1)  #@shape@L163 flat: [2, 64]
        #@shape@L166 flat: [2, 256]
        logits = self.fc(flat)  #@shape logits: [_, num_classes]
        #@shape@L163 logits: [2, 10]
        return logits


def grayscale_into_rgb_stem():
    model = ResNet(Bottleneck, [1, 1, 1, 1], num_classes=10, width=8)
    img = torch.randn(2, 1, 32, 32)
    h = model.conv1(img)  #@shape-error
    return h


if __name__ == "__main__":
    basic = ResNet(BasicBlock, [2, 1, 1, 1], num_classes=10, width=8)
    basic.eval()
    y = basic(torch.randn(2, 3, 64, 64))  #@shape y: [2, 10]
    deep = ResNet(Bottleneck, [1, 1, 1, 1], num_classes=10, width=8)
    deep.eval()
    y2 = deep(torch.randn(2, 3, 32, 32))  #@shape y2: [2, 10]
    blk = BasicBlock(8, 16, 2, nn.Sequential(conv1x1(8, 16, 2), nn.BatchNorm2d(16)))
    blk.eval()
    z = blk(torch.randn(2, 8, 15, 15))  #@shape z: [2, 16, 8, 8]
    bottleneck = Bottleneck(16, 4, 1, None)
    z2 = bottleneck(torch.randn(2, 16, 7, 7))  #@shape z2: [2, 16, 7, 7]
    try:
        grayscale_into_rgb_stem()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass
