# AlexNet (Krizhevsky, Sutskever & Hinton 2012, "ImageNet Classification with Deep
# Convolutional Neural Networks"), written from scratch after torchvision.models.alexnet
# (the one-GPU variant: 11x11/4 conv with padding 2, 5x5, three 3x3 convs, three 3x3/2 max
# pools, AdaptiveAvgPool2d((6, 6)), dropout + three Linears). The classifier's hidden width
# is a parameter so the driver runs fast on CPU; the conv widths are the paper's.
# Checked: the stride-4 conv and the 3x3/2 pools on symbolic H and W (nested floor
# divisions), step by step through Sequential indexing and slicing on an unpacked 4-D input,
# and through the whole Sequential on an input of unknown rank (3 or 4); requirements on the
# input channels; exact arithmetic for 224x224 (55, 27, 13, 6) and 127x127 (31, 15, 7, 3)
# inputs in the driver; one classifier fed with features that skipped the adaptive pool.
import torch
import torch.nn as nn


class AlexNet(nn.Module):
    def __init__(self, num_classes, hidden, dropout=0.5):
        super().__init__()
        self.features = nn.Sequential(
            nn.Conv2d(3, 64, kernel_size=11, stride=4, padding=2),
            nn.ReLU(inplace=True),
            nn.MaxPool2d(kernel_size=3, stride=2),
            nn.Conv2d(64, 192, kernel_size=5, padding=2),
            nn.ReLU(inplace=True),
            nn.MaxPool2d(kernel_size=3, stride=2),
            nn.Conv2d(192, 384, kernel_size=3, padding=1),
            nn.ReLU(inplace=True),
            nn.Conv2d(384, 256, kernel_size=3, padding=1),
            nn.ReLU(inplace=True),
            nn.Conv2d(256, 256, kernel_size=3, padding=1),
            nn.ReLU(inplace=True),
            nn.MaxPool2d(kernel_size=3, stride=2),
        )
        self.avgpool = nn.AdaptiveAvgPool2d((6, 6))
        self.classifier = nn.Sequential(
            nn.Dropout(p=dropout),
            nn.Linear(256 * 6 * 6, hidden),
            nn.ReLU(inplace=True),
            nn.Dropout(p=dropout),
            nn.Linear(hidden, hidden),
            nn.ReLU(inplace=True),
            nn.Linear(hidden, num_classes),
        )

    def forward(self, x):
        f = self.features(x)
        #@shape f: [*x.shape[:-3], 256, P, Q] where P=((((x.shape[-2] - 7)//4 - 2)//2 - 2)//2 - 2)//2 + 1, Q=((((x.shape[-1] - 7)//4 - 2)//2 - 2)//2 - 2)//2 + 1
        #@requires x.dim() in (3, 4); x.shape[-3] == 3
        #@shape@L101 f: [2, 256, 6, 6]
        #@shape@L102 f: [2, 256, 3, 3]
        pooled = self.avgpool(f)  #@shape pooled: [*x.shape[:-3], 256, 6, 6]
        #@requires none
        #@shape@L102 pooled: [2, 256, 6, 6]
        flat = torch.flatten(pooled, 1)  #@shape@L101 flat: [2, 9216]
        logits = self.classifier(flat)  #@shape logits: [_, num_classes]
        #@shape@L101 logits: [2, 10]
        return logits

    def forward_explicit(self, x):
        N, C, H, W = x.shape  #@dim N: x.shape[0]; C: x.shape[1]; H: x.shape[2]; W: x.shape[3]
        #@requires x.dim() == 4
        c1 = self.features[0](x)  #@shape c1: [x.shape[0], 64, (x.shape[2] - 7)//4 + 1, (x.shape[3] - 7)//4 + 1]
        #@requires x.shape[1] == 3
        #@shape@L103 c1: [2, 64, 55, 55]
        p1 = self.features[2](self.features[1](c1))
        #@shape p1: [x.shape[0], 64, ((x.shape[2] - 7)//4 - 2)//2 + 1, ((x.shape[3] - 7)//4 - 2)//2 + 1]
        #@requires none
        #@shape@L103 p1: [2, 64, 27, 27]
        c2 = self.features[3](p1)
        #@shape c2: [x.shape[0], 192, ((x.shape[2] - 7)//4 - 2)//2 + 1, ((x.shape[3] - 7)//4 - 2)//2 + 1]
        #@requires none
        #@shape@L103 c2: [2, 192, 27, 27]
        p2 = self.features[5](self.features[4](c2))
        #@shape p2: [x.shape[0], 192, (((x.shape[2] - 7)//4 - 2)//2 - 2)//2 + 1, (((x.shape[3] - 7)//4 - 2)//2 - 2)//2 + 1]
        #@shape@L103 p2: [2, 192, 13, 13]
        c5 = self.features[6:12](p2)
        #@shape c5: [x.shape[0], 256, (((x.shape[2] - 7)//4 - 2)//2 - 2)//2 + 1, (((x.shape[3] - 7)//4 - 2)//2 - 2)//2 + 1]
        #@requires none
        #@shape@L103 c5: [2, 256, 13, 13]
        p3 = self.features[12](c5)
        #@shape p3: [x.shape[0], 256, P, Q] where P=((((x.shape[2] - 7)//4 - 2)//2 - 2)//2 - 2)//2 + 1, Q=((((x.shape[3] - 7)//4 - 2)//2 - 2)//2 - 2)//2 + 1
        #@shape@L103 p3: [2, 256, 6, 6]
        pooled = self.avgpool(p3)  #@shape pooled: [x.shape[0], 256, 6, 6]
        flat = torch.flatten(pooled, 1)  #@shape flat: [x.shape[0], 9216]
        logits = self.classifier(flat)  #@shape logits: [x.shape[0], num_classes]
        #@requires none
        return logits


def skipped_avgpool():
    model = AlexNet(num_classes=10, hidden=128)
    f = model.features(torch.randn(2, 3, 127, 127))  #@shape f: [2, 256, 3, 3]
    flat = torch.flatten(f, 1)  #@shape flat: [2, 2304]
    logits = model.classifier(flat)  #@shape-error
    return logits


if __name__ == "__main__":
    model = AlexNet(num_classes=10, hidden=128)
    model.eval()
    x = torch.randn(2, 3, 224, 224)
    y = model(x)  #@shape y: [2, 10]
    small = model(torch.randn(2, 3, 127, 127))  #@shape small: [2, 10]
    y2 = model.forward_explicit(x)  #@shape y2: [2, 10]
    single = model.features(torch.randn(3, 224, 224))  #@shape single: [256, 6, 6]
    try:
        skipped_avgpool()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass
