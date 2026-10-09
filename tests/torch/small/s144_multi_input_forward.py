# a module whose forward takes tensors of different ranks, called positionally, by keyword in swapped order, and mixed
import torch
import torch.nn as nn


class Fuse(nn.Module):
    def __init__(self, d_img, d_txt, d):
        super().__init__()
        self.d = d
        self.img_proj = nn.Linear(d_img, d)
        self.txt_proj = nn.Linear(d_txt, d)

    def forward(self, img, txt):
        B, Di = img.shape
        B2, L, Dt = txt.shape
        i = self.img_proj(img).unsqueeze(1)  #@shape i: [img.shape[0], 1, self.d]
        #@requires img.shape[1] == d_img
        t = self.txt_proj(txt)  #@shape t: [txt.shape[0], txt.shape[1], self.d]
        #@requires txt.shape[2] == d_txt
        s = (t * i).sum(-1)  #@shape s: [max(img.shape[0], txt.shape[0]), txt.shape[1]]
        return s


class Wrapper(nn.Module):
    def __init__(self):
        super().__init__()
        self.f = Fuse(5, 7, 4)

    def forward(self, a, b):
        x = self.f(a, b)  #@shape x: [max(a.shape[0], b.shape[0]), b.shape[1]]
        #@requires a.dim() == 2; b.dim() == 3; a.shape[1] == 5; b.shape[2] == 7
        y = self.f(txt=b, img=a)  #@shape y: [max(a.shape[0], b.shape[0]), b.shape[1]]
        #@requires none
        z = self.f(a, txt=b)  #@shape z: [max(a.shape[0], b.shape[0]), b.shape[1]]
        #@requires none
        return x, y, z


if __name__ == "__main__":
    w = Wrapper()
    x, y, z = w(torch.randn(3, 5), torch.randn(3, 6, 7))  #@shape x: [3, 6]; y: [3, 6]; z: [3, 6]
    s = w.f(txt=torch.randn(1, 2, 7), img=torch.randn(4, 5))  #@shape s: [4, 2]
