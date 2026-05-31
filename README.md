# SERAPH

**A geometric substrate for structured knowledge.**

SERAPH is a storage and retrieval engine that organizes content through emergent geometric structure rather than explicit schemas. It provides cryptographic provenance, semantic search, and chain-of-custody guarantees from genesis to tip.

---

## Quick Start

### 1. Download

Download the latest release for your platform from [Releases](https://github.com/Tree-D-Interactive/seraph-releases/releases).

Each release includes CPU, GPU, and ONNX library builds and the CLI, organized by platform:

```
seraph-<version>/
├── ffi/
│   ├── windows-x86_64/
│   │   ├── seraph.dll                     (CPU runtime)
│   │   └── seraph.dll.lib                 (MSVC import lib)
│   ├── windows-x86_64-gpu/
│   │   ├── seraph.dll                     (GPU runtime — candle CUDA)
│   │   └── seraph.dll.lib                 (MSVC import lib)
│   ├── windows-x86_64-onnx/
│   │   ├── seraph.dll                     (GPU runtime — ONNX Runtime CUDA EP)
│   │   ├── seraph.dll.lib                 (MSVC import lib)
│   │   ├── onnxruntime.dll                (ORT core runtime)
│   │   ├── onnxruntime_providers_cuda.dll (ORT CUDA execution provider)
│   │   └── onnxruntime_providers_shared.dll
│   ├── linux-x86_64/libseraph.so          (CPU)
│   ├── linux-x86_64-gpu/libseraph.so      (GPU — candle CUDA)
│   └── linux-x86_64-onnx/
│       ├── libseraph.so                   (GPU — ONNX Runtime CUDA EP)
│       └── libonnxruntime*.so             (ORT runtime libs)
├── python/
│   ├── windows-x86_64/seraph.pyd          (CPU)
│   ├── windows-x86_64-gpu/seraph.pyd      (GPU — candle)
│   ├── windows-x86_64-onnx/
│   │   ├── seraph.pyd                     (GPU — ONNX Runtime)
│   │   └── onnxruntime*.dll               (ORT runtime libs)
│   ├── linux-x86_64/seraph.so             (CPU)
│   ├── linux-x86_64-gpu/seraph.so         (GPU — candle)
│   └── linux-x86_64-onnx/
│       ├── seraph.so                      (GPU — ONNX Runtime)
│       └── libonnxruntime*.so             (ORT runtime libs)
└── cli/
    ├── windows-x86_64/seraph.exe
    └── linux-x86_64/seraph
```

Three compute backends are available. All expose the same API — only the encoder backend differs:

| Variant | Backend | Best for |
|---------|---------|----------|
| **CPU** | candle (pure Rust) | Containers, CI, machines without GPU |
| **GPU** | candle + CUDA | General GPU acceleration |
| **ONNX** | ONNX Runtime CUDA EP | Maximum encoder throughput (~3x faster than candle GPU) |

**ONNX builds are recommended** for production workloads with NVIDIA GPUs — they deliver significantly higher encode throughput. GPU (candle) builds are a lighter alternative without the ORT dependency. CPU builds are provided for containers and environments without GPU access.

### Compatibility Matrix

| Platform | CPU | GPU (candle) | ONNX | Notes |
|----------|:---:|:---:|:----:|-------|
| Windows x86_64 | ✓ | ✓ | ✓ | GPU/ONNX require NVIDIA driver ≥ 520 |
| Linux x86_64 | ✓ | ✓ | ✓ | GPU/ONNX require NVIDIA driver ≥ 520 |
| macOS arm64 | ✓ | — | — | Apple Silicon; no CUDA support |
| macOS x86_64 | ✓ | — | — | Intel Mac; no CUDA support |

**GPU requirements (candle and ONNX):**
- NVIDIA GPU with compute capability ≥ 7.0 (Volta or newer)
- NVIDIA driver ≥ 520 (CUDA 11.8+ runtime compatibility)
- No separate CUDA toolkit install needed — both GPU backends load CUDA at runtime via the driver

**Additional ONNX requirements:**
- cuDNN 9.x (the `cudnn64_9.dll` / `libcudnn.so.9` family) must be on the library search path — either next to the binary or in the CUDA toolkit `bin/` directory
- The bundled ORT DLLs must remain alongside `seraph.dll` / `seraph.pyd` — the engine loads them at runtime via `ORT_DYLIB_PATH` or adjacent-file discovery
- If the CUDA EP fails to initialize (missing cuDNN, incompatible driver), the ONNX build falls back to CPU inference automatically

**Versions locked in this release:**
- Candle 0.10 (pure-Rust ML inference — GPU builds)
- ONNX Runtime 1.26.0 with CUDA 13 EP (ONNX builds)
- cudarc 0.19.7 (CUDA runtime loading — supports CUDA 11.8 through 13.x)
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

The C ABI surface uses opaque handles, thread-local error reporting, and caller-owned buffers. See [FFI_API.md](FFI_API.md) for the full API reference.

#### Windows (MSVC)

The release ships `seraph.dll` and `seraph.dll.lib` (the MSVC import library). To link:

1. **Rename the import lib.** MSVC's linker expects `seraph.lib`, not `seraph.dll.lib`:

   ```
   copy seraph.dll.lib seraph.lib
   ```

2. **Set the library search path** and link:

   ```
   cl /I. your_app.c /link /LIBPATH:path\to\ffi seraph.lib
   ```

3. **Place `seraph.dll` where your executable can find it** — either next to the `.exe`, or on `PATH`. For GPU builds, the NVIDIA driver DLLs must also be loadable (they normally are if the driver is installed). For ONNX builds, keep the bundled `onnxruntime*.dll` files alongside `seraph.dll`.

> **CMake example:**
> ```cmake
> add_executable(myapp main.c)
> target_link_directories(myapp PRIVATE ${SERAPH_LIB_DIR})
> target_link_libraries(myapp seraph)
> # Copy DLL to output directory
> add_custom_command(TARGET myapp POST_BUILD
>     COMMAND ${CMAKE_COMMAND} -E copy_if_different
>         "${SERAPH_LIB_DIR}/seraph.dll" $<TARGET_FILE_DIR:myapp>)
> ```

#### Linux / macOS

```bash
# Compile and link
gcc -o myapp myapp.c -L/path/to/ffi -lseraph -Wl,-rpath,/path/to/ffi

# Or set LD_LIBRARY_PATH at runtime
export LD_LIBRARY_PATH=/path/to/ffi:$LD_LIBRARY_PATH
```

On macOS, use `DYLD_LIBRARY_PATH` or install to a system library path.

#### Example

```c
#include <stdio.h>

// Opaque handles — full signatures in FFI_API.md
extern void* seraph_store_open(const char* path);
extern void  seraph_store_close(void* handle);
extern const char* seraph_last_error(void);

int main() {
    void* store = seraph_store_open("my_store.sfg");
    if (!store) {
        fprintf(stderr, "Error: %s\n", seraph_last_error());
        return 1;
    }
    // ... use store ...
    seraph_store_close(store);
    return 0;
}
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
| Built-in encoder (candle / ONNX Runtime) | ✓ | ✓ | ✓ |
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
