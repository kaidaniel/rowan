# LSTM part-of-speech tagger from the PyTorch tutorial "Sequence Models and Long Short-Term
# Memory Networks" (Embedding -> LSTM -> Linear -> log_softmax over one sentence, fed to the
# LSTM as [len, 1, emb] with view), plus a batched variant: a 2-layer bidirectional LSTM
# with batch_first=True whose per-token outputs are tagged and whose final hidden states of
# the last layer, h_n[-2] (forward) and h_n[-1] (backward), are concatenated into a sentence
# representation for a classifier (the usual BiLSTM text-classification idiom).
# Checks: view with -1 divided symbolically (`T*E / T = E`), LSTM output / h_n / c_n shapes
# for batch-first, bidirectional, 2 layers, batched and unbatched (`tokens.dim() in (1, 2)`
# through the Embedding), indexing h_n from the end, cat of the two directions, flattening
# B*T for the loss, requirements of the layer calls, the tutorial's training loop in the
# driver, and an LSTM fed vectors of the wrong size.
import torch
import torch.nn as nn
import torch.nn.functional as F
import torch.optim as optim


def prepare_sequence(seq, to_ix):
    idxs = [to_ix[w] for w in seq]
    t = torch.tensor(idxs, dtype=torch.long)  #@shape@L124 t: [5]
    return t


class LSTMTagger(nn.Module):
    def __init__(self, embedding_dim, hidden_dim, vocab_size, tagset_size):
        super().__init__()
        self.embedding_dim = embedding_dim
        self.hidden_dim = hidden_dim
        self.tagset_size = tagset_size
        self.word_embeddings = nn.Embedding(vocab_size, embedding_dim)
        # The LSTM takes word embeddings as inputs, and outputs hidden states
        # with dimensionality hidden_dim.
        self.lstm = nn.LSTM(embedding_dim, hidden_dim)
        # The linear layer that maps from hidden state space to tag space
        self.hidden2tag = nn.Linear(hidden_dim, tagset_size)

    def forward(self, sentence):
        assert sentence.dim() == 1, "one sentence of word indices"
        embeds = self.word_embeddings(sentence)  #@shape embeds: [sentence.shape[0], self.embedding_dim]
        #@requires none
        lstm_in = embeds.view(len(sentence), 1, -1)  #@shape lstm_in: [sentence.shape[0], 1, self.embedding_dim]
        lstm_out, (h_n, c_n) = self.lstm(lstm_in)  #@shape lstm_out: [sentence.shape[0], 1, self.hidden_dim]
        #@shape h_n: [1, 1, self.hidden_dim]; c_n: [1, 1, self.hidden_dim]
        #@requires none
        tag_space = self.hidden2tag(lstm_out.view(len(sentence), -1))  #@shape tag_space: [sentence.shape[0], self.tagset_size]
        #@requires none
        tag_scores = F.log_softmax(tag_space, dim=1)  #@shape tag_scores: [sentence.shape[0], self.tagset_size]
        #@shape@L124 tag_scores: [5, 3]
        return tag_scores


class BiLSTMTagger(nn.Module):
    def __init__(self, vocab_size, embedding_dim, hidden_dim, tagset_size, n_classes, dropout=0.1):
        super().__init__()
        self.embedding_dim = embedding_dim
        self.hidden_dim = hidden_dim
        self.tagset_size = tagset_size
        self.n_classes = n_classes
        self.embed = nn.Embedding(vocab_size, embedding_dim, padding_idx=0)
        self.lstm = nn.LSTM(embedding_dim, hidden_dim, num_layers=2, batch_first=True, bidirectional=True)
        self.dropout = nn.Dropout(dropout)
        self.hidden2tag = nn.Linear(2 * hidden_dim, tagset_size)
        self.classifier = nn.Linear(2 * hidden_dim, n_classes)

    def encode(self, tokens):
        emb = self.embed(tokens)  #@shape emb: [*tokens.shape, self.embedding_dim]
        #@requires none
        out, (h_n, c_n) = self.lstm(emb)
        #@requires tokens.dim() in (1, 2)
        #@shape out: [*tokens.shape, 2*self.hidden_dim]; h_n: [4, *tokens.shape[:-1], self.hidden_dim]
        #@shape c_n: [4, *tokens.shape[:-1], self.hidden_dim]
        return out, h_n

    def forward(self, tokens):
        B, T = tokens.shape  #@dim B: tokens.shape[0]; T: tokens.shape[1]
        #@requires tokens.dim() == 2
        out, h_n = self.encode(tokens)
        #@shape out: [tokens.shape[0], tokens.shape[1], 2*self.hidden_dim]; h_n: [4, tokens.shape[0], self.hidden_dim]
        #@requires none
        tag_scores = F.log_softmax(self.hidden2tag(self.dropout(out)), dim=-1)
        #@shape tag_scores: [tokens.shape[0], tokens.shape[1], self.tagset_size]
        #@requires none
        # final states of the last layer: h_n is (num_layers * 2, B, H), forward then backward
        last_fwd = h_n[-2]  #@shape last_fwd: [tokens.shape[0], self.hidden_dim]
        sent = torch.cat([last_fwd, h_n[-1]], dim=1)  #@shape sent: [tokens.shape[0], 2*self.hidden_dim]
        #@requires none
        #@shape@L137 sent: [3, 20]
        sent_logits = self.classifier(self.dropout(sent))  #@shape sent_logits: [tokens.shape[0], self.n_classes]
        #@requires none
        flat = tag_scores.view(B * T, -1)  #@shape flat: [tokens.shape[0]*tokens.shape[1], self.tagset_size]
        return flat, sent_logits


def tagging_loss(flat_scores, tags):
    targets = tags.view(-1)  #@shape targets: [?]
    loss = F.nll_loss(flat_scores, targets, ignore_index=-100)  #@shape loss: []
    return loss


def wrong_embedding_size():
    model = BiLSTMTagger(vocab_size=20, embedding_dim=8, hidden_dim=10, tagset_size=4, n_classes=3)
    feats = torch.randn(3, 7, 5)  # 5-dim features, but the LSTM was built for 8
    return model.lstm(feats)  #@shape-error


if __name__ == "__main__":
    training_data = [
        # Tags are: DET - determiner; NN - noun; V - verb
        ("The dog ate the apple".split(), ["DET", "NN", "V", "DET", "NN"]),
        ("Everybody read that book".split(), ["NN", "V", "DET", "NN"]),
    ]
    word_to_ix = {}
    for sent, tags in training_data:
        for word in sent:
            if word not in word_to_ix:
                word_to_ix[word] = len(word_to_ix)
    tag_to_ix = {"DET": 0, "NN": 1, "V": 2}
    EMBEDDING_DIM = 6
    HIDDEN_DIM = 6
    model = LSTMTagger(EMBEDDING_DIM, HIDDEN_DIM, len(word_to_ix), len(tag_to_ix))
    loss_function = nn.NLLLoss()
    optimizer = optim.SGD(model.parameters(), lr=0.1)
    with torch.no_grad():
        scores = model(prepare_sequence(training_data[0][0], word_to_ix))  #@shape scores: [5, 3]
    for epoch in range(2):
        for sentence, tags in training_data:
            model.zero_grad()
            sentence_in = prepare_sequence(sentence, word_to_ix)
            targets = prepare_sequence(tags, tag_to_ix)
            tag_scores = model(sentence_in)
            loss = loss_function(tag_scores, targets)  #@shape loss: []
            loss.backward()
            optimizer.step()

    bi = BiLSTMTagger(vocab_size=20, embedding_dim=8, hidden_dim=10, tagset_size=4, n_classes=3)
    tokens = torch.randint(1, 20, (3, 7))
    flat, sent_logits = bi(tokens)  #@shape flat: [21, 4]; sent_logits: [3, 3]
    tag_loss = tagging_loss(flat, torch.randint(0, 4, (3, 7)))  #@shape tag_loss: []
    one_out, one_h = bi.encode(torch.randint(1, 20, (7,)))  #@shape one_out: [7, 20]; one_h: [4, 10]
    try:
        wrong_embedding_size()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass
