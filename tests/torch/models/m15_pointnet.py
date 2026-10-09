# PointNet classification (Qi et al., "PointNet: Deep Learning on Point Sets for 3D
# Classification and Segmentation", CVPR 2017), after the widely used PyTorch port.
# A T-Net predicts a k x k transform from points [B, k, N] (shared MLP as Conv1d(…, 1),
# max over points with torch.max(x, 2)[0], FC layers, identity added via
# torch.eye(k).flatten() then view(-1, k, k)); the encoder aligns the input points (3x3) and the
# per-point features (64x64) with torch.bmm, and the classifier maps the 1024-d global feature
# to class log-probabilities. The feature-transform regulariser ||A A^T - I||.
# Checked: Conv1d / BatchNorm1d / Linear chains on [B, C, N] after rank facts, max over points,
# the identity trick (flatten, view, repeat, symbolic -1), bmm alignment, the regulariser;
# concrete shapes under the driver; a 4-channel cloud into the 3-channel T-Net.
import torch
import torch.nn as nn
import torch.nn.functional as F


class TNet(nn.Module):
    """Predicts a k x k alignment matrix from a point set [B, k, N]."""

    def __init__(self, k):
        super().__init__()
        self.k = k
        self.conv1 = nn.Conv1d(k, 64, 1)
        self.conv2 = nn.Conv1d(64, 128, 1)
        self.conv3 = nn.Conv1d(128, 1024, 1)
        self.fc1 = nn.Linear(1024, 512)
        self.fc2 = nn.Linear(512, 256)
        self.fc3 = nn.Linear(256, k * k)
        self.bn1 = nn.BatchNorm1d(64)
        self.bn2 = nn.BatchNorm1d(128)
        self.bn3 = nn.BatchNorm1d(1024)
        self.bn4 = nn.BatchNorm1d(512)
        self.bn5 = nn.BatchNorm1d(256)

    def forward(self, x):
        B, D, N = x.size()  #@dim B: x.shape[0]; D: x.shape[1]; N: x.shape[2]
        #@requires x.dim() == 3
        h1 = F.relu(self.bn1(self.conv1(x)))  #@shape h1: [x.shape[0], 64, x.shape[2]]
        #@requires x.shape[1] == self.k
        #@shape@L147 h1: [4, 64, 32]
        h2 = F.relu(self.bn2(self.conv2(h1)))  #@shape h2: [x.shape[0], 128, x.shape[2]]
        #@requires none
        h3 = F.relu(self.bn3(self.conv3(h2)))  #@shape h3: [x.shape[0], 1024, x.shape[2]]
        g = torch.max(h3, 2)[0]  #@shape g: [x.shape[0], 1024]
        #@shape@L147 g: [4, 1024]
        g = g.view(-1, 1024)  #@shape g: [x.shape[0], 1024]
        f1 = F.relu(self.bn4(self.fc1(g)))  #@shape f1: [x.shape[0], 512]
        #@requires none
        f2 = F.relu(self.bn5(self.fc2(f1)))  #@shape f2: [x.shape[0], 256]
        m = self.fc3(f2)  #@shape m: [x.shape[0], self.k * self.k]
        iden = torch.eye(self.k, dtype=m.dtype).flatten()  #@shape iden: [self.k * self.k]
        iden = iden.view(1, self.k * self.k).repeat(B, 1)  #@shape iden: [x.shape[0], self.k * self.k]
        t = (m + iden).view(-1, self.k, self.k)  #@shape t: [x.shape[0], self.k, self.k]
        return t


class PointNetEncoder(nn.Module):
    def __init__(self, global_feat=True, feature_transform=True, channel=3):
        super().__init__()
        self.channel = channel
        self.stn = TNet(channel)
        self.conv1 = nn.Conv1d(channel, 64, 1)
        self.conv2 = nn.Conv1d(64, 128, 1)
        self.conv3 = nn.Conv1d(128, 1024, 1)
        self.bn1 = nn.BatchNorm1d(64)
        self.bn2 = nn.BatchNorm1d(128)
        self.bn3 = nn.BatchNorm1d(1024)
        self.global_feat = global_feat
        self.feature_transform = feature_transform
        if self.feature_transform:
            self.fstn = TNet(64)

    def forward(self, x):
        B, D, N = x.size()
        #@requires x.dim() == 3
        if D != self.channel:
            raise ValueError(f"expected {self.channel}-d points, got {D}")
        trans = self.stn(x)  #@shape trans: [x.shape[0], self.channel, self.channel]
        pts = x.transpose(2, 1)  #@shape pts: [x.shape[0], x.shape[2], self.channel]
        aligned = torch.bmm(pts, trans)  #@shape aligned: [x.shape[0], x.shape[2], self.channel]
        #@requires none
        aligned = aligned.transpose(2, 1)  #@shape aligned: [x.shape[0], self.channel, x.shape[2]]
        h1 = F.relu(self.bn1(self.conv1(aligned)))  #@shape h1: [x.shape[0], 64, x.shape[2]]
        #@requires none
        if self.feature_transform:
            trans_feat = self.fstn(h1)  #@shape trans_feat: [x.shape[0], 64, 64]
            hf = torch.bmm(h1.transpose(2, 1), trans_feat)  #@shape hf: [x.shape[0], x.shape[2], 64]
            #@requires none
            h1 = hf.transpose(2, 1)  #@shape h1: [x.shape[0], 64, x.shape[2]]
        else:
            trans_feat = None
        pointfeat = h1  #@shape pointfeat: [x.shape[0], 64, x.shape[2]]
        h2 = F.relu(self.bn2(self.conv2(h1)))  #@shape h2: [x.shape[0], 128, x.shape[2]]
        h3 = self.bn3(self.conv3(h2))  #@shape h3: [x.shape[0], 1024, x.shape[2]]
        g = torch.max(h3, 2)[0]  #@shape g: [x.shape[0], 1024]
        #@shape@L147 g: [4, 1024]
        if self.global_feat:
            return g, trans, trans_feat
        expanded = g.view(-1, 1024, 1).repeat(1, 1, N)  #@shape expanded: [x.shape[0], 1024, x.shape[2]]
        seg = torch.cat([expanded, pointfeat], 1)  #@shape seg: [x.shape[0], 1088, x.shape[2]]
        return seg, trans, trans_feat


class PointNetCls(nn.Module):
    def __init__(self, num_classes=40, channel=3):
        super().__init__()
        self.num_classes = num_classes
        self.feat = PointNetEncoder(global_feat=True, feature_transform=True, channel=channel)
        self.fc1 = nn.Linear(1024, 512)
        self.fc2 = nn.Linear(512, 256)
        self.fc3 = nn.Linear(256, num_classes)
        self.dropout = nn.Dropout(p=0.4)
        self.bn1 = nn.BatchNorm1d(512)
        self.bn2 = nn.BatchNorm1d(256)

    def forward(self, x):
        g, trans, trans_feat = self.feat(x)  #@shape g: [x.shape[0], 1024]
        h1 = F.relu(self.bn1(self.fc1(g)))  #@shape h1: [x.shape[0], 512]
        #@requires none
        h2 = F.relu(self.bn2(self.dropout(self.fc2(h1))))  #@shape h2: [x.shape[0], 256]
        logits = self.fc3(h2)  #@shape logits: [x.shape[0], self.num_classes]
        #@requires none
        #@shape@L147 logits: [4, 10]
        out = F.log_softmax(logits, dim=1)  #@shape out: [x.shape[0], self.num_classes]
        return out, trans_feat


def feature_transform_regularizer(trans):
    B, d, _ = trans.size()  #@dim d: trans.shape[1]
    #@requires trans.dim() == 3
    I = torch.eye(d)[None, :, :]  #@shape I: [1, trans.shape[1], trans.shape[1]]
    prod = torch.bmm(trans, trans.transpose(2, 1))  #@shape prod: [trans.shape[0], trans.shape[1], trans.shape[1]]
    #@requires none
    diff = prod - I  #@shape diff: [trans.shape[0], trans.shape[1], trans.shape[1]]
    loss = torch.mean(torch.norm(diff, dim=(1, 2)))  #@shape loss: []
    return loss


def wrong_point_dim(model):
    cloud = torch.randn(4, 4, 32)
    return model.feat.stn.conv1(cloud)  #@shape-error Conv1d(3 -> 64) given 4 channels


if __name__ == "__main__":
    torch.manual_seed(0)
    model = PointNetCls(num_classes=10, channel=3)
    points = torch.randn(4, 3, 32)  #@shape points: [4, 3, 32]
    log_probs, trans_feat = model(points)  #@shape log_probs: [4, 10]; trans_feat: [4, 64, 64]
    reg = feature_transform_regularizer(trans_feat)  #@shape reg: []
    target = torch.randint(0, 10, (4,))
    loss = F.nll_loss(log_probs, target) + 0.001 * reg  #@shape loss: []
    seg_encoder = PointNetEncoder(global_feat=False, feature_transform=True, channel=3)
    seg, trans, _ = seg_encoder(points)  #@shape seg: [4, 1088, 32]; trans: [4, 3, 3]
    try:
        wrong_point_dim(model)
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass
