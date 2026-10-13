"""Driver for the timm benchmark: a short fine-tuning loop as a timm user writes it (models
from the registry, data config and transforms, optimizer and scheduler factories, mixup,
label smoothing, EMA, accuracy), plus inference with a few other architectures."""

import torch
import torch.nn.functional as F

import timm
from timm.data import Mixup, create_transform, resolve_data_config
from timm.loss import LabelSmoothingCrossEntropy, SoftTargetCrossEntropy
from timm.optim import create_optimizer_v2
from timm.scheduler import create_scheduler_v2
from timm.utils import AverageMeter, ModelEmaV3, accuracy

NUM_CLASSES = 10
BATCH = 4

model = timm.create_model("resnet18", pretrained=False, num_classes=NUM_CLASSES)
config = resolve_data_config({}, model=model)
train_tf = create_transform(**config, is_training=True, auto_augment="rand-m9-mstd0.5")
eval_tf = create_transform(**config)

mixup = Mixup(mixup_alpha=0.8, cutmix_alpha=1.0, label_smoothing=0.1, num_classes=NUM_CLASSES)
train_loss = SoftTargetCrossEntropy()
eval_loss = LabelSmoothingCrossEntropy(smoothing=0.1)

optimizer = create_optimizer_v2(model, opt="adamw", lr=1e-3, weight_decay=0.05)
scheduler, epochs = create_scheduler_v2(optimizer, sched="cosine", num_epochs=3, warmup_epochs=1)
ema = ModelEmaV3(model, decay=0.999)

size = config["input_size"]
losses = AverageMeter()
top1 = AverageMeter()
for epoch in range(epochs):
    model.train()
    for step in range(2):
        images = torch.randn(BATCH, *size)
        target = torch.randint(0, NUM_CLASSES, (BATCH,))
        images, soft = mixup(images, target)
        logits = model(images)
        loss = train_loss(logits, soft)
        optimizer.zero_grad()
        loss.backward()
        optimizer.step()
        ema.update(model, step=epoch * 2 + step)
        losses.update(loss.item(), images.size(0))
    scheduler.step(epoch + 1)

    model.eval()
    with torch.no_grad():
        images = torch.randn(BATCH, *size)
        target = torch.randint(0, NUM_CLASSES, (BATCH,))
        logits = ema.module(images)
        val = eval_loss(logits, target)
        acc1, acc5 = accuracy(logits, target, topk=(1, 5))
        top1.update(acc1.item(), BATCH)

features = {}
for name in ("vit_tiny_patch16_224", "convnext_atto", "efficientnet_b0", "mobilenetv3_small_100", "swin_tiny_patch4_window7_224"):
    net = timm.create_model(name, pretrained=False, num_classes=0)
    net.eval()
    cfg = resolve_data_config({}, model=net)
    x = torch.randn(1, *cfg["input_size"])
    with torch.no_grad():
        pooled = net(x)
        fmap = net.forward_features(x)
    features[name] = (tuple(pooled.shape), tuple(fmap.shape), net.num_features)

backbone = timm.create_model("resnet18", pretrained=False, features_only=True, out_indices=(1, 2, 3))
pyramid = [tuple(f.shape) for f in backbone(torch.randn(1, 3, 224, 224))]
probs = F.softmax(model(torch.randn(1, *size)), dim=-1)
summary = {
    "loss": losses.avg,
    "top1": top1.avg,
    "lr": optimizer.param_groups[0]["lr"],
    "features": features,
    "pyramid": pyramid,
    "channels": backbone.feature_info.channels(),
    "argmax": int(probs.argmax()),
    "models": len(timm.list_models("resnet*")),
}
print(summary)
