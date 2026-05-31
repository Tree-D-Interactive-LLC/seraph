# SERAPH

**A geometric substrate for structured knowledge.**

SERAPH is a storage and retrieval engine that organizes content through emergent geometric structure rather than explicit schemas. It provides cryptographic provenance, semantic search, and chain-of-custody guarantees from genesis to tip.

---

## Quick Start

### 1. Download

Download the latest release for your platform from [Releases](https://github.com/Tree-D-Interactive/seraph-releases/releases).

Each release includes two library builds and the CLI, organized by platform:

```
seraph-<version>/
├── ffi/
│   ├── windows-x86_64/seraph.dll
│   └── linux-x86_64/libseraph.so
├── python/
│   ├── windows-x86_64/seraph.pyd
│   └── linux-x86_64/seraph.so
└── cli/
    ├── windows-x86_64/seraph.exe
    └── linux-x86_64/seraph
```

Pick the library that matches your language and platform. Both library builds come from the same Rust codebase — the Python build includes PyO3 bindings, the C/FFI build does not.

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
| **Commercial** | Annual revenue $500K+ | $5,000/year | Production deployment on your own infrastructure. |
| **Enterprise** | Embedded redistribution, third-party deployment, bespoke arrangements | [Contact us](mailto:licensing@tree-d-interactive.net) | Per Order Form. |

The boundary between Commercial and Enterprise is where the SERAPH binary runs at runtime. If it runs on **your** infrastructure serving your end users, Commercial applies. If the binary ships to or runs on third-party systems, Enterprise with embedded-redistribution scope is required.

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
