# SERAPH

**A geometric substrate for structured knowledge.**

SERAPH is a storage and retrieval engine that organizes content through emergent geometric structure rather than explicit schemas. It provides cryptographic provenance, semantic search, and chain-of-custody guarantees from genesis to tip.

---

## Quick Start

### 1. Download

Download the latest release for your platform from [Releases](https://github.com/Tree-D-Interactive/seraph-releases/releases).

Each release includes CPU and GPU library builds and the CLI, organized by platform:

```
seraph-<version>/
├── ffi/
│   ├── windows-x86_64/seraph.dll          (CPU)
│   ├── windows-x86_64-gpu/seraph.dll      (GPU)
│   ├── linux-x86_64/libseraph.so          (CPU)
│   └── linux-x86_64-gpu/libseraph.so      (GPU)
├── python/
│   ├── windows-x86_64/seraph.pyd          (CPU)
│   ├── windows-x86_64-gpu/seraph.pyd      (GPU)
│   ├── linux-x86_64/seraph.so             (CPU)
│   └── linux-x86_64-gpu/seraph.so         (GPU)
└── cli/
    ├── windows-x86_64/seraph.exe
    └── linux-x86_64/seraph
```

Pick the library that matches your language and platform. **GPU builds are recommended** when a CUDA-capable GPU is available — they accelerate embedding and eigenframe scanning. CPU builds are provided for containers and environments without GPU access. Both variants expose the same API; the only difference is the compute backend.

### Compatibility Matrix

| Platform | CPU | GPU | Notes |
|----------|:---:|:---:|-------|
| Windows x86_64 | ✓ | ✓ | GPU requires NVIDIA driver ≥ 520 |
| Linux x86_64 | ✓ | ✓ | GPU requires NVIDIA driver ≥ 520 |
| macOS arm64 | ✓ | — | Apple Silicon; no CUDA support |
| macOS x86_64 | ✓ | — | Intel Mac; no CUDA support |

**GPU requirements:**
- NVIDIA GPU with compute capability ≥ 7.0 (Volta or newer)
- NVIDIA driver ≥ 520 (CUDA 11.8+ runtime compatibility)
- No separate CUDA toolkit install needed — the GPU build loads `libcudart` at runtime via the driver

**Versions locked in this release:**
- Candle 0.10 (pure-Rust ML inference)
- cudarc 0.19.7 (CUDA runtime loading — supports CUDA 11.8 through 12.x)
- Python 3.13 (PyO3 bindings)

### 2. Activate Your License

A license is required before the engine will accept operations. The CLI handles activation:

```bash
# Browser-based (opens your default browser)
seraph license activate

# Headless (email verification with 6-digit code)
seraph license activate --email you@company.com
```

Once activated, the license is stored locally and the engine is ready to use.

### 3. Verify

```bash
seraph license status
```

---

## Installation

### Python (PyO3)

Download the Python module (`seraph.pyd` on Windows, `seraph.so` on Linux/macOS) and place it where Python can find it:

```bash
# Copy into your project directory, or into site-packages:
cp seraph.pyd .   # Windows
cp seraph.so  .   # Linux / macOS
```

Then import directly:

```python
import seraph
```

> **Note:** The Python module and the C FFI library are separate downloads built from the same codebase. Use the Python module for Python projects; use the C FFI library for C, C++, Rust, or any language with a C FFI.

### C / C++ (FFI)

Link against the shared library. The C ABI surface uses opaque handles, thread-local error reporting, and caller-owned buffers:

```c
// Link: -lseraph (Linux/macOS) or seraph.lib (Windows MSVC)

// Example: open a store
void* handle = NULL;
int rc = seraph_store_open("/path/to/store.sfg", &handle);
if (rc != 0) {
    const char* err = seraph_last_error();
    // handle error
}

// ... use handle ...

seraph_store_close(handle);
```

A generated `seraph.h` header will be included in future releases.

### CLI

Add the binary to your `PATH`:

```bash
# Linux / macOS
sudo cp seraph /usr/local/bin/

# Windows - copy seraph.exe to a directory on your PATH
```

Run `seraph --help` for usage.

---

## CLI Reference

```
seraph license activate              Activate a free license (opens browser)
seraph license activate --email E    Headless activation via email verification
seraph license install [path]        Install a license file
seraph license status                Show current license details
```

---

## Licensing

SERAPH is proprietary software licensed by [Tree D Interactive LLC](https://tree-d-interactive.net).

| Tier | Eligibility | Fee | Scope |
|------|------------|-----|-------|
| **Free** | Annual revenue under $500K | $0 | Development, evaluation, prototyping, non-commercial research. No production deployment. |
| **Commercial** | Annual revenue $500K+ | $10,000/year | Production deployment on your own infrastructure. No multi-tenant SaaS, white-label, or redistribution. |
| **Enterprise** | Multi-tenant SaaS, embedded redistribution, third-party deployment | [Contact us](mailto:licensing@tree-d-interactive.net) | Per Order Form. |

The boundary between Commercial and Enterprise is the **location and control of the SERAPH binary at runtime**. If it runs exclusively on infrastructure you fully control, serving your own users, Commercial applies. Multi-tenant SaaS, redistribution, or any scenario where the binary runs outside your direct control requires Enterprise.

### Feature availability by tier

| Feature | Free | Commercial | Enterprise |
|---------|:----:|:----------:|:----------:|
| **Store Lifecycle** | | | |
| Create, open, close stores | ✓ | ✓ | ✓ |
| Append-only frame log with WAL | ✓ | ✓ | ✓ |
| Watermark chain verification | ✓ | ✓ | ✓ |
| Open & read sealed stores | ✓ | ✓ | ✓ |
| **Ingestion & Search** | | | |
| Frame ingestion (`put`) | ✓ | ✓ | ✓ |
| Semantic search (two-phase eigenframe + traversal) | ✓ | ✓ | ✓ |
| Tiered pre-filter (coarse / medium / auto) | ✓ | ✓ | ✓ |
| Batch operations | ✓ | ✓ | ✓ |
| **Graph & Topology** | | | |
| Eigenframe promotion | ✓ | ✓ | ✓ |
| Seeds & seed bundles | ✓ | ✓ | ✓ |
| Traversal & path chains | ✓ | ✓ | ✓ |
| Neighborhood queries | ✓ | ✓ | ✓ |
| Typed structural edges | ✓ | ✓ | ✓ |
| **Intelligence** | | | |
| Contradiction detection | ✓ | ✓ | ✓ |
| Consolidation (geometric / semantic) | ✓ | ✓ | ✓ |
| Temporal snapshots | ✓ | ✓ | ✓ |
| Warp (query steering) | ✓ | ✓ | ✓ |
| Analysis & metrics | ✓ | ✓ | ✓ |
| **Encoding** | | | |
| Built-in encoder (candle) | ✓ | ✓ | ✓ |
| Encode-and-put / encode-and-search | ✓ | ✓ | ✓ |
| **Events & Observability** | | | |
| Event subscriptions | ✓ | ✓ | ✓ |
| Writer audit / provenance | ✓ | ✓ | ✓ |
| **Security — Commercial** | | | |
| Encryption at rest (AES-256-GCM) | — | ✓ | ✓ |
| Store sealing (tamper-evident read-only) | — | ✓ | ✓ |
| Anchoring (external timestamping) | — | ✓ | ✓ |
| Standard secure mode (signed frames) | — | ✓ | ✓ |
| **Security — Enterprise** | | | |
| Gov secure mode (classification markings) | — | — | ✓ |

See the [License Agreement](https://www.seraph-db.com/license-agreement) and [Pricing Schedule](https://www.seraph-db.com/pricing) for full terms.

For licensing inquiries: **licensing@tree-d-interactive.net**

---

## Documentation

- [Product overview](https://seraph-db.com)
- [API Reference](https://seraph-db.com/docs)

---

## Support

- **Free & Commercial:** GitHub Issues on this repository
- **Enterprise:** Direct support channel per agreement

---

Copyright (c) 2025-2026 Tree D Interactive LLC. All rights reserved.
