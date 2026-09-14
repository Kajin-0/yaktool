# Hybrid V2 constraint-load development corpus

Gold-first, deterministic 480-case corpus for development only. Semantic load is recomputed from the expected ModelIntent: source, destination, category, age tuple, and size tuple count as five independent groups. This corpus is not a sealed holdout and must not be used to tune against individual failures after evaluation.

Seed: 20260915. Run `python3 generate.py` then `python3 validate.py` to reproduce and validate it.
