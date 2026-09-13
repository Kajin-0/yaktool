# YakTool Hybrid V1 Holdout

This directory contains the independently generated, cryptographically sealed
Hybrid V1 holdout. It has exactly 1200 cases and is generated gold-first: one
canonical semantic specification is rendered into both the request and the gold
ModelIntent. `validate.py` mechanically checks that grounding relationship.

> **HOLDOUT V1 MUST NOT BE USED FOR PROMPT TUNING, HYBRID-GATE TUNING, GOLD EDITING, OR CASE-SPECIFIC FIXES.**

Once any model is evaluated against this corpus, it becomes evaluation evidence.
If the architecture changes based on observed failures, create a future holdout
version instead of editing this one. Regeneration of the sealed corpus is
prohibited after this commit.

The fixed generator seed is `20260913`. Running `generate.py` with the same
source and seed produces byte-identical `corpus.jsonl`; it refuses accidental
overwrite unless `--regenerate` is explicitly supplied.

Planned post-sealing candidates are `qwen35-08b-text-q5:latest` (smallest
corrected-development candidate) and `llama3.2:3b` (highest corrected mutation
recall among practical small models). They are not run as part of holdout
creation.

Zero observed mutation errors does not prove zero true error probability. For
`n` accepted mutation cases with zero observed failures, the approximate 95%
upper error-rate bound is the rule of three: `3 / n`. Future reports must include
accepted mutation count, observed errors, precision point estimate, and this
bound when applicable; a ≥99% claim requires a sufficiently large sample.

The development corpus and its correction sidecar are historical evidence only.
