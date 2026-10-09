# Variational autoencoder (Kingma, Welling 2014, "Auto-Encoding Variational Bayes"), as in
# the pytorch/examples VAE for MNIST: an MLP encoder 784 -> hidden -> (mu, logvar), the
# reparameterization trick z = mu + eps * exp(0.5 * logvar) with eps from randn_like, an MLP
# decoder back to 784 sigmoid pixels, and the loss = binary cross-entropy (summed) + KL
# divergence. Inputs are images flattened with x.view(-1, 784).
# Checks: view(-1, 784) on a tensor of unknown rank (the -1 dim is unknown), Linear chains
# with symbolic hidden / latent sizes, broadcasting of two parameters of unknown shape
# (`broadcast(mu.shape, logvar.shape)`), *_like creation, reductions to scalars, sampling
# with a named int batch size, concrete shapes in the driver (one training step), and
# CIFAR-sized images fed to the 784-input model (a view whose element count does not
# divide).
import torch
import torch.nn as nn
import torch.nn.functional as F

IMG_PIXELS = 28 * 28


class VAE(nn.Module):
    def __init__(self, hidden_dim, latent_dim):
        super().__init__()
        self.hidden_dim = hidden_dim
        self.latent_dim = latent_dim
        self.fc1 = nn.Linear(784, hidden_dim)
        self.fc_mu = nn.Linear(hidden_dim, latent_dim)
        self.fc_logvar = nn.Linear(hidden_dim, latent_dim)
        self.fc3 = nn.Linear(latent_dim, hidden_dim)
        self.fc4 = nn.Linear(hidden_dim, 784)

    def encode(self, x):
        h1 = F.relu(self.fc1(x))  #@shape h1: [*x.shape[:-1], self.hidden_dim]
        #@requires x.shape[-1] == 784
        mu = self.fc_mu(h1)  #@shape mu: [*x.shape[:-1], self.latent_dim]
        #@requires none
        logvar = self.fc_logvar(h1)  #@shape logvar: [*x.shape[:-1], self.latent_dim]
        return mu, logvar

    def reparameterize(self, mu, logvar):
        std = torch.exp(0.5 * logvar)  #@shape std: [*logvar.shape]
        eps = torch.randn_like(std)  #@shape eps: [*logvar.shape]
        z = mu + eps * std  #@shape z: [*broadcast(mu.shape, logvar.shape)]
        #@requires none
        return z

    def decode(self, z):
        h3 = F.relu(self.fc3(z))  #@shape h3: [*z.shape[:-1], self.hidden_dim]
        #@requires z.shape[-1] == self.latent_dim
        out = torch.sigmoid(self.fc4(h3))  #@shape out: [*z.shape[:-1], 784]
        #@requires none
        return out

    def forward(self, x):
        flat = x.view(-1, 784)  #@shape flat: [?, 784]
        #@shape@L94 flat: [16, 784]
        mu, logvar = self.encode(flat)  #@shape mu: [?, self.latent_dim]; logvar: [?, self.latent_dim]
        #@requires none
        z = self.reparameterize(mu, logvar)  #@shape z: [?, self.latent_dim]
        #@shape@L94 z: [16, 8]
        recon = self.decode(z)  #@shape recon: [?, 784]
        #@requires none
        return recon, mu, logvar

    def sample(self, n):
        z = torch.randn(n, self.latent_dim)  #@shape z: [n, self.latent_dim]
        imgs = self.decode(z).view(n, 1, 28, 28)  #@shape imgs: [n, 1, 28, 28]
        #@requires none
        return imgs


def loss_function(recon_x, x, mu, logvar):
    # Reconstruction + KL divergence losses summed over all elements and batch
    target = x.view(-1, IMG_PIXELS)  #@shape target: [?, 784]
    bce = F.binary_cross_entropy(recon_x, target, reduction="sum")  #@shape bce: []
    # see Appendix B of the paper: 0.5 * sum(1 + log(sigma^2) - mu^2 - sigma^2)
    kl_terms = 1 + logvar - mu.pow(2) - logvar.exp()  #@shape kl_terms: [*broadcast(logvar.shape, mu.shape)]
    kld = -0.5 * torch.sum(kl_terms)  #@shape kld: []
    total = bce + kld  #@shape total: []
    return total


def cifar_into_mnist_vae():
    # 4 RGB 32x32 images hold 12288 values, not a multiple of 784
    imgs = torch.rand(4, 3, 32, 32)
    flat = imgs.view(-1, 784)  #@shape-error
    return flat


if __name__ == "__main__":
    torch.manual_seed(1)
    model = VAE(hidden_dim=64, latent_dim=8)
    optimizer = torch.optim.Adam(model.parameters(), lr=1e-3)
    data = torch.rand(16, 1, 28, 28)
    optimizer.zero_grad()
    recon, mu, logvar = model(data)  #@shape recon: [16, 784]; mu: [16, 8]; logvar: [16, 8]
    loss = loss_function(recon, data, mu, logvar)  #@shape loss: []
    loss.backward()
    optimizer.step()
    with torch.no_grad():
        samples = model.sample(5)  #@shape samples: [5, 1, 28, 28]
        codes, _ = model.encode(torch.rand(3, 784))  #@shape codes: [3, 8]
        z = model.reparameterize(torch.zeros(3, 8), torch.zeros(8))  #@shape z: [3, 8]
    try:
        cifar_into_mnist_vae()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass
