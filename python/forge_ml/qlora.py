"""Entrenamiento QLoRA con Unsloth (RF-03, RF-03b). Requiere el extra `[ml]` y GPU.

Perfil A: QLoRA 4-bit, gradient checkpointing de Unsloth y `paged_adamw_8bit`
(RF-03b). Perfil B: igual, con `device_map="auto"` para repartir el modelo
entre las dos GPUs (AGENTS.md 11.3). La pérdida solo cuenta los tokens de la
respuesta del asistente (el tool-call), no el system prompt ni el usuario.

Sin tests unitarios: se ejercita con `make train-lora` en hardware real
(AGENTS.md 11.11); por eso queda fuera de la cobertura de CI estándar.
"""

from __future__ import annotations

import logging
import random
from pathlib import Path
from typing import TYPE_CHECKING, Any

import numpy as np
import torch
import unsloth  # noqa: F401  (debe importarse antes que transformers/trl)
from datasets import load_dataset
from huggingface_hub import snapshot_download
from trl import SFTConfig, SFTTrainer
from unsloth import FastLanguageModel
from unsloth.chat_templates import train_on_responses_only

if TYPE_CHECKING:
    from forge_ml.train import TrainConfig

logger = logging.getLogger("forge_ml.qlora")

_TARGET_MODULES = ["q_proj", "k_proj", "v_proj", "o_proj", "gate_proj", "up_proj", "down_proj"]
# Marcadores de la plantilla de chat de Qwen2.5.
_USER_PART = "<|im_start|>user\n"
_ASSISTANT_PART = "<|im_start|>assistant\n"


def _seed_everything(seed: int) -> None:
    random.seed(seed)
    np.random.seed(seed)
    torch.manual_seed(seed)


def _load(cfg: TrainConfig) -> tuple[Any, Any]:
    # Se resuelve la revisión fijada a una ruta local: Unsloth no respeta
    # `revision` en su descarga previa y así el modelo base es exactamente el
    # que registra el manifest (RNF-09).
    local = snapshot_download(cfg.base_model, revision=cfg.revision)
    model, tokenizer = FastLanguageModel.from_pretrained(
        model_name=local,
        max_seq_length=cfg.max_seq_len,
        load_in_4bit=True,
        device_map="auto" if cfg.profile == "B" else "sequential",
    )
    model = FastLanguageModel.get_peft_model(
        model,
        r=cfg.lora_r,
        lora_alpha=cfg.lora_alpha,
        lora_dropout=0,
        bias="none",
        target_modules=_TARGET_MODULES,
        use_gradient_checkpointing="unsloth",
        random_state=cfg.seed,
    )
    return model, tokenizer


def _datasets(cfg: TrainConfig, tokenizer: Any) -> Any:
    files = {"train": str(cfg.dataset / "train.jsonl"), "eval": str(cfg.dataset / "eval.jsonl")}
    raw = load_dataset("json", data_files=files)

    def to_text(row: dict[str, Any]) -> dict[str, str]:
        return {"text": tokenizer.apply_chat_template(row["messages"], tokenize=False)}

    return raw.map(to_text, remove_columns=raw["train"].column_names)


def _trainer(cfg: TrainConfig, run_dir: Path, max_steps: int | None) -> Any:
    model, tokenizer = _load(cfg)
    data = _datasets(cfg, tokenizer)
    args = SFTConfig(
        output_dir=str(run_dir / "checkpoints"),
        per_device_train_batch_size=cfg.batch_size,
        gradient_accumulation_steps=cfg.grad_accum,
        num_train_epochs=cfg.epochs,
        max_steps=max_steps or -1,
        learning_rate=cfg.learning_rate,
        lr_scheduler_type="cosine",
        warmup_ratio=0.03,
        optim="paged_adamw_8bit",
        bf16=True,
        logging_steps=10,
        save_strategy="no",
        seed=cfg.seed,
        dataset_text_field="text",
        max_length=cfg.max_seq_len,
        report_to="none",
    )
    trainer = SFTTrainer(
        model=model,
        processing_class=tokenizer,
        train_dataset=data["train"],
        eval_dataset=data["eval"],
        args=args,
    )
    trainer = train_on_responses_only(
        trainer, instruction_part=_USER_PART, response_part=_ASSISTANT_PART
    )
    return trainer


def run(cfg: TrainConfig, run_dir: Path, max_steps: int | None) -> dict[str, float]:
    """Entrena, guarda el adaptador LoRA (safetensors) y devuelve métricas."""
    _seed_everything(cfg.seed)
    torch.cuda.reset_peak_memory_stats()
    trainer = _trainer(cfg, run_dir, max_steps)
    result = trainer.train()
    evaluation = trainer.evaluate()
    trainer.model.save_pretrained(str(run_dir / "adapter"))
    trainer.processing_class.save_pretrained(str(run_dir / "adapter"))
    peak = max(torch.cuda.max_memory_reserved(i) for i in range(torch.cuda.device_count()))
    metrics = {
        "train_loss": float(result.training_loss),
        "eval_loss": float(evaluation["eval_loss"]),
        "train_runtime_s": float(result.metrics["train_runtime"]),
        "steps": float(result.global_step),
        "peak_vram_mib_per_gpu": peak / 2**20,
    }
    torch.cuda.empty_cache()
    logger.info("métricas: %s", metrics)
    return metrics
