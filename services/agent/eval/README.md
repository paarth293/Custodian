# Agent evaluation and diagnostics

These developer entry points belong to the agent service. Build the service
before running diagnostics that import its `dist/` modules. Real inference
requires separately fetched local models; generated results belong in ignored
`artifacts/` or `target/`, not beside these scripts.

- `run.mjs`: frozen tool-use evaluation.
- `intent-planning.mjs` and `workflow-planning.mjs`: bounded planner evaluation.
- `runtime-startup.mjs`: pinned runtime startup and response diagnostic.
- `chat-quality.mjs`: factual regression diagnostic; semantic review is still required.
- `inference-benchmark.mjs`: the former root `test_inference.mjs`, relocated here.
  It measures raw model loading and token latency using the same repository
  model path as before. It is a developer benchmark, not a sandbox, product
  acceptance check or proof of factual correctness.

Run the relocated benchmark from the repository root:

```sh
node services/agent/eval/inference-benchmark.mjs
```

The original root filename had no application, test, CI or script consumers.
No runtime entry point or supported product route changed during this move.
