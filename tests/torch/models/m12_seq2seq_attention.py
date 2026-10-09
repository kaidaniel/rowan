# Sequence-to-sequence with additive attention (Bahdanau, Cho, Bengio 2015, "Neural Machine
# Translation by Jointly Learning to Align and Translate"), in the layout of the
# bentrevett/pytorch-seq2seq tutorial 3: a bidirectional GRU encoder (time-major
# [src_len, batch]) whose final forward / backward states go through a Linear to the decoder
# size; additive attention scoring the decoder state repeated over the source positions,
# concatenated with the encoder outputs, through Linear + tanh and a bias-free Linear to one
# score, softmax over the source positions; a GRU decoder fed [embedding; bmm context]; and a
# teacher-forcing loop over the target steps whose predictions are collected with
# torch.stack.
# Checks: GRU output / h_n shapes (bidirectional, time-major), h_n[-2, :, :] and
# h_n[-1, :, :], unsqueeze / repeat / permute / cat / squeeze in the attention, facts from
# cat (the earlier parameter's dim wins) and from an assert on the batch sizes, bmm, a loop
# whose body keeps the shapes, the if-expression join of teacher forcing, requirements
# collected through the encoder call, concrete shapes in the driver, and the attention
# context taken with bmm on the time-major encoder outputs (missing permute: a batch
# mismatch).
import torch
import torch.nn as nn
import torch.nn.functional as F


class Encoder(nn.Module):
    def __init__(self, input_dim, emb_dim, enc_hid_dim, dec_hid_dim, dropout):
        super().__init__()
        self.emb_dim = emb_dim
        self.enc_hid_dim = enc_hid_dim
        self.dec_hid_dim = dec_hid_dim
        self.embedding = nn.Embedding(input_dim, emb_dim)
        self.rnn = nn.GRU(emb_dim, enc_hid_dim, bidirectional=True)
        self.fc = nn.Linear(enc_hid_dim * 2, dec_hid_dim)
        self.dropout = nn.Dropout(dropout)

    def forward(self, src):
        src_len, batch_size = src.shape  #@dim src_len: src.shape[0]; batch_size: src.shape[1]
        embedded = self.dropout(self.embedding(src))  #@shape embedded: [src.shape[0], src.shape[1], self.emb_dim]
        #@requires none
        outputs, hidden = self.rnn(embedded)
        #@shape outputs: [src.shape[0], src.shape[1], 2*self.enc_hid_dim]; hidden: [2, src.shape[1], self.enc_hid_dim]
        #@requires none
        # hidden is stacked [forward_1, backward_1, forward_2, backward_2, ...]: the last two
        # are the final forward and backward states
        both = torch.cat((hidden[-2, :, :], hidden[-1, :, :]), dim=1)  #@shape both: [src.shape[1], 2*self.enc_hid_dim]
        #@requires none
        hidden = torch.tanh(self.fc(both))  #@shape hidden: [src.shape[1], self.dec_hid_dim]
        #@requires none
        return outputs, hidden


class Attention(nn.Module):
    def __init__(self, enc_hid_dim, dec_hid_dim):
        super().__init__()
        self.enc_hid_dim = enc_hid_dim
        self.dec_hid_dim = dec_hid_dim
        self.attn = nn.Linear((enc_hid_dim * 2) + dec_hid_dim, dec_hid_dim)
        self.v = nn.Linear(dec_hid_dim, 1, bias=False)

    def forward(self, hidden, encoder_outputs):
        # hidden: [batch, dec_hid_dim]; encoder_outputs: [src_len, batch, enc_hid_dim * 2]
        assert hidden.dim() == 2, "hidden must be [batch, dec_hid_dim]"
        src_len, batch_size, _ = encoder_outputs.shape  #@dim src_len: encoder_outputs.shape[0]
        # repeat the decoder hidden state src_len times
        h = hidden.unsqueeze(1).repeat(1, src_len, 1)  #@shape h: [hidden.shape[0], encoder_outputs.shape[0], hidden.shape[1]]
        eo = encoder_outputs.permute(1, 0, 2)
        #@shape eo: [encoder_outputs.shape[1], encoder_outputs.shape[0], encoder_outputs.shape[2]]
        cat_in = torch.cat((h, eo), dim=2)
        #@shape cat_in: [hidden.shape[0], encoder_outputs.shape[0], hidden.shape[1] + encoder_outputs.shape[2]]
        energy = torch.tanh(self.attn(cat_in))  #@shape energy: [hidden.shape[0], encoder_outputs.shape[0], self.dec_hid_dim]
        scores = self.v(energy).squeeze(2)  #@shape scores: [hidden.shape[0], encoder_outputs.shape[0]]
        weights = F.softmax(scores, dim=1)  #@shape weights: [hidden.shape[0], encoder_outputs.shape[0]]
        #@shape@L161 weights: [3, 7]
        return weights


class Decoder(nn.Module):
    def __init__(self, output_dim, emb_dim, enc_hid_dim, dec_hid_dim, dropout):
        super().__init__()
        self.output_dim = output_dim
        self.emb_dim = emb_dim
        self.enc_hid_dim = enc_hid_dim
        self.dec_hid_dim = dec_hid_dim
        self.attention = Attention(enc_hid_dim, dec_hid_dim)
        self.embedding = nn.Embedding(output_dim, emb_dim)
        self.rnn = nn.GRU((enc_hid_dim * 2) + emb_dim, dec_hid_dim)
        self.fc_out = nn.Linear((enc_hid_dim * 2) + dec_hid_dim + emb_dim, output_dim)
        self.dropout = nn.Dropout(dropout)

    def forward(self, token, hidden, encoder_outputs):
        # token: [batch]; hidden: [batch, dec_hid_dim]; encoder_outputs: [src_len, batch, enc_hid_dim * 2]
        assert token.dim() == 1, "one token per sentence"
        embedded = self.dropout(self.embedding(token.unsqueeze(0)))  #@shape embedded: [1, token.shape[0], self.emb_dim]
        a = self.attention(hidden, encoder_outputs)  #@shape a: [hidden.shape[0], encoder_outputs.shape[0]]
        a = a.unsqueeze(1)  #@shape a: [hidden.shape[0], 1, encoder_outputs.shape[0]]
        eo = encoder_outputs.permute(1, 0, 2)  #@shape eo: [hidden.shape[0], encoder_outputs.shape[0], _]
        weighted = torch.bmm(a, eo)  #@shape weighted: [hidden.shape[0], 1, _]
        weighted = weighted.permute(1, 0, 2)  #@shape weighted: [1, hidden.shape[0], _]
        rnn_input = torch.cat((embedded, weighted), dim=2)  #@shape rnn_input: [1, token.shape[0], _]
        output, h = self.rnn(rnn_input, hidden.unsqueeze(0))
        #@shape output: [1, token.shape[0], self.dec_hid_dim]; h: [1, token.shape[0], self.dec_hid_dim]
        # one step, one layer, one direction: output == h
        features = torch.cat((output.squeeze(0), weighted.squeeze(0), embedded.squeeze(0)), dim=1)
        #@shape features: [token.shape[0], self.dec_hid_dim + 2*self.enc_hid_dim + self.emb_dim]
        prediction = self.fc_out(features)  #@shape prediction: [token.shape[0], self.output_dim]
        #@requires none
        #@shape@L161 prediction: [3, 25]
        return prediction, h.squeeze(0)


class Seq2Seq(nn.Module):
    def __init__(self, src_vocab, trg_vocab, emb_dim, enc_hid_dim, dec_hid_dim, dropout):
        super().__init__()
        self.trg_vocab = trg_vocab
        self.enc_hid_dim = enc_hid_dim
        self.dec_hid_dim = dec_hid_dim
        self.encoder = Encoder(src_vocab, emb_dim, enc_hid_dim, dec_hid_dim, dropout)
        self.decoder = Decoder(trg_vocab, emb_dim, enc_hid_dim, dec_hid_dim, dropout)

    def forward(self, src, trg, teacher_forcing_ratio=0.5):
        # src: [src_len, batch]; trg: [trg_len, batch]
        trg_len, batch_size = trg.shape  #@dim trg_len: trg.shape[0]; batch_size: trg.shape[1]
        #@requires trg.dim() == 2
        encoder_outputs, hidden = self.encoder(src)
        #@shape encoder_outputs: [src.shape[0], src.shape[1], 2*self.enc_hid_dim]; hidden: [src.shape[1], self.dec_hid_dim]
        #@requires src.dim() == 2
        #@shape@L161 encoder_outputs: [7, 3, 32]
        assert src.shape[1] == batch_size, "source and target batches differ"
        #@dim batch_size: src.shape[1]
        # first input to the decoder is the <sos> tokens
        token = trg[0]  #@shape token: [src.shape[1]]
        outputs = []
        for t in range(1, trg_len):
            # insert the token embedding, previous hidden state and the encoder outputs;
            # receive the prediction and the new hidden state
            prediction, hidden = self.decoder(token, hidden, encoder_outputs)
            #@shape prediction: [src.shape[1], self.trg_vocab]; hidden: [src.shape[1], self.dec_hid_dim]
            #@requires none
            outputs.append(prediction)
            teacher_force = torch.rand(1).item() < teacher_forcing_ratio
            top1 = prediction.argmax(1)  #@shape top1: [src.shape[1]]
            # the next input is the ground-truth token if teacher forcing, else the prediction
            token = trg[t] if teacher_force else top1  #@shape token: [src.shape[1]]
        out = torch.stack(outputs)  #@shape out: [_, src.shape[1], self.trg_vocab]
        #@requires none
        return out


def context_without_permute():
    attention = Attention(enc_hid_dim=4, dec_hid_dim=6)
    hidden = torch.randn(2, 6)
    encoder_outputs = torch.randn(9, 2, 8)  # time-major: [src_len, batch, 2 * enc_hid_dim]
    a = attention(hidden, encoder_outputs).unsqueeze(1)  #@shape a: [2, 1, 9]
    # bmm needs batch-first encoder outputs: the batch dims 2 and 9 differ
    return torch.bmm(a, encoder_outputs)  #@shape-error


if __name__ == "__main__":
    SRC_V, TRG_V = 20, 25
    model = Seq2Seq(SRC_V, TRG_V, emb_dim=8, enc_hid_dim=16, dec_hid_dim=12, dropout=0.1)
    optimizer = torch.optim.Adam(model.parameters())
    src = torch.randint(0, SRC_V, (7, 3))
    trg = torch.randint(0, TRG_V, (5, 3))
    out = model(src, trg)  #@shape out: [4, 3, 25]
    # trg[1:] skips <sos>; cross_entropy over (steps * batch) rows
    logits = out.view(-1, TRG_V)  #@shape logits: [12, 25]
    loss = F.cross_entropy(logits, trg[1:].reshape(-1))  #@shape loss: []
    loss.backward()
    optimizer.step()
    model.eval()
    with torch.no_grad():
        greedy = model(src, trg, teacher_forcing_ratio=0.0)  #@shape greedy: [4, 3, 25]
    enc = Encoder(SRC_V, 8, 16, 12, 0.0)
    enc_out, enc_hidden = enc(torch.randint(0, SRC_V, (6, 2)))  #@shape enc_out: [6, 2, 32]; enc_hidden: [2, 12]
    try:
        context_without_permute()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass
