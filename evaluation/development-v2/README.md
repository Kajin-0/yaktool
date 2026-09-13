# Hybrid V2 development corpus

This is a deterministic, gold-first 600-case development corpus for diagnosing and evaluating Hybrid V2. It is not a sealed holdout. Requests and `ModelIntent` gold records derive from the same semantic specification, with fixed seed `20260913`.

Do not use this corpus for final production claims. Once model evaluation begins, it is development evidence only. Future prompt or architecture changes require a fresh holdout.
