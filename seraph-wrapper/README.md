# seraph — Rust API for the SERAPH engine

The idiomatic Rust interface to SERAPH (Deterministic Semantic Geometry). This
crate is **binding glue only** — safe `Store` / `Encoder` / `Federation` types
over the compiled engine's C ABI. It contains no engine logic; the engine ships
as a precompiled cdylib.

See `RUST_API.md` for the full API reference.

## Layout

- `src/lib.rs` — the safe wrapper (the only source you consume).
- `build.rs` — points the linker at the engine cdylib via `SERAPH_LIB_DIR`.
- `RUST_API.md` — generated API reference.

## Using it

Rust has no stable binary library ABI, so this crate is consumed as source. You
supply the matching engine cdylib for your platform — download the
`seraph-<ver>-ffi-<platform>-gpu` (or `-onnx`) archive from the same release and
unpack it somewhere; that directory holds `seraph.dll` (Windows) /
`libseraph.so` (Linux) / `libseraph.dylib` (macOS).

Add the crate to your `Cargo.toml` by path (or vendor it):

```toml
[dependencies]
seraph = { path = "vendor/seraph" }
```

Build with `SERAPH_LIB_DIR` pointing at the unpacked cdylib directory:

```bash
# Windows (PowerShell)
$env:SERAPH_LIB_DIR = "C:\path\to\seraph-ffi-windows-x86_64-gpu"
cargo build --release

# Linux / macOS
SERAPH_LIB_DIR=/path/to/seraph-ffi-linux-x86_64-gpu cargo build --release
```

At runtime the cdylib must be discoverable: place it alongside your binary, or
add its directory to `PATH` (Windows) / `LD_LIBRARY_PATH` (Linux) /
`DYLD_LIBRARY_PATH` (macOS). The ONNX variant additionally needs an ONNX Runtime
shared library — see the FFI archive's notes.

## Quick start

```rust
use seraph::{Store, Encoder};

let mut encoder = Encoder::from_pretrained("BAAI/bge-small-en-v1.5")?;
let mut store = Store::create("memory.sfg", None)?;

let emb = encoder.encode("the cat sat on the mat")?;
let id = store.put(b"the cat sat on the mat", &emb)?;

let query = encoder.encode("feline")?;
for hit in store.search(&query, 10, 0.0)? {
    println!("{}: {:.3}", hit.frame_id, hit.score);
}
# Ok::<(), seraph::Error>(())
```

---

SERAPH — Copyright (c) 2025-2026 Daniel S. Brooks. All rights reserved.
Proprietary and confidential. See `LICENSE.md`.
