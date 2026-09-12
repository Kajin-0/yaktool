# Future interpreter evaluation

No model, inference server, benchmark downloader, or Python runtime is implemented in V0.1.

Future work can compare constrained local interpreters against canonical `yaktool.intent.v1` output, using supported and refused requests from the current interpretation tests. Evaluation must measure unsupported/ambiguous refusal as well as successful slot extraction. A model's output remains untrusted and must pass the same resolver, static policy, frozen-plan confirmation, and execution checks. Model evaluation never grants shell or direct filesystem mutation access.
