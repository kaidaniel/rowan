# BERT embeddings, self-attention and pooler (Devlin et al., "BERT: Pre-training of Deep
# Bidirectional Transformers for Language Understanding", 2019), after the structure of the
# Hugging Face BertEmbeddings / BertSelfAttention / BertPooler modules.
# Embeddings: word + position + token-type embeddings, LayerNorm, dropout; the position ids are a
# buffer arange(max_position_embeddings) expanded to [1, P] and sliced to the sequence length;
# token-type ids default to zeros_like(input_ids). Self-attention: Q/K/V projections,
# transpose_for_scores (view to heads + permute(0, 2, 1, 3)), scaled scores, an additive padding
# mask broadcast as [B, 1, 1, S], softmax, context merged back to [B, S, H]. Pooler: dense+tanh
# on hidden[:, 0].
# Checked: lookups, the buffer slice (min), the default branch, size-tuple arithmetic, the head
# split and merge, matmul shapes, the mask view, the pooler; concrete shapes in the driver; a
# hidden size mismatch in the query projection.
#
# Every dim used in a method is held by a direct self attribute (self.hidden_size, …), so the
# canonical name is that attribute.
import math

import torch
import torch.nn as nn
import torch.nn.functional as F


class BertConfig:
    def __init__(self, vocab_size=30522, hidden_size=768, num_attention_heads=12, max_position_embeddings=512,
                 type_vocab_size=2, pad_token_id=0, layer_norm_eps=1e-12, hidden_dropout_prob=0.1,
                 attention_probs_dropout_prob=0.1):
        self.vocab_size = vocab_size
        self.hidden_size = hidden_size
        self.num_attention_heads = num_attention_heads
        self.max_position_embeddings = max_position_embeddings
        self.type_vocab_size = type_vocab_size
        self.pad_token_id = pad_token_id
        self.layer_norm_eps = layer_norm_eps
        self.hidden_dropout_prob = hidden_dropout_prob
        self.attention_probs_dropout_prob = attention_probs_dropout_prob


class BertEmbeddings(nn.Module):
    def __init__(self, config):
        super().__init__()
        self.hidden_size = config.hidden_size
        self.max_position_embeddings = config.max_position_embeddings
        self.word_embeddings = nn.Embedding(config.vocab_size, config.hidden_size, padding_idx=config.pad_token_id)
        self.position_embeddings = nn.Embedding(config.max_position_embeddings, config.hidden_size)
        self.token_type_embeddings = nn.Embedding(config.type_vocab_size, config.hidden_size)
        self.LayerNorm = nn.LayerNorm(config.hidden_size, eps=config.layer_norm_eps)
        self.dropout = nn.Dropout(config.hidden_dropout_prob)
        self.register_buffer("position_ids", torch.arange(config.max_position_embeddings).expand((1, -1)), persistent=False)

    def forward(self, input_ids, token_type_ids=None):
        batch_size, seq_length = input_ids.shape  #@dim batch_size: input_ids.shape[0]; seq_length: input_ids.shape[1]
        #@requires input_ids.dim() == 2
        buffered = self.position_ids  #@shape buffered: [1, self.max_position_embeddings]
        position_ids = self.position_ids[:, :seq_length]  #@shape position_ids: [1, min(input_ids.shape[1], self.max_position_embeddings)]
        #@shape@L161 position_ids: [1, 8]
        if token_type_ids is None:
            token_type_ids = torch.zeros_like(input_ids)  #@shape token_type_ids: [input_ids.shape[0], input_ids.shape[1]]
        inputs_embeds = self.word_embeddings(input_ids)  #@shape inputs_embeds: [input_ids.shape[0], input_ids.shape[1], self.hidden_size]
        #@requires none
        # token_type_ids joins [B, S] with a parameter of unknown shape: only the last dim is known
        token_type_embeddings = self.token_type_embeddings(token_type_ids)  #@shape token_type_embeddings: [*_, self.hidden_size]
        position_embeddings = self.position_embeddings(position_ids)  #@shape position_embeddings: [1, min(input_ids.shape[1], self.max_position_embeddings), self.hidden_size]
        #@requires none
        embeddings = inputs_embeds + token_type_embeddings + position_embeddings
        #@shape@L161 embeddings: [2, 8, 32]
        embeddings = self.LayerNorm(embeddings)  #@shape embeddings: [*_, self.hidden_size]
        #@requires none
        embeddings = self.dropout(embeddings)
        return embeddings


class BertSelfAttention(nn.Module):
    def __init__(self, config):
        super().__init__()
        if config.hidden_size % config.num_attention_heads != 0:
            raise ValueError("the hidden size is not a multiple of the number of attention heads")
        self.hidden_size = config.hidden_size
        self.num_attention_heads = config.num_attention_heads
        self.attention_head_size = config.hidden_size // config.num_attention_heads
        self.query = nn.Linear(config.hidden_size, config.hidden_size)
        self.key = nn.Linear(config.hidden_size, config.hidden_size)
        self.value = nn.Linear(config.hidden_size, config.hidden_size)
        self.dropout = nn.Dropout(config.attention_probs_dropout_prob)

    def transpose_for_scores(self, x):
        new_x_shape = x.size()[:-1] + (self.num_attention_heads, self.attention_head_size)  #@shape new_x_shape: [*x.shape[:-1], self.num_attention_heads, self.attention_head_size]
        x_heads = x.view(new_x_shape)  #@shape x_heads: [*x.shape[:-1], self.num_attention_heads, self.attention_head_size]
        return x_heads.permute(0, 2, 1, 3)

    def forward(self, hidden_states, attention_mask=None):
        batch_size, seq_len, _ = hidden_states.shape  #@dim seq_len: hidden_states.shape[1]
        #@requires hidden_states.dim() == 3
        mixed_query = self.query(hidden_states)  #@shape mixed_query: [hidden_states.shape[0], hidden_states.shape[1], self.hidden_size]
        #@requires hidden_states.shape[2] == self.hidden_size
        query_layer = self.transpose_for_scores(mixed_query)  #@shape query_layer: [hidden_states.shape[0], self.num_attention_heads, hidden_states.shape[1], self.attention_head_size]
        key_layer = self.transpose_for_scores(self.key(hidden_states))  #@shape key_layer: [hidden_states.shape[0], self.num_attention_heads, hidden_states.shape[1], self.attention_head_size]
        value_layer = self.transpose_for_scores(self.value(hidden_states))
        scores = torch.matmul(query_layer, key_layer.transpose(-1, -2))  #@shape scores: [hidden_states.shape[0], self.num_attention_heads, hidden_states.shape[1], hidden_states.shape[1]]
        #@requires none
        scores = scores / math.sqrt(self.attention_head_size)
        if attention_mask is not None:
            ext = attention_mask.view(batch_size, 1, 1, seq_len).to(scores.dtype)  #@shape ext: [hidden_states.shape[0], 1, 1, hidden_states.shape[1]]
            ext = (1.0 - ext) * torch.finfo(scores.dtype).min
            scores = scores + ext  #@shape scores: [hidden_states.shape[0], self.num_attention_heads, hidden_states.shape[1], hidden_states.shape[1]]
        probs = F.softmax(scores, dim=-1)  #@shape probs: [hidden_states.shape[0], self.num_attention_heads, hidden_states.shape[1], hidden_states.shape[1]]
        #@shape@L161 probs: [2, 4, 8, 8]
        probs = self.dropout(probs)
        context = torch.matmul(probs, value_layer)  #@shape context: [hidden_states.shape[0], self.num_attention_heads, hidden_states.shape[1], self.attention_head_size]
        #@requires none
        context = context.permute(0, 2, 1, 3).contiguous()  #@shape context: [hidden_states.shape[0], hidden_states.shape[1], self.num_attention_heads, self.attention_head_size]
        new_shape = context.size()[:-2] + (self.hidden_size,)  #@shape new_shape: [hidden_states.shape[0], hidden_states.shape[1], self.hidden_size]
        out = context.view(new_shape)  #@shape out: [hidden_states.shape[0], hidden_states.shape[1], self.hidden_size]
        #@shape@L161 out: [2, 8, 32]
        return out


class BertPooler(nn.Module):
    def __init__(self, config):
        super().__init__()
        self.hidden_size = config.hidden_size
        self.dense = nn.Linear(config.hidden_size, config.hidden_size)
        self.activation = nn.Tanh()

    def forward(self, hidden_states):
        assert hidden_states.dim() == 3
        first_token_tensor = hidden_states[:, 0]  #@shape first_token_tensor: [hidden_states.shape[0], hidden_states.shape[2]]
        #@requires none
        pooled = self.dense(first_token_tensor)  #@shape pooled: [hidden_states.shape[0], self.hidden_size]
        #@requires hidden_states.shape[2] == self.hidden_size
        pooled = self.activation(pooled)  #@shape pooled: [hidden_states.shape[0], self.hidden_size]
        return pooled


class TinyBert(nn.Module):
    """Embeddings, one self-attention layer and the pooler."""

    def __init__(self, config):
        super().__init__()
        self.embeddings = BertEmbeddings(config)
        self.attention = BertSelfAttention(config)
        self.pooler = BertPooler(config)

    def forward(self, input_ids, attention_mask=None, token_type_ids=None):
        emb = self.embeddings(input_ids, token_type_ids)
        hidden = self.attention(emb, attention_mask)
        pooled = self.pooler(hidden)
        return hidden, pooled


def mismatched_hidden(model):
    states = torch.randn(2, 8, 24)
    return model.attention.query(states)  #@shape-error Linear(32 -> 32) given last dim 24


if __name__ == "__main__":
    config = BertConfig(vocab_size=100, hidden_size=32, num_attention_heads=4, max_position_embeddings=16, type_vocab_size=2)
    model = TinyBert(config)
    input_ids = torch.randint(1, 100, (2, 8))  #@shape input_ids: [2, 8]
    attention_mask = torch.ones(2, 8, dtype=torch.long)
    attention_mask[1, 6:] = 0
    hidden, pooled = model(input_ids, attention_mask)  #@shape hidden: [2, 8, 32]; pooled: [2, 32]
    segments = torch.cat([torch.zeros(2, 4, dtype=torch.long), torch.ones(2, 4, dtype=torch.long)], dim=1)  #@shape segments: [2, 8]
    emb = model.embeddings(input_ids, segments)  #@shape emb: [2, 8, 32]
    short = model.embeddings(input_ids[:, :5])  #@shape short: [2, 5, 32]
    split = model.attention.transpose_for_scores(emb)  #@shape split: [2, 4, 8, 8]
    unmasked = model.attention(emb)  #@shape unmasked: [2, 8, 32]
    cls = model.pooler(unmasked)  #@shape cls: [2, 32]
    try:
        mismatched_hidden(model)
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass
