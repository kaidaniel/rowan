# word2vec skip-gram with negative sampling (Mikolov et al., "Distributed Representations of
# Words and Phrases and their Compositionality", 2013).
# Two embedding tables (input / output vectors); for a batch of centre words [B], their context
# words [B] and K negatives per pair [B, K], the positive score is <u_ctx, v_center> and the
# negative scores <u_neg, v_center>, both through torch.bmm on unsqueezed vectors; the loss is
# -(logsigmoid(pos) + sum_k logsigmoid(-neg_k)), averaged over the batch.
# Checked: embedding lookups on parameters of unknown rank and after rank facts, bmm with the
# batch requirement (rewritten to the earlier parameter), squeeze, reductions, a sampler with an
# explicit view, nearest neighbours by cosine similarity with topk; concrete shapes in the
# driver; a batch mismatch in bmm.
import torch
import torch.nn as nn
import torch.nn.functional as F


def unigram_table(counts):
    """Noise distribution: counts ** 0.75, normalised."""
    weights = counts.float() ** 0.75  #@shape weights: [*counts.shape]
    probs = weights / weights.sum()  #@shape probs: [*counts.shape]
    return probs


def sample_negatives(noise, batch, k):
    """Draws k negatives per pair from the noise distribution."""
    flat = torch.multinomial(noise, batch * k, replacement=True)
    neg = flat.view(batch, k)  #@shape neg: [batch, k]
    return neg


class SkipGramNS(nn.Module):
    def __init__(self, vocab_size, embed_dim):
        super().__init__()
        self.vocab_size = vocab_size
        self.embed_dim = embed_dim
        self.in_embed = nn.Embedding(vocab_size, embed_dim)
        self.out_embed = nn.Embedding(vocab_size, embed_dim)
        bound = 0.5 / embed_dim
        nn.init.uniform_(self.in_embed.weight, -bound, bound)
        nn.init.zeros_(self.out_embed.weight)

    def lookup(self, words):
        vecs = self.in_embed(words)  #@shape vecs: [*words.shape, self.embed_dim]
        #@requires none
        return vecs

    def forward(self, center, context, negatives):
        assert center.dim() == 1
        assert context.dim() == 1
        B, K = negatives.shape  #@dim B: negatives.shape[0]; K: negatives.shape[1]
        #@requires negatives.dim() == 2
        v = self.in_embed(center)  #@shape v: [center.shape[0], self.embed_dim]
        #@requires none
        u = self.out_embed(context)  #@shape u: [context.shape[0], self.embed_dim]
        n = self.out_embed(negatives)  #@shape n: [negatives.shape[0], negatives.shape[1], self.embed_dim]
        #@requires none
        u3 = u.unsqueeze(1)  #@shape u3: [context.shape[0], 1, self.embed_dim]
        v3 = v.unsqueeze(2)  #@shape v3: [center.shape[0], self.embed_dim, 1]
        pos = torch.bmm(u3, v3)  #@shape pos: [center.shape[0], 1, 1]
        #@requires context.shape[0] == center.shape[0]
        #@shape@L106 pos: [4, 1, 1]
        pos_score = pos.squeeze(2).squeeze(1)  #@shape pos_score: [center.shape[0]]
        neg = torch.bmm(n, v3)  #@shape neg: [center.shape[0], negatives.shape[1], 1]
        #@requires negatives.shape[0] == center.shape[0]
        #@shape@L106 neg: [4, 5, 1]
        neg_score = neg.squeeze(2)  #@shape neg_score: [center.shape[0], negatives.shape[1]]
        pos_loss = F.logsigmoid(pos_score)  #@shape pos_loss: [center.shape[0]]
        neg_loss = F.logsigmoid(-neg_score).sum(dim=1)  #@shape neg_loss: [center.shape[0]]
        per_pair = -(pos_loss + neg_loss)  #@shape per_pair: [center.shape[0]]
        loss = per_pair.mean()  #@shape loss: []
        return loss

    def nearest(self, word_ids, k):
        if word_ids.dim() != 1:
            raise ValueError("expected a 1-D tensor of word ids")
        table = F.normalize(self.in_embed.weight, dim=1)  #@shape table: [self.vocab_size, self.embed_dim]
        q = F.normalize(self.in_embed(word_ids), dim=-1)  #@shape q: [word_ids.shape[0], self.embed_dim]
        sims = q @ table.t()  #@shape sims: [word_ids.shape[0], self.vocab_size]
        #@requires none
        scores, idx = sims.topk(k, dim=1)  #@shape scores: [word_ids.shape[0], k]; idx: [word_ids.shape[0], k]
        return scores, idx


def train_step(model, optimizer, center, context, noise, k):
    negatives = sample_negatives(noise, center.shape[0], k)  #@shape negatives: [center.shape[0], k]
    optimizer.zero_grad()
    loss = model(center, context, negatives)  #@shape@L110 loss: []
    loss.backward()
    optimizer.step()
    return loss.item()


def mismatched_batch(model):
    v = model.in_embed(torch.randint(0, 50, (4,)))
    n = model.out_embed(torch.randint(0, 50, (3, 5)))
    return torch.bmm(n, v.unsqueeze(2))  #@shape-error batch 3 vs 4


if __name__ == "__main__":
    vocab, dim, batch, k = 50, 8, 4, 5
    counts = torch.randint(1, 100, (vocab,))
    noise = unigram_table(counts)  #@shape noise: [50]
    model = SkipGramNS(vocab, dim)
    center = torch.randint(0, vocab, (batch,))
    context = torch.randint(0, vocab, (batch,))
    negatives = sample_negatives(noise, batch, k)  #@shape negatives: [4, 5]
    loss = model(center, context, negatives)  #@shape loss: []
    vecs = model.lookup(torch.randint(0, vocab, (3, 7)))  #@shape vecs: [3, 7, 8]
    opt = torch.optim.SGD(model.parameters(), lr=0.1)
    for _ in range(2):
        train_step(model, opt, center, context, noise, k)
    scores, idx = model.nearest(torch.tensor([1, 2, 3]), 4)  #@shape scores: [3, 4]; idx: [3, 4]
    try:
        mismatched_batch(model)
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass
