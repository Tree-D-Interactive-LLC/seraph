# SERAPH

**A geometric substrate for structured knowledge.**

SERAPH is a storage and retrieval engine that organizes content through emergent geometric structure rather than explicit schemas. It provides cryptographic provenance, semantic search, and chain-of-custody guarantees from genesis to tip.

---

## Installation

### Python Package (pure Python core)

```bash
pip install https://github.com/Tree-D-Interactive/seraph/releases/download/v1.0.0/seraph-1.0.0-py3-none-any.whl
```

### Rust-Accelerated Runtime

The `seraph-rs` wheel provides native embedding, ONNX inference, and GPU-accelerated indexing via PyO3 bindings.

**Linux (x86_64, Python 3.13):**
```bash
pip install https://github.com/Tree-D-Interactive/seraph/releases/download/v1.0.0/seraph_rs-0.1.0-cp313-cp313-manylinux_2_17_x86_64.manylinux2014_x86_64.whl
```

**Windows (x86_64, Python 3.13):**
```bash
pip install https://github.com/Tree-D-Interactive/seraph/releases/download/v1.0.0/seraph_rs-0.1.0-cp313-cp313-win_amd64.whl
```

### Combined Install

Install both packages — `seraph-rs` is optional but recommended for production:

```bash
pip install \
  https://github.com/Tree-D-Interactive/seraph/releases/download/v1.0.0/seraph-1.0.0-py3-none-any.whl \
  https://github.com/Tree-D-Interactive/seraph/releases/download/v1.0.0/seraph_rs-0.1.0-cp313-cp313-manylinux_2_17_x86_64.manylinux2014_x86_64.whl
```

---

## Licensing

SERAPH is proprietary software licensed by [Tree D Interactive LLC](https://tree-d-interactive.net).

| Tier | Eligibility | Fee |
|------|------------|-----|
| **Free** | Annual revenue under $500K | $0 |
| **Commercial** | Annual revenue $500K+ | $5,000/year |
| **Enterprise** | Custom scope, Gov mode, redistribution | Contact us |

A license file is required for secure-mode operation. See [SERAPH Software License Agreement](LICENSE.md) for full terms.

For licensing inquiries: **licensing@tree-d-interactive.net**

---

## Documentation

- [Product overview](https://tree-d-interactive.net)
- [API Reference](https://tree-d-interactive.net/docs)

---

## Support

- **Free & Commercial:** GitHub Issues on this repository
- **Enterprise:** Direct support channel per agreement

---

Copyright (c) 2025-2026 Tree D Interactive LLC. All rights reserved.
