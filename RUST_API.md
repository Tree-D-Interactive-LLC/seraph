# SERAPH Rust API Reference

> Crate: `seraph` (the `seraph-wrapper` crate)
> Add via path/vendor: `seraph = { path = "vendor/seraph" }`

This reference documents the idiomatic Rust API for the people who **call**
SERAPH from Rust. It is the third public surface, alongside the
[C FFI](FFI_API.md) and the [Python bindings](PYTHON_API.md); it wraps the same
compiled engine as safe `Store` / `Encoder` / `Federation` types. Read
*Authentication & Licensing* and *Conventions* first — they apply to every
function and are not repeated per entry.

---

## Authentication & Licensing

SERAPH performs no network authentication. "Authorization" is **license-tier
gating**, enforced locally at the call site. Most methods work on every tier.
The capabilities below require a **Commercial or Enterprise** license; on the
Free tier they return `Err(Error)` with the message available via the error's
`Display`. These are tagged 🔒 **Licensed** inline.

| Capability | Methods |
|---|---|
| Encryption at rest | `Store::create_encrypted`, `Store::open_encrypted` |
| Sealing | `Store::seal` |
| Time anchoring | `Store::append_tip_anchor` |

**Writer identity (secure mode).** The license attests *who is licensed and at
what tier* — it does **not** carry a signing key. Per-frame writer attribution
uses a key the **calling application supplies**: pass a 32-byte Ed25519 private
seed to `Store::create_with_writer` / `Store::open_with_writer`. The seed stays
in the caller's process; SERAPH signs each frame with it and records only the
derived public key in the store. Secure mode requires both a permitting tier and
a supplied writer seed.

---

## Conventions

These rules hold for all functions unless an entry says otherwise:

- **Linking.** The crate links the compiled engine cdylib (`seraph.dll` /
  `libseraph.so` / `libseraph.dylib`). Set `SERAPH_LIB_DIR` to the directory
  holding it at build time, and ensure that directory is on the runtime library
  search path (`PATH` / `LD_LIBRARY_PATH` / `DYLD_LIBRARY_PATH`).
- **Errors are `Result`.** Fallible operations return `Result<_, Error>`;
  `Error` carries the engine's message (`impl Display`). Some read accessors
  return `Option<_>`, where `None` means *absent or failed* — call the `_json`
  or `Result`-returning variant if you need the reason.
- **Ownership.** Returned `String` / `Vec<_>` are owned Rust values; the wrapper
  copies out of engine allocations and frees them. A [`FrameView`](#types-reference)
  borrows engine memory for zero-copy reads — do not hold it across a mutation
  of the same store.
- **Embeddings are caller-supplied.** Pass any embedding as `&[f32]`; its length
  must equal the store dimension (`store.dim()`). The `Encoder` / `search_text`
  helpers exist for convenience but are not required.
- **Model ID is fixed at genesis.** The embedding model is declared at `create`
  and permanent for the store's lifetime. Mismatched writes are rejected.
- **`_json` methods** return the engine's JSON string verbatim. Deserialize with
  your own `serde` types. Typed variants (`region_health`, `similarity_stats`,
  `search`) avoid JSON where a struct exists.
- **Content is bytes.** `content` is `&[u8]` (any binary payload). Frame IDs are
  `String`.
- **Thread-safety.** `Store`, `Encoder`, and `Federation` are `Send`; mutable
  state is internally synchronized. A store is closed when its `Store` is
  dropped.

---

## Table of Contents

1. [Store Lifecycle](#store-lifecycle)
2. [Writer Identity](#writer-identity)
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
19. [Federation](#federation)
20. [Encoding](#encoding)
21. [Migration](#migration)
22. [Types Reference](#types-reference)

---

## Store Lifecycle

### `Store::create`

Create a new store on disk with a fixed embedding model declared at genesis.

```rust
pub fn create(path: impl AsRef<Path>, config: Option<StoreConfig>) -> Result<Self, Error>
```

**Parameters**

| Name | Type | Description |
|---|---|---|
| `path` | `impl AsRef<Path>` | Filesystem path for the new `.sfg` file. |
| `config` | `Option<StoreConfig>` | Store configuration; `None` → defaults (model `BAAI/bge-small-en-v1.5`). |

**Returns** `Ok(Store)`, or `Err` if the path is unwritable, the store exists, or the config is invalid.

```rust
use seraph::{Store, StoreConfig};
let mut store = Store::create("knowledge.sfg", Some(StoreConfig::new().model_id("BAAI/bge-small-en-v1.5")))?;
# Ok::<(), seraph::Error>(())
```

### `Store::open`

Open an existing store.

```rust
pub fn open(path: impl AsRef<Path>) -> Result<Self, Error>
```

### `Store::sync`

Flush pending writes to disk.

```rust
pub fn sync(&mut self) -> Result<(), Error>
```

---

## Writer Identity

### `Store::create_with_writer`

Create a store that signs each frame with an app-supplied writer identity
(Standard-mode attribution). `writer_seed` is a 32-byte Ed25519 private seed.

```rust
pub fn create_with_writer(
    path: impl AsRef<Path>,
    config: Option<StoreConfig>,
    writer_seed: &[u8; 32],
) -> Result<Self, Error>
```

### `Store::open_with_writer`

Open a store with an app-supplied writer identity for signing frames appended
during this session. `writer_seed` is a 32-byte Ed25519 seed.

```rust
pub fn open_with_writer(path: impl AsRef<Path>, writer_seed: &[u8; 32]) -> Result<Self, Error>
```

---

## Encryption

### `Store::create_encrypted`

🔒 **Licensed.** Create an encrypted store. The passphrase derives an
AES-256-GCM key via Argon2id; content is encrypted at rest while structural
metadata stays plaintext for keyless chain verification.

```rust
pub fn create_encrypted(
    path: impl AsRef<Path>,
    config: Option<StoreConfig>,
    passphrase: &str,
) -> Result<Self, Error>
```

### `Store::open_encrypted`

🔒 **Licensed.** Open an encrypted store with its passphrase.

```rust
pub fn open_encrypted(path: impl AsRef<Path>, passphrase: &str) -> Result<Self, Error>
```

---

## Ingestion

### `Store::put`

Ingest a frame with raw content bytes and a pre-computed embedding. Returns the
new frame's ID.

```rust
pub fn put(&mut self, content: &[u8], embedding: &[f32]) -> Result<String, Error>
```

**Parameters**

| Name | Type | Description |
|---|---|---|
| `content` | `&[u8]` | Frame content (any binary payload). |
| `embedding` | `&[f32]` | Embedding of length `store.dim()`. |

### `Store::put_with_metadata`

Ingest a frame with content, embedding, and a JSON metadata string.

```rust
pub fn put_with_metadata(
    &mut self,
    content: &[u8],
    embedding: &[f32],
    metadata_json: &str,
) -> Result<String, Error>
```

### `Store::put_batch_json`

Batch-ingest content/embedding pairs. `items_json` is a JSON array of
`{"content": "...", "embedding": [...]}` (or `{"content_b64": "..."}`). Returns
the new frame IDs. This is the bulk-ingest path — prefer it over a `put` loop.

```rust
pub fn put_batch_json(&mut self, items_json: &str) -> Result<Vec<String>, Error>
```

### `Store::begin_batch`

Enable buffered write mode (skip per-frame WAL + fsync) until `commit`.

```rust
pub fn begin_batch(&mut self) -> Result<(), Error>
```

### `Store::commit`

Flush buffered writes and re-enable per-write fsync.

```rust
pub fn commit(&mut self) -> Result<(), Error>
```

---

## Search

### `Store::search`

Search by a query embedding. Cosine selects Phase-1 entry points; the graph
traversal does the retrieval. Returns hits ranked by raw cosine (empty if
nothing passes `tau`).

```rust
pub fn search(&self, query_embedding: &[f32], top_k: usize, tau: f32) -> Result<Vec<Hit>, Error>
```

**Parameters**

| Name | Type | Description |
|---|---|---|
| `query_embedding` | `&[f32]` | Query vector of length `store.dim()`. |
| `top_k` | `usize` | Maximum number of hits to return. |
| `tau` | `f32` | Similarity floor for Phase-2 (e.g. `0.5`; `0.0` disables). |

### `Store::search_opts`

Search with options — same as [`search`](#storesearch) plus an
`include_superseded` flag.

```rust
pub fn search_opts(
    &self,
    query_embedding: &[f32],
    top_k: usize,
    tau: f32,
    include_superseded: bool,
) -> Result<Vec<Hit>, Error>
```

### `Store::search_temporal`

Search with a temporal view. Relevance selection is identical to
[`search_opts`](#storesearch_opts) — entry points, traversal, tau, and top_k
stay relevance-based. `after_us`/`before_us` form an inclusive visibility
window on the frame timestamp (microseconds since epoch, `None` = unbounded):
out-of-window frames are still traversed as routing nodes but cannot occupy
result slots, so `top_k` fills with in-window frames. `order` reorders the
already-selected top_k set as pure presentation (stable sort by timestamp;
scores and confidences untouched). `TemporalOrder::Relevance` with no window
matches `search_opts` exactly.

```rust
pub fn search_temporal(
    &self,
    query_embedding: &[f32],
    top_k: usize,
    tau: f32,
    include_superseded: bool,
    order: TemporalOrder,
    after_us: Option<u64>,
    before_us: Option<u64>,
) -> Result<Vec<Hit>, Error>
```

**Parameters**

| Name | Type | Description |
|---|---|---|
| `query_embedding` | `&[f32]` | Query vector of length `store.dim()`. |
| `top_k` | `usize` | Maximum number of hits to return. |
| `tau` | `f32` | Similarity floor for Phase-2 (`0.0` disables). |
| `include_superseded` | `bool` | Opt soft-forgotten frames back into results (as `search_opts`). |
| `order` | `TemporalOrder` | `Relevance` (score-ranked, default), `MostRecent` (timestamp descending), `Oldest` (timestamp ascending). |
| `after_us` | `Option<u64>` | Inclusive lower bound on frame timestamp (µs since epoch); `None` = unbounded. |
| `before_us` | `Option<u64>` | Inclusive upper bound on frame timestamp (µs since epoch); `None` = unbounded. |

### `Store::search_text`

Encode `query` with the store's model and search in one call.

```rust
pub fn search_text(&self, query: &str, top_k: usize, tau: f32) -> Result<Vec<Hit>, Error>
```

### `Store::set_search_visit_cap_multiplier`

Set the Phase-2 BFS visit-cap multiplier (completeness ↔ speed).

```rust
pub fn set_search_visit_cap_multiplier(&mut self, multiplier: u32) -> Result<(), Error>
```

### `Store::set_search_entry_k`

Fix the Phase-1 entry rank floor (`0` restores the default).

```rust
pub fn set_search_entry_k(&mut self, k: usize) -> Result<(), Error>
```

### `Store::set_search_entry_gap`

Relative-gap Phase-1 entry breadth (`0` falls back to the entry_k floor).

```rust
pub fn set_search_entry_gap(&mut self, gap: f32) -> Result<(), Error>
```

### `Store::set_search_use_stream`

EXPERIMENTAL: route the read path through the gradient-beam stream
(`beam` of `0` = unbounded).

```rust
pub fn set_search_use_stream(&mut self, on: bool, beam: usize) -> Result<(), Error>
```

### `Store::set_search_stream_threshold`

Threshold-gate: `search` auto-streams at/above `n` frames (`0` disables).

```rust
pub fn set_search_stream_threshold(&mut self, n: usize) -> Result<(), Error>
```

### `Store::set_search_max_visits`

Absolute Phase-2 visit budget, decoupled from `top_k` (`0` = multiplier fallback).

```rust
pub fn set_search_max_visits(&mut self, v: usize) -> Result<(), Error>
```

### `Store::set_search_ceiling_margin`

Stream ceiling slack margin (below `1.0` keeps the beam exploring past the
k-th-best score).

```rust
pub fn set_search_ceiling_margin(&mut self, m: f32) -> Result<(), Error>
```

### `Store::set_search_arena`

Toggle the Phase-2 contiguous pre-normalized embedding arena. Results unchanged.

```rust
pub fn set_search_arena(&mut self, on: bool) -> Result<(), Error>
```

### `Store::set_search_adjacency`

When on, the Phase-2 flood reads the pre-resolved search adjacency. Results
unchanged.

```rust
pub fn set_search_adjacency(&mut self, on: bool) -> Result<(), Error>
```

### `Store::set_walk_profile`

DIAGNOSTIC: gate Phase-2 walk profiling (thread-local, zero-overhead off).

```rust
pub fn set_walk_profile(&self, on: bool) -> Result<(), Error>
```

### `Store::take_walk_profile_json`

DIAGNOSTIC: read + reset the walk-profile accumulators. Returns JSON
`{"resolve_ns", "visited_ns", "score_ns", "push_ns", "visit_count"}`.

```rust
pub fn take_walk_profile_json(&self) -> Result<String, Error>
```

### `Store::entry_count`

DIAGNOSTIC: number of Phase-1 entry points selected for a query.

```rust
pub fn entry_count(&self, query_embedding: &[f32]) -> Result<usize, Error>
```

### `Store::last_search_visits`

DIAGNOSTIC: nodes visited by the last streamed search (ops cost).

```rust
pub fn last_search_visits(&self) -> Result<usize, Error>
```

---

## Frame Access

### `Store::get_content`

Get a frame's content bytes by ID (`None` if the frame has no content).

```rust
pub fn get_content(&self, frame_id: &str) -> Result<Option<Vec<u8>>, Error>
```

### `Store::get_metadata`

Get a frame's metadata as a JSON string.

```rust
pub fn get_metadata(&self, frame_id: &str) -> Option<String>
```

### `Store::get_frame_json`

Get a frame as a JSON object string with all properties.

```rust
pub fn get_frame_json(&self, frame_id: &str) -> Option<String>
```

### `Store::get_embedding`

Get a frame's embedding as an owned `Vec<f32>`.

```rust
pub fn get_embedding(&self, frame_id: &str) -> Result<Option<Vec<f32>>, Error>
```

### `Store::get_embedding_into`

Copy a frame's embedding into a caller-owned buffer (no allocation). Returns the
number of floats written.

```rust
pub fn get_embedding_into(&self, frame_id: &str, buf: &mut [f32]) -> Result<Option<usize>, Error>
```

### `Store::borrow_frame`

Borrow a frame for zero-copy reads. The returned [`FrameView`](#types-reference)
points into engine memory — do not hold it across a store mutation.

```rust
pub fn borrow_frame(&self, frame_id: &str) -> Option<FrameView>
```

### `Store::model_id`

The store's embedding model ID.

```rust
pub fn model_id(&self) -> String
```

### `Store::dim`

The embedding dimension.

```rust
pub fn dim(&self) -> usize
```

### `Store::frame_ids`

All frame IDs in the store.

```rust
pub fn frame_ids(&self) -> Vec<String>
```

### `Store::eigenframe_ids`

The IDs of all eigenframes.

```rust
pub fn eigenframe_ids(&self) -> Vec<String>
```

### `Store::stats`

Store statistics (frame/eigenframe/superseded/redirect counts).

```rust
pub fn stats(&self) -> Result<Stats, Error>
```

---

## Seeds & Promotion

### `Store::add_seed`

Add a seed (geometric attractor) to a live store. Returns the seed frame's ID.

```rust
pub fn add_seed(&mut self, label: &str, embedding: &[f32], metadata_json: Option<&str>) -> Result<String, Error>
```

### `Store::promote`

Promote an existing active frame to eigenframe.

```rust
pub fn promote(&mut self, frame_id: &str) -> Result<(), Error>
```

### `Store::promote_content`

Ingest content directly as an eigenframe (atomic ingest + promote). Returns the
new frame's ID.

```rust
pub fn promote_content(
    &mut self,
    content: &[u8],
    embedding: &[f32],
    metadata_json: Option<&str>,
) -> Result<String, Error>
```

### `Store::eigenframes_json`

Eigenframes as a JSON array of frame objects.

```rust
pub fn eigenframes_json(&self) -> String
```

---

## Traversal & Chains

### `Store::get_children`

The children of a frame (frames whose semantic parent is `frame_id`).

```rust
pub fn get_children(&self, frame_id: &str) -> Vec<String>
```

### `Store::lineage`

The parent chain from a frame up to genesis.

```rust
pub fn lineage(&self, frame_id: &str) -> Vec<String>
```

### `Store::path`

The graph path between two frames (empty if none).

```rust
pub fn path(&self, from_id: &str, to_id: &str) -> Vec<String>
```

### `Store::subtree`

All descendant frame IDs beneath `frame_id`, bounded by `max_depth`.

```rust
pub fn subtree(&self, frame_id: &str, max_depth: usize) -> Vec<String>
```

### `Store::chain_json`

A reasoning chain between two frames, as JSON.

```rust
pub fn chain_json(&self, from_id: &str, to_id: &str) -> Result<String, Error>
```

### `Store::chain_to_json`

A chain from the geometric entry point to the nearest frame for a query
embedding, as JSON.

```rust
pub fn chain_to_json(&self, query_embedding: &[f32]) -> Result<String, Error>
```

### `Store::chain_between_json`

A chain between the regions of two query embeddings, as JSON.

```rust
pub fn chain_between_json(&self, emb_a: &[f32], emb_b: &[f32]) -> Result<String, Error>
```

### `Store::gradient_json`

The semantic gradient along an ordered chain of frame IDs (`chain_json` is a
JSON array of id strings). Returns `{"frame_id","similarity","delta"}` objects.

```rust
pub fn gradient_json(&self, chain_json: &str) -> Result<String, Error>
```

### `Store::constitutive_score`

Constitutive score for a chain: `1 - jaccard(chain_intermediates, midpoint_NN)`.
`chain_json` is a JSON array of id strings. Returns `f32::NAN` on bad input.

```rust
pub fn constitutive_score(&self, chain_json: &str) -> f32
```

---

## Neighborhood

### `Store::neighborhood_json`

The three-population neighborhood (parent / children / similar) around a frame,
as JSON.

```rust
pub fn neighborhood_json(&self, frame_id: &str, tau: f32, hops: usize) -> Result<String, Error>
```

### `Store::neighborhood_spec_json`

Same as [`neighborhood_json`](#storeneighborhood_json), with tau given as a
`TauSpec` (`Auto` p95 / `Loose` p75 / `Strict` p99 / `Value(f32)`).

```rust
pub fn neighborhood_spec_json(&self, frame_id: &str, tau: TauSpec, hops: usize) -> Result<String, Error>
```

---

## Warp (Steering)

### `Store::search_with_warp`

Search with an inline warp: steer the query toward `targets_json` and away from
`suppress_json` (JSON arrays of `{"embedding":[...],"magnitude":f32}`).

```rust
pub fn search_with_warp(
    &self,
    query_embedding: &[f32],
    targets_json: &str,
    suppress_json: &str,
    profile: &str,
    bound_k: f32,
    top_k: usize,
    tau: f32,
) -> Result<Vec<Hit>, Error>
```

### `Store::search_with_warp_spec`

Search with a `WarpSpec` (JSON). Supports frame-ID targets and named magnitudes.

```rust
pub fn search_with_warp_spec(
    &self,
    query_embedding: &[f32],
    warp_spec_json: &str,
    top_k: usize,
    tau: f32,
) -> Result<Vec<Hit>, Error>
```

### `Store::warp_query`

Apply a `WarpSpec` to a query embedding (pure math, no search). Returns the
warped vector.

```rust
pub fn warp_query(&self, query_embedding: &[f32], warp_spec_json: &str) -> Result<Vec<f32>, Error>
```

### `Store::calibrate_warp`

Calibrate warp magnitudes by an empirical sweep. Returns JSON.

```rust
pub fn calibrate_warp(
    &self,
    n_queries: usize,
    n_targets: usize,
    magnitudes: Option<&[f32]>,
    top_k: usize,
    seed: u64,
) -> Result<String, Error>
```

### `Store::fixup_warp`

Run the warp-calibration fixup (self-tuning closed loop). Returns whether the
resident calibration changed.

```rust
pub fn fixup_warp(&mut self) -> Result<bool, Error>
```

### `Store::warp_calibration`

The store's resident warp calibration as a JSON object.

```rust
pub fn warp_calibration(&self) -> Result<String, Error>
```

### `Store::steerability`

How steerable a query is toward a target frame (a warp diagnostic).

```rust
pub fn steerability(&self, query_embedding: &[f32], target_id: &str) -> f32
```

### `apply_warp`

Apply a warp to a query vector (pure math, no store, no search). Free function.

```rust
pub fn apply_warp(
    query_embedding: &[f32],
    targets_json: &str,
    suppress_json: &str,
    profile: &str,
    bound_k: f32,
) -> Result<Vec<f32>, Error>
```

---

## Analysis & Metrics

### `Store::density_score`

Local density score for a frame.

```rust
pub fn density_score(&self, frame_id: &str) -> f32
```

### `Store::seed_margin`

Seed margin: geometric distinctness relative to the corpus centroid.

```rust
pub fn seed_margin(&self, frame_id: &str) -> f32
```

### `Store::region_health`

Region health for an eigenframe as a typed struct (no JSON parsing).

```rust
pub fn region_health(&self, eigenframe_id: &str) -> Result<RegionHealth, Error>
```

### `Store::region_health_json`

Region health for an eigenframe as JSON.

```rust
pub fn region_health_json(&self, eigenframe_id: &str) -> Option<String>
```

### `Store::similarity_stats`

Parent-child similarity distribution as a typed struct.

```rust
pub fn similarity_stats(&self) -> Result<SimilarityStats, Error>
```

### `Store::similarity_stats_json`

Parent-child similarity distribution as JSON.

```rust
pub fn similarity_stats_json(&self) -> Option<String>
```

### `Store::resolve_tau`

The store's auto-resolved tau (its p95 parent-child similarity).

```rust
pub fn resolve_tau(&self) -> f32
```

### `Store::resolve_tau_spec`

Resolve tau from a `TauSpec`: `Auto` (background p95), `Loose` (p75), `Strict`
(p99), or `Value(f32)` (passthrough).

```rust
pub fn resolve_tau_spec(&self, tau: TauSpec) -> f32
```

---

## Intelligence

### `Store::drift_report_json`

Semantic drift over a recent window, as JSON.

```rust
pub fn drift_report_json(&self, window: usize, threshold: f32) -> Option<String>
```

### `Store::contradictions_json`

Detected contradictions above a divergence threshold, as JSON.

```rust
pub fn contradictions_json(&self, min_divergence: f32) -> Option<String>
```

### `Store::contradictions_v2_json`

Three-population contradiction scan (geometric / lineage / consensus) at a
`TauSpec` threshold, as JSON.

```rust
pub fn contradictions_v2_json(&self, tau: TauSpec) -> Result<String, Error>
```

### `Store::evolution_json`

The evolution of a seed's region over time, as JSON.

```rust
pub fn evolution_json(&self, seed_frame_id: &str) -> Option<String>
```

### `Store::merge_candidates_json`

Frames that are consolidation candidates at similarity `tau`, as JSON.

```rust
pub fn merge_candidates_json(&self, tau: f32) -> Option<String>
```

---

## Consolidation

### `Store::supersede`

Mark a frame as superseded (soft forget — status flipped in place). Exempt
frames (Genesis/Seed/Redirect/System) error.

```rust
pub fn supersede(&mut self, frame_id: &str) -> Result<String, Error>
```

### `Store::consolidate_json`

Consolidate the given frames (JSON array of IDs) into a redirect. Returns JSON.

```rust
pub fn consolidate_json(&mut self, frame_ids_json: &str) -> Result<String, Error>
```

### `Store::consolidate_all_json`

Run consolidation across all eligible merge candidates. Returns JSON.

```rust
pub fn consolidate_all_json(&mut self) -> Result<String, Error>
```

### `Store::consolidate_with_strategy_json`

Consolidate specific frames under an explicit `MergeStrategy` (`Geometric` —
most-central source content; `Semantic` — LLM synthesis, not implemented in the
Rust runtime, always errors). Returns the same JSON report as
[`consolidate_json`](#storeconsolidate_json).

```rust
pub fn consolidate_with_strategy_json(&mut self, frame_ids: &[&str], strategy: MergeStrategy) -> Result<String, Error>
```

---

## Typed Structural Edges

### `Store::declare_relation`

Declare a typed relation, returning its numeric ID. `flags` sets relation
semantics (e.g. symmetric/transitive).

```rust
pub fn declare_relation(&mut self, name: &str, flags: u32) -> Result<u16, Error>
```

### `Store::list_relations_json`

The declared relations, as JSON.

```rust
pub fn list_relations_json(&self) -> Option<String>
```

### `Store::assert_relation`

Assert a typed edge `from_id --relation--> to_id` with a weight.

```rust
pub fn assert_relation(&mut self, from_id: &str, to_id: &str, relation: &str, weight: f32) -> Result<(), Error>
```

### `Store::retract_relation`

Retract a previously asserted typed edge.

```rust
pub fn retract_relation(&mut self, from_id: &str, to_id: &str, relation: &str) -> Result<(), Error>
```

### `Store::structural_neighbors_json`

Exact structural neighbors of a frame along the given relations, as JSON.
`direction` is `"outgoing"` | `"incoming"` | `"both"`.

```rust
pub fn structural_neighbors_json(
    &self,
    frame_id: &str,
    relations_json: &str,
    direction: &str,
) -> Result<String, Error>
```

### `Store::filter_by_relation_json`

Filter a hit list by structural relation presence. `hits_json` is a JSON array
of `{"frame_id","score"}`; when `require_present` is true, keeps hits that HAVE
a matching relation, otherwise those that LACK one.

```rust
pub fn filter_by_relation_json(
    &self,
    hits_json: &str,
    relations_json: &str,
    direction: &str,
    require_present: bool,
) -> Result<String, Error>
```

### `Store::structural_expand_json`

Expand a hit list by one hop along structural relations. `score_combine` is
`"multiply"` | `"min"` | `"max"` | `"keep_input"`.

```rust
pub fn structural_expand_json(
    &self,
    hits_json: &str,
    relations_json: &str,
    direction: &str,
    score_combine: &str,
) -> Result<String, Error>
```

---

## Provenance & Sealing

### `Store::verify`

Verify watermark-chain integrity. Returns `(all_ok, failing_frame_ids)`.

```rust
pub fn verify(&self) -> Result<(bool, Vec<String>), Error>
```

### `Store::seal`

🔒 **Licensed.** Seal the store, making it immutable. Returns the seal record.

```rust
pub fn seal(&mut self, reason: Option<&str>) -> Result<String, Error>
```

### `Store::is_sealed`

Whether the store is sealed.

```rust
pub fn is_sealed(&self) -> bool
```

### `Store::writer_audit_json`

The writer-attribution audit (which keys signed which frames), as JSON.

```rust
pub fn writer_audit_json(&self) -> Option<String>
```

### `Store::verify_attestation_json`

Verify the store's attestation chain. Returns JSON.

```rust
pub fn verify_attestation_json(&self) -> Result<String, Error>
```

### `Store::genesis_seed`

The genesis seed bytes.

```rust
pub fn genesis_seed(&self) -> Option<Vec<u8>>
```

### `Store::genesis_attestation_json`

The genesis attestation, as JSON.

```rust
pub fn genesis_attestation_json(&self) -> Option<String>
```

---

## Anchoring

### `Store::append_tip_anchor`

🔒 **Licensed.** Append a tip-anchor sentinel frame. `token` is the anchor proof
bytes (may be empty); `supersedes` optionally names an anchor this one replaces.
Returns the anchor frame's ID.

```rust
pub fn append_tip_anchor(
    &mut self,
    backend: &str,
    token: &[u8],
    tip_frame_id: &str,
    status: &str,
    supersedes: Option<&str>,
) -> Result<String, Error>
```

### `Store::list_tip_anchors_json`

List tip-anchor records as JSON. `resolved` filters to anchors whose proof has
resolved.

```rust
pub fn list_tip_anchors_json(&self, resolved: bool) -> Result<String, Error>
```

---

## Snapshot / Temporal

### `Store::snapshot`

A snapshot view of the store at a timestamp (microseconds since epoch).

```rust
pub fn snapshot(&self, at_timestamp_us: i64) -> Option<Snapshot>
```

### `Store::snapshot_search`

Search within a snapshot. Returns a JSON array of `{"frame_id","score"}`.

```rust
pub fn snapshot_search(
    &self,
    snapshot: &Snapshot,
    query_embedding: &[f32],
    top_k: usize,
) -> Result<String, Error>
```

### `Snapshot::frame_ids`

Frame IDs visible at this snapshot's timestamp.

```rust
pub fn frame_ids(&self) -> Vec<String>
```

### `Snapshot::timestamp`

The timestamp this snapshot was created at.

```rust
pub fn timestamp(&self) -> i64
```

---

## Events & Subscriptions

### `Store::subscribe`

Subscribe to substrate events (`"promote"` | `"drift"` | `"contradiction"` |
`"consolidation"` | `"milestone"` | `"*"`). The callback receives the event as a
JSON C string valid only for the call's duration. Returns a subscriber ID.

```rust
pub fn subscribe(
    &mut self,
    event_name: &str,
    callback: extern "C" fn(*const c_char, *mut c_void),
    user_data: *mut c_void,
) -> Result<u64, Error>
```

### `Store::unsubscribe`

Unsubscribe. Returns whether the subscriber was found.

```rust
pub fn unsubscribe(&mut self, subscriber_id: u64) -> bool
```

### `Store::subscriber_count`

The number of active event subscribers.

```rust
pub fn subscriber_count(&self) -> usize
```

---

## Federation

A `Federation` routes and fans a query across many single-file stores, merging
by raw cosine. Latency is bounded by per-store eigenframe count, not total size.

### `Store::federation_vector`

The store's routing vector (its geometric signature for federation routing).

```rust
pub fn federation_vector(&self) -> Option<Vec<f32>>
```

### `Federation::open_dir`

Open every `.sfg` store in a directory (named by file stem; `_*` / `router*`
files are skipped).

```rust
pub fn open_dir(dir: impl AsRef<Path>) -> Result<Self, Error>
```

### `Federation::new`

Create an empty federation.

```rust
pub fn new() -> Self
```

### `Federation::add_path`

Register a store file under `name`.

```rust
pub fn add_path(&mut self, name: &str, path: impl AsRef<Path>) -> Result<(), Error>
```

### `Federation::len`

The number of member stores.

```rust
pub fn len(&self) -> usize
```

### `Federation::is_empty`

Whether the federation has no members.

```rust
pub fn is_empty(&self) -> bool
```

### `Federation::member_names`

Member store names, in add order.

```rust
pub fn member_names(&self) -> Vec<String>
```

### `Federation::refresh`

Re-pull routing vectors for members that promoted since the last refresh.
Returns whether anything changed.

```rust
pub fn refresh(&mut self) -> bool
```

### `Federation::route`

Rank the member stores for a query. Returns `(store_name, score)` for up to
`top_stores` members.

```rust
pub fn route(&self, query: &[f32], top_stores: usize) -> Result<Vec<(String, f32)>, Error>
```

### `Federation::search`

Fan a query across the members selected by `mode` and merge the results.

```rust
pub fn search(&self, query: &[f32], top_k: usize, tau: f32, mode: RoutingMode) -> Result<Vec<FederatedHit>, Error>
```

---

## Encoding

### `Encoder::from_pretrained`

Load an encoder by model ID (downloads from HuggingFace if not cached).

```rust
pub fn from_pretrained(model_id: &str) -> Result<Self, Error>
```

### `Encoder::encode`

Encode a text string to an embedding vector.

```rust
pub fn encode(&mut self, text: &str) -> Result<Vec<f32>, Error>
```

### `Encoder::encode_into`

Encode text into a caller-owned buffer (no allocation). Returns floats written.

```rust
pub fn encode_into(&mut self, text: &str, buf: &mut [f32]) -> Result<usize, Error>
```

### `Encoder::encode_batch`

Encode many texts in a single batched forward pass.

```rust
pub fn encode_batch(&mut self, texts: &[&str]) -> Result<Vec<Vec<f32>>, Error>
```

### `Encoder::encode_batch_json`

Batch-encode texts (`texts_json` is a JSON array of strings). Returns a JSON
array of embedding arrays.

```rust
pub fn encode_batch_json(&mut self, texts_json: &str) -> Result<String, Error>
```

---

## Migration

### `reencode_store`

Re-encode a source store into a fresh v3 destination store: streams active
content frames, re-encodes them with `model_id`, and bulk-ingests into a new v3
store. `n_max == 0` re-encodes all; `pbatch == 0` → 2000; `ebatch == 0` → 16.
Returns the number of frames ingested. Free function.

```rust
pub fn reencode_store(
    src_path: impl AsRef<Path>,
    dst_path: impl AsRef<Path>,
    model_id: &str,
    eigen_threshold: u32,
    n_max: usize,
    pbatch: usize,
    ebatch: usize,
) -> Result<usize, Error>
```

---

## Types Reference

Auxiliary types returned or accepted by the methods above. Their fields and
builder methods are stable; see `src/lib.rs` for exact definitions.

| Type | Kind | Summary |
|---|---|---|
| `StoreConfig` | builder | Config for `create`. Builder methods: `new`, `model_id`, `eigen_threshold`, `eigen_min_age_s`, `tau_similarity`, `tau_merge`, `search_max_depth`, `search_visit_cap_multiplier`, `eigen_matmul_min`. |
| `Hit` | struct | A search result: `frame_id: String`, `score: f32`, `confidence: f32`, `path_length: u32`. |
| `Stats` | struct | Store counts: `total_frames`, `active_frames`, `eigenframes`, `superseded`, `redirects`. |
| `FrameView` | borrow | Zero-copy frame view (from `borrow_frame`): `id()`, `parent_id()`, `embedding()`, `content()`, `in_degree()`, `status()`, `timestamp()`. Valid until dropped; not across mutations. |
| `RegionHealth` | struct | `coherence: f32`, `direction: RegionDirection`, `candidate_split: bool`. |
| `RegionDirection` | enum | `Stable` \| `Consolidating` \| `Fragmenting`. |
| `SimilarityStats` | struct | `count`, `mean`, `p75`, `p90`, `p95`, `p99`. |
| `RoutingMode` | enum | Federation routing: `BestMatch` \| `Top(n)` \| `All`. |
| `FederatedHit` | struct | A federated result: the store name plus the underlying `Hit`. |
| `Snapshot` | handle | A temporal view (from `snapshot`); see `Snapshot::frame_ids` / `Snapshot::timestamp`. |
| `Error` | error | SERAPH error carrying the engine's message (`impl std::error::Error + Display`). |
