# Corrected offline analysis

The repaired scorer computes gold-move denominators and semantic load from the
gold intent (not class names). On the 480-case corpus, baseline A is 33.96%
exact with 143/440 correct accepted moves (32.50% recall); structured B is
38.12% exact with 163/440 (37.05%). Both have 0 unsafe and 0 wrong-slot
accepted mutations. Pre-gate recovery E* and deterministic-first replay H/F
cannot recover additional cases from the saved B outputs: the dominant failures
are action/slot omissions before a valid move reaches the gate.

The 600-case rerun is 62.67% exact (81% straightforward, 83% paraphrased,
10.67% constraint-rich, 50% read-only), with 180/650 correct accepted moves,
100% precision, and zero mutation errors. The historical 75% read-only figure
used a different full-V2 evaluator; the old runner's parser and route handling
were not identical, so those figures are not directly comparable.

No additional inference was performed. Variant G was intentionally deferred:
raw action accuracy must be measured from model outputs before paying for a
two-pass benchmark. No candidate met the capability targets; no architecture
was promoted and no sealed V2 holdout was created.
