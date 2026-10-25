# SERAPH

**A geometric substrate for structured knowledge.**

SERAPH is a storage and retrieval engine that organizes content through emergent geometric structure rather than explicit schemas. It provides cryptographic provenance, semantic search, and chain-of-custody guarantees from genesis to tip.

**0.4.1**

- **Clearer license messages.** When a license needs replacing, SERAPH says so and how: run `seraph license activate`, or download it again from your account.
- **`seraph license status` tells you whether your license will work**, and exits non-zero when it won't. `seraph license activate` replaces a license that won't work instead of declining to run, and `seraph license install` warns when the license you installed won't be accepted.

**0.4.0**

- **One call per operation, every option in it.** `put`, `search`, `chain`, `provenance`, `influence`, `history`, `neighborhood`, `open` and `migrate_store` each take their options in one call (keyword arguments in Python, an options struct in C, an options type in Rust), and the options combine freely: a search can take a time window, a region, a warp and a trace at once. The earlier variants (`put_with_metadata`, `search_with_warp`, `chain_between`, the `set_*` tuning calls, ...) keep working and are marked deprecated.
- **Bulk ingest carries everything.** Each `put_batch_incremental` item takes metadata, an event time and a region, so a bulk load no longer needs a per-frame loop; a batch that stops partway reports what it committed.
- **Two times, kept apart.** A frame records when it was committed, the engine's own time, and can record when its content happened (`occurred_at`), the caller's claim, bound into the frame's record. Search and `history` take a window on each: `occurred` for when the content happened, `committed` for what the store held at a moment. Routes and strands take `as_of`. Results carry both times.
- **Reads of the past return what the store answered then.** A read of the store as it stood at a moment now uses the store's structure at that moment, not today's. `lifecycle(frame_id)` reports when a frame was committed, when it became a door and when it was retired.
- **Runtime settings in one place.** `tuning` holds every runtime setting; the full traversal is a search mode, with no switching by store size.
- **Stricter input.** Malformed metadata, unknown warp profiles, relation names and configuration keys are errors instead of being ignored. Python errors are `SeraphError` subclasses (still `RuntimeError`s), and a `SeraphStore` can be shared between threads.
- **C API.** New canonical functions (`seraph_put`, `seraph_search`, `seraph_chain`, ...) take versioned options structs and return status codes; every call checks its output pointers.
- **`migrate_store` carries history and takes seeds.** A rebuilt store keeps each source frame's original time as its event time; `seed_data` creates seeds before the first frame.
- **Fixes.** Route measures are exact for near-identical frames. A store created with one redirect per position reopened with one per frame. A snapshot dropped a frame retired with `supersede` even before it was retired. Submissions through `SubmissionAgent` commit in submission order. A store closed through the C API, or dropped in Rust or Python without `close()`, now keeps every frame it committed; frames from a batch that had not been synced could be lost before.

> **Upgrading from 0.3.x?** Stores created by 0.4.0 use frame format 6; open them
> with 0.4.0 or later. Stores from earlier releases open unchanged, and
> `migrate_store` carries them to format 6. Deprecated calls emit warnings in Python
> and are marked `#[deprecated]` in Rust. Input that was silently ignored is now an
> error. If SERAPH reports that your license needs replacing, run
> `seraph license activate` or download it again from your account.

**0.3.1**

- **Ingest is 6 to 10× faster on large stores.** Placement now stops on the same line search does (`set_commit_ceiling_slack`, default 0.05; `0` restores the previous behaviour), so its cost tracks the neighbourhood rather than the store.
- **Search uses the bounded walk at every store size.** Small stores answer in well under a millisecond where they could take tens of milliseconds before; `set_search_use_stream(False)` keeps the full traversal.
- **`search_with_trace` reports the walk `search` actually runs**, and `last_commit_visits` reports what the last placement examined.

**New in 0.3.0**

- **Vector precision is yours.** Choose at `create` what each frame persists — the calibration-free 4-bit codes (the default), or `fp32`, `fp16`, `bf16`, `fp64` — and separately what search scores from (`resident_precision`: `fp32`, `fp16`, `bf16`, or none), switchable at runtime. A half-precision resident halves search memory at the same quality. Stores from every earlier release open unchanged.
- **Reads that explain the record.** `history` answers how the store came to hold what a query finds; `provenance` and `influence` follow what a frame drew on and what later drew on it; `connections` lists the distinct ways two frames connect; `chain_forward` shows how A led to B. Each has a warp-steered variant, and `chain` now returns the shortest route over committed edges (a frame's own ancestry is `lineage`).
- **Evidence.** The frame id, commit time and committed neighbours are bound into every watermark, and seals and anchors record a digest over commit order that `verify` checks — reordering, removing or inserting frames under a seal or anchor is caught. New: `seal_with_anchor`, `append_tip_anchor`, `anchor_claims`, `order_digest`.
- **As-of reads.** A time window's `before` end reads the store as it stood, so a historical query is as fast and as complete as a current one.
- **Faster at scale.** Search on large stores returns in a fraction of the time, ingest does less work per frame, and opening a store no longer grows its `.gidx` sidecar.
- **Windows GPU builds run on any NVIDIA GPU from Turing up** (RTX 20-series, T4 and newer). Earlier Windows releases loaded their GPU kernels only on compute 8.9 parts (RTX 40-series); the build now pins and verifies the target on every release.
- **Full C and Rust parity.** The C ABI (175 functions) and the Rust crate reach every engine operation, including streamed and traced search, subtrees, and warp-steered chain and neighborhood reads. All three API references are generated from a single declaration of each operation, so the surfaces cannot drift.

> **Upgrading from 0.2.x?** Stores created by 0.3.0 use frame format 5 and need
> 0.3.0 or later to open; stores from earlier releases open unchanged. The default
> search stop is tighter (`search_ceiling_slack` 0.05, was 0.10) — call
> `set_search_ceiling_slack(0.10)` for the previous completeness setting. Seeds take
> no similarity edges in new stores, and `put` accepts `seed_term`.

**Foundations**

- **TurboQuant storage** — 4-bit codes are the canonical embedding, so stores are **~4× smaller** (up to ~8× on short content) with **sub-millisecond search** at scale (with the SIMD embedding arena enabled — see note below).
- **Bring-your-own encoder, zero setup** — any embedding model works out of the box with **no calibration** and nothing per-encoder to configure or persist. The quantizer is universal, so stores are self-describing and portable across machines.

> **On the sub-millisecond figure.** Search scores from a contiguous, pre-normalized
> **SIMD embedding arena**, on by default. A 900k-frame store answers text queries in
> **~0.7–0.9 ms** at the operating threshold. `set_search_arena(False)` gives the
> arena's memory back (N × D floats) for 20–40% more per query with identical
> results; `set_resident_precision("fp16")` keeps the speed at half the memory.

---

## Quick Start

### 1. Download

Download the latest release for your platform from [Releases](https://github.com/Tree-D-Interactive-LLC/seraph/releases).

Each release includes GPU and ONNX library builds and the CLI, organized by platform:

```
seraph-<version>/
├── ffi/
│   ├── windows-x86_64/
│   │   ├── seraph.dll                     (GPU candle, auto CPU fallback)
│   │   └── seraph.dll.lib                 (MSVC import lib)
│   ├── windows-x86_64-onnx/
│   │   ├── seraph.dll                     (ONNX Runtime CUDA EP)
│   │   └── seraph.dll.lib                 (MSVC import lib)
│   ├── linux-x86_64/libseraph.so          (GPU candle, auto CPU fallback)
│   └── linux-x86_64-onnx/libseraph.so     (ONNX Runtime CUDA EP)
├── python/
│   ├── windows-x86_64/seraph.pyd          (GPU candle, auto CPU fallback)
│   ├── windows-x86_64-onnx/seraph.pyd     (ONNX Runtime — ORT via pip)
│   ├── linux-x86_64/seraph.so             (GPU candle, auto CPU fallback)
│   └── linux-x86_64-onnx/seraph.so        (ONNX Runtime — ORT via pip)
└── cli/
    ├── windows-x86_64/seraph.exe
    └── linux-x86_64/seraph
```

Two compute backends are available. Both expose the same API — only the encoder backend differs. All builds include automatic CPU fallback when no GPU is available.

| Variant | Backend | Best for |
|---------|---------|----------|
| **GPU** | candle (pure Rust) + CUDA | General use. Zero native deps. Works everywhere. |
| **ONNX** | ONNX Runtime CUDA EP | Maximum encoder throughput (~3x faster than candle GPU) |

**ONNX builds are recommended** for production workloads with NVIDIA GPUs — they deliver significantly higher encode throughput. GPU (candle) builds are a lighter alternative without the ORT dependency. Both fall back to CPU automatically when no GPU is present.

### Compatibility Matrix

Prebuilt binaries and wheels in this release cover **Windows** and **Linux x86_64**.

| Platform | GPU (candle) | ONNX | Notes |
|----------|:---:|:----:|-------|
| Windows x86_64 | ✓ | ✓ | GPU requires NVIDIA driver ≥ 580 and compute capability ≥ 7.5 (Turing) |
| Linux x86_64 | ✓ | ✓ | GPU requires NVIDIA driver ≥ 560 and compute capability ≥ 7.0 (Volta) |

> macOS is not shipped as a prebuilt binary or wheel in this release — build from source (candle, CPU only).

**GPU requirements (candle and ONNX):**
- NVIDIA GPU with compute capability ≥ 7.0 (Volta or newer) for Linux builds, and
  ≥ 7.5 (Turing — RTX 20-series, T4 — or newer) for Windows builds, whose CUDA 13
  toolkit no longer targets Volta
- NVIDIA driver ≥ 580 for Windows builds, ≥ 560 for Linux builds. The two are
  compiled with different CUDA toolkits (13.1 and 12.6), and the PTX ISA version
  that produces sets the driver floor
- **CUDA toolkit is optional, but it is what enables the GPU.** The binary loads
  without it — CUDA libraries are resolved lazily, so a machine with no CUDA at
  all starts normally and runs on CPU. Actually *using* the GPU needs the toolkit
  runtime libraries (`cudart`, `cublas`, `curand`), because candle creates a cuBLAS
  handle when it opens the device. Driver-only machines fall back to CPU

**ONNX requirements:**
- ONNX Runtime 1.19+ with CUDA EP — download from [ORT releases](https://github.com/microsoft/onnxruntime/releases), or install via vcpkg/nuget/apt
- Place ORT libraries on your library search path, next to your binary, or set `ORT_DYLIB_PATH`
- cuDNN 9.x (`cudnn64_9.dll` / `libcudnn.so.9`) for GPU acceleration
- If the CUDA EP fails to initialize, encoding falls back to CPU automatically

**ONNX Python builds** use `onnxruntime-gpu` as a pip dependency — install via `pip install seraph-db-onnx` and pip handles ORT automatically.

**Versions locked in this release:**
- Candle 0.10 (pure-Rust ML inference — GPU builds), patched so CUDA is loaded
  lazily rather than at image load; this is what makes the CPU fallback reachable
- ONNX Runtime 1.19+ with CUDA EP (ONNX builds)
- cudarc 0.19.7 (lazy CUDA loading) — detects CUDA runtimes 10.x through 13.x
- Python 3.8+ (abi3 wheels — `cp38-abi3`; built with 3.13)

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

### Python (pip)

The recommended way to install SERAPH for Python:

```bash
# Standard build (candle GPU, auto CPU fallback)
pip install seraph-db

# ONNX build (maximum throughput, auto-installs onnxruntime-gpu)
pip install seraph-db-onnx
```

Then import directly:

```python
import seraph
```

> **Note:** The Python module and the C FFI library are separate builds from the same codebase. Use pip for Python projects; use the FFI release for C, C++, Rust, or any language with a C FFI.

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

3. **Place `seraph.dll` where your executable can find it** — either next to the `.exe`, or on `PATH`. For ONNX builds, also ensure `onnxruntime.dll` is discoverable (next to your app, on PATH, or via `ORT_DYLIB_PATH`).

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
| TurboQuant compact storage (4-bit codes, ~4× smaller) | ✓ | ✓ | ✓ |
| Sub-millisecond search at scale (SIMD arena) | ✓ | ✓ | ✓ |
| Batch operations | ✓ | ✓ | ✓ |
| **Graph & Topology** | | | |
| Eigenframe promotion | ✓ | ✓ | ✓ |
| Seeds & seed bundles | ✓ | ✓ | ✓ |
| Traversal & path chains | ✓ | ✓ | ✓ |
| Neighborhood queries | ✓ | ✓ | ✓ |
| Typed structural edges | ✓ | ✓ | ✓ |
| Federation (route & merge across stores) | ✓ | ✓ | ✓ |
| **Intelligence** | | | |
| Contradiction detection | ✓ | ✓ | ✓ |
| Consolidation (geometric / semantic) | ✓ | ✓ | ✓ |
| Temporal snapshots | ✓ | ✓ | ✓ |
| Warp (query steering) | ✓ | ✓ | ✓ |
| Analysis & metrics | ✓ | ✓ | ✓ |
| **Encoding** | | | |
| Built-in encoders — bge-small (384d) / bge-base (768d) | ✓ | ✓ | ✓ |
| Encode-and-put / encode-and-search | ✓ | ✓ | ✓ |
| Bring-your-own encoder (any model, no calibration) | ✓ | ✓ | ✓ |
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

- [Product overview](https://www.seraph-db.com)


---

## Support

- **Free & Commercial:** GitHub Issues on this repository
- **Enterprise:** Direct support channel per agreement

---

Copyright (c) 2025-2026 Tree D Interactive LLC. All rights reserved.
