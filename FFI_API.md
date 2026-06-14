# SERAPH C FFI API Reference

> Library: `seraph.dll` (Windows) / `libseraph.so` (Linux) / `libseraph.dylib` (macOS)
> Link: `-lseraph` (Unix) or `seraph.lib` (MSVC)

This reference documents the C ABI for the people who **call** SERAPH from C,
C++, or any language with a C FFI. Read *Authentication & Licensing* and
*Conventions* first — they apply to every function and are not repeated per
entry.

---

## Authentication & Licensing

SERAPH performs no network authentication. "Authorization" is **license-tier
gating**, enforced locally at the call site. Most functions work on every tier.
The functions below require a **Commercial or Enterprise** license; on the Free
tier they fail (returning `NULL` or a negative code) with an error set —
retrieve it via `seraph_last_error()`. These are tagged 🔒 **Licensed** inline.

| Capability | Functions |
|---|---|
| Encryption at rest | `seraph_store_create_encrypted`, `seraph_store_open_encrypted` |
| Sealing | `seraph_store_seal` |
| Time anchoring | `seraph_store_append_tip_anchor` |

**Writer identity (secure mode).** The license attests *who is licensed and at
what tier* — it does **not** carry a signing key. Per-frame writer attribution
uses a key the **calling application supplies**: pass a 32-byte Ed25519 private
seed to `seraph_store_create_with_writer` / `seraph_store_open_with_writer`. The
seed stays in the caller's process; SERAPH signs each frame with it and records
only the derived public key in the store. Secure mode requires both a permitting
tier and a supplied writer seed.

---

## Conventions

These rules hold for all functions unless an entry says otherwise:

- **Opaque handles:** Store and encoder are `void*` handles. The caller owns the
  handle and must call the matching `_close` function to free it.
- **Error reporting:** Functions that return a pointer signal failure with
  `NULL`; functions that return `int` use `0` for success and a negative value
  for error. In both cases, call `seraph_last_error()` for the message (valid
  until the next FFI call on the same thread).
- **Strings returned:** UTF-8 `char*` owned by the caller. Free with
  `seraph_string_free()`.
- **Byte arrays returned:** `uint8_t*` + length, owned by the caller. Free with
  `seraph_bytes_free()`.
- **Float arrays returned:** `float*` + length, owned by the caller. Free with
  `seraph_floats_free()`.
- **JSON:** Complex structures are passed and returned as JSON strings. Returned
  JSON strings are freed with `seraph_string_free()`.

**Standard error pattern:**

```c
void* store = seraph_store_open("/path/to/store.sfg");
if (!store) {
    fprintf(stderr, "Error: %s\n", seraph_last_error());
    return -1;
}
```

---

## Table of Contents

1. [Store Lifecycle](#store-lifecycle)
2. [Encryption](#encryption)
3. [Ingestion](#ingestion)
4. [Search](#search)
5. [Frame Access](#frame-access)
6. [Seeds & Promotion](#seeds--promotion)
7. [Traversal & Chains](#traversal--chains)
8. [Neighborhood](#neighborhood)
9. [Warp (Steering)](#warp-steering)
10. [Analysis & Metrics](#analysis--metrics)
11. [Intelligence](#intelligence)
12. [Consolidation](#consolidation)
13. [Typed Structural Edges](#typed-structural-edges)
14. [Provenance & Sealing](#provenance--sealing)
15. [Anchoring](#anchoring)
16. [Snapshot / Temporal](#snapshot--temporal)
17. [Events & Subscriptions](#events--subscriptions)
18. [Encoder](#encoder)
19. [Batch Operations](#batch-operations)
20. [Memory Management](#memory-management)
21. [Structs Reference](#structs-reference)

---

## Store Lifecycle

### `seraph_store_create`

Create a new store file with a fixed embedding model declared at genesis.

```c
void* seraph_store_create(const char* path, const char* model_id, const char* config_json);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `path` | `const char*` | yes | Filesystem path (UTF-8) for the new `.sfg` file. |
| `model_id` | `const char*` | no | Embedding model ID. `NULL` → default model. Permanent for the store's lifetime. |
| `config_json` | `const char*` | no | JSON config overrides. `NULL` → defaults. Fields below. |

**Config JSON fields** (all optional)

| Field | Type | Default | Description |
|---|---|---|---|
| `eigen_threshold` | int | 5 | In-degree at which a frame is promoted to eigenframe. |
| `eigen_min_age_s` | float | 60.0 | Minimum age (seconds) before a frame is promotion-eligible. |
| `tau_similarity` | float | 0.5 | Default similarity-edge threshold. |
| `tau_merge` | float | 0.92 | Consolidation merge threshold. |
| `max_index_elements` | int | 100000 | Index capacity hint. |
| `search_max_depth` | int | 50 | Max BFS traversal depth during search. |
| `search_visit_cap_multiplier` | int | 5 | BFS frontier cap = `top_k ×` this value. |
| `redirect_per_frame` | bool | true | Emit one redirect frame per superseded source on consolidation. |
| `eigen_matmul_min` | int | 256 | Eigenframe count at which scan switches to batched matmul. |

**Returns:** Opaque store handle (`void*`). `NULL` on error.

**Errors:** `NULL` if `path` is unwritable or already exists, `model_id` is unknown, or `config_json` is malformed.

**Example**

```c
void* store = seraph_store_create("memory.sfg", "BAAI/bge-small-en-v1.5", NULL);
if (!store) { fprintf(stderr, "create failed: %s\n", seraph_last_error()); return -1; }
```

### `seraph_store_create_with_writer`

Create a store with an **app-supplied writer identity**. The caller holds a
32-byte Ed25519 private seed; SERAPH signs every frame with it and records the
derived public key in the store's genesis attestation, so the chain is
verifiable later without the key. The seed never leaves the caller — SERAPH
neither persists nor transmits it. Pass `NULL` to create without a writer
identity (the store is non-secure).

```c
void* seraph_store_create_with_writer(
    const char* path,
    const char* model_id,       // or NULL for default
    const char* config_json,    // or NULL for defaults
    const uint8_t* writer_seed  // 32-byte Ed25519 seed, or NULL
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `path` | `const char*` | yes | Filesystem path (UTF-8) for the new `.sfg` file. |
| `model_id` | `const char*` | no | Embedding model ID. `NULL` → default model. |
| `config_json` | `const char*` | no | JSON config overrides (see `seraph_store_create`). `NULL` → defaults. |
| `writer_seed` | `const uint8_t*` | no | Pointer to a 32-byte Ed25519 private seed held by the caller, or `NULL`. When supplied, the store is secure and every frame is signed by this key. |

**Returns:** Opaque store handle, or `NULL` on error.

**Errors:** `NULL` if the license tier does not permit secure mode, the path is unwritable, or `config_json` is malformed.

**Example**

```c
uint8_t seed[32] = { /* app-held Ed25519 private seed */ };
void* store = seraph_store_create_with_writer("memory.sfg", NULL, NULL, seed);
if (!store) { fprintf(stderr, "create failed: %s\n", seraph_last_error()); return -1; }
```

### `seraph_store_open`

Open an existing unencrypted store.

```c
void* seraph_store_open(const char* path);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `path` | `const char*` | yes | Path to an existing `.sfg` file. |

**Returns:** Store handle, or `NULL` on error.

**Errors:** `NULL` if the file is missing, corrupt, or **encrypted** (use `seraph_store_open_encrypted`).

**Example**

```c
void* store = seraph_store_open("memory.sfg");
if (!store) { /* handle error */ }
```

### `seraph_store_open_with_writer`

Open an existing store with an app-supplied writer identity for signing frames
appended during this session. Reading and verification need no writer key.

```c
void* seraph_store_open_with_writer(
    const char* path,
    const uint8_t* writer_seed  // 32-byte Ed25519 seed, or NULL
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `path` | `const char*` | yes | Path to an existing `.sfg` file. |
| `writer_seed` | `const uint8_t*` | no | 32-byte Ed25519 private seed for signing appended frames, or `NULL` to open without a writer identity (read/verify only; secure appends require a writer). |

**Returns:** Store handle, or `NULL` on error.

**Errors:** `NULL` if the file is missing, corrupt, or **encrypted** (use `seraph_store_open_encrypted`).

**Example**

```c
uint8_t seed[32] = { /* app-held Ed25519 private seed */ };
void* store = seraph_store_open_with_writer("memory.sfg", seed);
if (!store) { /* handle error */ }
```

### `seraph_store_close`

Close the store and free all resources. The handle is invalid after this call.

```c
void seraph_store_close(void* handle);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle to close. |

**Returns:** Nothing.

**Errors:** None. Passing `NULL` is a no-op.

**Example**

```c
seraph_store_close(store);
```

### `seraph_store_sync`

Flush pending writes to disk.

```c
int seraph_store_sync(void* handle);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |

**Returns:** `0` on success, negative on error.

**Errors:** Negative if the handle is `NULL` or the underlying write fails.

**Example**

```c
if (seraph_store_sync(store) != 0) { fprintf(stderr, "%s\n", seraph_last_error()); }
```

### `seraph_store_set_search_visit_cap_multiplier`

Set the BFS visit-cap multiplier at runtime. Higher values widen the search
frontier at minimal latency cost (the Phase-1 eigenframe scan dominates search
time). Default comes from `search_visit_cap_multiplier` in the config JSON (5).

```c
int seraph_store_set_search_visit_cap_multiplier(void* handle, uint32_t multiplier);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `multiplier` | `uint32_t` | yes | New frontier multiplier. Frontier cap = `top_k × multiplier`. |

**Returns:** `0` on success, `-1` on error.

**Errors:** `-1` if the handle is `NULL`.

**Example**

```c
seraph_store_set_search_visit_cap_multiplier(store, 10);  // wider frontier
```

### `seraph_reencode_store`

Re-encode a source store into a fresh **v3** destination store (`format_version = 3`). Streams active-content frames from `src_path`, re-encodes them with `model_id` on a dedicated encoder thread, and bulk-ingests the resulting f32 embeddings via the batched snapshot path. The `.gidx` is not written (frame log only); similarity edges reconstruct from the log on the first open.

```c
int seraph_reencode_store(
    const char* src_path,
    const char* dst_path,
    const char* model_id,
    uint32_t eigen_threshold,
    size_t n_max,
    size_t pbatch,
    size_t ebatch,
    size_t* out_n
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `src_path` | `const char*` | yes | Source `.sfg` to re-encode from. |
| `dst_path` | `const char*` | yes | Destination `.sfg` (overwritten if present). |
| `model_id` | `const char*` | yes | Embedding model for the destination genesis. |
| `eigen_threshold` | `uint32_t` | yes | In-degree promotion threshold for the destination. |
| `n_max` | `size_t` | yes | Cap on frames re-encoded; `0` = all active-content frames. |
| `pbatch` | `size_t` | yes | Frames per `put_batch`; `0` → default 2000. |
| `ebatch` | `size_t` | yes | Frames per encode batch; `0` → default 16. |
| `out_n` | `size_t*` | no | Out: number of frames ingested. Ignored if `NULL`. |

**Returns:** `0` on success, `-1` on error.

**Errors:** `-1` (with `seraph_last_error()` set) on `NULL` path/model strings, an unreadable source, or an encode/ingest failure.

---

## Encryption

> 🔒 **Licensed.** Requires Commercial or Enterprise. Free-tier calls return `NULL` with an error set.

### `seraph_store_create_encrypted`

Create a new store with AES-256-GCM encryption at rest. Content fields are
encrypted with a random DEK wrapped by an Argon2id-derived KEK. Structural
metadata (watermarks, parent links) stays plaintext for keyless chain
verification.

```c
void* seraph_store_create_encrypted(
    const char* path,
    const char* model_id,      // or NULL
    const char* config_json,   // or NULL
    const uint8_t* passphrase,
    size_t passphrase_len
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `path` | `const char*` | yes | Path for the new encrypted `.sfg` file. |
| `model_id` | `const char*` | no | Embedding model ID. `NULL` → default. |
| `config_json` | `const char*` | no | Config overrides (see `seraph_store_create`). `NULL` → defaults. |
| `passphrase` | `const uint8_t*` | yes | Passphrase bytes (not NUL-terminated; length given separately). |
| `passphrase_len` | `size_t` | yes | Length of `passphrase` in bytes. |

**Returns:** Store handle, or `NULL` on error.

**Errors:** `NULL` on Free tier, unwritable path, or empty passphrase.

**Example**

```c
const char* pass = "my secret";
void* store = seraph_store_create_encrypted(
    "encrypted.sfg", NULL, NULL, (const uint8_t*)pass, strlen(pass));
if (!store) { /* handle error */ }
seraph_store_close(store);
```

### `seraph_store_open_encrypted`

Open an existing encrypted store.

```c
void* seraph_store_open_encrypted(
    const char* path,
    const uint8_t* passphrase,
    size_t passphrase_len
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `path` | `const char*` | yes | Path to an existing encrypted `.sfg` file. |
| `passphrase` | `const uint8_t*` | yes | Passphrase bytes used at creation. |
| `passphrase_len` | `size_t` | yes | Length of `passphrase` in bytes. |

**Returns:** Store handle, or `NULL` on error.

**Errors:** `NULL` on Free tier, wrong passphrase, missing file, or if the store is not encrypted.

**Example**

```c
const char* pass = "my secret";
void* store = seraph_store_open_encrypted(
    "encrypted.sfg", (const uint8_t*)pass, strlen(pass));
// All put/search/get operations then work transparently.
```

---

## Ingestion

### `seraph_store_put`

Ingest a frame with pre-computed embedding.

```c
char* seraph_store_put(
    void* handle,
    const uint8_t* content, size_t content_len,
    const float* embedding, size_t embedding_len
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `content` | `const uint8_t*` | yes | Content bytes (any binary payload). |
| `content_len` | `size_t` | yes | Length of `content`. |
| `embedding` | `const float*` | yes | Embedding vector. |
| `embedding_len` | `size_t` | yes | Vector length; must equal the store dimension. |

**Returns:** Frame ID string (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` if the embedding dimension mismatches the store, the store is sealed, or the handle is `NULL`.

**Example**

```c
char* fid = seraph_store_put(store, (const uint8_t*)"hello", 5, emb, dim);
if (!fid) { /* handle error */ }
seraph_string_free(fid);
```

### `seraph_store_put_with_metadata`

Ingest a frame with an attached JSON metadata object.

```c
char* seraph_store_put_with_metadata(
    void* handle,
    const uint8_t* content, size_t content_len,
    const float* embedding, size_t embedding_len,
    const char* metadata_json
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `content` | `const uint8_t*` | yes | Content bytes. |
| `content_len` | `size_t` | yes | Length of `content`. |
| `embedding` | `const float*` | yes | Embedding vector. |
| `embedding_len` | `size_t` | yes | Vector length; must equal the store dimension. |
| `metadata_json` | `const char*` | yes | JSON object string. Use `"{}"` for none. |

**Returns:** Frame ID string (free with `seraph_string_free`), or `NULL` on error.

**Errors:** As `seraph_store_put`, plus `NULL` if `metadata_json` is not a valid JSON object.

**Example**

```c
char* fid = seraph_store_put_with_metadata(
    store, (const uint8_t*)"hello", 5, emb, dim, "{\"source\":\"doc1\"}");
seraph_string_free(fid);
```

### `seraph_store_put_batch`

Two-phase batch ingest: all items see the same pre-batch state for parent
selection, so insertion order does not bias topology.

```c
char* seraph_store_put_batch(void* handle, const char* items_json);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `items_json` | `const char*` | yes | JSON array of `{content_b64, embedding, metadata?}` objects. |

**Returns:** JSON array of frame IDs (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` if `items_json` is malformed or any embedding dimension mismatches.

**Example**

```c
char* ids = seraph_store_put_batch(store,
    "[{\"content_b64\":\"aGVsbG8=\",\"embedding\":[0.1,0.2]}]");
// ids -> ["<frame-id>"]
seraph_string_free(ids);
```

---

## Search

### `seraph_store_search`

Graph-native ANN search for the nearest frames to a query embedding.

```c
int seraph_store_search(
    void* handle,
    const float* embedding, size_t embedding_len,
    size_t top_k,
    float tau,
    SeraphHit** out_hits,
    size_t* out_count
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `embedding` | `const float*` | yes | Query vector. |
| `embedding_len` | `size_t` | yes | Vector length; must equal the store dimension. |
| `top_k` | `size_t` | yes | Maximum number of hits to return. |
| `tau` | `float` | yes | Minimum similarity threshold. Pass `0.0` for no filtering. |
| `out_hits` | `SeraphHit**` | yes | Out: array of hits. Free with `seraph_hits_free`. |
| `out_count` | `size_t*` | yes | Out: number of hits written. |

**Returns:** `0` on success, negative on error.

**Errors:** Negative if the handle is `NULL` or the embedding dimension mismatches.

**Result struct** — see [`SeraphHit`](#seraphhit).

**Example**

```c
SeraphHit* hits = NULL; size_t n = 0;
if (seraph_store_search(store, q, dim, 10, 0.0f, &hits, &n) == 0) {
    for (size_t i = 0; i < n; i++)
        printf("%s score=%.3f\n", hits[i].frame_id, hits[i].score);
    seraph_hits_free(hits, n);
}
```

### `seraph_store_search_text`

Search by text using the store's built-in encoder (store must have an encoder).

```c
int seraph_store_search_text(
    void* handle,
    const char* query,
    size_t top_k,
    float tau,
    SeraphHit** out_hits,
    size_t* out_count
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `query` | `const char*` | yes | Query text (UTF-8). |
| `top_k` | `size_t` | yes | Maximum number of hits. |
| `tau` | `float` | yes | Minimum similarity threshold (`0.0` = none). |
| `out_hits` | `SeraphHit**` | yes | Out: hit array. Free with `seraph_hits_free`. |
| `out_count` | `size_t*` | yes | Out: number of hits. |

**Returns:** `0` on success, negative on error.

**Errors:** Negative if the store has no encoder, the handle is `NULL`, or encoding fails.

**Example**

```c
SeraphHit* hits = NULL; size_t n = 0;
seraph_store_search_text(store, "feline", 10, 0.0f, &hits, &n);
seraph_hits_free(hits, n);
```

### `seraph_store_search_with_warp`

Search with warp-field steering (target pull + suppression).

```c
int seraph_store_search_with_warp(
    void* handle,
    const float* query_embedding, size_t query_len,
    const char* targets_json,
    const char* suppress_json,
    const char* profile,
    float bound_k,
    size_t top_k,
    float tau,
    SeraphHit** out_hits,
    size_t* out_count
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `query_embedding` | `const float*` | yes | Query vector. |
| `query_len` | `size_t` | yes | Vector length. |
| `targets_json` | `const char*` | yes | Warp entry array (pull). `[]`, `"null"`, or `""` = none. |
| `suppress_json` | `const char*` | yes | Warp entry array (suppression). Same empty conventions. |
| `profile` | `const char*` | yes | `"routing"`, `"saturation"`, or `"bounded"`. Any other value is an error. |
| `bound_k` | `float` | yes | Suppression-norm cap. Used only by the `"bounded"` profile. |
| `top_k` | `size_t` | yes | Maximum number of hits. |
| `tau` | `float` | yes | Minimum similarity threshold. |
| `out_hits` | `SeraphHit**` | yes | Out: hit array. Free with `seraph_hits_free`. |
| `out_count` | `size_t*` | yes | Out: number of hits. |

**Warp entry JSON format** (`targets_json` / `suppress_json`): a JSON **array of
objects**, each with an `embedding` array and a `magnitude` number, read by key.
A positional `[embedding, magnitude]` pair is **not** accepted — it fails with
`warp entry missing embedding`.

```json
[
  { "embedding": [0.012, -0.044], "magnitude": 0.2 },
  { "embedding": [0.103,  0.071], "magnitude": 0.5 }
]
```

**Returns:** `0` on success, negative on error.

**Errors:** Negative if `profile` is unrecognized, a warp entry is missing `embedding`/`magnitude`, or a dimension mismatches.

**Example**

```c
SeraphHit* hits = NULL; size_t n = 0;
seraph_store_search_with_warp(store, q, dim,
    "[{\"embedding\":[...],\"magnitude\":0.2}]", "[]",
    "bounded", 1.0f, 10, 0.0f, &hits, &n);
seraph_hits_free(hits, n);
```

### `seraph_store_search_with_warp_spec`

Search with a full WarpSpec JSON in one call.

```c
int seraph_store_search_with_warp_spec(
    void* handle,
    const float* query_embedding, size_t query_len,
    const char* warp_spec_json,
    size_t top_k,
    float tau,
    SeraphHit** out_hits,
    size_t* out_count
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `query_embedding` | `const float*` | yes | Query vector. |
| `query_len` | `size_t` | yes | Vector length. |
| `warp_spec_json` | `const char*` | yes | Full WarpSpec JSON. [PLACEHOLDER: WarpSpec JSON schema] |
| `top_k` | `size_t` | yes | Maximum number of hits. |
| `tau` | `float` | yes | Minimum similarity threshold. |
| `out_hits` | `SeraphHit**` | yes | Out: hit array. Free with `seraph_hits_free`. |
| `out_count` | `size_t*` | yes | Out: number of hits. |

**Returns:** `0` on success, negative on error.

**Errors:** Negative if `warp_spec_json` is malformed or a dimension mismatches.

**Example**

```c
SeraphHit* hits = NULL; size_t n = 0;
seraph_store_search_with_warp_spec(store, q, dim, spec_json, 10, 0.0f, &hits, &n);
seraph_hits_free(hits, n);
```

---

## Frame Access

### `seraph_store_get_frame`

Fetch all fields of a frame as JSON.

```c
char* seraph_store_get_frame(void* handle, const char* frame_id);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `frame_id` | `const char*` | yes | Frame ID to fetch. |

**Returns:** JSON string of all frame fields (free with `seraph_string_free`), or `NULL` if not found.

**Errors:** `NULL` if the frame does not exist or the handle is `NULL`.

**Response:** [PLACEHOLDER: frame JSON shape — id, parent_id, status, in_degree, timestamp, metadata, …]

**Example**

```c
char* json = seraph_store_get_frame(store, fid);
if (json) { /* parse */ seraph_string_free(json); }
```

### `seraph_store_get_content`

Fetch a frame's raw content bytes.

```c
int seraph_store_get_content(
    void* handle, const char* frame_id,
    uint8_t** out_content, size_t* out_len
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `frame_id` | `const char*` | yes | Frame ID. |
| `out_content` | `uint8_t**` | yes | Out: content bytes. Free with `seraph_bytes_free`. |
| `out_len` | `size_t*` | yes | Out: content length. |

**Returns:** `0` on success, negative on error.

**Errors:** Negative if the frame is missing or the handle is `NULL`.

**Example**

```c
uint8_t* buf = NULL; size_t len = 0;
if (seraph_store_get_content(store, fid, &buf, &len) == 0) {
    /* use buf[0..len] */ seraph_bytes_free(buf, len);
}
```

### `seraph_store_get_embedding`

Fetch a frame's embedding into a freshly allocated buffer.

```c
int seraph_store_get_embedding(
    void* handle, const char* frame_id,
    float** out_embedding, size_t* out_dim
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `frame_id` | `const char*` | yes | Frame ID. |
| `out_embedding` | `float**` | yes | Out: float array. Free with `seraph_floats_free`. |
| `out_dim` | `size_t*` | yes | Out: dimensionality. |

**Returns:** `0` on success, negative on error.

**Errors:** Negative if the frame is missing or the handle is `NULL`.

**Example**

```c
float* emb = NULL; size_t dim = 0;
if (seraph_store_get_embedding(store, fid, &emb, &dim) == 0)
    seraph_floats_free(emb, dim);
```

### `seraph_store_get_embedding_into`

Zero-copy variant: writes the embedding into a caller-supplied buffer.

```c
int seraph_store_get_embedding_into(
    void* handle, const char* frame_id,
    float* buf, size_t buf_len, size_t* out_written
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `frame_id` | `const char*` | yes | Frame ID. |
| `buf` | `float*` | yes | Caller-owned destination buffer. |
| `buf_len` | `size_t` | yes | Buffer capacity (elements). |
| `out_written` | `size_t*` | yes | Out: number of elements written. |

**Returns:** `0` on success, negative on error.

**Errors:** Negative if the frame is missing, the handle is `NULL`, or `buf_len` is smaller than the store dimension.

**Example**

```c
float buf[1024]; size_t written = 0;
seraph_store_get_embedding_into(store, fid, buf, 1024, &written);
```

### `seraph_store_get_metadata`

Fetch a frame's metadata object as JSON.

```c
char* seraph_store_get_metadata(void* handle, const char* frame_id);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `frame_id` | `const char*` | yes | Frame ID. |

**Returns:** JSON metadata string (free with `seraph_string_free`), or `NULL`.

**Errors:** `NULL` if the frame is missing or has no metadata.

**Example**

```c
char* meta = seraph_store_get_metadata(store, fid);
if (meta) seraph_string_free(meta);
```

### `seraph_store_borrow_frame` / `seraph_store_release_frame`

Zero-copy frame view. Pointers in the view borrow engine-owned memory — do not
mutate the store while the view is held, and do not free individual fields.

```c
SeraphFrameView* seraph_store_borrow_frame(void* handle, const char* frame_id);
void seraph_store_release_frame(SeraphFrameView* view);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `frame_id` | `const char*` | yes | Frame ID to borrow. |
| `view` | `SeraphFrameView*` | yes | (release) View to release. |

**Returns:** `seraph_store_borrow_frame` → pointer to a [`SeraphFrameView`](#seraphframeview), or `NULL` if not found. `seraph_store_release_frame` → nothing.

**Errors:** `NULL` if the frame is missing or the handle is `NULL`.

**Example**

```c
SeraphFrameView* v = seraph_store_borrow_frame(store, fid);
if (v) { /* read v->embedding, v->status, ... */ seraph_store_release_frame(v); }
```

### `seraph_store_frame_ids`

List all frame IDs in the store.

```c
char* seraph_store_frame_ids(void* handle);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |

**Returns:** JSON array of frame ID strings (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` if the handle is `NULL`.

**Example**

```c
char* ids = seraph_store_frame_ids(store);  // ["id1","id2",...]
seraph_string_free(ids);
```

### `seraph_store_eigenframe_ids`

List the IDs of all eigenframes.

```c
char* seraph_store_eigenframe_ids(void* handle);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |

**Returns:** JSON array of eigenframe ID strings (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` if the handle is `NULL`.

**Example**

```c
char* ids = seraph_store_eigenframe_ids(store);
seraph_string_free(ids);
```

### `seraph_store_eigenframes`

Fetch full frame objects for all eigenframes.

```c
char* seraph_store_eigenframes(void* handle);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |

**Returns:** JSON array of full frame objects (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` if the handle is `NULL`.

**Response:** [PLACEHOLDER: array of frame JSON objects — same shape as `get_frame`]

**Example**

```c
char* json = seraph_store_eigenframes(store);
seraph_string_free(json);
```

### `seraph_store_federation_vector`

Mean of all eigenframe embeddings — a single vector representing the store's
semantic center, used for federation routing (D2 dispatch).

```c
int seraph_store_federation_vector(void* handle, float** out_embedding, size_t* out_dim);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `out_embedding` | `float**` | yes | Out: float array. Free with `seraph_floats_free`. |
| `out_dim` | `size_t*` | yes | Out: dimensionality. |

**Returns:** `0` on success (including the no-eigenframe case), `-1` on error.

**Errors:** `-1` if the handle is `NULL`. If the store has no eigenframes, returns `0` with `out_embedding = NULL` and `out_dim = 0`.

**Example**

```c
float* fv = NULL; size_t dim = 0;
if (seraph_store_federation_vector(store, &fv, &dim) == 0 && fv)
    seraph_floats_free(fv, dim);
```

### `seraph_store_model_id`

Return the store's permanent embedding model ID.

```c
char* seraph_store_model_id(void* handle);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |

**Returns:** Model ID string (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` if the handle is `NULL`.

**Example**

```c
char* mid = seraph_store_model_id(store);
seraph_string_free(mid);
```

### `seraph_store_dim`

Return the store's embedding dimensionality.

```c
size_t seraph_store_dim(void* handle);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |

**Returns:** Embedding dimension. Returns `0` if the handle is `NULL`.

**Errors:** No error channel; `0` signals an invalid handle.

**Example**

```c
size_t dim = seraph_store_dim(store);
```

### `seraph_store_stats`

Write store statistics into a caller-supplied struct.

```c
int seraph_store_stats(void* handle, SeraphStats* out);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `out` | `SeraphStats*` | yes | Out: caller-owned [`SeraphStats`](#seraphstats) struct. |

**Returns:** `0` on success, negative on error.

**Errors:** Negative if the handle or `out` is `NULL`.

**Example**

```c
SeraphStats s;
if (seraph_store_stats(store, &s) == 0)
    printf("frames=%zu eigen=%zu\n", s.total_frames, s.eigenframes);
```

---

## Seeds & Promotion

### `seraph_store_add_seed`

Add a domain-attractor seed frame (always an eigenframe).

```c
char* seraph_store_add_seed(
    void* handle,
    const char* label,
    const float* embedding, size_t embedding_len,
    const char* metadata_json
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `label` | `const char*` | yes | Human-readable seed label. |
| `embedding` | `const float*` | yes | Seed embedding vector. |
| `embedding_len` | `size_t` | yes | Vector length; must equal the store dimension. |
| `metadata_json` | `const char*` | no | JSON metadata object, or `NULL`. |

**Returns:** Seed frame ID (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on dimension mismatch, sealed store, or `NULL` handle.

**Example**

```c
char* sid = seraph_store_add_seed(store, "physics", emb, dim, NULL);
seraph_string_free(sid);
```

### `seraph_store_promote`

Promote an existing frame to eigenframe. Promotion is monotonic — eigenframes
are never demoted.

```c
int seraph_store_promote(void* handle, const char* frame_id);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `frame_id` | `const char*` | yes | Frame ID to promote. |

**Returns:** `0` on success, negative on error.

**Errors:** Negative if the frame is missing or the handle is `NULL`.

**Example**

```c
seraph_store_promote(store, fid);
```

### `seraph_store_promote_content`

Ingest new content and promote it to eigenframe in one step.

```c
char* seraph_store_promote_content(
    void* handle,
    const uint8_t* content, size_t content_len,
    const float* embedding, size_t embedding_len,
    const char* metadata_json
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `content` | `const uint8_t*` | yes | Content bytes. |
| `content_len` | `size_t` | yes | Length of `content`. |
| `embedding` | `const float*` | yes | Embedding vector. |
| `embedding_len` | `size_t` | yes | Vector length; must equal the store dimension. |
| `metadata_json` | `const char*` | no | JSON metadata object, or `NULL`. |

**Returns:** New eigenframe ID (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on dimension mismatch, sealed store, or `NULL` handle.

**Example**

```c
char* fid = seraph_store_promote_content(store, (const uint8_t*)"axiom", 5, emb, dim, NULL);
seraph_string_free(fid);
```

---

## Traversal & Chains

### `seraph_store_path`

BFS path between two frames.

```c
char* seraph_store_path(void* handle, const char* from_id, const char* to_id);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `from_id` | `const char*` | yes | Start frame ID. |
| `to_id` | `const char*` | yes | End frame ID. |

**Returns:** JSON array of frame objects along the BFS path (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` if either frame is missing or no path exists.

**Response:** [PLACEHOLDER: array of frame JSON objects]

**Example**

```c
char* json = seraph_store_path(store, a, b);
seraph_string_free(json);
```

### `seraph_store_chain`

Semantic chain between two frames, with distance and coherence metrics.

```c
char* seraph_store_chain(void* handle, const char* from_id, const char* to_id);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `from_id` | `const char*` | yes | Start frame ID. |
| `to_id` | `const char*` | yes | End frame ID. |

**Returns:** JSON chain result with frames, primitive distance, and coherence metrics (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` if either frame is missing.

**Response:** [PLACEHOLDER: chain JSON shape — frames, distance, coherence]

**Example**

```c
char* json = seraph_store_chain(store, a, b);
seraph_string_free(json);
```

### `seraph_store_chain_to`

Chain from the store toward a query embedding.

```c
char* seraph_store_chain_to(void* handle, const float* query_embedding, size_t query_len);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `query_embedding` | `const float*` | yes | Target query vector. |
| `query_len` | `size_t` | yes | Vector length. |

**Returns:** JSON chain result (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on dimension mismatch or `NULL` handle.

**Response:** [PLACEHOLDER: chain JSON shape]

**Example**

```c
char* json = seraph_store_chain_to(store, q, dim);
seraph_string_free(json);
```

### `seraph_store_chain_between`

Chain between two arbitrary embeddings.

```c
char* seraph_store_chain_between(
    void* handle,
    const float* emb_a, size_t len_a,
    const float* emb_b, size_t len_b
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `emb_a` | `const float*` | yes | First embedding. |
| `len_a` | `size_t` | yes | Length of `emb_a`. |
| `emb_b` | `const float*` | yes | Second embedding. |
| `len_b` | `size_t` | yes | Length of `emb_b`. |

**Returns:** JSON chain result (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on dimension mismatch or `NULL` handle.

**Response:** [PLACEHOLDER: chain JSON shape]

**Example**

```c
char* json = seraph_store_chain_between(store, a, dim, b, dim);
seraph_string_free(json);
```

### `seraph_store_lineage`

Ancestor chain (parent links) for a frame.

```c
char* seraph_store_lineage(void* handle, const char* frame_id);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `frame_id` | `const char*` | yes | Frame ID. |

**Returns:** JSON array of ancestor frame objects (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` if the frame is missing.

**Response:** [PLACEHOLDER: array of frame JSON objects, root → frame]

**Example**

```c
char* json = seraph_store_lineage(store, fid);
seraph_string_free(json);
```

### `seraph_store_subtree`

Descendants of a frame up to `max_depth`.

```c
char* seraph_store_subtree(void* handle, const char* frame_id, size_t max_depth);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `frame_id` | `const char*` | yes | Root frame ID. |
| `max_depth` | `size_t` | yes | Maximum descent depth. |

**Returns:** JSON subtree structure (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` if the frame is missing.

**Response:** [PLACEHOLDER: subtree JSON shape]

**Example**

```c
char* json = seraph_store_subtree(store, fid, 3);
seraph_string_free(json);
```

### `seraph_store_get_children`

Direct children of a frame.

```c
char* seraph_store_get_children(void* handle, const char* frame_id);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `frame_id` | `const char*` | yes | Parent frame ID. |

**Returns:** JSON array of child frame objects (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` if the frame is missing.

**Response:** [PLACEHOLDER: array of frame JSON objects]

**Example**

```c
char* json = seraph_store_get_children(store, fid);
seraph_string_free(json);
```

---

## Neighborhood

### `seraph_store_neighborhood`

Local region around a frame: nearby frames, edges, and the region eigenframe.

```c
char* seraph_store_neighborhood(void* handle, const char* frame_id, float tau, size_t hops);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `frame_id` | `const char*` | yes | Center frame ID. |
| `tau` | `float` | yes | Similarity threshold for included edges. |
| `hops` | `size_t` | yes | Traversal radius in hops. |

**Returns:** JSON with frames, edges, and region eigenframe (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` if the frame is missing.

**Response:** [PLACEHOLDER: neighborhood JSON shape — frames, edges, region_eigenframe]

**Example**

```c
char* json = seraph_store_neighborhood(store, fid, 0.5f, 2);
seraph_string_free(json);
```

---

## Warp (Steering)

> **Profile values:** `profile` accepts `"routing"`, `"saturation"`, or
> `"bounded"`. Any other value is an error. `"bounded"` is the default used by
> the higher-level wrappers. Target pull is always uncapped; only `"bounded"`
> caps suppression norm (at `bound_k`). Magnitude is the caller's throttle.

### `seraph_store_apply_warp`

Apply warp to a query embedding without searching. Pure vector math; no handle
required.

```c
int seraph_store_apply_warp(
    const float* query_embedding, size_t query_len,
    const char* targets_json,
    const char* suppress_json,
    const char* profile,
    float bound_k,
    float** out_embedding,
    size_t* out_dim
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `query_embedding` | `const float*` | yes | Query vector. |
| `query_len` | `size_t` | yes | Vector length. |
| `targets_json` | `const char*` | yes | Warp entry array (pull). See [Warp entry JSON format](#seraph_store_search_with_warp). |
| `suppress_json` | `const char*` | yes | Warp entry array (suppression). |
| `profile` | `const char*` | yes | `"routing"`, `"saturation"`, or `"bounded"`. |
| `bound_k` | `float` | yes | Suppression cap (only used by `"bounded"`). |
| `out_embedding` | `float**` | yes | Out: warped vector. Free with `seraph_floats_free`. |
| `out_dim` | `size_t*` | yes | Out: vector length. |

**Returns:** `0` on success, negative on error.

**Errors:** Negative if `profile` is unrecognized or a warp entry is malformed.

**Example**

```c
float* warped = NULL; size_t dim = 0;
seraph_store_apply_warp(q, qd,
    "[{\"embedding\":[...],\"magnitude\":0.2}]", "[]", "bounded", 1.0f, &warped, &dim);
seraph_floats_free(warped, dim);
```

### `seraph_store_warp_query`

Apply a WarpSpec JSON to a query embedding.

```c
int seraph_store_warp_query(
    void* handle,
    const float* query_embedding, size_t query_len,
    const char* warp_spec_json,
    float** out_embedding,
    size_t* out_dim
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `query_embedding` | `const float*` | yes | Query vector. |
| `query_len` | `size_t` | yes | Vector length. |
| `warp_spec_json` | `const char*` | yes | Full WarpSpec JSON. [PLACEHOLDER: WarpSpec JSON schema] |
| `out_embedding` | `float**` | yes | Out: warped vector. Free with `seraph_floats_free`. |
| `out_dim` | `size_t*` | yes | Out: vector length. |

**Returns:** `0` on success, negative on error.

**Errors:** Negative if `warp_spec_json` is malformed or a dimension mismatches.

**Example**

```c
float* warped = NULL; size_t dim = 0;
seraph_store_warp_query(store, q, qd, spec_json, &warped, &dim);
seraph_floats_free(warped, dim);
```

### `seraph_store_calibrate_warp`

Sweep magnitudes over sampled query/target pairs and report calibration results.

```c
char* seraph_store_calibrate_warp(
    void* handle,
    size_t n_queries,
    size_t n_targets,
    const char* magnitudes_json,
    size_t top_k,
    uint64_t seed
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `n_queries` | `size_t` | yes | Number of sampled queries. |
| `n_targets` | `size_t` | yes | Number of sampled targets. |
| `magnitudes_json` | `const char*` | yes | JSON array of magnitudes to sweep. |
| `top_k` | `size_t` | yes | Hits per probe. |
| `seed` | `uint64_t` | yes | RNG seed for reproducible sampling. |

**Returns:** JSON calibration report (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` if `magnitudes_json` is malformed or the handle is `NULL`.

**Response:** [PLACEHOLDER: calibration report JSON shape]

**Example**

```c
char* report = seraph_store_calibrate_warp(store, 50, 5, "[0.1,0.2,0.5]", 10, 42);
seraph_string_free(report);
```

---

## Analysis & Metrics

All scalar metrics return a `float`; struct/JSON variants are noted. Scalar
functions return a sentinel (e.g. `NaN` / negative) on error — check
`seraph_last_error()` when a result looks invalid.

### `seraph_store_density_score`

```c
float seraph_store_density_score(void* handle, const char* frame_id);
```

**Parameters:** `handle` (`void*`, yes), `frame_id` (`const char*`, yes).
**Returns:** Local density score. **Errors:** sentinel value on missing frame / `NULL` handle.

### `seraph_store_seed_margin`

```c
float seraph_store_seed_margin(void* handle, const char* frame_id);
```

**Parameters:** `handle` (`void*`, yes), `frame_id` (`const char*`, yes).
**Returns:** Distance to the nearest seed. **Errors:** sentinel value on error.

### `seraph_store_steerability`

```c
float seraph_store_steerability(void* handle, const float* query_embedding, size_t query_len, const char* target_id);
```

**Parameters:** `handle` (`void*`, yes), `query_embedding` (`const float*`, yes), `query_len` (`size_t`, yes), `target_id` (`const char*`, yes).
**Returns:** Steerability score for warping the query toward `target_id`. **Errors:** sentinel on dimension mismatch / missing target.

### `seraph_store_region_health`

```c
char* seraph_store_region_health(void* handle, const char* eigenframe_id);
```

**Parameters:** `handle` (`void*`, yes), `eigenframe_id` (`const char*`, yes).
**Returns:** JSON health report (free with `seraph_string_free`), or `NULL`. **Response:** [PLACEHOLDER: region health JSON shape]

### `seraph_store_region_health_struct`

```c
int seraph_store_region_health_struct(void* handle, const char* eigenframe_id, SeraphRegionHealth* out);
```

**Parameters:** `handle` (`void*`, yes), `eigenframe_id` (`const char*`, yes), `out` ([`SeraphRegionHealth`](#seraphregionhealth)`*`, yes).
**Returns:** `0` on success, negative on error.

### `seraph_store_similarity_stats`

```c
char* seraph_store_similarity_stats(void* handle);
```

**Parameters:** `handle` (`void*`, yes).
**Returns:** JSON similarity distribution (free with `seraph_string_free`), or `NULL`. **Response:** [PLACEHOLDER: similarity stats JSON shape]

### `seraph_store_similarity_stats_struct`

```c
int seraph_store_similarity_stats_struct(void* handle, SeraphSimilarityStats* out);
```

**Parameters:** `handle` (`void*`, yes), `out` ([`SeraphSimilarityStats`](#seraphsimilaritystats)`*`, yes).
**Returns:** `0` on success, negative on error.

### `seraph_store_resolve_tau`

```c
float seraph_store_resolve_tau(void* handle);
```

**Parameters:** `handle` (`void*`, yes).
**Returns:** Auto-resolved `tau` value for the store. **Errors:** sentinel on `NULL` handle.

---

## Intelligence

All functions return JSON strings (free with `seraph_string_free`), or `NULL` on
error (`NULL` handle / malformed args).

### `seraph_store_drift_report`

```c
char* seraph_store_drift_report(void* handle, size_t window, float threshold);
```

**Parameters:** `handle` (`void*`, yes), `window` (`size_t`, yes — recent-frame window), `threshold` (`float`, yes — drift trigger).
**Returns:** JSON drift report. **Response:** [PLACEHOLDER: drift report JSON shape]

### `seraph_store_contradictions`

```c
char* seraph_store_contradictions(void* handle, float min_divergence);
```

**Parameters:** `handle` (`void*`, yes), `min_divergence` (`float`, yes — minimum semantic divergence to flag).
**Returns:** JSON contradiction list. **Response:** [PLACEHOLDER: contradictions JSON shape]

### `seraph_store_evolution`

```c
char* seraph_store_evolution(void* handle, const char* seed_frame_id);
```

**Parameters:** `handle` (`void*`, yes), `seed_frame_id` (`const char*`, yes).
**Returns:** JSON evolution trace for the seed's region. **Response:** [PLACEHOLDER: evolution JSON shape]

### `seraph_store_merge_candidates`

```c
char* seraph_store_merge_candidates(void* handle, float tau);
```

**Parameters:** `handle` (`void*`, yes), `tau` (`float`, yes — similarity threshold for candidacy).
**Returns:** JSON list of candidate frame groups. **Response:** [PLACEHOLDER: merge candidates JSON shape]

---

## Consolidation

### `seraph_store_consolidate`

Consolidate a specified set of frames into one, emitting redirect frames for the
superseded sources. Consolidation-exempt statuses (Genesis, Seed, Redirect,
System) are skipped.

```c
char* seraph_store_consolidate(void* handle, const char* frame_ids_json);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `frame_ids_json` | `const char*` | yes | JSON array of frame ID strings to consolidate. |

**Returns:** JSON consolidation report (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` if `frame_ids_json` is malformed, the store is sealed, or the handle is `NULL`.

**Response:** [PLACEHOLDER: consolidation report JSON shape — merged_count, new_frame_id, redirect_frame_ids, superseded_ids]

**Example**

```c
char* report = seraph_store_consolidate(store, "[\"id1\",\"id2\"]");
seraph_string_free(report);
```

### `seraph_store_consolidate_all`

Consolidate every eligible merge group across the store.

```c
char* seraph_store_consolidate_all(void* handle);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |

**Returns:** JSON bulk-consolidation report (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` if the store is sealed or the handle is `NULL`.

**Response:** [PLACEHOLDER: bulk consolidation report JSON shape]

**Example**

```c
char* report = seraph_store_consolidate_all(store);
seraph_string_free(report);
```

### `seraph_store_supersede`

Soft-forget a single frame: flips its status to `Superseded` **in place**
(status is not part of the watermark). Unlike consolidation, this creates **no**
new frame and **no** redirect — and it does not reparent the frame's children,
which continue to chain to the now-superseded frame. Returns the same frame ID
echoed back. Consolidation-exempt frames (Genesis, Seed, Redirect, System) are
rejected.

```c
char* seraph_store_supersede(void* handle, const char* frame_id);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `frame_id` | `const char*` | yes | Frame ID to supersede. |

**Returns:** The same `frame_id` echoed back (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` if the frame is missing, is consolidation-exempt, or the handle is `NULL`.

**Example**

```c
char* id = seraph_store_supersede(store, fid);
if (!id) fprintf(stderr, "%s\n", seraph_last_error());
seraph_string_free(id);
```

---

## Typed Structural Edges

### `seraph_store_declare_relation`

Declare a typed relation kind before asserting edges of that kind.

```c
int seraph_store_declare_relation(void* handle, const char* name, uint32_t flags);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `name` | `const char*` | yes | Relation name. |
| `flags` | `uint32_t` | yes | Relation flag bitset. [PLACEHOLDER: flag bit meanings] |

**Returns:** `0` on success, negative on error.

**Errors:** Negative on duplicate name or `NULL` handle.

**Example**

```c
seraph_store_declare_relation(store, "cites", 0);
```

### `seraph_store_list_relations`

List all declared relation kinds.

```c
char* seraph_store_list_relations(void* handle);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |

**Returns:** JSON array of relation descriptors (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` if the handle is `NULL`.

**Response:** [PLACEHOLDER: relation descriptor JSON shape]

**Example**

```c
char* rels = seraph_store_list_relations(store);
seraph_string_free(rels);
```

### `seraph_store_assert_relation`

Assert a typed edge between two frames.

```c
int seraph_store_assert_relation(
    void* handle,
    const char* from_id, const char* to_id,
    const char* relation, float weight
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `from_id` | `const char*` | yes | Source frame ID. |
| `to_id` | `const char*` | yes | Target frame ID. |
| `relation` | `const char*` | yes | Declared relation name. |
| `weight` | `float` | yes | Edge weight. |

**Returns:** `0` on success, negative on error.

**Errors:** Negative if either frame is missing, the relation is undeclared, or the handle is `NULL`.

**Example**

```c
seraph_store_assert_relation(store, a, b, "cites", 1.0f);
```

### `seraph_store_retract_relation`

Retract a previously asserted typed edge.

```c
int seraph_store_retract_relation(
    void* handle,
    const char* from_id, const char* to_id,
    const char* relation
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `from_id` | `const char*` | yes | Source frame ID. |
| `to_id` | `const char*` | yes | Target frame ID. |
| `relation` | `const char*` | yes | Relation name. |

**Returns:** `0` on success, negative on error.

**Errors:** Negative if the edge does not exist or the handle is `NULL`.

**Example**

```c
seraph_store_retract_relation(store, a, b, "cites");
```

### `seraph_store_structural_neighbors`

List frames connected to a frame by typed edges.

```c
char* seraph_store_structural_neighbors(
    void* handle,
    const char* frame_id,
    const char* relations_json,
    const char* direction
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `frame_id` | `const char*` | yes | Center frame ID. |
| `relations_json` | `const char*` | no | JSON array of relation names, or `NULL` for all. |
| `direction` | `const char*` | yes | `"outgoing"`, `"incoming"`, or `"both"`. |

**Returns:** JSON array of neighbor frames/edges (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` if the frame is missing, `direction` is invalid, or `relations_json` is malformed.

**Response:** [PLACEHOLDER: structural neighbors JSON shape]

**Example**

```c
char* json = seraph_store_structural_neighbors(store, fid, NULL, "both");
seraph_string_free(json);
```

### `seraph_store_filter_by_relation`

Filter a hit list by structural-relation presence.

```c
char* seraph_store_filter_by_relation(
    void* handle,
    const char* hits_json,
    const char* relations_json,
    const char* direction,
    bool require_present
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `hits_json` | `const char*` | yes | JSON array of `{"frame_id": "...", "score": 0.9}` objects. |
| `relations_json` | `const char*` | yes | JSON array of relation names. |
| `direction` | `const char*` | yes | `"outgoing"`, `"incoming"`, or `"both"`. |
| `require_present` | `bool` | yes | If `true`, keep hits that HAVE a matching relation; if `false`, keep hits that LACK one. |

**Returns:** JSON array of `{"frame_id","score"}` objects (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on `NULL` handle or `NULL` `hits_json` / `relations_json`.

### `seraph_store_structural_expand`

Expand a hit list by one hop along structural relations (pipeline composition).

```c
char* seraph_store_structural_expand(
    void* handle,
    const char* hits_json,
    const char* relations_json,
    const char* direction,
    const char* score_combine
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `hits_json` | `const char*` | yes | JSON array of `{"frame_id": "...", "score": 0.9}` objects. |
| `relations_json` | `const char*` | yes | JSON array of relation names. |
| `direction` | `const char*` | yes | `"outgoing"`, `"incoming"`, or `"both"`. |
| `score_combine` | `const char*` | yes | `"multiply"`, `"min"`, `"max"`, or `"keep_input"` (unrecognized values default to `keep_input`). |

**Returns:** JSON array of `{"frame_id","score"}` objects (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on `NULL` handle or `NULL` `hits_json` / `relations_json`.

### `seraph_store_gradient`

Compute the semantic gradient along an ordered chain of frame IDs.

```c
char* seraph_store_gradient(void* handle, const char* chain_json);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `chain_json` | `const char*` | yes | JSON array of frame-id strings. |

**Returns:** JSON array of `{"frame_id": "...", "similarity": 0.9, "delta": 0.1}` objects, one per chain step (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on `NULL` handle, `NULL` `chain_json`, or malformed JSON.

### `seraph_store_constitutive_score`

Constitutive score for a chain: `1 - jaccard(chain_intermediates, midpoint_NN)`. Values below `0.20` indicate the chain carries constitutive information beyond what a midpoint search would surface.

```c
float seraph_store_constitutive_score(void* handle, const char* chain_json);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `chain_json` | `const char*` | yes | JSON array of frame-id strings. |

**Returns:** The `float` constitutive score.

**Errors:** `f32::NAN` sentinel on `NULL` handle, `NULL` `chain_json`, or malformed JSON — check `seraph_last_error()`.

---

## Provenance & Sealing

### `seraph_store_verify`

Verify the watermark chain.

```c
int seraph_store_verify(void* handle, char** out_failures_json);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `out_failures_json` | `char**` | yes | Out: JSON array of failure descriptions when invalid. Free with `seraph_string_free`. |

**Returns:** `0` if the chain is valid; non-zero if invalid.

**Errors:** When invalid, `out_failures_json` is populated with the failing frames.

**Response:** [PLACEHOLDER: failures JSON shape — array of {frame_id, reason}]

**Example**

```c
char* fails = NULL;
if (seraph_store_verify(store, &fails) != 0) { puts(fails); seraph_string_free(fails); }
```

### `seraph_store_seal`

> 🔒 **Licensed.** Requires Commercial or Enterprise. Free-tier calls return `NULL` with an error set.

Seal the store. All subsequent writes fail.

```c
char* seraph_store_seal(void* handle, const char* reason);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `reason` | `const char*` | yes | Human-readable seal reason. |

**Returns:** Seal frame ID (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on Free tier, already-sealed store, or `NULL` handle.

**Example**

```c
char* sid = seraph_store_seal(store, "final release");
seraph_string_free(sid);
```

### `seraph_store_is_sealed`

Check whether the store is sealed. Available to all tiers.

```c
int seraph_store_is_sealed(void* handle);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |

**Returns:** `1` if sealed, `0` if not. (All tiers can open and read sealed stores.)

**Errors:** `0` is also returned for a `NULL` handle — check `seraph_last_error()` if ambiguous.

**Example**

```c
if (seraph_store_is_sealed(store)) puts("sealed");
```

### `seraph_store_writer_audit`

Audit of writers/attestations in the store.

```c
char* seraph_store_writer_audit(void* handle);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |

**Returns:** JSON writer-audit report (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` if the handle is `NULL`.

**Response:** [PLACEHOLDER: writer audit JSON shape]

**Example**

```c
char* json = seraph_store_writer_audit(store);
seraph_string_free(json);
```

### `seraph_store_verify_attestation`

Verify the store's attestation signatures.

```c
char* seraph_store_verify_attestation(void* handle);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |

**Returns:** JSON attestation-verification result (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` if the handle is `NULL`.

**Response:** [PLACEHOLDER: attestation verification JSON shape]

**Example**

```c
char* json = seraph_store_verify_attestation(store);
seraph_string_free(json);
```

### `seraph_store_genesis_seed`

Fetch the genesis seed bytes.

```c
int seraph_store_genesis_seed(void* handle, uint8_t** out_seed, size_t* out_len);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `out_seed` | `uint8_t**` | yes | Out: seed bytes. Free with `seraph_bytes_free`. |
| `out_len` | `size_t*` | yes | Out: seed length. |

**Returns:** `0` on success, negative on error.

**Errors:** Negative if the handle is `NULL`.

**Example**

```c
uint8_t* seed = NULL; size_t len = 0;
if (seraph_store_genesis_seed(store, &seed, &len) == 0) seraph_bytes_free(seed, len);
```

### `seraph_store_genesis_attestation`

Fetch the genesis attestation.

```c
char* seraph_store_genesis_attestation(void* handle);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |

**Returns:** JSON genesis attestation (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` if the handle is `NULL`.

**Response:** [PLACEHOLDER: genesis attestation JSON shape]

**Example**

```c
char* json = seraph_store_genesis_attestation(store);
seraph_string_free(json);
```

---

## Anchoring

### `seraph_store_append_tip_anchor`

> 🔒 **Licensed.** Requires Commercial or Enterprise. Free-tier calls return `NULL` with an error set.

Append a time-anchor proof covering a tip frame.

```c
char* seraph_store_append_tip_anchor(
    void* handle,
    const char* backend,
    const uint8_t* token, size_t token_len,
    const char* tip_frame_id,
    const char* status,
    const char* supersedes
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `backend` | `const char*` | yes | Anchor backend, e.g. `"opentimestamps"`, `"rfc3161"`. |
| `token` | `const uint8_t*` | yes | Proof/token bytes. |
| `token_len` | `size_t` | yes | Length of `token`. |
| `tip_frame_id` | `const char*` | yes | Frame ID this anchor covers. |
| `status` | `const char*` | yes | `"pending"` or `"confirmed"`. |
| `supersedes` | `const char*` | no | ID of a previous anchor to supersede, or `NULL`. |

**Returns:** Anchor frame ID (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on Free tier, unknown `backend`, invalid `status`, or `NULL` handle.

**Example**

```c
char* aid = seraph_store_append_tip_anchor(
    store, "rfc3161", token, token_len, tip, "pending", NULL);
seraph_string_free(aid);
```

### `seraph_store_list_tip_anchors`

List tip-anchor records. Available to all tiers.

```c
char* seraph_store_list_tip_anchors(void* handle, int resolved);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `resolved` | `int` | yes | Non-zero collapses supersession chains to the latest version. |

**Returns:** JSON array of tip-anchor records (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` if the handle is `NULL`.

**Response:** [PLACEHOLDER: tip anchor record JSON shape]

**Example**

```c
char* json = seraph_store_list_tip_anchors(store, 1);
seraph_string_free(json);
```

---

## Snapshot / Temporal

### `seraph_store_snapshot`

Capture a temporal snapshot of the store as of a timestamp.

```c
SeraphSnapshot* seraph_store_snapshot(void* handle, int64_t at_timestamp_us);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `at_timestamp_us` | `int64_t` | yes | Snapshot time, microseconds since epoch. |

**Returns:** Snapshot handle, or `NULL` on error. Free with `seraph_snapshot_free`.

**Errors:** `NULL` if the handle is `NULL`.

**Example**

```c
SeraphSnapshot* snap = seraph_store_snapshot(store, 1700000000000000LL);
```

### `seraph_snapshot_frame_ids`

List the frame IDs visible in a snapshot.

```c
char* seraph_snapshot_frame_ids(const SeraphSnapshot* snapshot);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `snapshot` | `const SeraphSnapshot*` | yes | Snapshot handle. |

**Returns:** JSON array of frame IDs (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` if the snapshot is `NULL`.

**Example**

```c
char* ids = seraph_snapshot_frame_ids(snap);
seraph_string_free(ids);
```

### `seraph_snapshot_timestamp`

Return a snapshot's timestamp.

```c
int64_t seraph_snapshot_timestamp(const SeraphSnapshot* snapshot);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `snapshot` | `const SeraphSnapshot*` | yes | Snapshot handle. |

**Returns:** Timestamp in microseconds since epoch. Returns a sentinel for a `NULL` snapshot.

**Errors:** Sentinel value on `NULL` snapshot.

**Example**

```c
int64_t ts = seraph_snapshot_timestamp(snap);
```

### `seraph_store_snapshot_search`

Search within a snapshot.

```c
char* seraph_store_snapshot_search(
    void* handle,
    const SeraphSnapshot* snapshot,
    const float* query_embedding, size_t query_len,
    size_t top_k
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `snapshot` | `const SeraphSnapshot*` | yes | Snapshot to search within. |
| `query_embedding` | `const float*` | yes | Query vector. |
| `query_len` | `size_t` | yes | Vector length. |
| `top_k` | `size_t` | yes | Maximum number of hits. |

**Returns:** JSON array of hits as of the snapshot (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on dimension mismatch, `NULL` snapshot, or `NULL` handle.

**Response:** [PLACEHOLDER: snapshot search hits JSON shape]

**Example**

```c
char* json = seraph_store_snapshot_search(store, snap, q, dim, 10);
seraph_string_free(json);
```

### `seraph_snapshot_free`

Free a snapshot handle.

```c
void seraph_snapshot_free(SeraphSnapshot* snapshot);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `snapshot` | `SeraphSnapshot*` | yes | Snapshot to free. |

**Returns:** Nothing.

**Errors:** None. Passing `NULL` is a no-op.

**Example**

```c
seraph_snapshot_free(snap);
```

---

## Events & Subscriptions

### Callback type

```c
typedef void (*SeraphEventCallback)(const char* event_json, void* user_data);
```

The callback receives a JSON event payload and the `user_data` pointer supplied
at subscription. [PLACEHOLDER: event_json shape per event type]

### `seraph_store_subscribe`

Subscribe a callback to store events.

```c
uint64_t seraph_store_subscribe(
    void* handle,
    const char* event_name,
    SeraphEventCallback callback,
    void* user_data
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `event_name` | `const char*` | yes | `"promote"`, `"contradiction"`, `"drift"`, `"consolidation"`, `"milestone"`, or `"*"` for all. |
| `callback` | `SeraphEventCallback` | yes | Callback invoked per event. |
| `user_data` | `void*` | no | Opaque pointer passed back to the callback. |

**Returns:** Subscriber ID (non-zero), or `0` on error.

**Errors:** `0` if `event_name` is unknown, `callback` is `NULL`, or the handle is `NULL`.

**Example**

```c
uint64_t sub = seraph_store_subscribe(store, "promote", on_event, NULL);
```

### `seraph_store_unsubscribe`

Remove a subscription.

```c
int seraph_store_unsubscribe(void* handle, uint64_t subscriber_id);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `subscriber_id` | `uint64_t` | yes | ID returned by `seraph_store_subscribe`. |

**Returns:** `0` on success, negative on error.

**Errors:** Negative if the ID is unknown or the handle is `NULL`.

**Example**

```c
seraph_store_unsubscribe(store, sub);
```

### `seraph_store_subscriber_count`

Return the number of active subscribers.

```c
size_t seraph_store_subscriber_count(void* handle);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |

**Returns:** Active subscriber count. `0` for a `NULL` handle.

**Errors:** No error channel; `0` may indicate a `NULL` handle.

**Example**

```c
size_t n = seraph_store_subscriber_count(store);
```

---

## Encoder

### `seraph_encoder_create`

Create a standalone encoder. Downloads and caches the model on first use.

```c
void* seraph_encoder_create(const char* model_id);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `model_id` | `const char*` | yes | Embedding model ID, e.g. `"BAAI/bge-small-en-v1.5"`. |

**Returns:** Encoder handle, or `NULL` on error.

**Errors:** `NULL` if the model is unknown or download fails.

**Example**

```c
void* enc = seraph_encoder_create("BAAI/bge-small-en-v1.5");
if (!enc) { /* handle error */ }
```

### `seraph_encoder_encode`

Encode one text into a freshly allocated embedding buffer.

```c
int seraph_encoder_encode(void* handle, const char* text, float** out_embedding, size_t* out_dim);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Encoder handle. |
| `text` | `const char*` | yes | Text to encode (UTF-8). |
| `out_embedding` | `float**` | yes | Out: embedding. Free with `seraph_floats_free`. |
| `out_dim` | `size_t*` | yes | Out: dimensionality. |

**Returns:** `0` on success, negative on error.

**Errors:** Negative if the handle is `NULL` or encoding fails.

**Example**

```c
float* emb = NULL; size_t dim = 0;
if (seraph_encoder_encode(enc, "the cat sat", &emb, &dim) == 0)
    seraph_floats_free(emb, dim);
```

### `seraph_encoder_encode_into`

Zero-copy variant: encode into a caller-owned buffer.

```c
int seraph_encoder_encode_into(void* handle, const char* text, float* buf, size_t buf_len, size_t* out_written);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Encoder handle. |
| `text` | `const char*` | yes | Text to encode. |
| `buf` | `float*` | yes | Caller-owned destination buffer. |
| `buf_len` | `size_t` | yes | Buffer capacity (elements). |
| `out_written` | `size_t*` | yes | Out: elements written. |

**Returns:** `0` on success, negative on error.

**Errors:** Negative if `buf_len` is smaller than the model dimension or the handle is `NULL`.

**Example**

```c
float buf[1024]; size_t written = 0;
seraph_encoder_encode_into(enc, "the cat sat", buf, 1024, &written);
```

### `seraph_encoder_encode_batch`

Encode a batch of texts, returning JSON.

```c
char* seraph_encoder_encode_batch(void* handle, const char* texts_json);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Encoder handle. |
| `texts_json` | `const char*` | yes | JSON array of strings. |

**Returns:** JSON array of embedding arrays (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` if `texts_json` is malformed or the handle is `NULL`.

**Example**

```c
char* json = seraph_encoder_encode_batch(enc, "[\"a\",\"b\"]");
seraph_string_free(json);
```

### `seraph_encoder_encode_batch_flat`

Batch-encode texts into a contiguous `float*` buffer in a single batched forward
pass, amortising GPU kernel-launch and per-call marshalling overhead. **This is
the recommended batch encode path** — it matches native Rust throughput
(~600 enc/s on GPU). Output layout:
`[emb0[0..dim], emb1[0..dim], ..., emb_{count-1}[0..dim]]`.

```c
int seraph_encoder_encode_batch_flat(
    void* handle,
    const char** texts, size_t count,
    float** out_embeddings, size_t* out_dim
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Encoder handle. |
| `texts` | `const char**` | yes | Array of C strings. |
| `count` | `size_t` | yes | Number of texts. |
| `out_embeddings` | `float**` | yes | Out: contiguous `[count × dim]` array. Free with `seraph_floats_free(out_embeddings, count * out_dim)`. |
| `out_dim` | `size_t*` | yes | Out: embedding dimension. |

**Returns:** `0` on success, `-1` on error.

**Errors:** `-1` if the handle is `NULL` or encoding fails.

**Example**

```c
const char* texts[] = {"first document", "second document", "third document"};
float* embeddings = NULL; size_t dim = 0;
if (seraph_encoder_encode_batch_flat(enc, texts, 3, &embeddings, &dim) == 0) {
    for (size_t i = 0; i < 3; i++) {
        float* emb_i = embeddings + (i * dim);  // use emb_i[0..dim]
    }
    seraph_floats_free(embeddings, 3 * dim);
}
```

### `seraph_encoder_close`

Close an encoder and free its resources.

```c
void seraph_encoder_close(void* handle);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Encoder handle. |

**Returns:** Nothing.

**Errors:** None. Passing `NULL` is a no-op.

**Example**

```c
seraph_encoder_close(enc);
```

---

## Batch Operations

### `seraph_store_begin_batch`

Begin a batch write session. Index updates are deferred until commit.

```c
int seraph_store_begin_batch(void* handle);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |

**Returns:** `0` on success, negative on error.

**Errors:** Negative if a batch is already open or the handle is `NULL`.

**Example**

```c
seraph_store_begin_batch(store);
/* many puts ... */
seraph_store_commit(store);
```

### `seraph_store_commit`

Commit the open batch and update all indices.

```c
int seraph_store_commit(void* handle);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |

**Returns:** `0` on success, negative on error.

**Errors:** Negative if no batch is open or the handle is `NULL`.

**Example**

```c
seraph_store_commit(store);
```

---

## Memory Management

Every owned pointer returned across the FFI has a matching free function. Do not
mix allocators — always free with the listed function.

| Function | Frees |
|----------|-------|
| `seraph_string_free(char* s)` | Any string returned by `seraph_*` functions |
| `seraph_string_array_free(char** arr, size_t count)` | String arrays |
| `seraph_bytes_free(uint8_t* ptr, size_t len)` | Byte arrays from `get_content`, `genesis_seed` |
| `seraph_floats_free(float* ptr, size_t len)` | Float arrays from `get_embedding`, `encode` |
| `seraph_hits_free(SeraphHit* hits, size_t count)` | Hit arrays from search functions |
| `seraph_snapshot_free(SeraphSnapshot* s)` | Snapshot handles |
| `seraph_store_release_frame(SeraphFrameView* v)` | Borrowed frame views |

---

## Structs Reference

### `SeraphHit`

Returned (as an array) by all search functions. Free the array with
`seraph_hits_free`.

```c
typedef struct {
    char* frame_id;       // owned string
    float score;          // raw similarity
    float confidence;     // score with path-length decay
    uint32_t path_length; // BFS depth from entry point
} SeraphHit;
```

### `SeraphStats`

Filled by `seraph_store_stats`.

```c
typedef struct {
    size_t total_frames;
    size_t active_frames;
    size_t eigenframes;
    size_t superseded;
    size_t redirects;
} SeraphStats;
```

### `SeraphRegionHealth`

Filled by `seraph_store_region_health_struct`.

```c
typedef struct {
    float   coherence;
    uint8_t direction;        // 0 = stable, 1 = consolidating, 2 = fragmenting
    uint8_t candidate_split;  // 1 if the region is a split candidate, else 0
} SeraphRegionHealth;
```

### `SeraphSimilarityStats`

Filled by `seraph_store_similarity_stats_struct`.

```c
typedef struct {
    size_t count;
    float  mean;
    float  p75;
    float  p90;
    float  p95;
    float  p99;
} SeraphSimilarityStats;
```

### `SeraphFrameView`

Returned by `seraph_store_borrow_frame`. All pointers point into engine-owned
memory and must **not** be freed individually — release the whole view with
`seraph_store_release_frame`. The struct carries three trailing private pointers
used for cleanup; treat the view as opaque beyond the documented fields and
never construct one yourself. Field order below matches the `#[repr(C)]` layout
exactly.

```c
typedef struct {
    const char*    id;
    const char*    parent_id;     // NULL if no parent
    const float*   embedding;
    size_t         embedding_dim;
    const uint8_t* content;
    size_t         content_len;
    uint32_t       in_degree;
    uint8_t        status;        // 0=Genesis 1=Seed 2=Active 3=Eigenframe 4=Superseded 5=Redirect 6=System
    int64_t        timestamp;
    // ... three private cleanup pointers follow (do not access) ...
} SeraphFrameView;
```

> **Note:** this `status` byte uses a different numbering than the Python
> `FrameStatus` enum. The values above are authoritative for the C ABI.

---

Copyright (c) 2025-2026 Tree D Interactive LLC. All rights reserved.
