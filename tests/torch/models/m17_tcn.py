# Temporal Convolutional Network (Bai, Kolter, Koltun, "An Empirical Evaluation of Generic
# Convolutional and Recurrent Networks for Sequence Modeling", 2018), after the reference
# implementation (locuslab/TCN) without weight_norm.
# TemporalBlock: two dilated causal Conv1d (padding = (k-1)*dilation on both sides, then
# Chomp1d drops the trailing padding with x[:, :, :-chomp]), ReLU and dropout, plus a residual
# through a 1x1 Conv1d when the channel counts differ. TemporalConvNet stacks blocks with
# dilation 2**i; the sequence model applies a Linear head on the last time step y[:, :, -1].
# Also a functional causal convolution (F.pad on the left, F.conv1d).
# Checked: the conv length formula with symbolic padding / dilation / kernel, the chomp (`?`:
# a symbolic `:-c` slice is not a known length, c may be 0), the residual join, F.pad, F.conv1d
# with its requirements, the last-step head; lengths preserved under the driver; a wrong
# channel count into the first block.
import torch
import torch.nn as nn
import torch.nn.functional as F


class Chomp1d(nn.Module):
    """Removes the trailing `chomp_size` steps added by symmetric padding."""

    def __init__(self, chomp_size):
        super().__init__()
        self.chomp_size = chomp_size

    def forward(self, x):
        out = x[:, :, :-self.chomp_size].contiguous()  #@shape out: [x.shape[0], x.shape[1], ?, *x.shape[3:]]
        #@requires x.dim() >= 3
        return out


class TemporalBlock(nn.Module):
    def __init__(self, n_inputs, n_outputs, kernel_size, dilation, padding, dropout=0.2):
        super().__init__()
        self.n_inputs = n_inputs
        self.n_outputs = n_outputs
        self.kernel_size = kernel_size
        self.dilation = dilation
        self.padding = padding
        self.conv1 = nn.Conv1d(n_inputs, n_outputs, kernel_size, stride=1, padding=padding, dilation=dilation)
        self.chomp1 = Chomp1d(padding)
        self.relu1 = nn.ReLU()
        self.dropout1 = nn.Dropout(dropout)
        self.conv2 = nn.Conv1d(n_outputs, n_outputs, kernel_size, stride=1, padding=padding, dilation=dilation)
        self.chomp2 = Chomp1d(padding)
        self.relu2 = nn.ReLU()
        self.dropout2 = nn.Dropout(dropout)
        self.downsample = nn.Conv1d(n_inputs, n_outputs, 1) if n_inputs != n_outputs else None
        self.relu = nn.ReLU()
        self.init_weights()

    def init_weights(self):
        self.conv1.weight.data.normal_(0, 0.01)
        self.conv2.weight.data.normal_(0, 0.01)
        if self.downsample is not None:
            self.downsample.weight.data.normal_(0, 0.01)

    def forward(self, x):
        batch, channels, length = x.shape  #@dim batch: x.shape[0]; length: x.shape[2]
        #@requires x.dim() == 3
        h = self.conv1(x)  #@shape h: [x.shape[0], self.n_outputs, x.shape[2] + 2*self.padding - self.dilation*(self.kernel_size - 1)]
        #@requires x.shape[1] == self.n_inputs
        h = self.chomp1(h)  #@shape h: [x.shape[0], self.n_outputs, ?]
        #@shape@L139 h: [2, _, 20]
        h = self.dropout1(self.relu1(h))
        h = self.conv2(h)  #@shape h: [x.shape[0], self.n_outputs, ?]
        #@requires none
        h = self.chomp2(h)
        h = self.dropout2(self.relu2(h))  #@shape h: [x.shape[0], self.n_outputs, ?]
        #@shape@L139 h: [2, _, 20]
        # n_inputs (no downsample) joins n_outputs (downsample): `?`
        res = x if self.downsample is None else self.downsample(x)  #@shape res: [x.shape[0], ?, x.shape[2]]
        #@shape@L139 res: [2, _, 20]
        out = self.relu(h + res)  #@shape out: [x.shape[0], ?, ?]
        #@shape@L139 out: [2, _, 20]
        return out


class TemporalConvNet(nn.Module):
    def __init__(self, num_inputs, num_channels, kernel_size=2, dropout=0.2):
        super().__init__()
        layers = []
        num_levels = len(num_channels)
        for i in range(num_levels):
            dilation_size = 2 ** i
            in_channels = num_inputs if i == 0 else num_channels[i - 1]
            out_channels = num_channels[i]
            layers += [TemporalBlock(in_channels, out_channels, kernel_size, dilation=dilation_size,
                                     padding=(kernel_size - 1) * dilation_size, dropout=dropout)]
        self.network = nn.Sequential(*layers)

    def forward(self, x):
        return self.network(x)


class TCN(nn.Module):
    """Sequence classifier: TCN over [N, C_in, L], Linear on the last step."""

    def __init__(self, input_size, output_size, num_channels, kernel_size, dropout):
        super().__init__()
        self.output_size = output_size
        self.tcn = TemporalConvNet(input_size, num_channels, kernel_size=kernel_size, dropout=dropout)
        self.linear = nn.Linear(num_channels[-1], output_size)
        self.linear.weight.data.normal_(0, 0.01)

    def forward(self, inputs):
        y1 = self.tcn(inputs)
        #@shape@L139 y1: [2, 16, 20]
        last = y1[:, :, -1]
        #@shape@L139 last: [2, 16]
        o = self.linear(last)  #@shape o: [*_, self.output_size]
        #@shape@L139 o: [2, 5]
        logp = F.log_softmax(o, dim=1)  #@shape logp: [*_, self.output_size]
        return logp


def causal_conv1d(x, weight, dilation):
    """Causal dilated convolution: pad (k-1)*dilation steps on the left only."""
    out_ch, in_ch, k = weight.shape  #@dim out_ch: weight.shape[0]; k: weight.shape[2]
    #@requires weight.dim() == 3
    pad = (k - 1) * dilation  #@dim pad: (weight.shape[2] - 1)*dilation
    xp = F.pad(x, (pad, 0))  #@shape xp: [*x.shape[:-1], x.shape[-1] + (weight.shape[2] - 1)*dilation]
    y = F.conv1d(xp, weight, dilation=dilation)  #@shape y: [*x.shape[:-2], weight.shape[0], x.shape[-1]]
    #@requires x.dim() in (2, 3); x.shape[-2] == weight.shape[1]
    return y


def receptive_field(kernel_size, levels):
    return 1 + 2 * (kernel_size - 1) * (2 ** levels - 1)


def wrong_input_channels(model):
    seq = torch.randn(2, 4, 20)
    return model.tcn.network[0].conv1(seq)  #@shape-error Conv1d(3 -> 8) given 4 channels


if __name__ == "__main__":
    model = TCN(input_size=3, output_size=5, num_channels=[8, 8, 16], kernel_size=3, dropout=0.1)
    seq = torch.randn(2, 3, 20)  #@shape seq: [2, 3, 20]
    logp = model(seq)  #@shape logp: [2, 5]
    features = model.tcn(seq)  #@shape features: [2, 16, 20]
    block = TemporalBlock(3, 3, 3, dilation=2, padding=4, dropout=0.0)
    same = block(seq)  #@shape same: [2, 3, 20]
    chomped = Chomp1d(4)(torch.randn(2, 3, 24))  #@shape chomped: [2, 3, 20]
    w = torch.randn(6, 3, 3)
    y = causal_conv1d(seq, w, 4)  #@shape y: [2, 6, 20]
    unbatched = causal_conv1d(torch.randn(3, 11), w, 1)  #@shape unbatched: [6, 11]
    rf = receptive_field(3, 3)  #@dim rf: 29
    try:
        wrong_input_channels(model)
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass
