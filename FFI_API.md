# SERAPH C FFI API Reference

> Library: `seraph.dll` (Windows) / `libseraph.so` (Linux) / `libseraph.dylib` (macOS)
> Link: `-lseraph` (Unix) or `seraph.lib` (MSVC)

---

## Conventions

- **Opaque handles:** Store and encoder are `void*` handles. The caller owns the handle and must call the corresponding `_close` function to free it.
- **Error reporting:** Functions return `0` on success, negative on error. Call `seraph_last_error()` for the error message.
- **Strings returned:** UTF-8 `char*` owned by the caller. Free with `seraph_string_free()`.
- **Byte arrays returned:** `uint8_t*` + length, owned by the caller. Free with `seraph_bytes_free()`.
- **Float arrays returned:** `float*` + length, owned by the caller. Free with `seraph_floats_free()`.
- **JSON:** Complex structures are passed and returned as JSON strings.

---

## Table of Contents

1. [Error Handling](#error-handling)
2. [Store Lifecycle](#store-lifecycle)
3. [Encryption](#encryption)
4. [Ingestion](#ingestion)
5. [Search](#search)
6. [Frame Access](#frame-access)
7. [Seeds & Promotion](#seeds--promotion)
8. [Traversal & Chains](#traversal--chains)
9. [Neighborhood](#neighborhood)
10. [Warp (Steering)](#warp-steering)
11. [Analysis & Metrics](#analysis--metrics)
12. [Intelligence](#intelligence)
13. [Consolidation](#consolidation)
14. [Typed Structural Edges](#typed-structural-edges)
15. [Provenance & Sealing](#provenance--sealing)
16. [Anchoring](#anchoring)
17. [Snapshot / Temporal](#snapshot--temporal)
18. [Events & Subscriptions](#events--subscriptions)
19. [Encoder](#encoder)
20. [Batch Operations](#batch-operations)
21. [Memory Management](#memory-management)
22. [Structs Reference](#structs-reference)

---

## Error Handling

```c
// Get the last error message (valid until next FFI call on this thread)
const char* seraph_last_error(void);
```

Pattern:

```c
void* store = seraph_store_open("/path/to/store.sfg");
if (!store) {
    fprintf(stderr, "Error: %s\n", seraph_last_error());
    return -1;
}
```

---

## Store Lifecycle

### `seraph_store_create`

```c
void* seraph_store_create(
    const char* path,        // filesystem path (UTF-8)
    const char* model_id,    // embedding model ID, or NULL for default
    const char* config_json  // JSON config overrides, or NULL for defaults
);
```

**Returns:** Opaque store handle, or `NULL` on error.

**Config JSON fields** (all optional):

```json
{
    "eigen_threshold": 5,
    "eigen_min_age_s": 60.0,
    "tau_similarity": 0.5,
    "tau_merge": 0.92,
    "max_index_elements": 100000,
    "search_max_depth": 50,
    "search_visit_cap_multiplier": 5,
    "redirect_per_frame": true,
    "eigen_matmul_min": 256
}
```

### `seraph_store_open`

```c
void* seraph_store_open(const char* path);
```

**Returns:** Store handle, or `NULL` on error. Fails if the store is encrypted.

### `seraph_store_close`

```c
void seraph_store_close(void* handle);
```

Closes the store and frees all resources. The handle is invalid after this call.

### `seraph_store_sync`

```c
int seraph_store_sync(void* handle);
```

Flush pending writes to disk. Returns `0` on success.

### `seraph_store_set_search_visit_cap_multiplier`

Set the BFS visit-cap multiplier at runtime. Higher values widen the search frontier at minimal latency cost (Phase-1 eigenframe scan dominates search time). The default is set by `search_visit_cap_multiplier` in the config JSON (5).

```c
int seraph_store_set_search_visit_cap_multiplier(void* handle, uint32_t multiplier);
```

**Returns:** `0` on success, `-1` on error.

---

## Encryption

> **Requires Commercial or Enterprise license.** Free-tier calls return `NULL` with an error set.

### `seraph_store_create_encrypted`

Create a new store with AES-256-GCM encryption at rest.

```c
void* seraph_store_create_encrypted(
    const char* path,
    const char* model_id,      // or NULL
    const char* config_json,   // or NULL
    const uint8_t* passphrase, // passphrase bytes
    size_t passphrase_len      // length in bytes
);
```

**Returns:** Store handle, or `NULL` on error.

All content fields are encrypted with a random DEK wrapped by an Argon2id-derived KEK. Structural metadata (watermarks, parent links) remains plaintext for keyless chain verification.

### `seraph_store_open_encrypted`

Open an existing encrypted store.

```c
void* seraph_store_open_encrypted(
    const char* path,
    const uint8_t* passphrase,
    size_t passphrase_len
);
```

**Returns:** Store handle, or `NULL` on error.

**Example:**

```c
const char* pass = "my secret";
void* store = seraph_store_create_encrypted(
    "encrypted.sfg", NULL, NULL,
    (const uint8_t*)pass, strlen(pass)
);
if (!store) { /* handle error */ }

// All put/search/get operations work transparently
seraph_store_close(store);

// Re-open with same passphrase
store = seraph_store_open_encrypted(
    "encrypted.sfg",
    (const uint8_t*)pass, strlen(pass)
);
```

---

## Ingestion

### `seraph_store_put`

```c
char* seraph_store_put(
    void* handle,
    const uint8_t* content,   size_t content_len,
    const float* embedding,   size_t embedding_len
);
```

**Returns:** Frame ID string (caller frees with `seraph_string_free`), or `NULL` on error.

### `seraph_store_put_with_metadata`

```c
char* seraph_store_put_with_metadata(
    void* handle,
    const uint8_t* content,   size_t content_len,
    const float* embedding,   size_t embedding_len,
    const char* metadata_json // JSON object string
);
```

### `seraph_store_put_batch`

```c
char* seraph_store_put_batch(
    void* handle,
    const char* items_json  // JSON array of {content_b64, embedding, metadata?}
);
```

**Returns:** JSON array of frame IDs.

---

## Search

### `seraph_store_search`

```c
int seraph_store_search(
    void* handle,
    const float* embedding,   size_t embedding_len,
    size_t top_k,
    float tau,
    SeraphHit** out_hits,     // out: array of hits
    size_t* out_count         // out: number of hits
);
```

**Returns:** `0` on success. Caller frees hits with `seraph_hits_free(hits, count)`.

```c
// SeraphHit layout
typedef struct {
    char* frame_id;   // owned string
    float score;
    float confidence;
    uint32_t path_length;
} SeraphHit;
```

### `seraph_store_search_text`

Search by text using the store's built-in encoder.

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

### `seraph_store_search_with_warp`

Search with warp field steering.

```c
int seraph_store_search_with_warp(
    void* handle,
    const float* query_embedding,  size_t query_len,
    const char* targets_json,      // JSON array of [embedding, magnitude]
    const char* suppress_json,     // JSON array of [embedding, magnitude]
    const char* profile,           // "routing", "saturation", "bounded"
    float bound_k,
    size_t top_k,
    float tau,
    SeraphHit** out_hits,
    size_t* out_count
);
```

### `seraph_store_search_with_warp_spec`

Search with a full WarpSpec JSON.

```c
int seraph_store_search_with_warp_spec(
    void* handle,
    const float* query_embedding,  size_t query_len,
    const char* warp_spec_json,
    size_t top_k,
    float tau,
    SeraphHit** out_hits,
    size_t* out_count
);
```

---

## Frame Access

### `seraph_store_get_frame`

```c
char* seraph_store_get_frame(void* handle, const char* frame_id);
```

**Returns:** JSON string with all frame fields, or `NULL` if not found.

### `seraph_store_get_content`

```c
int seraph_store_get_content(
    void* handle,
    const char* frame_id,
    uint8_t** out_content,   // out: content bytes
    size_t* out_len          // out: content length
);
```

Caller frees with `seraph_bytes_free(out_content, out_len)`.

### `seraph_store_get_embedding`

```c
int seraph_store_get_embedding(
    void* handle,
    const char* frame_id,
    float** out_embedding,   // out: float array
    size_t* out_dim          // out: dimensionality
);
```

Caller frees with `seraph_floats_free(out_embedding, out_dim)`.

### `seraph_store_get_embedding_into`

Zero-copy variant: writes into a caller-supplied buffer.

```c
int seraph_store_get_embedding_into(
    void* handle,
    const char* frame_id,
    float* buf,              // caller-owned buffer
    size_t buf_len,          // buffer capacity
    size_t* out_written      // out: elements written
);
```

### `seraph_store_get_metadata`

```c
char* seraph_store_get_metadata(void* handle, const char* frame_id);
```

**Returns:** JSON string of metadata, or `NULL`.

### `seraph_store_borrow_frame`

Zero-copy frame view. The view borrows from the store -- do not mutate the store while the view is held.

```c
SeraphFrameView* seraph_store_borrow_frame(void* handle, const char* frame_id);
void seraph_store_release_frame(SeraphFrameView* view);
```

### `seraph_store_frame_ids`

```c
char* seraph_store_frame_ids(void* handle);
```

**Returns:** JSON array of all frame IDs.

### `seraph_store_eigenframe_ids`

```c
char* seraph_store_eigenframe_ids(void* handle);
```

**Returns:** JSON array of eigenframe IDs.

### `seraph_store_eigenframes`

```c
char* seraph_store_eigenframes(void* handle);
```

**Returns:** JSON array of full frame objects for all eigenframes.

### `seraph_store_federation_vector`

Mean of all eigenframe embeddings — a single vector representing the store's semantic center. Used for federation routing (D2 dispatch).

```c
int seraph_store_federation_vector(
    void* handle,
    float** out_embedding,   // out: float array (caller frees with seraph_floats_free)
    size_t* out_dim          // out: dimensionality
);
```

**Returns:** `0` on success. If the store has no eigenframes, `out_embedding` is set to `NULL` and `out_dim` to `0` (still returns `0`). Returns `-1` on error (null handle).

### `seraph_store_model_id`

```c
char* seraph_store_model_id(void* handle);
```

### `seraph_store_dim`

```c
size_t seraph_store_dim(void* handle);
```

### `seraph_store_stats`

```c
int seraph_store_stats(void* handle, SeraphStats* out);
```

Writes into a caller-supplied `SeraphStats` struct.

---

## Seeds & Promotion

### `seraph_store_add_seed`

```c
char* seraph_store_add_seed(
    void* handle,
    const char* label,
    const float* embedding,  size_t embedding_len,
    const char* metadata_json  // or NULL
);
```

### `seraph_store_promote`

```c
int seraph_store_promote(void* handle, const char* frame_id);
```

### `seraph_store_promote_content`

```c
char* seraph_store_promote_content(
    void* handle,
    const uint8_t* content,  size_t content_len,
    const float* embedding,  size_t embedding_len,
    const char* metadata_json
);
```

---

## Traversal & Chains

### `seraph_store_path`

```c
char* seraph_store_path(void* handle, const char* from_id, const char* to_id);
```

**Returns:** JSON array of frame objects along the BFS path.

### `seraph_store_chain`

```c
char* seraph_store_chain(void* handle, const char* from_id, const char* to_id);
```

**Returns:** JSON chain result with frames, primitive distance, and coherence metrics.

### `seraph_store_chain_to`

```c
char* seraph_store_chain_to(
    void* handle,
    const float* query_embedding, size_t query_len
);
```

### `seraph_store_chain_between`

```c
char* seraph_store_chain_between(
    void* handle,
    const float* emb_a, size_t len_a,
    const float* emb_b, size_t len_b
);
```

### `seraph_store_lineage`

```c
char* seraph_store_lineage(void* handle, const char* frame_id);
```

### `seraph_store_subtree`

```c
char* seraph_store_subtree(void* handle, const char* frame_id, size_t max_depth);
```

### `seraph_store_get_children`

```c
char* seraph_store_get_children(void* handle, const char* frame_id);
```

---

## Neighborhood

### `seraph_store_neighborhood`

```c
char* seraph_store_neighborhood(
    void* handle,
    const char* frame_id,
    float tau,
    size_t hops
);
```

**Returns:** JSON with frames, edges, and region eigenframe.

---

## Warp (Steering)

> **Profile values:** the `profile` string accepts `"routing"`, `"saturation"`, or `"bounded"`. Any other value is an error. `"bounded"` is the default used by the higher-level wrappers.

### `seraph_store_apply_warp`

Apply warp to a query embedding without searching.

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

### `seraph_store_calibrate_warp`

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

**Returns:** JSON calibration report.

---

## Analysis & Metrics

| Function | Signature | Returns |
|----------|-----------|---------|
| `seraph_store_density_score` | `(handle, frame_id) -> float` | Local density |
| `seraph_store_seed_margin` | `(handle, frame_id) -> float` | Distance to nearest seed |
| `seraph_store_steerability` | `(handle, query_emb, len, target_id) -> float` | Steerability score |
| `seraph_store_region_health` | `(handle, eigenframe_id) -> char*` | JSON health report |
| `seraph_store_region_health_struct` | `(handle, eigenframe_id, out) -> int` | Writes to `SeraphRegionHealth` struct |
| `seraph_store_similarity_stats` | `(handle) -> char*` | JSON similarity distribution |
| `seraph_store_similarity_stats_struct` | `(handle, out) -> int` | Writes to `SeraphSimilarityStats` struct |
| `seraph_store_resolve_tau` | `(handle) -> float` | Auto-resolved tau value |

---

## Intelligence

### `seraph_store_drift_report`

```c
char* seraph_store_drift_report(void* handle, size_t window, float threshold);
```

### `seraph_store_contradictions`

```c
char* seraph_store_contradictions(void* handle, float min_divergence);
```

### `seraph_store_evolution`

```c
char* seraph_store_evolution(void* handle, const char* seed_frame_id);
```

### `seraph_store_merge_candidates`

```c
char* seraph_store_merge_candidates(void* handle, float tau);
```

All return JSON strings (caller frees with `seraph_string_free`).

---

## Consolidation

### `seraph_store_consolidate`

```c
char* seraph_store_consolidate(void* handle, const char* frame_ids_json);
```

**`frame_ids_json`:** JSON array of frame ID strings.
**Returns:** JSON consolidation report.

### `seraph_store_consolidate_all`

```c
char* seraph_store_consolidate_all(void* handle);
```

---

## Typed Structural Edges

### `seraph_store_declare_relation`

```c
int seraph_store_declare_relation(void* handle, const char* name, uint32_t flags);
```

### `seraph_store_list_relations`

```c
char* seraph_store_list_relations(void* handle);
```

### `seraph_store_assert_relation`

```c
int seraph_store_assert_relation(
    void* handle,
    const char* from_id,
    const char* to_id,
    const char* relation,
    float weight
);
```

### `seraph_store_retract_relation`

```c
int seraph_store_retract_relation(
    void* handle,
    const char* from_id,
    const char* to_id,
    const char* relation
);
```

### `seraph_store_structural_neighbors`

```c
char* seraph_store_structural_neighbors(
    void* handle,
    const char* frame_id,
    const char* relations_json,  // JSON array of relation names, or NULL for all
    const char* direction        // "outgoing", "incoming", "both"
);
```

---

## Provenance & Sealing

### `seraph_store_verify`

```c
int seraph_store_verify(void* handle, char** out_failures_json);
```

Returns `0` if chain is valid. If invalid, `out_failures_json` contains a JSON array of failure descriptions.

### `seraph_store_seal`

> **Requires Commercial or Enterprise license.** Free-tier calls return `NULL` with an error set.

```c
char* seraph_store_seal(void* handle, const char* reason);
```

**Returns:** Seal frame ID. All subsequent writes will fail.

### `seraph_store_is_sealed`

```c
int seraph_store_is_sealed(void* handle);
```

Returns `1` if sealed, `0` if not. All tiers can open and read sealed stores.

### `seraph_store_writer_audit`

```c
char* seraph_store_writer_audit(void* handle);
```

### `seraph_store_verify_attestation`

```c
char* seraph_store_verify_attestation(void* handle);
```

### `seraph_store_genesis_seed`

```c
int seraph_store_genesis_seed(void* handle, uint8_t** out_seed, size_t* out_len);
```

### `seraph_store_genesis_attestation`

```c
char* seraph_store_genesis_attestation(void* handle);
```

---

## Anchoring

### `seraph_store_append_tip_anchor`

> **Requires Commercial or Enterprise license.** Free-tier calls return `NULL` with an error set.

```c
char* seraph_store_append_tip_anchor(
    void* handle,
    const char* backend,         // e.g., "opentimestamps", "rfc3161"
    const uint8_t* token,        // proof/token bytes
    size_t token_len,
    const char* tip_frame_id,    // frame ID this anchor covers
    const char* status,          // "pending" or "confirmed"
    const char* supersedes       // nullable — ID of previous anchor to supersede
);
```

**Returns:** Anchor frame ID (caller frees with `seraph_string_free()`), or `NULL` on error.

### `seraph_store_list_tip_anchors`

```c
char* seraph_store_list_tip_anchors(void* handle, int resolved);
```

**Returns:** JSON array string of tip-anchor records (caller frees). When `resolved` is non-zero, supersession chains are collapsed to the latest version. Available to all tiers.

---

### `seraph_store_supersede`

```c
char* seraph_store_supersede(void* handle, const char* frame_id);
```

---

## Snapshot / Temporal

### `seraph_store_snapshot`

```c
SeraphSnapshot* seraph_store_snapshot(void* handle, int64_t at_timestamp_us);
```

### `seraph_snapshot_frame_ids`

```c
char* seraph_snapshot_frame_ids(const SeraphSnapshot* snapshot);
```

### `seraph_snapshot_timestamp`

```c
int64_t seraph_snapshot_timestamp(const SeraphSnapshot* snapshot);
```

### `seraph_store_snapshot_search`

```c
char* seraph_store_snapshot_search(
    void* handle,
    const SeraphSnapshot* snapshot,
    const float* query_embedding, size_t query_len,
    size_t top_k
);
```

### `seraph_snapshot_free`

```c
void seraph_snapshot_free(SeraphSnapshot* snapshot);
```

---

## Events & Subscriptions

### Callback type

```c
typedef void (*SeraphEventCallback)(const char* event_json, void* user_data);
```

### `seraph_store_subscribe`

```c
uint64_t seraph_store_subscribe(
    void* handle,
    const char* event_name,       // "promote", "contradiction", "drift", "consolidation", "milestone", "*"
    SeraphEventCallback callback,
    void* user_data
);
```

**Returns:** Subscriber ID (non-zero), or `0` on error.

### `seraph_store_unsubscribe`

```c
int seraph_store_unsubscribe(void* handle, uint64_t subscriber_id);
```

### `seraph_store_subscriber_count`

```c
size_t seraph_store_subscriber_count(void* handle);
```

---

## Encoder

### `seraph_encoder_create`

```c
void* seraph_encoder_create(const char* model_id);
```

Creates a standalone encoder. Downloads and caches the model on first use.

### `seraph_encoder_encode`

```c
int seraph_encoder_encode(
    void* handle,
    const char* text,
    float** out_embedding,
    size_t* out_dim
);
```

### `seraph_encoder_encode_into`

Zero-copy variant with caller-owned buffer.

```c
int seraph_encoder_encode_into(
    void* handle,
    const char* text,
    float* buf,
    size_t buf_len,
    size_t* out_written
);
```

### `seraph_encoder_encode_batch`

```c
char* seraph_encoder_encode_batch(void* handle, const char* texts_json);
```

**Returns:** JSON array of embedding arrays.

### `seraph_encoder_encode_batch_flat`

Batch-encode texts into a contiguous `float*` buffer. One FFI call encodes all texts in a single batched forward pass, amortising GPU kernel launch and per-call marshalling overhead. **This is the recommended batch encode path** — it matches native Rust throughput (~600 enc/s on GPU).

```c
int seraph_encoder_encode_batch_flat(
    void* handle,
    const char** texts,        // array of C strings
    size_t count,              // number of texts
    float** out_embeddings,    // out: contiguous float array [count * dim]
    size_t* out_dim            // out: embedding dimension
);
```

**Returns:** `0` on success, `-1` on error. Caller frees with `seraph_floats_free(out_embeddings, count * out_dim)`.

The output buffer is laid out as `[emb0[0..dim], emb1[0..dim], ..., emb_{count-1}[0..dim]]`.

**Example:**

```c
const char* texts[] = {"first document", "second document", "third document"};
float* embeddings = NULL;
size_t dim = 0;

int rc = seraph_encoder_encode_batch_flat(encoder, texts, 3, &embeddings, &dim);
if (rc != 0) { /* handle error */ }

// Access embedding for text i: embeddings + (i * dim), length dim
for (size_t i = 0; i < 3; i++) {
    float* emb_i = embeddings + (i * dim);
    // use emb_i[0..dim]
}

seraph_floats_free(embeddings, 3 * dim);
```

### `seraph_encoder_close`

```c
void seraph_encoder_close(void* handle);
```

---

## Batch Operations

### `seraph_store_begin_batch`

```c
int seraph_store_begin_batch(void* handle);
```

Begin a batch write session. Index updates are deferred until commit.

### `seraph_store_commit`

```c
int seraph_store_commit(void* handle);
```

Commit the batch and update all indices.

---

## Memory Management

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

```c
typedef struct {
    char* frame_id;
    float score;
    float confidence;
    uint32_t path_length;
} SeraphHit;
```

### `SeraphStats`

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

```c
typedef struct {
    float   coherence;
    uint8_t direction;        // 0 = stable, 1 = consolidating, 2 = fragmenting
    uint8_t candidate_split;  // 1 if the region is a split candidate, else 0
} SeraphRegionHealth;
```

### `SeraphSimilarityStats`

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
`seraph_store_release_frame`. The struct carries three trailing private
pointers used for cleanup; treat the view as opaque beyond the documented
fields and never construct one yourself. Field order below matches the
`#[repr(C)]` layout exactly.

```c
typedef struct {
    const char*  id;
    const char*  parent_id;     // NULL if no parent
    const float* embedding;
    size_t       embedding_dim;
    const uint8_t* content;
    size_t       content_len;
    uint32_t     in_degree;
    uint8_t      status;        // 0=Genesis 1=Seed 2=Active 3=Eigenframe 4=Superseded 5=Redirect 6=System
    int64_t      timestamp;
    // ... three private cleanup pointers follow (do not access) ...
} SeraphFrameView;
```

> **Note:** this `status` byte uses a different numbering than the Python
> `FrameStatus` enum. The values above are authoritative for the C ABI.

---

Copyright (c) 2025-2026 Tree D Interactive LLC. All rights reserved.
