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
20. [Federation](#federation)
21. [Memory Management](#memory-management)
22. [Structs Reference](#structs-reference)

---

## Store Lifecycle

### `seraph_store_create`

Create a new store on disk.

```c
void* seraph_store_create(const char* path, const char* model_id, const char* config_json);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `path` | `const char*` | yes | Filesystem path for the `.sfg` file. |
| `model_id` | `const char*` | yes | Embedding model ID; `NULL` for the default model. |
| `config_json` | `const char*` | yes | JSON config overrides; `NULL` for the defaults. |

**Returns:** Opaque store handle (`void*`). `NULL` on error.

**Errors:** `NULL` if `path` is unwritable or already exists, `model_id` is unknown, or `config_json` is malformed.

**Example**

```c
void* store = seraph_store_create("memory.sfg", "BAAI/bge-small-en-v1.5", NULL);
if (!store) { fprintf(stderr, "create failed: %s\n", seraph_last_error()); return -1; }
```

### `seraph_store_create_with_writer`

Create a store that signs every frame with an app-supplied writer identity and records the derived public key in the genesis attestation, so the chain verifies later without the key. The seed never leaves the caller.

```c
void* seraph_store_create_with_writer(
    const char* path,
    const char* model_id,
    const char* config_json,
    const uint8_t* writer_seed
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `path` | `const char*` | yes | Filesystem path for the .sfg file. |
| `model_id` | `const char*` | yes | Embedding model ID; `NULL` for the default model. |
| `config_json` | `const char*` | yes | JSON config overrides; `NULL` for the defaults. |
| `writer_seed` | `const uint8_t*` | yes | 32-byte Ed25519 private seed held by the caller; NULL on C creates without a writer identity. |

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

Open an existing store with an app-supplied writer identity, so frames appended in this session are signed with it. Reading and verification need no key, and the seed is never persisted or transmitted.

```c
void* seraph_store_open_with_writer(const char* path, const uint8_t* writer_seed);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `path` | `const char*` | yes | Path to the .sfg file. |
| `writer_seed` | `const uint8_t*` | yes | 32-byte Ed25519 private seed held by the caller. |

**Returns:** Store handle, or `NULL` on error.

**Errors:** `NULL` if the file is missing, corrupt, or **encrypted** (use `seraph_store_open_encrypted`).

**Example**

```c
uint8_t seed[32] = { /* app-held Ed25519 private seed */ };
void* store = seraph_store_open_with_writer("memory.sfg", seed);
if (!store) { /* handle error */ }
```

### `seraph_store_close`

Close the store and flush all pending writes. The store object raises on any subsequent operation.

```c
void seraph_store_close(void* handle);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |

**Returns:** Nothing.

**Errors:** None. Passing `NULL` is a no-op.

**Example**

```c
seraph_store_close(store);
```

### `seraph_store_sync`

Flush pending writes to disk without closing (`flush` on the Python surface).

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

Legacy visit budget for a search as a multiple of `top_k`, used only when the absolute budget (`set_search_max_visits`) is `0`. Default `20`. Runtime only.

```c
int seraph_store_set_search_visit_cap_multiplier(void* handle, uint32_t multiplier);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `multiplier` | `uint32_t` | yes | Multiple of `top_k` for the legacy visit budget. |

**Returns:** `0` on success, `-1` on error.

**Errors:** `-1` if the handle is `NULL`.

**Example**

```c
seraph_store_set_search_visit_cap_multiplier(store, 10);  // wider legacy budget
```

### `seraph_store_set_search_entry_k`

Entry-point floor: at least this many eigenframes become entry points for a search. `0` restores the default of `2`. Runtime only.

```c
int seraph_store_set_search_entry_k(void* handle, size_t k);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `k` | `size_t` | yes | Entry rank floor; `0` = default. |

**Returns:** `0` on success, `-1` on error.

**Errors:** `-1` if the handle is `NULL`.

### `seraph_store_set_search_entry_gap`

Entry-point breadth: every eigenframe within `gap` of the best entry score becomes an entry point, in addition to the floor. `0` uses the floor alone. Default `0.05`. Runtime only.

```c
int seraph_store_set_search_entry_gap(void* handle, float gap);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `gap` | `float` | yes | Score gap below the best entry; `0` disables. |

**Returns:** `0` on success, `-1` on error.

**Errors:** `-1` if the handle is `NULL`.

### `seraph_store_set_search_use_stream`

Run search through the bounded walk, the default at every store size, or with on=False through the full traversal below search_stream_threshold, the completeness reference. Runtime only.

```c
int seraph_store_set_search_use_stream(void* handle, bool on, size_t beam);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `on` | `bool` | yes | True (the default) runs the bounded walk; False keeps the full traversal below search_stream_threshold. |
| `beam` | `size_t` | yes | Beam width; 0 = unbounded. |

**Returns:** `0` on success, `-1` on error.

**Errors:** `-1` if the handle is `NULL`.

### `seraph_store_set_search_stream_threshold`

With the bounded walk switched off, the store size from which search uses it anyway. Default 50000; 0 disables the size rule. Runtime only.

```c
int seraph_store_set_search_stream_threshold(void* handle, size_t n);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `n` | `size_t` | yes | Members searched under `"top"`. |

**Returns:** `0` on success, `-1` on error.

**Errors:** `-1` if the handle is `NULL`.

### `seraph_store_set_seed_term`

Weight of the seed term in parent selection. When the store has seeds, a new frame's parent is chosen by its similarity to the candidate plus `beta` times its similarity to the candidate's seed, which settles near-ties between seed regions. It affects the parent only; the stored embedding, the similarity edges and search results are unchanged. Default `0.6`; `0` disables it. Per-frame opt-out: `seed_term=False` on the put calls. Runtime only, not persisted.

```c
int seraph_store_set_seed_term(void* handle, float beta);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `beta` | `float` | yes | Term weight (`>= 0`; `0` disables). |

**Returns:** `0` on success, `-1` on error.

**Errors:** `-1` if the handle is `NULL`.

### `seraph_store_set_commit_ceiling_margin`

Stop line of the walk that picks a new frame's parent and similarity edges, as a fraction of the best score it holds. Lower values look further for a parent and cost ingest time; higher values ingest faster with slightly worse placement. Default `0.85`. Runtime only, not persisted.

```c
int seraph_store_set_commit_ceiling_margin(void* handle, float m);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `m` | `float` | yes | Ceiling slack; `1.0` is the tight stop. |

**Returns:** `0` on success, `-1` on error.

**Errors:** `-1` if the handle is `NULL`.

### `seraph_store_set_commit_ceiling_slack`

Set the stop-line slack of commit-time placement, the cosine form of commit_ceiling_margin: placement stops once the best remaining candidate is no more than the k-th best minus slack. Default 0.05, the line search stops on; 0 falls back to the multiplicative margin. Runtime only; frames store their parent and edges, so changing it never forces a migration.

```c
int seraph_store_set_commit_ceiling_slack(void* handle, float slack);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `slack` | `float` | yes | Cosine distance below the k-th best candidate at which placement stops; 0 falls back to the margin. |

**Returns:** `0` on success, `-1` on error.

**Errors:** `-1` if the handle is `NULL`.

### `seraph_store_set_sim_edge_policy`

How a new frame chooses its similarity edges. By default the top `edge_budget` content frames above `tau_similarity`, the parent included and never a Genesis or Seed frame. `occlusion > 0` drops a candidate when a nearer kept neighbour already covers it (`1.0` strict; larger keeps more); `pool` widens the candidate list at ingest cost. Runtime only, not persisted; each frame records the edges it chose.

```c
int seraph_store_set_sim_edge_policy(
    void* handle,
    bool routing,
    bool parent,
    float occlusion,
    size_t pool
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `routing` | `bool` | yes | Allow edges to Genesis/Seed frames. |
| `parent` | `bool` | yes | Allow an edge to the frame's own parent. |
| `occlusion` | `float` | yes | `0` = top-k; `> 0` = occlusion pruning with this alpha. |
| `pool` | `size_t` | yes | Candidates the commit walk returns; `0` = `edge_budget + 1`. |

**Returns:** `0` on success, `-1` on error.

**Errors:** `-1` if the handle is `NULL`.

### `seraph_store_set_search_rank_mode`

How result slots are filled by `search` and the reads built on it. `"cosine"` (the default) ranks every slot by similarity. `"two_channel"` gives the last `round(share * top_k)` slots to frames reached by a descent from the `doors` nearest eigenframes, bounded by `budget` frames, that the similarity ranking lacks. Runtime only, not persisted.

```c
int seraph_store_set_search_rank_mode(
    void* handle,
    uint8_t mode,
    float share,
    size_t doors,
    size_t budget
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `mode` | `uint8_t` | yes | Routing mode: `"best"`, `"top"` or `"all"`. |
| `share` | `float` | yes | Fraction of `top_k` given to the descent channel (two_channel only). |
| `doors` | `size_t` | yes | Eigenframes the descent enters at (two_channel only). |
| `budget` | `size_t` | yes | Frames the descent admits before stopping (two_channel only). |

**Returns:** `0` on success, `-1` on error (unknown mode).

**Errors:** `-1` if the handle is `NULL`.

### `seraph_store_set_search_max_visits`

Absolute visit budget for a search, independent of `top_k`. Default `50000`; `0` falls back to `top_k` times the legacy multiplier. Runtime only.

```c
int seraph_store_set_search_max_visits(void* handle, size_t v);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `v` | `size_t` | yes | Absolute visit budget; `0` = multiplier fallback. |

**Returns:** `0` on success, `-1` on error.

**Errors:** `-1` if the handle is `NULL`.

### `seraph_store_set_search_ceiling_slack`

How far past the current top-k a search keeps looking before it stops, as a cosine distance below the k-th best score. Smaller values stop sooner; larger values find more on weak-match queries at more cost. Default `0.05`; `0.10` is the completeness setting. `0` uses the multiplicative margin instead (`set_search_ceiling_margin`). Runtime only, not persisted.

```c
int seraph_store_set_search_ceiling_slack(void* handle, float slack);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `slack` | `float` | yes | Cosine slack below the k-th best; `0` = multiplicative margin. |

**Returns:** `0` on success, `-1` on error.

**Errors:** `-1` if the handle is `NULL`.

### `seraph_store_set_door_index`

Use the door index for entry-point selection: an index over the eigenframes that replaces the full scan once the store has `door_index_min` of them (default `4096`). Default on. Entry selection is then approximate to the index's accuracy (see `set_door_index_ef`); replay and as-of reads always use the exact scan. Turning it on builds the index at once; off drops it. Runtime only, not persisted.

```c
int seraph_store_set_door_index(void* handle, bool on);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `on` | `bool` | yes | `True` to pick entry points through the door index. |

**Returns:** `0` on success, `-1` on error.

**Errors:** `-1` if the handle is `NULL`.

### `seraph_store_set_door_index_ef`

Candidates the door index examines per query. Higher is closer to the exact scan at more cost per query. Default `128`. Runtime only, not persisted.

```c
int seraph_store_set_door_index_ef(void* handle, size_t ef);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `ef` | `size_t` | yes | Candidates per query (at least the entry scan's 64). |

**Returns:** `0` on success, `-1` on error.

**Errors:** `-1` if the handle is `NULL`.

### `seraph_store_vector_precision`

The precision each frame's vector is stored at, fixed at create (`StoreConfig(vector_precision=...)`): `"codes4"` (4-bit codes, the default), `"fp32"`, `"fp16"`, `"bf16"` or `"fp64"`. Stores below format 5 report `"codes4"`.

```c
char* seraph_store_vector_precision(void* handle);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |

**Returns:** the precision name (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on a `NULL` handle or a failed operation; `seraph_last_error` has the message.

### `seraph_store_resident_precision`

The precision search scores from: `"fp32"`, `"fp16"`, `"bf16"` or `"frame"` (no resident copy). The default is set at create (`StoreConfig(resident_precision=...)`) or derived from the vector precision when unset; `set_resident_precision` changes it at runtime.

```c
char* seraph_store_resident_precision(void* handle);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |

**Returns:** the precision name (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on a `NULL` handle or a failed operation; `seraph_last_error` has the message.

### `seraph_store_set_resident_precision`

Change the precision search scores from and rebuild the resident copy at once. `"fp16"` and `"bf16"` halve its memory; `"frame"` keeps no resident copy, at some cost per search. Runtime only; the store's default is the one set at create.

```c
int seraph_store_set_resident_precision(void* handle, const char* name);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `name` | `const char*` | yes | `"fp32"`, `"fp16"`, `"bf16"` or `"frame"`. |

**Returns:** `0` on success, `-1` on error (unknown or reserved name).

**Errors:** `-1` if the handle is `NULL`.

### `seraph_store_set_search_ceiling_margin`

Multiplicative stop line for search, used only when the slack (`set_search_ceiling_slack`) is `0`: search stops once nothing left can score above `m` times the k-th best. Default `0.85`. Runtime only, not persisted.

```c
int seraph_store_set_search_ceiling_margin(void* handle, float m);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `m` | `float` | yes | Fraction of the k-th best; `1.0` is the tight stop. |

**Returns:** `0` on success, `-1` on error.

**Errors:** `-1` if the handle is `NULL`.

### `seraph_store_set_search_arena`

Keep a contiguous resident copy of the vectors for search (default on). Off saves that memory at more cost per search; results are unchanged. Runtime only.

```c
int seraph_store_set_search_arena(void* handle, bool on);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `on` | `bool` | yes | Enable or disable. |

**Returns:** `0` on success, `-1` on error.

**Errors:** `-1` if the handle is `NULL`.

### `seraph_store_set_search_adjacency`

Use the pre-resolved search adjacency (default on). Results are unchanged. Runtime only.

```c
int seraph_store_set_search_adjacency(void* handle, bool on);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `on` | `bool` | yes | Enable or disable. |

**Returns:** `0` on success, `-1` on error.

**Errors:** `-1` if the handle is `NULL`.

### `seraph_store_set_walk_profile`

Diagnostic: enable walk profiling for this thread (no overhead when off).

```c
int seraph_store_set_walk_profile(void* handle, bool on);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `on` | `bool` | yes | Enable or disable. |

**Returns:** `0` on success, `-1` on error.

**Errors:** `-1` if the handle is `NULL`.

### `seraph_store_take_walk_profile`

Diagnostic: read and reset the walk-profile accumulators.

```c
char* seraph_store_take_walk_profile(void* handle);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |

**Returns:** JSON `{"resolve_ns", "visited_ns", "score_ns", "push_ns", "visit_count"}` (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on a `NULL` handle or a failed operation; `seraph_last_error` has the message.

### `seraph_store_entry_count`

Diagnostic: the number of entry points a search for `query` would start from.

```c
int64_t seraph_store_entry_count(void* handle, const float* query_embedding, size_t query_len);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `query_embedding` | `const float*` | yes | Query vector. |
| `query_len` | `size_t` | yes | Length of `query`. |

**Returns:** Entry count (`>= 0`), or `-1` on error.

**Errors:** `NULL` on a `NULL` handle or a failed operation; `seraph_last_error` has the message.

### `seraph_store_last_search_visits`

Diagnostic: frames visited by the last `search_stream` call.

```c
int64_t seraph_store_last_search_visits(void* handle);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |

**Returns:** Visit count (`>= 0`), or `-1` on error.

**Errors:** `NULL` on a `NULL` handle or a failed operation; `seraph_last_error` has the message.

### `seraph_store_last_commit_visits`

The number of frames examined to place the last frame committed. An operations cost counter, the ingest counterpart of last_search_visits.

```c
int64_t seraph_store_last_commit_visits(void* handle);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |

**Returns:** `int64_t`.

**Errors:** `NULL` on a `NULL` handle or a failed operation; `seraph_last_error` has the message.

### `seraph_migrate_store`

Rebuild a store into a fresh one at `dst_path`: every frame is re-committed with its identity intact (id, timestamp, content, embedding, metadata, status references) and its parent chosen against the live destination, so sentinels and redirects that name frames by id stay valid. Use it to correct an old topology or to carry a store to the current format or another `vector_precision` (`config`). Embeddings are carried as they are; use `reencode_store` to change them. Watermarks are recomputed, so they differ from the source; the destination verifies. Free function; the Python binding releases the GIL for the whole migration.

```c
char* seraph_migrate_store(const char* src_path, const char* dst_path);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `src_path` | `const char*` | yes | Source `.sfg` path. Opened read-only. |
| `dst_path` | `const char*` | yes | Destination `.sfg` path. Created fresh. |

**Returns:** JSON object `{content, seeds, sentinels, superseded, redirects, skipped, on_genesis}` (free with `seraph_string_free`), or `NULL` on error. A high `on_genesis` relative to `content` indicates a star rather than a tree.

**Errors:** `NULL` if either path is null, the source cannot be opened, or the destination model does not match the source.

**Example**

```c
char* rep = seraph_migrate_store("old.sfg", "new.sfg");
// rep -> {"content":1335,"seeds":0,"sentinels":22,...,"on_genesis":1}
seraph_string_free(rep);
```

### `seraph_reencode_store`

Build a fresh store from the content of an existing one, re-encoding every frame with a new model. Frames are ingested in source commit order, so the destination's topology is the one a per-frame ingest would produce.

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
| `src_path` | `const char*` | yes | Source .sfg path. |
| `dst_path` | `const char*` | yes | Destination .sfg path; must not exist. |
| `model_id` | `const char*` | yes | Embedding model ID; `NULL` for the default model. |
| `eigen_threshold` | `uint32_t` | yes | Eigenframe promotion threshold for the destination. |
| `n_max` | `size_t` | yes | Stop after this many frames; 0 means all. |
| `pbatch` | `size_t` | yes | Write buffering batch size; does not affect topology. |
| `ebatch` | `size_t` | yes | Encode batch size. |
| `out_n` | `size_t*` | yes | Out: n. |

**Returns:** `0` on success, `-1` on error.

**Errors:** `-1` (with `seraph_last_error()` set) on `NULL` path/model strings, an unreadable source, or an encode/ingest failure.

---

## Encryption

> 🔒 **Licensed.** Requires Commercial or Enterprise. Free-tier calls return `NULL` with an error set.

### `seraph_store_create_encrypted`

Create a store encrypted at rest with AES-256-GCM. Content is encrypted under a key derived from the passphrase with Argon2id; watermarks and parent links stay in the clear so the chain verifies without the passphrase. Licensed feature.

```c
void* seraph_store_create_encrypted(
    const char* path,
    const char* model_id,
    const char* config_json,
    const uint8_t* passphrase,
    size_t passphrase_len
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `path` | `const char*` | yes | Filesystem path for the .sfg file. |
| `model_id` | `const char*` | yes | Embedding model ID; `NULL` for the default model. |
| `config_json` | `const char*` | yes | JSON config overrides; `NULL` for the defaults. |
| `passphrase` | `const uint8_t*` | yes | Passphrase the content key is derived from. |
| `passphrase_len` | `size_t` | yes | Length of `passphrase`. |

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

Open an encrypted store with a passphrase.

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
| `passphrase` | `const uint8_t*` | yes | Decryption passphrase. |
| `passphrase_len` | `size_t` | yes | Length of `passphrase`. |

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

Ingest content with a pre-computed embedding.

```c
char* seraph_store_put(
    void* handle,
    const uint8_t* content,
    size_t content_len,
    const float* embedding,
    size_t embedding_len
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `content` | `const uint8_t*` | yes | Raw content bytes. |
| `content_len` | `size_t` | yes | Length of `content`. |
| `embedding` | `const float*` | yes | Embedding vector; length must equal the store dimension. |
| `embedding_len` | `size_t` | yes | Length of `embedding`. |

**Returns:** Frame ID string (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` if the embedding dimension mismatches the store, the store is sealed, or the handle is `NULL`.

**Example**

```c
char* fid = seraph_store_put(store, (const uint8_t*)"hello", 5, emb, dim);
if (!fid) { /* handle error */ }
seraph_string_free(fid);
```

### `seraph_store_put_with_metadata`

Ingest content with an embedding and JSON metadata.

```c
char* seraph_store_put_with_metadata(
    void* handle,
    const uint8_t* content,
    size_t content_len,
    const float* embedding,
    size_t embedding_len,
    const char* metadata_json
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `content` | `const uint8_t*` | yes | Raw content bytes. |
| `content_len` | `size_t` | yes | Length of `content`. |
| `embedding` | `const float*` | yes | Embedding vector. |
| `embedding_len` | `size_t` | yes | Length of `embedding`. |
| `metadata_json` | `const char*` | yes | JSON object string of metadata key-value pairs. |

**Returns:** Frame ID string (free with `seraph_string_free`), or `NULL` on error.

**Errors:** As `seraph_store_put`, plus `NULL` if `metadata_json` is not a valid JSON object.

**Example**

```c
char* fid = seraph_store_put_with_metadata(
    store, (const uint8_t*)"hello", 5, emb, dim, "{\"source\":\"doc1\"}");
seraph_string_free(fid);
```

### `seraph_store_put_batch`

Deprecated alias of `put_batch_incremental`; calls take the incremental path. Leaves the store buffered: call `flush()` when the load is done.

```c
char* seraph_store_put_batch(void* handle, const char* items_json);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `items_json` | `const char*` | yes | JSON array of `{"content": "...", "embedding": [...]}` items (`content_b64` for binary content). |

**Returns:** JSON array of frame IDs (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` if `items_json` is malformed or any embedding dimension mismatches.

**Example**

```c
char* ids = seraph_store_put_batch(store,
    "[{\"content_b64\":\"aGVsbG8=\",\"embedding\":[0.1,0.2]}]");
// ids -> ["<frame-id>"]
seraph_string_free(ids);
```

### `seraph_store_put_batch_incremental`

Bulk ingest. Each item's parent is chosen against the live store, so an item may attach to an earlier item of the same batch, and the result is identical to calling `put` in a loop at a fraction of the cost. Insertion order shapes the tree, so order the batch meaningfully. The store is left buffered: call `flush()` when the load is finished.

```c
char* seraph_store_put_batch_incremental(void* handle, const char* items_json);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `items_json` | `const char*` | yes | JSON array of `{"content": "...", "embedding": [...]}` items (`content_b64` for binary content). |

**Returns:** JSON array of frame IDs (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` if `items_json` is malformed, any embedding dimension mismatches, or the store is sealed.

**Example**

```c
char* ids = seraph_store_put_batch_incremental(store,
    "[{\"content_b64\":\"aGVsbG8=\",\"embedding\":[0.1,0.2]}]");
seraph_string_free(ids);
seraph_store_sync(store);
```

### `seraph_store_commit_marker`

Commit an application marker: a System frame with no embedding that search never returns, durable in the store like any other frame. Use it for application records that belong with the store (a run manifest, a review note). `kind` becomes the content prefix `_{kind}:` and is echoed in the metadata as `_marker`.

```c
char* seraph_store_commit_marker(void* handle, const char* kind, const char* metadata_json);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `kind` | `const char*` | yes | Marker kind. Non-empty, no `:` and no whitespace. The kinds the load path interprets (`promote`, `supersede`, `seal`, `seraph_seeds`, `seraph_warp_cal`, anything starting `seraph`, case-insensitive, with or without a leading `_`) are refused. |
| `metadata_json` | `const char*` | yes | JSON object. Keys the load path acts on (`_promote`, `_supersede`, any `_seraph*`) are refused, so a marker can never forge a promotion, a supersede or a registry on reopen. |

**Returns:** The marker's frame ID (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` if `kind` is reserved or invalid, `metadata_json` is not a JSON object or carries a load-path key, the store is sealed, or the handle is `NULL` (see `seraph_last_error()`).

### `seraph_store_markers`

Markers of one kind, in commit order.

```c
char* seraph_store_markers(void* handle, const char* kind);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `kind` | `const char*` | yes | The kind passed to `commit_marker`. |

**Returns:** JSON array of `{"frame_id": "...", "metadata": {...}}` (free with `seraph_string_free`); `[]` for an unknown kind; `NULL` on error.

**Errors:** `NULL` on a `NULL` handle or a failed operation; `seraph_last_error` has the message.

---

## Search

### `seraph_store_search`

Search for the frames nearest a query embedding, ranked by similarity.

```c
int seraph_store_search(
    void* handle,
    const float* embedding,
    size_t embedding_len,
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
| `embedding_len` | `size_t` | yes | Length of `embedding`. |
| `top_k` | `size_t` | yes | Maximum number of hits. |
| `tau` | `float` | yes | Similarity floor (cosine); `0.0` disables it. |
| `out_hits` | `SeraphHit**` | yes | Out: hits. Free with `seraph_hits_free`. |
| `out_count` | `size_t*` | yes | Out: count. |

**Returns:** `0` on success, negative on error.

**Errors:** Negative if the handle is `NULL` or the embedding dimension mismatches.

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

Encode a text query with the store's model and search in one call. The encoder loads on first use.

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
| `query` | `const char*` | yes | Query text. |
| `top_k` | `size_t` | yes | Maximum number of hits. |
| `tau` | `float` | yes | Similarity floor (cosine); `0.0` disables it. |
| `out_hits` | `SeraphHit**` | yes | Out: hits. Free with `seraph_hits_free`. |
| `out_count` | `size_t*` | yes | Out: count. |

**Returns:** `0` on success, negative on error.

**Errors:** Negative if the store has no encoder, the handle is `NULL`, or encoding fails.

**Example**

```c
SeraphHit* hits = NULL; size_t n = 0;
seraph_store_search_text(store, "feline", 10, 0.0f, &hits, &n);
seraph_hits_free(hits, n);
```

### `seraph_store_search_opts`

Search with a `SearchOpts` options object: `top_k`, `tau_similarity`, `max_depth`, `include_superseded`, and the time-window fields (`order`, `after_us`, `before_us`).

```c
int seraph_store_search_opts(
    void* handle,
    const float* embedding,
    size_t embedding_len,
    size_t top_k,
    float tau,
    bool include_superseded,
    SeraphHit** out_hits,
    size_t* out_count
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `embedding` | `const float*` | yes | Query vector. |
| `embedding_len` | `size_t` | yes | Length of `embedding`. |
| `top_k` | `size_t` | yes | Maximum number of hits. |
| `tau` | `float` | yes | Similarity floor (cosine); `0.0` disables it. |
| `include_superseded` | `bool` | yes | `true` opts soft-forgotten frames back into results. |
| `out_hits` | `SeraphHit**` | yes | Out: hits. Free with `seraph_hits_free`. |
| `out_count` | `size_t*` | yes | Out: count. |

**Returns:** `0` on success, negative on error.

**Errors:** `-1` if the handle is `NULL`.

### `seraph_store_search_temporal`

Search with a time window. Relevance selection is unchanged; `after_us` and `before_us` (microseconds since the epoch) set an inclusive window on which frames may fill result slots, and with `before_us` set the search runs over the store as it stood at that time. `order` sorts the selected results by time without changing their scores. In Python the window travels in `SearchOpts`.

```c
int seraph_store_search_temporal(
    void* handle,
    const float* embedding,
    size_t embedding_len,
    size_t top_k,
    float tau,
    int include_superseded,
    int order,
    uint64_t after_us,
    uint64_t before_us,
    SeraphHit** out_hits,
    size_t* out_count
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `embedding` | `const float*` | yes | Query vector. |
| `embedding_len` | `size_t` | yes | Length of `embedding`. |
| `top_k` | `size_t` | yes | Maximum number of hits. |
| `tau` | `float` | yes | Similarity floor (cosine); `0.0` disables it. |
| `include_superseded` | `int` | yes | `true` opts soft-forgotten frames back into results. |
| `order` | `int` | yes | Result order: relevance, or by time. |
| `after_us` | `uint64_t` | yes | Earliest commit time a result may have; unbounded when unset. |
| `before_us` | `uint64_t` | yes | Latest commit time a result may have; the search runs over the store as it stood then. |
| `out_hits` | `SeraphHit**` | yes | Out: hits. Free with `seraph_hits_free`. |
| `out_count` | `size_t*` | yes | Out: count. |

**Returns:** `0` on success, negative on error (including an invalid `order`).

**Errors:** `-1` if the handle is `NULL`.

### `seraph_store_search_stream`

Search through the bounded walk with an explicit beam width and visit budget, recording the visit count for `last_search_visits`. With the store's own beam and budget it returns the same results as `search`.

```c
int seraph_store_search_stream(
    void* handle,
    const float* embedding,
    size_t embedding_len,
    size_t top_k,
    float tau,
    size_t beam_width,
    size_t max_visits,
    SeraphHit** out_hits,
    size_t* out_count
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `embedding` | `const float*` | yes | Query vector. |
| `embedding_len` | `size_t` | yes | Length of `embedding`. |
| `top_k` | `size_t` | yes | Maximum number of hits. |
| `tau` | `float` | yes | Similarity floor (cosine); `0.0` disables it. |
| `beam_width` | `size_t` | yes | Frontier entries expanded per round; `0` = unbounded. |
| `max_visits` | `size_t` | yes | Visit budget; `0` = the store's `search_max_visits`. |
| `out_hits` | `SeraphHit**` | yes | Out: hits. Free with `seraph_hits_free`. |
| `out_count` | `size_t*` | yes | Out: count. |

**Returns:** `0` on success, `-1` on error.

**Errors:** `-1` if the handle is `NULL`.

### `seraph_store_search_with_trace`

Search returning the hits and the frames visited, in visit order.

```c
char* seraph_store_search_with_trace(
    void* handle,
    const float* embedding,
    size_t embedding_len,
    size_t top_k,
    float tau
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `embedding` | `const float*` | yes | Query vector. |
| `embedding_len` | `size_t` | yes | Length of `embedding`. |
| `top_k` | `size_t` | yes | Maximum number of hits. |
| `tau` | `float` | yes | Similarity floor (cosine); `0.0` disables it. |

**Returns:** JSON `{"hits": [{"frame_id", "score", "confidence", "path_length"}], "trace": [{"frame_id", "depth", "score", "visit_order", "source", "parent_id"}]}` (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on a `NULL` handle or a failed operation; `seraph_last_error` has the message.

### `seraph_store_search_with_warp`

Search with warp-field steering: the query is pulled toward `targets` and away from `suppress` before the search.

```c
int seraph_store_search_with_warp(
    void* handle,
    const float* query_embedding,
    size_t query_len,
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
| `query_len` | `size_t` | yes | Length of `query`. |
| `targets_json` | `const char*` | yes | (embedding, magnitude) pull pairs. |
| `suppress_json` | `const char*` | yes | (embedding, magnitude) suppression pairs. |
| `profile` | `const char*` | yes | `"routing"`, `"saturation"`, or `"bounded"`. |
| `bound_k` | `float` | yes | Suppression-norm cap (used only by `"bounded"`). |
| `top_k` | `size_t` | yes | Maximum number of hits. |
| `tau` | `float` | yes | Similarity floor (cosine); `0.0` disables it. |
| `out_hits` | `SeraphHit**` | yes | Out: hits. Free with `seraph_hits_free`. |
| `out_count` | `size_t*` | yes | Out: count. |

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

Search steered by a warp specification in one call. Frame-ID targets and named magnitudes in the spec are resolved against the store.

```c
int seraph_store_search_with_warp_spec(
    void* handle,
    const float* query_embedding,
    size_t query_len,
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
| `query_len` | `size_t` | yes | Length of `query`. |
| `warp_spec_json` | `const char*` | yes | WarpSpec JSON in the `seraph_store_search_with_warp_spec` format. |
| `top_k` | `size_t` | yes | Maximum number of hits. |
| `tau` | `float` | yes | Similarity floor (cosine); `0.0` disables it. |
| `out_hits` | `SeraphHit**` | yes | Out: hits. Free with `seraph_hits_free`. |
| `out_count` | `size_t*` | yes | Out: count. |

**Returns:** `0` on success, negative on error.

**Errors:** Negative if `warp_spec_json` is malformed or a dimension mismatches.

**Example**

```c
SeraphHit* hits = NULL; size_t n = 0;
seraph_store_search_with_warp_spec(store, q, dim, spec_json, 10, 0.0f, &hits, &n);
seraph_hits_free(hits, n);
```

### `seraph_store_fixup_warp`

Refresh the store's warp calibration from its own corpus when the persisted one has drifted or none exists, and persist the result. Call from the single writer after open.

```c
int seraph_store_fixup_warp(void* handle);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |

**Returns:** `1` if it (re)calibrated, `0` if it skipped (not stale / geometry not developed), `-1` on error.

**Errors:** `-1` if the handle is `NULL`.

### `seraph_store_warp_calibration`

The store's resident warp calibration as a mapping from named magnitude to value, or `None` if the store has not been calibrated. Named-magnitude warps resolve against this; there are no built-in presets.

```c
char* seraph_store_warp_calibration(void* handle);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |

**Returns:** A newly-allocated JSON string (caller frees with `seraph_string_free`), or `null` on a null handle.

**Errors:** `NULL` on a `NULL` handle or a failed operation; `seraph_last_error` has the message.

---

## Frame Access

### `seraph_store_get_frame`

Get a frame by ID, with its full data including embedding and metadata.

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

**Response:** JSON object with `id`, `parent_id`, `status`, `content_b64`, `embedding`, `metadata`, `watermark_hex`, `content_hash_hex`, `codes_hex` (the packed 4-bit codes — the canonical persisted embedding on a v3 store and the bytes `seraph_store_content_set_root` hashes; empty on pre-v3 frames), `in_degree`, `model_id`, `timestamp`, `customer_uuid_hex`, `writer_signature_hex`, `marking_bytes_hex`, `status_ref`, `status_sources`.

**Example**

```c
char* json = seraph_store_get_frame(store, fid);
if (json) { /* parse */ seraph_string_free(json); }
```

### `seraph_store_get_content`

A frame's content bytes by ID, without the rest of the frame.

```c
int seraph_store_get_content(
    void* handle,
    const char* frame_id,
    uint8_t** out_content,
    size_t* out_len
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `frame_id` | `const char*` | yes | Frame ID. |
| `out_content` | `uint8_t**` | yes | Out: content. |
| `out_len` | `size_t*` | yes | Out: len. |

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

A frame's embedding by ID, as an owned vector.

```c
int seraph_store_get_embedding(
    void* handle,
    const char* frame_id,
    float** out_embedding,
    size_t* out_dim
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `frame_id` | `const char*` | yes | Frame ID. |
| `out_embedding` | `float**` | yes | Out: embedding. |
| `out_dim` | `size_t*` | yes | Out: dim. |

**Returns:** `0` on success, negative on error.

**Errors:** Negative if the frame is missing or the handle is `NULL`.

**Example**

```c
float* emb = NULL; size_t dim = 0;
if (seraph_store_get_embedding(store, fid, &emb, &dim) == 0)
    seraph_floats_free(emb, dim);
```

### `seraph_store_get_embedding_into`

Copy a frame's embedding into a caller-owned buffer, with no allocation.

```c
int seraph_store_get_embedding_into(
    void* handle,
    const char* frame_id,
    float* buf,
    size_t buf_len,
    size_t* out_written
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `frame_id` | `const char*` | yes | Frame ID. |
| `buf` | `float*` | yes | Caller-owned buffer of at least the store's dimension. |
| `buf_len` | `size_t` | yes | Length of `buf`. |
| `out_written` | `size_t*` | yes | Out: written. |

**Returns:** `0` on success, negative on error.

**Errors:** Negative if the frame is missing, the handle is `NULL`, or `buf_len` is smaller than the store dimension.

**Example**

```c
float buf[1024]; size_t written = 0;
seraph_store_get_embedding_into(store, fid, buf, 1024, &written);
```

### `seraph_store_quantize_query`

The query vector exactly as the store scores it: the embedding after a round trip through the store's vector precision. Use it to reproduce scores outside the engine.

```c
int seraph_store_quantize_query(
    void* handle,
    const float* embedding,
    size_t embedding_len,
    float** out_query,
    size_t* out_dim
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `embedding` | `const float*` | yes | Query vector of the store's dimension. |
| `embedding_len` | `size_t` | yes | Length of `embedding`. |
| `out_query` | `float**` | yes | Out: query. |
| `out_dim` | `size_t*` | yes | Out: dim. |

**Returns:** `0` on success, `-1` on error.

**Errors:** `-1` if the handle or an out pointer is `NULL`.

### `seraph_store_encode_codes`

The packed 4-bit codes the store would persist for an embedding: the bytes a frame's codes carry and content_set_root hashes. Lets a caller build a leaf for content it has not ingested.

```c
int seraph_store_encode_codes(
    void* handle,
    const float* embedding,
    size_t embedding_len,
    uint8_t** out_codes,
    size_t* out_len
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `embedding` | `const float*` | yes | Vector of the store's dimension. |
| `embedding_len` | `size_t` | yes | Length of `embedding`. |
| `out_codes` | `uint8_t**` | yes | Out: codes. |
| `out_len` | `size_t*` | yes | Out: len. |

**Returns:** `0` on success, `-1` on error.

**Errors:** `-1` if the handle or an out pointer is `NULL`.

### `seraph_store_get_metadata`

A frame's metadata by ID, as a JSON object string.

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

### `seraph_store_borrow_frame`

Borrow a frame for zero-copy reads. The view points into engine memory, so release it before the store is written to.

```c
SeraphFrameView* seraph_store_borrow_frame(void* handle, const char* frame_id);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `frame_id` | `const char*` | yes | Frame ID. |

**Returns:** Pointer to a [`SeraphFrameView`](#seraphframeview), or `NULL` if not found.

**Errors:** `NULL` if the frame is missing or the handle is `NULL`.

**Example**

```c
SeraphFrameView* v = seraph_store_borrow_frame(store, fid);
if (v) { /* read v->embedding, v->status, ... */ seraph_store_release_frame(v); }
```

### `seraph_store_release_frame`

Release a frame view obtained from borrow_frame. Do not free the view's fields individually.

```c
void seraph_store_release_frame(SeraphFrameView* view);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `view` | `SeraphFrameView*` | yes | The view to release; NULL is ignored. |

**Returns:** Nothing.

### `seraph_store_frame_ids`

All frame IDs in insertion order.

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

IDs of the eigenframes.

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

Full Frame objects for all eigenframes.

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

Mean of all eigenframe embeddings: a single vector representing the store's semantic centre, used for federation routing.

```c
int seraph_store_federation_vector(void* handle, float** out_embedding, size_t* out_dim);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `out_embedding` | `float**` | yes | Out: embedding. |
| `out_dim` | `size_t*` | yes | Out: dim. |

**Returns:** `0` on success (including the no-eigenframe case), `-1` on error.

**Errors:** `-1` if the handle is `NULL`. If the store has no eigenframes, returns `0` with `out_embedding = NULL` and `out_dim = 0`.

**Example**

```c
float* fv = NULL; size_t dim = 0;
if (seraph_store_federation_vector(store, &fv, &dim) == 0 && fv)
    seraph_floats_free(fv, dim);
```

### `seraph_store_model_id`

The embedding model ID declared at genesis.

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

Embedding dimensionality of the store.

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

Store statistics.

```c
int seraph_store_stats(void* handle, SeraphStats* out);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `out` | `SeraphStats*` | yes | Out: struct filled on success. |

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

Add a seed frame to a live store (immediately an eigenframe).

```c
char* seraph_store_add_seed(
    void* handle,
    const char* label,
    const float* embedding,
    size_t embedding_len,
    const char* metadata_json
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `label` | `const char*` | yes | Human-readable seed label. |
| `embedding` | `const float*` | yes | Seed embedding vector. |
| `embedding_len` | `size_t` | yes | Length of `embedding`. |
| `metadata_json` | `const char*` | yes | JSON metadata object. |

**Returns:** Seed frame ID (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on dimension mismatch, sealed store, or `NULL` handle.

**Example**

```c
char* sid = seraph_store_add_seed(store, "physics", emb, dim, NULL);
seraph_string_free(sid);
```

### `seraph_store_get_seeds`

The authored seed list: `label -> frame_id` bindings in authored order. A binding may name any frame, not only a Seed. When no list has been authored, the store's Seed frames are returned instead; `seeds_are_authored()` tells the two apart.

```c
char* seraph_store_get_seeds(void* handle);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |

**Returns:** JSON array of `{"label","frame_id"}` in authored order (free with
`seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on a `NULL` handle.

**Example**

```c
char* json = seraph_store_get_seeds(store);   // [{"label":"physics","frame_id":"01a0..."}]
seraph_string_free(json);
```

### `seraph_store_seeds_are_authored`

Whether the seed list comes from an authored registry rather than being derived from Seed-status frames. An authored empty registry is a real state and is authoritative; it does not fall back to the derived list.

```c
int seraph_store_seeds_are_authored(void* handle);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |

**Returns:** `1` if authored, `0` if derived, `-1` on error.

**Errors:** `-1` if the handle is `NULL`.

### `seraph_store_resolve_seed`

Resolve a seed label to its frame ID. Labels are unique within a store's seed registry.

```c
char* seraph_store_resolve_seed(void* handle, const char* label);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `label` | `const char*` | yes | Registered seed label. |

**Returns:** Frame ID (free with `seraph_string_free`), or `NULL` if the label
is not registered, or `NULL` on error.

**Errors:** The export clears the error slot on entry and sets it only on
failure, so an unregistered label returns `NULL` with `seraph_last_error()`
still `NULL`. Check the error slot to tell "not found" from "failed".

### `seraph_store_set_seeds`

Replace the authored seed list. Removing a label removes it from the list only; the frame keeps its status.

```c
char* seraph_store_set_seeds(void* handle, const char* entries_json);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `entries_json` | `const char*` | yes | The complete new registry. |

**Returns:** The snapshot sentinel's frame ID (free with `seraph_string_free`),
or `NULL` on error.

**Errors:** `NULL` on malformed JSON, an empty or duplicated label, a
`frame_id` absent from the store, a sealed store, or a `NULL` handle.

**Example**

```c
char* sid = seraph_store_set_seeds(store,
    "[{\"label\":\"evidence\",\"frame_id\":\"01a0...\"}]");
seraph_string_free(sid);
```

### `seraph_store_register_seed`

Bind a label to a frame in the seed registry, appending a new registry snapshot. Rebinding an existing label updates it in place and keeps its position.

```c
char* seraph_store_register_seed(void* handle, const char* label, const char* frame_id);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `label` | `const char*` | yes | Seed label; unique within the registry. |
| `frame_id` | `const char*` | yes | Frame the label names. |

**Returns:** The snapshot sentinel's frame ID (free with `seraph_string_free`),
or `NULL` on error.

**Errors:** `NULL` on an empty label, an unknown `frame_id`, a sealed store, or
a `NULL` handle.

### `seraph_store_unregister_seed`

Drop a label from the seed registry, appending a new registry snapshot. The frame it named is untouched.

```c
char* seraph_store_unregister_seed(void* handle, const char* label);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `label` | `const char*` | yes | Registered seed label. |

**Returns:** The snapshot sentinel's frame ID (free with `seraph_string_free`),
or `NULL` on error.

**Errors:** `NULL` if the label is not registered, on a sealed store, or on a
`NULL` handle.

### `seraph_store_put_subtree`

Ingest content under a named subtree: the parent is chosen from inside `root`'s subtree instead of the whole store, with no similarity floor. Similarity edges are chosen globally as `put` chooses them, so the frame stays reachable by ordinary search. The placement is recorded on the frame so a migration keeps it inside the same region.

```c
char* seraph_store_put_subtree(
    void* handle,
    const char* root,
    const uint8_t* content,
    size_t content_len,
    const float* embedding,
    size_t embedding_len,
    const char* metadata_json
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `root` | `const char*` | yes | Subtree root: a frame ID or a seed label. A frame ID wins if it resolves as one. |
| `content` | `const uint8_t*` | yes | Content bytes. |
| `content_len` | `size_t` | yes | Length of `content`. |
| `embedding` | `const float*` | yes | Embedding vector. |
| `embedding_len` | `size_t` | yes | Length of `embedding`. |
| `metadata_json` | `const char*` | yes | JSON metadata object. A caller-supplied `_placed_under` is overwritten. |

**Returns:** New frame ID (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on an unknown root, dimension mismatch, sealed store, or
`NULL` handle.

**Example**

```c
char* fid = seraph_store_put_subtree(
    store, "evidence", body, body_len, emb, dim, NULL);
seraph_string_free(fid);
```

### `seraph_store_search_subtree`

Search confined to a named subtree: the root is the entry point and only edges inside the subtree are followed. Ordinary `search` is unaffected and still reaches these frames.

```c
int seraph_store_search_subtree(
    void* handle,
    const char* root,
    const float* query,
    size_t query_len,
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
| `root` | `const char*` | yes | Subtree root: a frame ID or a seed label. |
| `query` | `const float*` | yes | Query embedding. |
| `query_len` | `size_t` | yes | Length of `query`. |
| `top_k` | `size_t` | yes | Maximum number of hits. |
| `tau` | `float` | yes | Similarity floor (cosine); `0.0` disables it. |
| `out_hits` | `SeraphHit**` | yes | Out: hits. Free with `seraph_hits_free`. |
| `out_count` | `size_t*` | yes | Out: count. |

**Returns:** `0` on success, `-1` on error.

**Errors:** `-1` on an unknown root or a `NULL` handle.

### `seraph_store_promote`

Promote an active frame to eigenframe unconditionally. Promotion is monotonic.

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

Ingest content and immediately promote it to eigenframe (atomic ingest plus promote).

```c
char* seraph_store_promote_content(
    void* handle,
    const uint8_t* content,
    size_t content_len,
    const float* embedding,
    size_t embedding_len,
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
| `embedding_len` | `size_t` | yes | Length of `embedding`. |
| `metadata_json` | `const char*` | yes | JSON metadata object. |

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

Shortest path between two frames over parent-child links.

```c
char* seraph_store_path(void* handle, const char* from_id, const char* to_id);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `from_id` | `const char*` | yes | Start frame ID. |
| `to_id` | `const char*` | yes | End frame ID. |

**Returns:** JSON array of frame objects along the path (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` if either frame is missing or no path exists.

**Response:** [PLACEHOLDER: array of frame JSON objects]

**Example**

```c
char* json = seraph_store_path(store, a, b);
seraph_string_free(json);
```

### `seraph_store_chain`

The shortest route between two frames over committed edges (similarity edges and parent/child links), each step weighted by how similar its two frames are. Exact; bounded by `search_max_visits`. For a frame's own ancestry use `lineage`.

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

**Response:** the chain JSON shape documented under `seraph_store_chain_between`; `primitive` is `"geodesic"`.

**Example**

```c
char* json = seraph_store_chain(store, a, b);
seraph_string_free(json);
```

### `seraph_store_chain_opts`

chain with the include_superseded option: superseded frames may be steps of the route, as they may be results of search_opts.

```c
char* seraph_store_chain_opts(
    void* handle,
    const char* from_id,
    const char* to_id,
    bool include_superseded
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `from_id` | `const char*` | yes | Start frame ID. |
| `to_id` | `const char*` | yes | End frame ID. |
| `include_superseded` | `bool` | yes | `true` opts soft-forgotten frames back into results. |

**Returns:** JSON chain result (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on a null handle or id.

**Response:** the chain JSON shape documented under `seraph_store_chain_between`; `primitive` is `"geodesic"`.

**Example**

```c
char* json = seraph_store_chain_opts(store, a, b, true);
seraph_string_free(json);
```

### `seraph_store_chain_forward`

How `from_id` led to `to_id`: the shortest route over committed edges taking only steps forward in commit order, so each step is to a frame that attached to the one before it when it was committed. `from_id` must precede `to_id`; `reached` is `False` when no such route exists. Crosses lineages, which `lineage` cannot.

```c
char* seraph_store_chain_forward(
    void* handle,
    const char* from_id,
    const char* to_id,
    bool include_superseded
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `from_id` | `const char*` | yes | Earlier frame ID. |
| `to_id` | `const char*` | yes | Later frame ID. |
| `include_superseded` | `bool` | yes | `true` opts soft-forgotten frames back into results. |

**Returns:** JSON chain result (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on a null handle or id.

**Response:** the chain JSON shape documented under `seraph_store_chain_between`; `primitive` is `"geodesic_forward"`. `reached` is false when the record holds no forward route, or `from_id` does not precede `to_id`.

**Example**

```c
char* json = seraph_store_chain_forward(store, earlier, later, false);
seraph_string_free(json);
```

### `seraph_store_chain_forward_between`

How the idea at `emb_a` led to the idea at `emb_b`. Each query's `search` results (under `top_k` and `tau`) stand for it; the best-fitting earlier/later pair that a forward route joins is chosen, and the route is the forward chain between them. `reached` is `False` when no forward route joins the two result sets.

```c
char* seraph_store_chain_forward_between(
    void* handle,
    const float* emb_a,
    size_t len_a,
    const float* emb_b,
    size_t len_b,
    size_t top_k,
    float tau,
    bool include_superseded
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `emb_a` | `const float*` | yes | The earlier idea. |
| `len_a` | `size_t` | yes | Length of `a`. |
| `emb_b` | `const float*` | yes | The later idea. |
| `len_b` | `size_t` | yes | Length of `b`. |
| `top_k` | `size_t` | yes | Maximum number of hits. |
| `tau` | `float` | yes | Similarity floor (cosine); `0.0` disables it. |
| `include_superseded` | `bool` | yes | `true` opts soft-forgotten frames back into results. |

**Returns:** JSON chain result (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on a null handle.

**Response:** the chain JSON shape documented under `seraph_store_chain_between`; `primitive` is `"geodesic_forward"`. `reached` is false when no forward route joins the two result sets.

**Example**

```c
char* json = seraph_store_chain_forward_between(store, a, 384, b, 384, 10, 0.5f, false);
seraph_string_free(json);
```

### `seraph_store_provenance`

What `frame_id` was built on: a route back in commit order through the frames it attached to when it was committed (its parent and the similarity neighbours it chose), stopping when the next step is no more related than the store's background. `tau` overrides that stop; `reached` is `True` when at least one step was taken.

```c
char* seraph_store_provenance(
    void* handle,
    const char* frame_id,
    float tau,
    bool include_superseded
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `frame_id` | `const char*` | yes | Start frame. |
| `tau` | `float` | yes | Similarity floor (cosine); `0.0` disables it. |
| `include_superseded` | `bool` | yes | `true` opts soft-forgotten frames back into results. |

**Returns:** Chain JSON, start frame first (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on a null handle or id.

**Response:** the chain JSON shape documented under `seraph_store_chain_between`.

**Example**

```c
char* json = seraph_store_provenance(store, id, NAN, false);
seraph_string_free(json);
```

### `seraph_store_influence`

What was built on `frame_id`: a route forward in commit order through the frames that attached to it when they were committed, stopping when the next step is no more related than the store's background. `tau` overrides that stop; `reached` is `True` when at least one step was taken.

```c
char* seraph_store_influence(
    void* handle,
    const char* frame_id,
    float tau,
    bool include_superseded
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `frame_id` | `const char*` | yes | Start frame. |
| `tau` | `float` | yes | Similarity floor (cosine); `0.0` disables it. |
| `include_superseded` | `bool` | yes | `true` opts soft-forgotten frames back into results. |

**Returns:** Chain JSON, start frame first (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on a null handle or id.

**Response:** the chain JSON shape documented under `seraph_store_chain_between`.

**Example**

```c
char* json = seraph_store_influence(store, id, NAN, false);
seraph_string_free(json);
```

### `seraph_store_provenance_with_warp_spec`

`provenance` steered by a warp: steps are chosen, and the stop measured, against the start frame pulled toward `targets` and away from `suppress`, so the warp selects which strand is followed.

```c
char* seraph_store_provenance_with_warp_spec(
    void* handle,
    const char* frame_id,
    const char* warp_spec_json,
    float tau,
    bool include_superseded
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `frame_id` | `const char*` | yes | Start frame. |
| `warp_spec_json` | `const char*` | yes | WarpSpec JSON in the `seraph_store_search_with_warp_spec` format. |
| `tau` | `float` | yes | Similarity floor (cosine); `0.0` disables it. |
| `include_superseded` | `bool` | yes | `true` opts soft-forgotten frames back into results. |

**Returns:** Chain JSON, start frame first (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on a null handle or id, or a malformed / unresolvable warp spec.

**Response:** the chain JSON shape documented under `seraph_store_chain_between`.

**Example**

```c
char* json = seraph_store_provenance_with_warp_spec(store, id, spec, NAN, false);
seraph_string_free(json);
```

### `seraph_store_influence_with_warp_spec`

`influence` steered by a warp: steps are chosen, and the stop measured, against the start frame pulled toward `targets` and away from `suppress`, so the warp selects which strand is followed.

```c
char* seraph_store_influence_with_warp_spec(
    void* handle,
    const char* frame_id,
    const char* warp_spec_json,
    float tau,
    bool include_superseded
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `frame_id` | `const char*` | yes | Start frame. |
| `warp_spec_json` | `const char*` | yes | WarpSpec JSON in the `seraph_store_search_with_warp_spec` format. |
| `tau` | `float` | yes | Similarity floor (cosine); `0.0` disables it. |
| `include_superseded` | `bool` | yes | `true` opts soft-forgotten frames back into results. |

**Returns:** Chain JSON, start frame first (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on a null handle or id, or a malformed / unresolvable warp spec.

**Response:** the chain JSON shape documented under `seraph_store_chain_between`.

**Example**

```c
char* json = seraph_store_influence_with_warp_spec(store, id, spec, NAN, false);
seraph_string_free(json);
```

### `seraph_store_connections`

The distinct ways two frames connect: up to `max_routes` routes over committed edges that share no interior frames, shortest first, each with its bridge frame (the interior frame closest to both ends). With `forward` set, every route runs forward in commit order: the ways `from_id` led to `to_id`.

```c
char* seraph_store_connections(
    void* handle,
    const char* from_id,
    const char* to_id,
    size_t max_routes,
    bool forward,
    bool include_superseded
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `from_id` | `const char*` | yes | Start frame. |
| `to_id` | `const char*` | yes | End frame. |
| `max_routes` | `size_t` | yes | Most routes returned. |
| `forward` | `bool` | yes | Routes forward in commit order only (`from_id` must precede `to_id`). |
| `include_superseded` | `bool` | yes | `true` opts soft-forgotten frames back into results. |

**Returns:** JSON `[{"chain": <chain JSON>, "bridge": id|null}]` (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on a null handle or id.

**Example**

```c
char* json = seraph_store_connections(store, a, b, 5, false, false);
seraph_string_free(json);
```

### `seraph_store_chain_to`

Chain from the frame nearest a query embedding.

```c
char* seraph_store_chain_to(void* handle, const float* query_embedding, size_t query_len);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `query_embedding` | `const float*` | yes | Target query vector. |
| `query_len` | `size_t` | yes | Length of `query`. |

**Returns:** JSON chain result (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on dimension mismatch or `NULL` handle.

**Response:** [PLACEHOLDER: chain JSON shape]

**Example**

```c
char* json = seraph_store_chain_to(store, q, dim);
seraph_string_free(json);
```

### `seraph_store_history`

How the store came to hold what the query finds. The top `anchors` search hits choose their lineages, and the best `top_k` frames along those lineages come back oldest first, each with the route that reached it. On a store ingested in time order the result reads as the development of a topic.

```c
char* seraph_store_history(
    void* handle,
    const float* query_embedding,
    size_t query_len,
    size_t top_k,
    float tau,
    size_t anchors
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `query_embedding` | `const float*` | yes | Query vector. |
| `query_len` | `size_t` | yes | Length of `query`. |
| `top_k` | `size_t` | yes | Maximum number of hits. |
| `tau` | `float` | yes | Similarity floor (cosine); `0.0` disables it. |
| `anchors` | `size_t` | yes | Search hits used to choose the lineages. |

**Returns:** JSON `{"hits": [{"frame_id", "score", "timestamp", "depth", "path"}], "anchors", "doors", "visits"}`;
free with `seraph_string_free`. NULL on error.

**Errors:** `NULL` on a `NULL` handle or a failed operation; `seraph_last_error` has the message.

### `seraph_store_history_with_warp_spec`

`history` from a warped query. The warp moves which lineages the anchors fall in, so it chooses which strand of a topic's history is read: toward a finding, away from a thread. Pass a frame's embedding as a target to steer toward that frame.

```c
char* seraph_store_history_with_warp_spec(
    void* handle,
    const float* query_embedding,
    size_t query_len,
    const char* warp_spec_json,
    size_t top_k,
    float tau,
    size_t anchors
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `query_embedding` | `const float*` | yes | Query vector. |
| `query_len` | `size_t` | yes | Length of `query`. |
| `warp_spec_json` | `const char*` | yes | WarpSpec JSON in the `seraph_store_search_with_warp_spec` format. |
| `top_k` | `size_t` | yes | Maximum number of hits. |
| `tau` | `float` | yes | Similarity floor (cosine); `0.0` disables it. |
| `anchors` | `size_t` | yes | Search hits used to choose the lineages. |

**Returns:** the same JSON as `seraph_store_history`; free with `seraph_string_free`. NULL on error.

**Errors:** `NULL` on a `NULL` handle or a failed operation; `seraph_last_error` has the message.

### `seraph_store_chain_to_with_warp_spec`

`chain_to` with a warp applied to the query before the route is found.

```c
char* seraph_store_chain_to_with_warp_spec(
    void* handle,
    const float* query_embedding,
    size_t query_len,
    const char* warp_spec_json
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `query_embedding` | `const float*` | yes | Target query vector. |
| `query_len` | `size_t` | yes | Length of `query`. |
| `warp_spec_json` | `const char*` | yes | WarpSpec JSON in the `seraph_store_search_with_warp_spec` format. |

**Returns:** JSON chain result (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on a `NULL` handle or a failed operation; `seraph_last_error` has the message.

### `seraph_store_chain_between_with_warp_spec`

`chain_between` with a warp applied to both endpoints before the route is found.

```c
char* seraph_store_chain_between_with_warp_spec(
    void* handle,
    const float* emb_a,
    size_t len_a,
    const float* emb_b,
    size_t len_b,
    const char* warp_spec_json
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `emb_a` | `const float*` | yes | First embedding. |
| `len_a` | `size_t` | yes | Length of `a`. |
| `emb_b` | `const float*` | yes | Second embedding. |
| `len_b` | `size_t` | yes | Length of `b`. |
| `warp_spec_json` | `const char*` | yes | WarpSpec JSON in the `seraph_store_search_with_warp_spec` format. |

**Returns:** JSON chain result (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on a `NULL` handle or a failed operation; `seraph_last_error` has the message.

### `seraph_store_chain_between`

Chain between two points in embedding space: the route between the frames nearest each embedding.

```c
char* seraph_store_chain_between(
    void* handle,
    const float* emb_a,
    size_t len_a,
    const float* emb_b,
    size_t len_b
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `emb_a` | `const float*` | yes | First embedding. |
| `len_a` | `size_t` | yes | Length of `a`. |
| `emb_b` | `const float*` | yes | Second embedding. |
| `len_b` | `size_t` | yes | Length of `b`. |

**Returns:** JSON chain result (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on dimension mismatch or `NULL` handle.

**Response:** `{"frame_ids": [...], "primitive": "geodesic", "reached": bool, "coherence": {"mean_gradient", "min_gradient", "fragmentation_events", "pass_strict", "pass_loose", "stretch"}}`. The route is the geodesic over committed edges (similarity edges and parent/child links, each step costing its arc); `stretch` is its arc over the direct arc between its ends. `reached` is false when the search exhausted `search_max_visits` before the target: `frame_ids` then ends at the frame it reached nearest the target and both coherence passes are false.

**Example**

```c
char* json = seraph_store_chain_between(store, a, dim, b, dim);
seraph_string_free(json);
```

### `seraph_store_lineage`

Parent chain back to genesis: [frame, parent, ..., GENESIS].

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

All descendant frames under `frame_id`, optionally bounded by `max_depth`.

```c
char* seraph_store_subtree(void* handle, const char* frame_id, size_t max_depth);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `frame_id` | `const char*` | yes | Root frame ID. |
| `max_depth` | `size_t` | yes | Maximum descent depth; `0` = unbounded. |

**Returns:** JSON subtree structure (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` if the frame is missing.

**Response:** [PLACEHOLDER: subtree JSON shape]

**Example**

```c
char* json = seraph_store_subtree(store, fid, 3);
seraph_string_free(json);
```

### `seraph_store_subtrees`

Enumerate seed-rooted subtrees with at least `min_size` descendants.

```c
char* seraph_store_subtrees(void* handle, size_t min_size);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `min_size` | `size_t` | yes | Minimum descendant count. |

**Returns:** JSON array of `{"root", "root_status", "descendant_count", "eigenframe_count", "max_depth"}` (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on a `NULL` handle or a failed operation; `seraph_last_error` has the message.

### `seraph_store_get_children`

Child frame IDs of a frame.

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

Three-population neighborhood of a frame: its consensus, geometric and lineage neighbours.

```c
char* seraph_store_neighborhood(void* handle, const char* frame_id, float tau, size_t hops);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `frame_id` | `const char*` | yes | Center frame ID. |
| `tau` | `float` | yes | Similarity floor (cosine); `0.0` disables it. |
| `hops` | `size_t` | yes | Traversal radius in hops. |

**Returns:** JSON with frames, edges, and region eigenframe (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` if the frame is missing.

**Response:** [PLACEHOLDER: neighborhood JSON shape — frames, edges, region_eigenframe]

**Example**

```c
char* json = seraph_store_neighborhood(store, fid, 0.5f, 2);
seraph_string_free(json);
```

### `seraph_store_neighborhood_spec`

The three-population neighborhood of a frame, with tau given as a spec: auto (p95), loose (p75), strict (p99) or an explicit value.

```c
char* seraph_store_neighborhood_spec(
    void* handle,
    const char* frame_id,
    int tau_mode,
    float tau_value,
    size_t hops
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `frame_id` | `const char*` | yes | Center frame ID. |
| `tau_mode` | `int` | yes | Tau spec mode: `0` auto (background p95), `1` loose (p75), `2` strict (p99), `3` explicit `tau_value`. |
| `tau_value` | `float` | yes | Explicit tau, used when `tau_mode` is `3`. |
| `hops` | `size_t` | yes | Traversal radius in hops. |

**Returns:** Same JSON shape as `seraph_store_neighborhood` (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on a `NULL` handle/frame_id or an invalid `tau_mode`.

**Example**

```c
char* json = seraph_store_neighborhood_spec(store, fid, 2 /* strict */, 0.0f, 2);
seraph_string_free(json);
```

---

## Warp (Steering)

> **Profile values:** `profile` accepts `"routing"`, `"saturation"`, or
> `"bounded"`. Any other value is an error. `"bounded"` is the default used by
> the higher-level wrappers. Target pull is always uncapped; only `"bounded"`
> caps suppression norm (at `bound_k`). Magnitude is the caller's throttle.

### `seraph_store_neighborhood_with_warp_spec`

`neighborhood` with a warp field applied.

```c
char* seraph_store_neighborhood_with_warp_spec(
    void* handle,
    const char* frame_id,
    const char* warp_spec_json,
    float tau,
    size_t hops
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `frame_id` | `const char*` | yes | Center frame ID. |
| `warp_spec_json` | `const char*` | yes | WarpSpec JSON in the `seraph_store_search_with_warp_spec` format. |
| `tau` | `float` | yes | Similarity floor (cosine); `0.0` disables it. |
| `hops` | `size_t` | yes | Traversal radius in hops. |

**Returns:** the same JSON as `seraph_store_neighborhood` (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on a `NULL` handle or a failed operation; `seraph_last_error` has the message.

### `seraph_store_apply_warp`

Apply a warp field to a query embedding without searching (pure vector math).

```c
int seraph_store_apply_warp(
    const float* query_embedding,
    size_t query_len,
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
| `query_len` | `size_t` | yes | Length of `query`. |
| `targets_json` | `const char*` | yes | (embedding, magnitude) pull pairs. |
| `suppress_json` | `const char*` | yes | (embedding, magnitude) suppression pairs. |
| `profile` | `const char*` | yes | `"routing"`, `"saturation"`, or `"bounded"`. |
| `bound_k` | `float` | yes | Suppression-norm cap. |
| `out_embedding` | `float**` | yes | Out: embedding. |
| `out_dim` | `size_t*` | yes | Out: dim. |

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

Apply a warp specification to a query and return the warped vector, without searching.

```c
int seraph_store_warp_query(
    void* handle,
    const float* query_embedding,
    size_t query_len,
    const char* warp_spec_json,
    float** out_embedding,
    size_t* out_dim
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `query_embedding` | `const float*` | yes | Query vector, or on Python a text string that is encoded first. |
| `query_len` | `size_t` | yes | Length of `query`. |
| `warp_spec_json` | `const char*` | yes | WarpSpec JSON in the `seraph_store_search_with_warp_spec` format. |
| `out_embedding` | `float**` | yes | Out: embedding. |
| `out_dim` | `size_t*` | yes | Out: dim. |

**Returns:** `0` on success, negative on error.

**Errors:** Negative if `warp_spec_json` is malformed or a dimension mismatches.

**Example**

```c
float* warped = NULL; size_t dim = 0;
seraph_store_warp_query(store, q, qd, spec_json, &warped, &dim);
seraph_floats_free(warped, dim);
```

### `seraph_store_calibrate_warp`

Run an empirical magnitude calibration sweep against the store.

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
| `magnitudes_json` | `const char*` | yes | Magnitudes to sweep; `NULL` uses defaults. |
| `top_k` | `size_t` | yes | Maximum number of hits. |
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

Local density score for a frame.

```c
float seraph_store_density_score(void* handle, const char* frame_id);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `frame_id` | `const char*` | yes | Frame ID. |

**Returns:** Local density score. **Errors:** sentinel value on missing frame / `NULL` handle.

**Errors:** `NULL` on a `NULL` handle or a failed operation; `seraph_last_error` has the message.

### `seraph_store_seed_margin`

Seed margin: a frame's geometric distinctness relative to the corpus centroid.

```c
float seraph_store_seed_margin(void* handle, const char* frame_id);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `frame_id` | `const char*` | yes | Frame ID. |

**Returns:** Distance to the nearest seed. **Errors:** sentinel value on error.

**Errors:** `NULL` on a `NULL` handle or a failed operation; `seraph_last_error` has the message.

### `seraph_store_steerability`

Warp steerability: angular displacement of a query toward a target at routing magnitude.

```c
float seraph_store_steerability(
    void* handle,
    const float* query_embedding,
    size_t query_len,
    const char* target_id
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `query_embedding` | `const float*` | yes | Query vector. |
| `query_len` | `size_t` | yes | Length of `query`. |
| `target_id` | `const char*` | yes | Target frame ID. |

**Returns:** Steerability score for warping the query toward `target_id`. **Errors:** sentinel on dimension mismatch / missing target.

**Errors:** `NULL` on a `NULL` handle or a failed operation; `seraph_last_error` has the message.

### `seraph_store_region_health`

Region health metrics for an eigenframe.

```c
char* seraph_store_region_health(void* handle, const char* eigenframe_id);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `eigenframe_id` | `const char*` | yes | Eigenframe ID. |

**Returns:** JSON health report (free with `seraph_string_free`), or `NULL`. **Response:** [PLACEHOLDER: region health JSON shape]

**Errors:** `NULL` on a `NULL` handle or a failed operation; `seraph_last_error` has the message.

### `seraph_store_region_health_struct`

Region health for an eigenframe as a fixed struct instead of JSON.

```c
int seraph_store_region_health_struct(
    void* handle,
    const char* eigenframe_id,
    SeraphRegionHealth* out
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `eigenframe_id` | `const char*` | yes | Eigenframe ID. |
| `out` | `SeraphRegionHealth*` | yes | Out: struct filled on success. |

**Returns:** `0` on success, negative on error.

**Errors:** `-1` if the handle is `NULL`.

### `seraph_store_similarity_stats`

Similarity distribution computed from parent-child relationships.

```c
char* seraph_store_similarity_stats(void* handle);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |

**Returns:** JSON similarity distribution (free with `seraph_string_free`), or `NULL`. **Response:** [PLACEHOLDER: similarity stats JSON shape]

**Errors:** `NULL` on a `NULL` handle or a failed operation; `seraph_last_error` has the message.

### `seraph_store_sim_neighbors`

Similarity neighbours of a frame: the live edge set. Includes edges other frames aimed at this one, so it is the traversable neighbourhood rather than the frame's own choice; for that, see `committed_sim_edges`.

```c
char* seraph_store_sim_neighbors(void* handle, const char* frame_id);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `frame_id` | `const char*` | yes | Frame whose neighbours to list. |

**Returns:** JSON array of `{"frame_id": "...", "weight": 0.87}` (free with `seraph_string_free`), `[]` for an unknown frame, or `NULL` on error.

**Errors:** `NULL` on a `NULL` handle or a failed operation; `seraph_last_error` has the message.

### `seraph_store_committed_sim_edges`

The similarity neighbours this frame chose when it was committed, resolved to ids. Always a subset of `sim_neighbors`. `None` for a frame written before the choice was recorded.

```c
char* seraph_store_committed_sim_edges(void* handle, const char* frame_id);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `frame_id` | `const char*` | yes | Frame whose committed choice to read. |

**Returns:** JSON array of `{"frame_id": "...", "weight": 0.87}`, JSON `null` if unrecorded (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on a `NULL` handle or a failed operation; `seraph_last_error` has the message.

### `seraph_store_similarity_stats_struct`

The parent-child similarity distribution as a fixed struct instead of JSON.

```c
int seraph_store_similarity_stats_struct(void* handle, SeraphSimilarityStats* out);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `out` | `SeraphSimilarityStats*` | yes | Out: struct filled on success. |

**Returns:** `0` on success, negative on error.

**Errors:** `-1` if the handle is `NULL`.

### `seraph_store_resolve_tau`

Resolve a tau value against the store's background similarity: `None` gives the p95, `"loose"` the p75, `"strict"` the p99, and a float passes through.

```c
float seraph_store_resolve_tau(void* handle);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |

**Returns:** Auto-resolved `tau` value for the store. **Errors:** sentinel on `NULL` handle.

**Errors:** `NULL` on a `NULL` handle or a failed operation; `seraph_last_error` has the message.

### `seraph_store_resolve_tau_spec`

Resolve tau from a spec against the store's background similarity: auto (p95), loose (p75), strict (p99) or an explicit value passed through.

```c
float seraph_store_resolve_tau_spec(void* handle, int tau_mode, float tau_value);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `tau_mode` | `int` | yes | Tau spec mode: `0` auto (background p95), `1` loose (p75), `2` strict (p99), `3` explicit `tau_value`. |
| `tau_value` | `float` | yes | Explicit tau, used when `tau_mode` is `3`. |

**Returns:** Resolved tau, or the `0.5` sentinel on error (check `seraph_last_error()`).

**Errors:** `NULL` handle or an out-of-range `tau_mode`.

---

## Intelligence

All functions return JSON strings (free with `seraph_string_free`), or `NULL` on
error (`NULL` handle / malformed args).

### `seraph_store_drift_report`

Scan eigenframes for semantic drift: children drifting away from their eigenframe.

```c
char* seraph_store_drift_report(void* handle, size_t window, float threshold);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `window` | `size_t` | yes | Recent-frame window. |
| `threshold` | `float` | yes | Drift trigger threshold. |

**Returns:** JSON drift report. **Response:** [PLACEHOLDER: drift report JSON shape]

**Errors:** `NULL` on a `NULL` handle or a failed operation; `seraph_last_error` has the message.

### `seraph_store_contradictions`

Detect high-divergence siblings under a shared parent.

```c
char* seraph_store_contradictions(void* handle, float min_divergence);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `min_divergence` | `float` | yes | Minimum divergence to flag. |

**Returns:** JSON contradiction list. **Response:** [PLACEHOLDER: contradictions JSON shape]

**Errors:** `NULL` on a `NULL` handle or a failed operation; `seraph_last_error` has the message.

### `seraph_store_contradictions_v2`

Three-population contradiction detection: geometric, lineage and consensus pairs.

```c
char* seraph_store_contradictions_v2(void* handle, int tau_mode, float tau_value);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `tau_mode` | `int` | yes | Tau spec mode: `0` auto (background p95), `1` loose (p75), `2` strict (p99), `3` explicit `tau_value`. |
| `tau_value` | `float` | yes | Explicit tau, used when `tau_mode` is `3`. |

**Returns:** JSON `{"geometric": [...], "lineage": [...], "consensus": [...]}`; each pair is `{"frame_a_id", "frame_b_id", "cosine", "shared_parent", "content_divergence"}`. **Errors:** `NULL` on a `NULL` handle or invalid `tau_mode`.

**Errors:** `NULL` on a `NULL` handle or a failed operation; `seraph_last_error` has the message.

### `seraph_store_evolution`

Concept evolution trajectory from a seed frame, in timestamp order.

```c
char* seraph_store_evolution(void* handle, const char* seed_frame_id);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `seed_frame_id` | `const char*` | yes | Seed frame ID. |

**Returns:** JSON evolution trace for the seed's region. **Response:** [PLACEHOLDER: evolution JSON shape]

**Errors:** `NULL` on a `NULL` handle or a failed operation; `seraph_last_error` has the message.

### `seraph_store_merge_candidates`

Find clusters of similar frames that could be consolidated.

```c
char* seraph_store_merge_candidates(void* handle, float tau);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `tau` | `float` | yes | Similarity floor (cosine); `0.0` disables it. |

**Returns:** JSON list of candidate frame groups. **Response:** [PLACEHOLDER: merge candidates JSON shape]

**Errors:** `NULL` on a `NULL` handle or a failed operation; `seraph_last_error` has the message.

---

## Consolidation

### `seraph_store_consolidate`

Consolidate a specific set of frames into one centroid frame.

```c
char* seraph_store_consolidate(void* handle, const char* frame_ids_json);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `frame_ids_json` | `const char*` | yes | Frame IDs to consolidate. |

**Returns:** JSON consolidation report (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` if `frame_ids_json` is malformed, the store is sealed, or the handle is `NULL`.

**Response:** [PLACEHOLDER: consolidation report JSON shape — merged_count, new_frame_id, redirect_frame_ids, superseded_ids]

**Example**

```c
char* report = seraph_store_consolidate(store, "[\"id1\",\"id2\"]");
seraph_string_free(report);
```

### `seraph_store_consolidate_with_strategy`

Consolidate with an explicit `MergeStrategy`.

```c
char* seraph_store_consolidate_with_strategy(
    void* handle,
    const char* const* frame_ids,
    size_t count,
    int strategy
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `frame_ids` | `const char* const*` | yes | Frame IDs to consolidate. |
| `count` | `size_t` | yes | Number of items. |
| `strategy` | `int` | yes | `MergeStrategy.Geometric` (supported); `MergeStrategy.Semantic` raises. |

**Returns:** JSON consolidation report (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on a `NULL` handle, a null/invalid frame ID entry, an out-of-range `strategy`, or `strategy = 1`.

**Example**

```c
const char* ids[] = { "id1", "id2" };
char* report = seraph_store_consolidate_with_strategy(store, ids, 2, 0);
seraph_string_free(report);
```

### `seraph_store_consolidate_all`

Auto-detect and consolidate all merge candidates in one pass.

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

Soft-forget a single frame: flips its status to Superseded in place. Creates no new frame and no redirect, and does not reparent children.

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

Declare a named structural relation type. Returns the slot ID.

```c
int seraph_store_declare_relation(void* handle, const char* name, uint32_t flags);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `name` | `const char*` | yes | Relation name (e.g. `"references"`, `"contradicts"`). |
| `flags` | `uint32_t` | yes | Relation flag bitfield (see `RelationFlags`); `1` is Active. |

**Returns:** `0` on success, negative on error.

**Errors:** Negative on duplicate name or `NULL` handle.

**Example**

```c
seraph_store_declare_relation(store, "cites", 0);
```

### `seraph_store_list_relations`

List all declared relation types.

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
    const char* from_id,
    const char* to_id,
    const char* relation,
    float weight
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

Retract a typed edge.

```c
int seraph_store_retract_relation(
    void* handle,
    const char* from_id,
    const char* to_id,
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

Find neighbours via typed edges.

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
| `frame_id` | `const char*` | yes | Source frame. |
| `relations_json` | `const char*` | yes | Relation names to traverse. |
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

Filter a (frame_id, score) hit list to those participating in any of the given relations.

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
| `hits_json` | `const char*` | yes | Input (frame_id, score) hits. |
| `relations_json` | `const char*` | yes | Relation names. |
| `direction` | `const char*` | yes | `"outgoing"`, `"incoming"`, or `"both"`. |
| `require_present` | `bool` | yes | Keep hits participating in the relation (`True`) or exclude them. |

**Returns:** JSON array of `{"frame_id","score"}` objects (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on `NULL` handle or `NULL` `hits_json` / `relations_json`.

### `seraph_store_structural_expand`

Expand an existing (frame_id, score) hit list by one hop along structural relations.

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
| `hits_json` | `const char*` | yes | Input (frame_id, score) hits. |
| `relations_json` | `const char*` | yes | Relation names to traverse. |
| `direction` | `const char*` | yes | `"outgoing"`, `"incoming"`, or `"both"`. |
| `score_combine` | `const char*` | yes | `"multiply"`, `"min"`, `"max"`, or `"keep_input"`. |

**Returns:** JSON array of `{"frame_id","score"}` objects (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on `NULL` handle or `NULL` `hits_json` / `relations_json`.

### `seraph_store_gradient`

Semantic gradient along an ordered list of frame IDs: one GradientPoint per step.

```c
char* seraph_store_gradient(void* handle, const char* chain_json);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `chain_json` | `const char*` | yes | Ordered frame IDs. |

**Returns:** JSON array of `{"frame_id": "...", "similarity": 0.9, "delta": 0.1}` objects, one per chain step (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on `NULL` handle, `NULL` `chain_json`, or malformed JSON.

### `seraph_store_constitutive_score`

How much a chain says beyond a search at its midpoint: `1 - jaccard(chain_intermediates, midpoint_NN)` for an ordered frame-ID list. Values below `0.20` indicate the chain carries little beyond that search.

```c
float seraph_store_constitutive_score(void* handle, const char* chain_json);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `chain_json` | `const char*` | yes | Ordered frame IDs. |

**Returns:** The `float` constitutive score.

**Errors:** `f32::NAN` sentinel on `NULL` handle, `NULL` `chain_json`, or malformed JSON — check `seraph_last_error()`.

---

## Provenance & Sealing

### `seraph_store_verify`

Verify watermark-chain integrity.

```c
int seraph_store_verify(void* handle, char** out_failures_json);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `out_failures_json` | `char**` | yes | Out: failures json. |

**Returns:** `0` if the chain is valid; non-zero if invalid.

**Errors:** When invalid, `out_failures_json` is populated with the failing frames.

**Response:** [PLACEHOLDER: failures JSON shape — array of {frame_id, reason}]

**Example**

```c
char* fails = NULL;
if (seraph_store_verify(store, &fails) != 0) { puts(fails); seraph_string_free(fails); }
```

### `seraph_store_seal_with_anchor`

Seal with an external time anchor over the order digest of every frame before the seal (take it with `order_digest` after the last write). The engine records `backend` and the token; it does not contact the backend. Requires Commercial or higher. On the Python surface this is `seal(reason, anchor_backend, anchor_token)`.

```c
char* seraph_store_seal_with_anchor(
    void* handle,
    const char* reason,
    const char* backend,
    const uint8_t* token,
    size_t token_len
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `reason` | `const char*` | yes | Reason recorded in the seal. |
| `backend` | `const char*` | yes | Anchor backend name. |
| `token` | `const uint8_t*` | yes | The backend's token over the order digest. |
| `token_len` | `size_t` | yes | Length of `token`. |

**Returns:** Seal frame ID (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` if already sealed, the tier does not support sealing, or `backend` is `NULL`.

**Example**

```c
char* id = seraph_store_seal_with_anchor(store, "final", "opentimestamps", tok, tok_len);
seraph_string_free(id);
```

### `seraph_store_order_digest`

The digest to anchor: a running SHA-256 over the watermarks in commit order (`d0 = SHA256("SERAPH:ORDER:v1")`, `d_i = SHA256(d_{i-1} || watermark_i)`), through `through_frame_id` or the whole store. From format 4 a watermark binds a frame's content, lineage, id, commit time and committed edges, so the 32 bytes fix all of them and their order. Anchor them, then record the token with `append_tip_anchor` or `seal`.

```c
char* seraph_store_order_digest(void* handle, const char* through_frame_id);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `through_frame_id` | `const char*` | yes | Last frame covered; `NULL` for the whole log. |

**Returns:** JSON `{"frame_count", "digest_hex"}` (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` if the frame is not found or on a null handle.

**Example**

```c
char* json = seraph_store_order_digest(store, NULL);
seraph_string_free(json);
```

### `seraph_store_anchor_claims`

Every seal and tip anchor that commits to an order digest. `verify` checks each claim's digest against the log; the token is checked against the claim's `digest` by your anchor backend, directly or through `verify_attestation(anchor_verifier=...)`.

```c
char* seraph_store_anchor_claims(void* handle);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |

**Returns:** JSON `[{"kind", "sentinel_id", "frame_count", "digest_hex", "backend", "token_b64"}]` (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on a null handle.

**Example**

```c
char* json = seraph_store_anchor_claims(store);
seraph_string_free(json);
```

### `seraph_store_seal`

Seal the store: commit a final sentinel and refuse further writes. The seal records the order digest of every frame before it (see `order_digest`), so `verify` detects any frame reordered, removed or added before the seal, and any frame after it. To anchor the seal externally: stop writing, take `order_digest()`, anchor its 32 bytes with your backend, then seal with `anchor_backend` and `anchor_token`; the engine records the token and never contacts the backend. Requires Commercial or higher.

```c
char* seraph_store_seal(void* handle, const char* reason);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `reason` | `const char*` | yes | Free-text reason recorded in the seal. |

**Returns:** Seal frame ID (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on Free tier, already-sealed store, or `NULL` handle.

**Example**

```c
char* sid = seraph_store_seal(store, "final release");
seraph_string_free(sid);
```

### `seraph_store_is_sealed`

Whether the store has been sealed (read-only). All tiers can open and read sealed stores.

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

Audit writer attribution across all frames.

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

Full attestation audit: chain validity, genesis seed, operator identity, anchors, cross-customer writes.

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

### `seraph_store_content_set_root`

An order-free fingerprint of the store's retrievable content: sha256 over the sorted per-frame leaves `sha256(content_hash || codes)` under the header `"seraph-content-set/v1|<model_id>|<dim>|<n>"`. Two stores holding the same content under the same model give the same root regardless of commit order or graph. It ranges over every frame search can return; markers, redirects and superseded frames are excluded, and it is recomputable from the frame log alone.

```c
char* seraph_store_content_set_root(void* handle);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |

**Returns:** 64 hex characters (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` if the handle is `NULL`.

### `seraph_store_genesis_seed`

Raw genesis seed bytes (v2-attested stores only).

```c
int seraph_store_genesis_seed(void* handle, uint8_t** out_seed, size_t* out_len);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `out_seed` | `uint8_t**` | yes | Out: seed. |
| `out_len` | `size_t*` | yes | Out: len. |

**Returns:** `0` on success, negative on error.

**Errors:** Negative if the handle is `NULL`.

**Example**

```c
uint8_t* seed = NULL; size_t len = 0;
if (seraph_store_genesis_seed(store, &seed, &len) == 0) seraph_bytes_free(seed, len);
```

### `seraph_store_genesis_attestation`

Genesis attestation header as a JSON string.

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

Append a trusted-time anchor record to the chain tip. Requires a Commercial or Enterprise license.

```c
char* seraph_store_append_tip_anchor(
    void* handle,
    const char* backend,
    const uint8_t* token,
    size_t token_len,
    const char* tip_frame_id,
    const char* status,
    const char* supersedes
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `backend` | `const char*` | yes | Anchor backend (e.g. `"opentimestamps"`, `"rfc3161"`). |
| `token` | `const uint8_t*` | yes | Proof/token bytes. |
| `token_len` | `size_t` | yes | Length of `token`. |
| `tip_frame_id` | `const char*` | yes | Frame ID this anchor covers. |
| `status` | `const char*` | yes | `"pending"` or `"confirmed"`. |
| `supersedes` | `const char*` | yes | Frame ID of a previous anchor this one supersedes. |

**Returns:** Anchor frame ID (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on Free tier, unknown `backend`, invalid `status`, or `NULL` handle.

**Example**

```c
char* aid = seraph_store_append_tip_anchor(
    store, "rfc3161", token, token_len, tip, "pending", NULL);
seraph_string_free(aid);
```

### `seraph_store_list_tip_anchors`

List tip anchor records as JSON strings. Available to all tiers.

```c
char* seraph_store_list_tip_anchors(void* handle, int resolved);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `resolved` | `int` | yes | Return only resolved anchors. |

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

Frame IDs active at the given timestamp (microseconds).

```c
SeraphSnapshot* seraph_store_snapshot(void* handle, int64_t at_timestamp_us);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `at_timestamp_us` | `int64_t` | yes | Point in time, microseconds since the epoch. |

**Returns:** Snapshot handle, or `NULL` on error. Free with `seraph_snapshot_free`.

**Errors:** `NULL` if the handle is `NULL`.

**Example**

```c
SeraphSnapshot* snap = seraph_store_snapshot(store, 1700000000000000LL);
```

### `seraph_snapshot_frame_ids`

The frame IDs visible in a snapshot.

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

The point in time a snapshot was taken at, in microseconds since the epoch.

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

Brute-force cosine search within a snapshot.

```c
char* seraph_store_snapshot_search(
    void* handle,
    const SeraphSnapshot* snapshot,
    const float* query_embedding,
    size_t query_len,
    size_t top_k
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `snapshot` | `const SeraphSnapshot*` | yes | A snapshot view from `snapshot()`. |
| `query_embedding` | `const float*` | yes | Query vector. |
| `query_len` | `size_t` | yes | Length of `query`. |
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
| `snapshot` | `SeraphSnapshot*` | yes | Snapshot handle; NULL is ignored. |

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

Subscribe a callback to a store event (promote, drift, contradiction, consolidate, milestone); `on` is an alias of `subscribe`.

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
| `event_name` | `const char*` | yes | Event name. |
| `callback` | `SeraphEventCallback` | yes | Callable receiving one payload object. |
| `user_data` | `void*` | yes | Opaque pointer handed back to the callback. |

**Returns:** Subscriber ID (non-zero), or `0` on error.

**Errors:** `0` if `event_name` is unknown, `callback` is `NULL`, or the handle is `NULL`.

**Example**

```c
uint64_t sub = seraph_store_subscribe(store, "promote", on_event, NULL);
```

### `seraph_store_unsubscribe`

Detach an event subscriber by ID.

```c
int seraph_store_unsubscribe(void* handle, uint64_t subscriber_id);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Store handle. |
| `subscriber_id` | `uint64_t` | yes | ID returned by subscribe. |

**Returns:** `0` on success, negative on error.

**Errors:** Negative if the ID is unknown or the handle is `NULL`.

**Example**

```c
seraph_store_unsubscribe(store, sub);
```

### `seraph_store_subscriber_count`

Number of active subscribers.

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

Load an encoder by model ID, downloading and caching the model files on first use.

```c
void* seraph_encoder_create(const char* model_id);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `model_id` | `const char*` | yes | Embedding model ID; `NULL` for the default model. |

**Returns:** Encoder handle, or `NULL` on error.

**Errors:** `NULL` if the model is unknown or download fails.

**Example**

```c
void* enc = seraph_encoder_create("BAAI/bge-small-en-v1.5");
if (!enc) { /* handle error */ }
```

### `seraph_encoder_encode`

Encode one text into a normalized embedding with the loaded model. The FFI and wrapper also offer an `encode_into` variant that writes into a caller-supplied buffer.

```c
int seraph_encoder_encode(void* handle, const char* text, float** out_embedding, size_t* out_dim);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Encoder handle. |
| `text` | `const char*` | yes | Text to encode. |
| `out_embedding` | `float**` | yes | Out: embedding. |
| `out_dim` | `size_t*` | yes | Out: dim. |

**Returns:** `0` on success, negative on error.

**Errors:** Negative if the handle is `NULL` or encoding fails.

**Example**

```c
float* emb = NULL; size_t dim = 0;
if (seraph_encoder_encode(enc, "the cat sat", &emb, &dim) == 0)
    seraph_floats_free(emb, dim);
```

### `seraph_encoder_encode_into`

Encode one text into a caller-owned buffer, with no allocation.

```c
int seraph_encoder_encode_into(
    void* handle,
    const char* text,
    float* buf,
    size_t buf_len,
    size_t* out_written
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Encoder handle. |
| `text` | `const char*` | yes | Text to encode. |
| `buf` | `float*` | yes | Caller-owned buffer of at least the model's dimension. |
| `buf_len` | `size_t` | yes | Length of `buf`. |
| `out_written` | `size_t*` | yes | Out: written. |

**Returns:** `0` on success, negative on error.

**Errors:** Negative if `buf_len` is smaller than the model dimension or the handle is `NULL`.

**Example**

```c
float buf[1024]; size_t written = 0;
seraph_encoder_encode_into(enc, "the cat sat", buf, 1024, &written);
```

### `seraph_encoder_encode_batch`

Encode several texts in one batched forward pass. The FFI also offers `encode_batch_flat`, which writes all embeddings into one contiguous buffer and is the recommended batch path from C.

```c
char* seraph_encoder_encode_batch(void* handle, const char* texts_json);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Encoder handle. |
| `texts_json` | `const char*` | yes | Texts to encode. |

**Returns:** JSON array of embedding arrays (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` if `texts_json` is malformed or the handle is `NULL`.

**Example**

```c
char* json = seraph_encoder_encode_batch(enc, "[\"a\",\"b\"]");
seraph_string_free(json);
```

### `seraph_encoder_encode_batch_flat`

Encode many texts in one batched pass into a single contiguous float buffer, laid out one embedding after another. The recommended batch encode path on C.

```c
int seraph_encoder_encode_batch_flat(
    void* handle,
    const char* const* texts,
    size_t count,
    float** out_embeddings,
    size_t* out_dim
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Encoder handle. |
| `texts` | `const char* const*` | yes | Texts to encode. |
| `count` | `size_t` | yes | Number of items. |
| `out_embeddings` | `float**` | yes | Out: embeddings. |
| `out_dim` | `size_t*` | yes | Out: dim. |

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

Begin a buffered write batch. Writes skip the per-frame WAL and fsync until commit, which flushes once.

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

End a buffered write batch: flush every buffered write to disk and restore per-write durability.

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

## Federation

Route a query across many independent single-file stores and fan out search in
parallel. A federation holds no state of its own — point it at a directory of
`.sfg` files, or add them by path. All member stores must share the same embedding
model and dimension (enforced at add). See also the `Federation` class in the
Python API and `seraph_wrapper::Federation` in Rust.

Federated search results are a JSON array of objects:

```json
[{ "store": "physics", "frame_id": "…", "score": 0.83, "confidence": 0.81,
   "path_length": 2, "snippet": "…" }]
```

### `seraph_federation_open_dir`

Open every `.sfg` store in a directory as a federation, each member named by its file stem (`physics.sfg` becomes `"physics"`); files starting with `_` or `router` are skipped.

```c
void* seraph_federation_open_dir(const char* dir);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `dir` | `const char*` | yes | Directory holding the member stores. |

**Returns:** Federation handle, or `NULL` on error.

**Errors:** `NULL` on a `NULL` handle or a failed operation; `seraph_last_error` has the message.

### `seraph_federation_new`

Create an empty federation. Add member stores with `add_path`, or open a directory of stores with `open_dir`.

```c
void* seraph_federation_new(void);
```

*No parameters.*

**Returns:** Federation handle (never `NULL`).

**Errors:** `NULL` on a `NULL` handle or a failed operation; `seraph_last_error` has the message.

### `seraph_federation_add_path`

Register a store file under `name` as a member of the federation. The store must share the federation's embedding model and dimension.

```c
int seraph_federation_add_path(void* handle, const char* name, const char* path);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Federation handle. |
| `name` | `const char*` | yes | Member name. |
| `path` | `const char*` | yes | Path to the member's `.sfg` file. |

**Returns:** `0` on success, `-1` on error.

**Errors:** `-1` if the handle is `NULL`.

### `seraph_federation_len`

Number of member stores (`len(fed)` in Python).

```c
size_t seraph_federation_len(void* handle);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Federation handle. |

**Returns:** Member count (`0` if the handle is `NULL`).

**Errors:** `NULL` on a `NULL` handle or a failed operation; `seraph_last_error` has the message.

### `seraph_federation_member_names`

Member store names, in add order.

```c
char* seraph_federation_member_names(void* handle);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Federation handle. |

**Returns:** JSON array of strings (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on a `NULL` handle or a failed operation; `seraph_last_error` has the message.

### `seraph_federation_refresh`

Re-pull routing vectors for any member that promoted an eigenframe since the last refresh. Cheap: only changed members are read.

```c
int seraph_federation_refresh(void* handle);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Federation handle. |

**Returns:** `1` if the router changed, `0` if not, `-1` on error.

**Errors:** `-1` if the handle is `NULL`.

### `seraph_federation_route`

Route a query to the members most likely to hold its answer, without searching them.

```c
char* seraph_federation_route(
    void* handle,
    const float* embedding,
    size_t embedding_len,
    size_t top_stores
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Federation handle. |
| `embedding` | `const float*` | yes | Query vector. |
| `embedding_len` | `size_t` | yes | Length of `embedding`. |
| `top_stores` | `size_t` | yes | Members to return. |

**Returns:** JSON array of `{store, score}` (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on a `NULL` handle or a failed operation; `seraph_last_error` has the message.

### `seraph_federation_search`

Search the federation: route the query to the best-fitting members, search each, and merge the hits into one ranked list.

```c
char* seraph_federation_search(
    void* handle,
    const float* embedding,
    size_t embedding_len,
    size_t top_k,
    float tau,
    const char* mode,
    size_t n
);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Federation handle. |
| `embedding` | `const float*` | yes | Query vector. |
| `embedding_len` | `size_t` | yes | Length of `embedding`. |
| `top_k` | `size_t` | yes | Maximum number of hits. |
| `tau` | `float` | yes | Similarity floor (cosine); `0.0` disables it. |
| `mode` | `const char*` | yes | Routing mode: `"best"`, `"top"` or `"all"`. |
| `n` | `size_t` | yes | Members searched under `"top"`. |

**Returns:** JSON array of hits (free with `seraph_string_free`), or `NULL` on error.

**Errors:** `NULL` on a `NULL` handle or a failed operation; `seraph_last_error` has the message.

### `seraph_federation_free`

Free a federation handle and close its member stores.

```c
void seraph_federation_free(void* handle);
```

**Parameters**

| Name | Type | Required | Description |
|---|---|---|---|
| `handle` | `void*` | yes | Federation handle. |

**Returns:** nothing.

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
    uint32_t path_length; // steps from the entry point
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
