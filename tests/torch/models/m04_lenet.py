# LeNet-5 (LeCun et al. 1998, "Gradient-Based Learning Applied to Document Recognition"),
# written from scratch: C1 5x5 conv 1->6 on a 1x32x32 image, S2 2x2 average subsampling,
# C3 5x5 conv 6->16, S4, flatten 16*5*5, F5 120, F6 84, output; tanh as in the paper. Plus
# the common MNIST variant (padding=2 on 28x28, ReLU, max pooling, nn.Sequential blocks, the
# input reshaped with view(-1, 1, 28, 28)).
# Checked: the conv / pool formulas on symbolic H and W (rank 3 or 4 inside `features`, rank
# 4 after unpacking in `forward`), the flattened width as a product of floor divisions and
# its requirement against fc1's 400, the folded arithmetic of the MNIST variant with an
# unknown batch from view(-1, …); concrete shapes for a batched 32x32 input and an unbatched
# 36x44 one; one 28x28 input whose flattened features do not fit fc1.
import torch
import torch.nn as nn


class LeNet5(nn.Module):
    def __init__(self, num_classes):
        super().__init__()
        self.conv1 = nn.Conv2d(1, 6, kernel_size=5)
        self.pool1 = nn.AvgPool2d(kernel_size=2, stride=2)
        self.conv2 = nn.Conv2d(6, 16, kernel_size=5)
        self.pool2 = nn.AvgPool2d(kernel_size=2, stride=2)
        self.fc1 = nn.Linear(16 * 5 * 5, 120)
        self.fc2 = nn.Linear(120, 84)
        self.fc3 = nn.Linear(84, num_classes)

    def features(self, x):
        c1 = torch.tanh(self.conv1(x))  #@shape c1: [*x.shape[:-3], 6, x.shape[-2] - 4, x.shape[-1] - 4]
        #@requires x.dim() in (3, 4); x.shape[-3] == 1
        #@shape@L106 c1: [6, 32, 40]
        s2 = self.pool1(c1)  #@shape s2: [*x.shape[:-3], 6, (x.shape[-2] - 6)//2 + 1, (x.shape[-1] - 6)//2 + 1]
        #@shape@L106 s2: [6, 16, 20]
        c3 = torch.tanh(self.conv2(s2))
        #@shape c3: [*x.shape[:-3], 16, (x.shape[-2] - 6)//2 - 3, (x.shape[-1] - 6)//2 - 3]
        #@requires none
        #@shape@L106 c3: [16, 12, 16]
        s4 = self.pool2(c3)
        #@shape s4: [*x.shape[:-3], 16, ((x.shape[-2] - 6)//2 - 5)//2 + 1, ((x.shape[-1] - 6)//2 - 5)//2 + 1]
        #@shape@L106 s4: [16, 6, 8]
        #@shape@L105 s4: [4, 16, 5, 5]
        return s4

    def forward(self, x):
        B, C, H, W = x.shape  #@dim B: x.shape[0]; C: x.shape[1]; H: x.shape[2]; W: x.shape[3]
        #@requires x.dim() == 4
        f = self.features(x)  #@shape f: [x.shape[0], 16, H2, W2] where H2=((x.shape[2] - 6)//2 - 5)//2 + 1, W2=((x.shape[3] - 6)//2 - 5)//2 + 1
        flat = torch.flatten(f, 1)  #@shape flat: [x.shape[0], 16*H2*W2] where H2=((x.shape[2] - 6)//2 - 5)//2 + 1, W2=((x.shape[3] - 6)//2 - 5)//2 + 1
        #@shape@L105 flat: [4, 400]
        h5 = torch.tanh(self.fc1(flat))  #@shape h5: [x.shape[0], 120]
        #@requires 16*H2*W2 == 400 where H2=((x.shape[2] - 6)//2 - 5)//2 + 1, W2=((x.shape[3] - 6)//2 - 5)//2 + 1
        h6 = torch.tanh(self.fc2(h5))  #@shape h6: [x.shape[0], 84]
        #@requires none
        out = self.fc3(h6)  #@shape out: [x.shape[0], num_classes]
        #@requires none
        return out


class LeNet5Mnist(nn.Module):
    def __init__(self, num_classes):
        super().__init__()
        self.features = nn.Sequential(
            nn.Conv2d(1, 6, kernel_size=5, padding=2),
            nn.ReLU(),
            nn.MaxPool2d(2),
            nn.Conv2d(6, 16, kernel_size=5),
            nn.ReLU(),
            nn.MaxPool2d(2),
        )
        self.classifier = nn.Sequential(
            nn.Flatten(),
            nn.Linear(16 * 5 * 5, 120),
            nn.ReLU(),
            nn.Linear(120, 84),
            nn.ReLU(),
            nn.Linear(84, num_classes),
        )

    def forward(self, x):
        imgs = x.view(-1, 1, 28, 28)  #@shape imgs: [?, 1, 28, 28]
        f = self.features(imgs)  #@shape f: [?, 16, 5, 5]
        #@requires none
        #@shape@L108 f: [3, 16, 5, 5]
        out = self.classifier(f)  #@shape out: [?, num_classes]
        #@requires none
        return out

    def feature_map(self, x):
        conv = self.features[0]
        c1 = conv(x)  #@shape c1: [*x.shape[:-3], 6, x.shape[-2], x.shape[-1]]
        #@requires x.dim() in (3, 4); x.shape[-3] == 1
        p1 = self.features[2](c1)  #@shape p1: [*x.shape[:-3], 6, (x.shape[-2] - 2)//2 + 1, (x.shape[-1] - 2)//2 + 1]
        return p1


def wrong_resolution():
    model = LeNet5(num_classes=10)
    f = model.features(torch.randn(2, 1, 28, 28))  #@shape f: [2, 16, 4, 4]
    flat = torch.flatten(f, 1)  #@shape flat: [2, 256]
    h = model.fc1(flat)  #@shape-error
    return h


if __name__ == "__main__":
    model = LeNet5(num_classes=10)
    x = torch.randn(4, 1, 32, 32)
    logits = model(x)  #@shape logits: [4, 10]
    feats = model.features(torch.randn(1, 36, 44))  #@shape feats: [16, 6, 8]
    mnist = LeNet5Mnist(num_classes=10)
    out = mnist(torch.randn(3, 784))  #@shape out: [3, 10]
    fm = mnist.feature_map(torch.randn(2, 1, 28, 28))  #@shape fm: [2, 6, 14, 14]
    try:
        wrong_resolution()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass
