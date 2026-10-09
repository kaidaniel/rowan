# VGG-11 ("configuration A", Simonyan & Zisserman 2015, "Very Deep Convolutional Networks
# for Large-Scale Image Recognition"), written from scratch after torchvision.models.vgg:
# the feature extractor is built by a loop over a config list (3x3 convs with padding 1,
# "M" = 2x2 max pool) into an nn.Sequential, then AdaptiveAvgPool2d((7, 7)), flatten and
# the three-Linear classifier. Channel widths are the paper's divided by 8 and the hidden
# width is a parameter, so the driver runs fast on CPU.
# Checked: five halvings of a symbolic H and W through the whole Sequential (nested floor
# divisions), the channel requirement at its first conv, rank 3 or 4 without BatchNorm and
# rank 4 with it, the adaptive pool on an unknown body (`self.features` an open argument),
# the classifier width; concrete shapes for 32x32, an unbatched 64x48, a 40x40 BN run, and
# one classifier fed with features that skipped the adaptive pool.
import torch
import torch.nn as nn

# VGG-11 at 1/8 width: 64, M, 128, M, 256, 256, M, 512, 512, M, 512, 512, M
VGG11_CFG = [8, "M", 16, "M", 32, 32, "M", 64, 64, "M", 64, 64, "M"]


def make_layers(cfg, batch_norm):
    layers = []
    in_channels = 3
    for v in cfg:
        if v == "M":
            layers += [nn.MaxPool2d(kernel_size=2, stride=2)]
        else:
            conv2d = nn.Conv2d(in_channels, v, kernel_size=3, padding=1)
            if batch_norm:
                layers += [conv2d, nn.BatchNorm2d(v), nn.ReLU(inplace=True)]
            else:
                layers += [conv2d, nn.ReLU(inplace=True)]
            in_channels = v
    return nn.Sequential(*layers)


class VGG(nn.Module):
    def __init__(self, features, num_classes, hidden, last_channels, dropout=0.5):
        super().__init__()
        self.features = features
        self.avgpool = nn.AdaptiveAvgPool2d((7, 7))
        self.classifier = nn.Sequential(
            nn.Linear(last_channels * 7 * 7, hidden),
            nn.ReLU(True),
            nn.Dropout(p=dropout),
            nn.Linear(hidden, hidden),
            nn.ReLU(True),
            nn.Dropout(p=dropout),
            nn.Linear(hidden, num_classes),
        )
        for m in self.modules():
            if isinstance(m, nn.Conv2d):
                nn.init.kaiming_normal_(m.weight, mode="fan_out", nonlinearity="relu")
                if m.bias is not None:
                    nn.init.constant_(m.bias, 0)
            elif isinstance(m, nn.BatchNorm2d):
                nn.init.constant_(m.weight, 1)
                nn.init.constant_(m.bias, 0)
            elif isinstance(m, nn.Linear):
                nn.init.normal_(m.weight, 0, 0.01)
                nn.init.constant_(m.bias, 0)

    def forward(self, x):
        f = self.features(x)  #@shape@L112 f: [2, 64, 1, 1]
        pooled = self.avgpool(f)  #@shape pooled: [*f.shape[:-2], 7, 7]
        #@requires f.dim() in (3, 4)
        #@shape@L112 pooled: [2, 64, 7, 7]
        flat = torch.flatten(pooled, 1)  #@shape@L112 flat: [2, 3136]
        logits = self.classifier(flat)  #@shape logits: [f.shape[0], num_classes]
        #@shape@L112 logits: [2, 10]
        return logits


class VGG11(VGG):
    def __init__(self, num_classes, hidden):
        super().__init__(make_layers(VGG11_CFG, batch_norm=False), num_classes, hidden, VGG11_CFG[-2])

    def feature_map(self, x):
        f = self.features(x)  #@shape f: [*x.shape[:-3], 64, P, Q] where P=(((((x.shape[-2] - 2)//2 - 1)//2 - 1)//2 - 1)//2 - 1)//2 + 1, Q=(((((x.shape[-1] - 2)//2 - 1)//2 - 1)//2 - 1)//2 - 1)//2 + 1
        #@requires x.dim() in (3, 4); x.shape[-3] == 3
        #@shape@L113 f: [64, 2, 1]
        pooled = self.avgpool(f)  #@shape pooled: [*x.shape[:-3], 64, 7, 7]
        #@requires none
        return pooled


class VGG11BN(VGG):
    def __init__(self, num_classes, hidden):
        super().__init__(make_layers(VGG11_CFG, batch_norm=True), num_classes, hidden, VGG11_CFG[-2])

    def feature_map(self, x):
        first = self.features[0](x)  #@shape first: [*x.shape[:-3], 8, x.shape[-2], x.shape[-1]]
        #@requires x.dim() in (3, 4); x.shape[-3] == 3
        f = self.features(x)  #@shape f: [x.shape[0], 64, P, Q] where P=(((((x.shape[2] - 2)//2 - 1)//2 - 1)//2 - 1)//2 - 1)//2 + 1, Q=(((((x.shape[3] - 2)//2 - 1)//2 - 1)//2 - 1)//2 - 1)//2 + 1
        #@requires x.dim() == 4
        #@shape@L116 f: [2, 64, 1, 1]
        pooled = self.avgpool(f)  #@shape pooled: [x.shape[0], 64, 7, 7]
        #@requires none
        flat = torch.flatten(pooled, 1)  #@shape flat: [x.shape[0], 3136]
        return flat


def skipped_avgpool():
    model = VGG11(num_classes=10, hidden=64)
    f = model.features(torch.randn(2, 3, 32, 32))  #@shape f: [2, 64, 1, 1]
    flat = torch.flatten(f, 1)  #@shape flat: [2, 64]
    logits = model.classifier(flat)  #@shape-error
    return logits


if __name__ == "__main__":
    model = VGG11(num_classes=10, hidden=64)
    model.eval()
    logits = model(torch.randn(2, 3, 32, 32))  #@shape logits: [2, 10]
    fm = model.feature_map(torch.randn(3, 64, 48))  #@shape fm: [64, 7, 7]
    bn = VGG11BN(num_classes=10, hidden=64)
    bn.eval()
    flat = bn.feature_map(torch.randn(2, 3, 40, 40))  #@shape flat: [2, 3136]
    out = bn(torch.randn(1, 3, 224, 224))  #@shape out: [1, 10]
    try:
        skipped_avgpool()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass
