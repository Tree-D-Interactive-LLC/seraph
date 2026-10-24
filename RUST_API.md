# SERAPH Rust API Reference

> Crate: `seraph`
> Add via path/vendor: `seraph = { path = "vendor/seraph" }`

This reference documents the idiomatic Rust API for the people who **call**
SERAPH from Rust. It is one of three call surfaces over the same compiled
engine — the others are the C ABI and the Python bindings, documented in
`FFI_API.md` and `PYTHON_API.md` alongside this file in the release — and it
presents that engine as safe `Store` / `Encoder` / `Federation` types. Read
*Authentication & Licensing* and *Conventions* first — they apply to every
function and are not repeated per entry.

This reference covers the 0.4.0 API: one method per question, its options in a struct. The methods 0.4.0 replaced still compile through 0.x as `#[deprecated]` aliases whose warning names the replacement; they are documented in the 0.3.1 release's reference, not repeated here.

---

## Authentication & Licensing

SERAPH performs no network authentication. "Authorization" is **license-tier
gating**, enforced locally at the call site. Most methods work on every tier.
The capabilities below require a **Commercial or Enterprise** license; on the
Free tier they return `Err(Error)` with the message available via the error's
`Display`. These are tagged 🔒 **Licensed** inline.

| Capability | Methods |
|---|---|
| Encryption at rest | `Store::create_encrypted`, `Store::open_with` (`OpenOptions::passphrase`) |
| Sealing | `Store::seal` |
| Time anchoring | `Store::append_tip_anchor` |

**Writer identity (secure mode).** The license attests *who is licensed and at
what tier* — it does **not** carry a signing key. Per-frame writer attribution
uses a key the **calling application supplies**: pass a 32-byte Ed25519 private
seed to `Store::create_with_writer` / `Store::open_with` (`OpenOptions::writer_seed`). The seed stays
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
  must equal the store dimension (`store.dim()`). The `Encoder` and `Store::encode`
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

Create a new store on disk. The model and the vector precision are fixed for the store's life; `model_id` is a shortcut for a default configuration on that model.

```rust
pub fn create(path: impl AsRef<Path>, config: Option<StoreConfig>) -> Result<Self, Error>
```

### `Store::open_with`

Open an existing store. Options supply the writer identity for frames appended in the session, the passphrase of an encrypted store, read-only access, and the runtime tuning to start with, in any combination.

```rust
pub fn open_with(path: impl AsRef<Path>, options: &OpenOptions) -> Result<Self, Error>
```

### `Store::open`

Open an existing unencrypted store with every option at its default: the same as `open` with no options.

```rust
pub fn open(path: impl AsRef<Path>) -> Result<Self, Error>
```

### `abi_version`

The version of the C ABI conventions this library implements (the options structs, status codes and hit layouts).

```rust
pub fn abi_version() -> u32
```

### `Store::sync`

Flush pending writes to disk without closing (`flush` on the Python surface).

```rust
pub fn sync(&mut self) -> Result<(), Error>
```

---

## Writer Identity

### `Store::create_with_writer`

Create a store that signs every frame with an app-supplied writer identity and records the derived public key in the genesis attestation, so the chain verifies later without the key. The seed never leaves the caller.

```rust
pub fn create_with_writer( path: impl AsRef<Path>, config: Option<StoreConfig>, writer_seed: &[u8; 32], ) -> Result<Self, Error>
```

---

## Encryption

### `Store::create_encrypted`

Create a store encrypted at rest with AES-256-GCM. Content is encrypted under a key derived from the passphrase with Argon2id; watermarks and parent links stay in the clear so the chain verifies without the passphrase. Licensed feature.

```rust
pub fn create_encrypted( path: impl AsRef<Path>, config: Option<StoreConfig>, passphrase: &str, ) -> Result<Self, Error>
```

---

## Ingestion

### `Store::put_with`

Ingest content with a pre-computed embedding. Options set its metadata, when the content happened (`occurred_at`), a region to place it in (`within`), whether the seed term chooses its parent, and whether it becomes a door at once (`promote`), in any combination.

```rust
pub fn put_with(&mut self, content: &[u8], embedding: &[f32], options: &PutOptions) -> Result<String, Error>
```

### `Store::put`

Ingest content with every option at its default: the same as `put` with no options.

```rust
pub fn put(&mut self, content: &[u8], embedding: &[f32]) -> Result<String, Error>
```

### `Store::put_batch`

Bulk ingest. Each item carries the options `put` takes (metadata, event time, region, seed term, promotion) and is placed against the live store, so an item may attach to an earlier item of the same batch and the result is identical to calling `put` in a loop at a fraction of the cost. Insertion order shapes the tree, so order the batch meaningfully. Every item is checked before the first is committed; a batch that stops partway reports the frames it committed. The store stays buffered until it is synced or closed.

```rust
pub fn put_batch(&mut self, items: &[PutItem]) -> Result<Vec<String>, Error>
```

### `Store::begin_batch`

Begin a buffered write batch. Writes skip the per-frame WAL and fsync until commit, which flushes once.

```rust
pub fn begin_batch(&mut self) -> Result<(), Error>
```

### `Store::commit`

End a buffered write batch: flush every buffered write to disk and restore per-write durability.

```rust
pub fn commit(&mut self) -> Result<(), Error>
```

### `Store::commit_marker`

Commit an application marker: a System frame with no embedding that search never returns, durable in the store like any other frame. Use it for application records that belong with the store (a run manifest, a review note). `kind` becomes the content prefix `_{kind}:` and is echoed in the metadata as `_marker`.

```rust
pub fn commit_marker(&mut self, kind: &str, metadata_json: &str) -> Result<String, Error>
```

### `Store::markers_json`

Markers of one kind, in commit order.

```rust
pub fn markers_json(&self, kind: &str) -> Option<String>
```

---

## Search

### `Store::search_with`

Search for the frames that best answer a query: a query vector, or text encoded with the store's model. Options limit which frames may fill a result slot (superseded frames, a window on when the content happened, a window on when it was committed, a region), warp the query, order the selected hits, and return the visits made. A `committed` window's end reads the store as it stood then.

```rust
pub fn search_with(&self, query: &[f32], options: &SearchOptions) -> Result<SearchOutcome, Error>
```

### `Store::search`

Search with `top_k` and `tau` and every other option at its default: the same as `search` with no options.

```rust
pub fn search(&self, query_embedding: &[f32], top_k: usize, tau: f32) -> Result<Vec<Hit>, Error>
```

### `Store::tuning`

The runtime settings this handle runs with: how searches walk and stop, and how new frames are placed. Never persisted; every open starts from the defaults or the tuning it was opened with.

```rust
pub fn tuning(&self) -> Result<Tuning, Error>
```

### `Store::set_tuning`

Replace the runtime settings. Takes effect on the next read or commit; frames record the parent and edges they committed to, so no setting forces a migration. In Python, assign `store.tuning`.

```rust
pub fn set_tuning(&mut self, tuning: &Tuning) -> Result<(), Error>
```

### `Store::vector_precision`

The precision each frame's vector is stored at, fixed at create (`StoreConfig(vector_precision=...)`): `"codes4"` (4-bit codes, the default), `"fp32"`, `"fp16"`, `"bf16"` or `"fp64"`. Stores below format 5 report `"codes4"`.

```rust
pub fn vector_precision(&self) -> String
```

### `Store::resident_precision`

The precision search scores from: `"fp32"`, `"fp16"`, `"bf16"` or `"frame"` (no resident copy). The default is set at create (`StoreConfig(resident_precision=...)`) or derived from the vector precision when unset; `set_resident_precision` changes it at runtime.

```rust
pub fn resident_precision(&self) -> String
```

### `Store::set_resident_precision`

Change the precision search scores from and rebuild the resident copy at once. `"fp16"` and `"bf16"` halve its memory; `"frame"` keeps no resident copy, at some cost per search. Runtime only; the store's default is the one set at create.

```rust
pub fn set_resident_precision(&mut self, name: &str) -> Result<(), Error>
```

### `Store::set_search_arena`

Keep a contiguous resident copy of the vectors for search (default on). Off saves that memory at more cost per search; results are unchanged. Runtime only.

```rust
pub fn set_search_arena(&mut self, on: bool) -> Result<(), Error>
```

### `Store::set_walk_profile`

Diagnostic: enable walk profiling for this thread (no overhead when off).

```rust
pub fn set_walk_profile(&self, on: bool) -> Result<(), Error>
```

### `Store::take_walk_profile_json`

Diagnostic: read and reset the walk-profile accumulators.

```rust
pub fn take_walk_profile_json(&self) -> Result<String, Error>
```

### `Store::entry_count`

Diagnostic: the number of entry points a search for `query` would start from.

```rust
pub fn entry_count(&self, query_embedding: &[f32]) -> Result<usize, Error>
```

### `Store::last_search_visits`

Diagnostic: frames visited by the last search.

```rust
pub fn last_search_visits(&self) -> Result<usize, Error>
```

### `Store::last_commit_visits`

The number of frames examined to place the last frame committed. An operations cost counter, the ingest counterpart of last_search_visits.

```rust
pub fn last_commit_visits(&self) -> Result<usize, Error>
```

---

## Frame Access

### `Store::get_content`

A frame's content bytes by ID, without the rest of the frame.

```rust
pub fn get_content(&self, frame_id: &str) -> Result<Option<Vec<u8>>, Error>
```

### `Store::get_metadata`

A frame's metadata by ID, as a JSON object string.

```rust
pub fn get_metadata(&self, frame_id: &str) -> Option<String>
```

### `Store::get_frame_json`

Get a frame by ID, with its full data including embedding and metadata.

```rust
pub fn get_frame_json(&self, frame_id: &str) -> Option<String>
```

### `Store::lifecycle`

The record's account of one frame: when it was committed (and the event time it claims), when it became a door and how (genesis, seed, threshold or manual promotion), and when it was retired and how (superseded, or consolidated into a frame). Each event carries its position in record order and the commit time there.

```rust
pub fn lifecycle(&self, frame_id: &str) -> Result<Option<FrameLifecycle>, Error>
```

### `Store::get_embedding`

A frame's embedding by ID, as an owned vector.

```rust
pub fn get_embedding(&self, frame_id: &str) -> Result<Option<Vec<f32>>, Error>
```

### `Store::get_embedding_into`

Copy a frame's embedding into a caller-owned buffer, with no allocation.

```rust
pub fn get_embedding_into(&self, frame_id: &str, buf: &mut [f32]) -> Result<Option<usize>, Error>
```

### `Store::quantize_query`

The query vector exactly as the store scores it: the embedding after a round trip through the store's vector precision. Use it to reproduce scores outside the engine.

```rust
pub fn quantize_query(&self, embedding: &[f32]) -> Result<Vec<f32>, Error>
```

### `Store::encode_codes`

The packed 4-bit codes the store would persist for an embedding: the bytes a frame's codes carry and content_set_root hashes. Lets a caller build a leaf for content it has not ingested.

```rust
pub fn encode_codes(&self, embedding: &[f32]) -> Result<Vec<u8>, Error>
```

### `Store::borrow_frame`

Borrow a frame for zero-copy reads. The view points into engine memory, so release it before the store is written to.

```rust
pub fn borrow_frame(&self, frame_id: &str) -> Option<FrameView>
```

### `Store::model_id`

The embedding model ID declared at genesis.

```rust
pub fn model_id(&self) -> String
```

### `Store::dim`

Embedding dimensionality of the store.

```rust
pub fn dim(&self) -> usize
```

### `Store::frame_ids`

All frame IDs in insertion order.

```rust
pub fn frame_ids(&self) -> Vec<String>
```

### `Store::eigenframe_ids`

IDs of the eigenframes.

```rust
pub fn eigenframe_ids(&self) -> Vec<String>
```

### `Store::stats`

Store statistics.

```rust
pub fn stats(&self) -> Result<Stats, Error>
```

---

## Seeds & Promotion

### `Store::add_seed`

Add a seed frame to a live store (immediately an eigenframe).

```rust
pub fn add_seed(&mut self, label: &str, embedding: &[f32], metadata_json: Option<&str>) -> Result<String, Error>
```

### `Store::get_seeds`

The authored seed list: `label -> frame_id` bindings in authored order. A binding may name any frame, not only a Seed. When no list has been authored, the store's Seed frames are returned instead; `seeds_are_authored()` tells the two apart.

```rust
pub fn get_seeds(&self) -> Result<String, Error>
```

### `Store::seeds_are_authored`

Whether the seed list comes from an authored registry rather than being derived from Seed-status frames. An authored empty registry is a real state and is authoritative; it does not fall back to the derived list.

```rust
pub fn seeds_are_authored(&self) -> Result<bool, Error>
```

### `Store::resolve_seed`

Resolve a seed label to its frame ID. Labels are unique within a store's seed registry.

```rust
pub fn resolve_seed(&self, label: &str) -> Result<Option<String>, Error>
```

### `Store::set_seeds`

Replace the authored seed list. Removing a label removes it from the list only; the frame keeps its status.

```rust
pub fn set_seeds(&mut self, entries_json: &str) -> Result<String, Error>
```

### `Store::register_seed`

Bind a label to a frame in the seed registry, appending a new registry snapshot. Rebinding an existing label updates it in place and keeps its position.

```rust
pub fn register_seed(&mut self, label: &str, frame_id: &str) -> Result<String, Error>
```

### `Store::unregister_seed`

Drop a label from the seed registry, appending a new registry snapshot. The frame it named is untouched.

```rust
pub fn unregister_seed(&mut self, label: &str) -> Result<String, Error>
```

### `Store::promote`

Promote an active frame to eigenframe unconditionally. Promotion is monotonic.

```rust
pub fn promote(&mut self, frame_id: &str) -> Result<(), Error>
```

### `Store::eigenframes_json`

Full Frame objects for all eigenframes.

```rust
pub fn eigenframes_json(&self) -> String
```

---

## Traversal & Chains

### `Store::get_children`

Child frame IDs of a frame.

```rust
pub fn get_children(&self, frame_id: &str) -> Vec<String>
```

### `Store::lineage`

Parent chain back to genesis: [frame, parent, ..., GENESIS].

```rust
pub fn lineage(&self, frame_id: &str) -> Vec<String>
```

### `Store::path`

Shortest path between two frames over parent-child links.

```rust
pub fn path(&self, from_id: &str, to_id: &str) -> Vec<String>
```

### `Store::subtree`

All descendant frames under `frame_id`, optionally bounded by `max_depth`.

```rust
pub fn subtree(&self, frame_id: &str, max_depth: usize) -> Vec<String>
```

### `Store::subtrees_json`

Enumerate seed-rooted subtrees with at least `min_size` descendants.

```rust
pub fn subtrees_json(&self, min_size: usize) -> Result<String, Error>
```

### `Store::chain_with`

The shortest route between two endpoints over committed edges, each step weighted by how similar its two frames are. Each end is a frame ID or a query vector, which stands for the frames that fit it best. With `forward`, every step runs forward in record order: how the start led to the end. For a frame's own ancestry use `lineage`.

```rust
pub fn chain_with(&self, from: Endpoint, to: Endpoint, options: &RouteOptions) -> Result<ChainResult, Error>
```

### `Store::provenance_with`

What `frame_id` was built on: a route back in record order through the frames it attached to when it was committed (its parent and the similarity neighbours it chose), stopping when the next step is no more related than the frame's own background. A warp steers which strand is followed; `as_of` reads the record as it stood. `reached` is `True` when at least one step was taken.

```rust
pub fn provenance_with(&self, frame_id: &str, options: &StrandOptions) -> Result<ChainResult, Error>
```

### `Store::influence_with`

What was built on `frame_id`: a route forward in record order through the frames that attached to it when they were committed, stopping when the next step is no more related than the frame's own background. A warp steers which strand is followed; `as_of` reads the record as it stood. `reached` is `True` when at least one step was taken.

```rust
pub fn influence_with(&self, frame_id: &str, options: &StrandOptions) -> Result<ChainResult, Error>
```

### `Store::connections_with`

The distinct ways two frames connect: up to `max_routes` routes over committed edges that share no interior frames, shortest first, each with its bridge frame (the interior frame closest to both ends). With `forward`, every route runs forward in record order: the ways `from_id` led to `to_id`.

```rust
pub fn connections_with( &self, from: Endpoint, to: Endpoint, options: &RouteOptions, max_routes: usize, ) -> Result<Vec<Connection>, Error>
```

### `Store::chain_to_json`

The route from the door a query enters through to the frame nearest the query. On C and Rust use `chain` with an entry endpoint.

```rust
pub fn chain_to_json(&self, query_embedding: &[f32]) -> Result<String, Error>
```

### `Store::history_with`

How the store came to hold what the query finds. The top `anchors` search hits choose their lineages, and the best `top_k` frames along those lineages come back oldest first (by event time under the default basis), each with the route that reached it. The time windows limit which frames come back, as in `search`; a warp chooses which strand of the history is read.

```rust
pub fn history_with(&self, query: &[f32], options: &HistoryOptions) -> Result<HistoryResult, Error>
```

### `Store::gradient_json`

Semantic gradient along an ordered list of frame IDs: one GradientPoint per step.

```rust
pub fn gradient_json(&self, chain_json: &str) -> Result<String, Error>
```

### `Store::constitutive_score`

How much a chain says beyond a search at its midpoint: `1 - jaccard(chain_intermediates, midpoint_NN)` for an ordered frame-ID list. Values below `0.20` indicate the chain carries little beyond that search.

```rust
pub fn constitutive_score(&self, chain_json: &str) -> f32
```

---

## Neighborhood

### `Store::neighborhood_with`

Three-population neighborhood of a frame: its consensus, geometric and lineage neighbours. A warp moves the anchor for the geometric population only.

```rust
pub fn neighborhood_with(&self, frame_id: &str, options: &NeighborhoodOptions) -> Result<NeighborhoodResult, Error>
```

---

## Warp (Steering)

### `Store::warp_query`

Apply a warp specification to a query and return the warped vector, without searching.

```rust
pub fn warp_query( &self, query_embedding: &[f32], warp_spec_json: &str, ) -> Result<Vec<f32>, Error>
```

### `Store::compile_warp`

Compile a warp specification once against a store: frame-ID targets and named magnitudes are resolved, and the compiled warp is passed to any read that takes one. Python passes a `WarpSpec` directly.

```rust
pub fn compile_warp(&self, spec_json: &str) -> Result<Warp, Error>
```

### `Store::compile_warp_spec`

Compile a typed `WarpSpec` against the store: the same as `compile_warp` without writing the JSON by hand.

```rust
pub fn compile_warp_spec(&self, spec: &WarpSpec) -> Result<Warp, Error>
```

### `Store::calibrate_warp`

Run an empirical magnitude calibration sweep against the store.

```rust
pub fn calibrate_warp( &self, n_queries: usize, n_targets: usize, magnitudes: Option<&[f32]>, top_k: usize, seed: u64, ) -> Result<String, Error>
```

### `Store::fixup_warp`

Refresh the store's warp calibration from its own corpus when the persisted one has drifted or none exists, and persist the result. Call from the single writer after open.

```rust
pub fn fixup_warp(&mut self) -> Result<bool, Error>
```

### `Store::warp_calibration`

The store's resident warp calibration as a mapping from named magnitude to value, or `None` if the store has not been calibrated. Named-magnitude warps resolve against this; there are no built-in presets.

```rust
pub fn warp_calibration(&self) -> Result<String, Error>
```

### `Store::steerability`

Warp steerability: angular displacement of a query toward a target at routing magnitude.

```rust
pub fn steerability(&self, query_embedding: &[f32], target_id: &str) -> f32
```

---

## Analysis & Metrics

### `Store::density_score`

Local density score for a frame.

```rust
pub fn density_score(&self, frame_id: &str) -> f32
```

### `Store::seed_margin`

Seed margin: a frame's geometric distinctness relative to the corpus centroid.

```rust
pub fn seed_margin(&self, frame_id: &str) -> f32
```

### `Store::region_health`

Region health for an eigenframe as a fixed struct instead of JSON.

```rust
pub fn region_health(&self, eigenframe_id: &str) -> Result<RegionHealth, Error>
```

### `Store::region_health_json`

Region health metrics for an eigenframe.

```rust
pub fn region_health_json(&self, eigenframe_id: &str) -> Option<String>
```

### `Store::similarity_stats`

The parent-child similarity distribution as a fixed struct instead of JSON.

```rust
pub fn similarity_stats(&self) -> Result<SimilarityStats, Error>
```

### `Store::similarity_stats_json`

Similarity distribution computed from parent-child relationships.

```rust
pub fn similarity_stats_json(&self) -> Option<String>
```

### `Store::sim_neighbors_json`

Similarity neighbours of a frame: the live edge set. Includes edges other frames aimed at this one, so it is the traversable neighbourhood rather than the frame's own choice; for that, see `committed_sim_edges`.

```rust
pub fn sim_neighbors_json(&self, frame_id: &str) -> Option<String>
```

### `Store::committed_sim_edges_json`

The similarity neighbours this frame chose when it was committed, resolved to ids. Always a subset of `sim_neighbors`. `None` for a frame written before the choice was recorded.

```rust
pub fn committed_sim_edges_json(&self, frame_id: &str) -> Option<String>
```

### `Store::resolve_tau`

Resolve a tau value against the store's background similarity: `None` gives the p95, `"loose"` the p75, `"strict"` the p99, and a float passes through.

```rust
pub fn resolve_tau(&self) -> f32
```

### `Store::resolve_tau_spec`

Resolve tau from a spec against the store's background similarity: auto (p95), loose (p75), strict (p99) or an explicit value passed through.

```rust
pub fn resolve_tau_spec(&self, tau: TauSpec) -> f32
```

---

## Intelligence

### `Store::drift_report_json`

Scan eigenframes for semantic drift: children drifting away from their eigenframe.

```rust
pub fn drift_report_json(&self, window: usize, threshold: f32) -> Option<String>
```

### `Store::contradictions_json`

Detect high-divergence siblings under a shared parent.

```rust
pub fn contradictions_json(&self, min_divergence: f32) -> Option<String>
```

### `Store::contradictions_v2_json`

Three-population contradiction detection: geometric, lineage and consensus pairs.

```rust
pub fn contradictions_v2_json(&self, tau: TauSpec) -> Result<String, Error>
```

### `Store::evolution_json`

Concept evolution trajectory from a seed frame, in timestamp order.

```rust
pub fn evolution_json(&self, seed_frame_id: &str) -> Option<String>
```

### `Store::merge_candidates_json`

Find clusters of similar frames that could be consolidated.

```rust
pub fn merge_candidates_json(&self, tau: f32) -> Option<String>
```

---

## Consolidation

### `Store::supersede`

Soft-forget a single frame: flips its status to Superseded in place. Creates no new frame and no redirect, and does not reparent children.

```rust
pub fn supersede(&mut self, frame_id: &str) -> Result<String, Error>
```

### `Store::consolidate_json`

Consolidate a specific set of frames into one frame at their centroid, carrying the most central frame's content, metadata and event time. Frames that carry different markings are not consolidated.

```rust
pub fn consolidate_json(&mut self, frame_ids_json: &str) -> Result<String, Error>
```

### `Store::consolidate_all_json`

Auto-detect and consolidate all merge candidates in one pass.

```rust
pub fn consolidate_all_json(&mut self) -> Result<String, Error>
```

---

## Typed Structural Edges

### `Store::declare_relation`

Declare a named structural relation type. Returns the slot ID.

```rust
pub fn declare_relation(&mut self, name: &str, flags: u32) -> Result<u16, Error>
```

### `Store::list_relations_json`

List all declared relation types.

```rust
pub fn list_relations_json(&self) -> Option<String>
```

### `Store::assert_relation`

Assert a typed edge between two frames.

```rust
pub fn assert_relation(&mut self, from_id: &str, to_id: &str, relation: &str, weight: f32) -> Result<(), Error>
```

### `Store::retract_relation`

Retract a typed edge.

```rust
pub fn retract_relation(&mut self, from_id: &str, to_id: &str, relation: &str) -> Result<(), Error>
```

### `Store::structural_neighbors_json`

Find neighbours via typed edges.

```rust
pub fn structural_neighbors_json( &self, frame_id: &str, relations_json: &str, direction: &str, ) -> Result<String, Error>
```

### `Store::filter_by_relation_json`

Filter a (frame_id, score) hit list to those participating in any of the given relations.

```rust
pub fn filter_by_relation_json( &self, hits_json: &str, relations_json: &str, direction: &str, require_present: bool, ) -> Result<String, Error>
```

### `Store::structural_expand_json`

Expand an existing (frame_id, score) hit list by one hop along structural relations.

```rust
pub fn structural_expand_json( &self, hits_json: &str, relations_json: &str, direction: &str, score_combine: &str, ) -> Result<String, Error>
```

---

## Provenance & Sealing

### `Store::verify`

Verify watermark-chain integrity.

```rust
pub fn verify(&self) -> Result<(bool, Vec<String>), Error>
```

### `Store::seal_with_anchor`

Seal with an external time anchor over the order digest of every frame before the seal (take it with `order_digest` after the last write). The engine records `backend` and the token; it does not contact the backend. Requires Commercial or higher. On the Python surface this is `seal(reason, anchor_backend, anchor_token)`.

```rust
pub fn seal_with_anchor(&mut self, reason: Option<&str>, backend: &str, token: &[u8]) -> Result<String, Error>
```

### `Store::order_digest_json`

The digest to anchor: a running SHA-256 over the watermarks in commit order (`d0 = SHA256("SERAPH:ORDER:v1")`, `d_i = SHA256(d_{i-1} || watermark_i)`), through `through_frame_id` or the whole store. From format 4 a watermark binds a frame's content, lineage, id, commit time and committed edges, so the 32 bytes fix all of them and their order. Anchor them, then record the token with `append_tip_anchor` or `seal`.

```rust
pub fn order_digest_json(&self, through_frame_id: Option<&str>) -> Result<String, Error>
```

### `Store::anchor_claims_json`

Every seal and tip anchor that commits to an order digest. `verify` checks each claim's digest against the log; the token is checked against the claim's `digest` by your anchor backend, directly or through `verify_attestation(anchor_verifier=...)`.

```rust
pub fn anchor_claims_json(&self) -> Result<String, Error>
```

### `Store::seal`

Seal the store: commit a final sentinel and refuse further writes. The seal records the order digest of every frame before it (see `order_digest`), so `verify` detects any frame reordered, removed or added before the seal, and any frame after it. To anchor the seal externally: stop writing, take `order_digest()`, anchor its 32 bytes with your backend, then seal with `anchor_backend` and `anchor_token`; the engine records the token and never contacts the backend. Requires Commercial or higher.

```rust
pub fn seal(&mut self, reason: Option<&str>) -> Result<String, Error>
```

### `Store::is_sealed`

Whether the store has been sealed (read-only). All tiers can open and read sealed stores.

```rust
pub fn is_sealed(&self) -> bool
```

### `Store::writer_audit_json`

Audit writer attribution across all frames.

```rust
pub fn writer_audit_json(&self) -> Option<String>
```

### `Store::verify_attestation_json`

Full attestation audit: chain validity, genesis seed, operator identity, anchors, cross-customer writes.

```rust
pub fn verify_attestation_json(&self) -> Result<String, Error>
```

### `Store::genesis_seed`

Raw genesis seed bytes (v2-attested stores only).

```rust
pub fn genesis_seed(&self) -> Option<Vec<u8>>
```

### `Store::genesis_attestation_json`

Genesis attestation header as a JSON string.

```rust
pub fn genesis_attestation_json(&self) -> Option<String>
```

### `Store::content_set_root`

An order-free fingerprint of the store's retrievable content: sha256 over the sorted per-frame leaves `sha256(content_hash || codes)` under the header `"seraph-content-set/v1|<model_id>|<dim>|<n>"`. Two stores holding the same content under the same model give the same root regardless of commit order or graph. It ranges over every frame search can return; markers, redirects and superseded frames are excluded, and it is recomputable from the frame log alone.

```rust
pub fn content_set_root(&self) -> Result<String, Error>
```

---

## Anchoring

### `Store::append_tip_anchor`

Append a trusted-time anchor record to the chain tip. Requires a Commercial or Enterprise license.

```rust
pub fn append_tip_anchor( &mut self, backend: &str, token: &[u8], tip_frame_id: &str, status: &str, supersedes: Option<&str>, ) -> Result<String, Error>
```

### `Store::list_tip_anchors_json`

List tip anchor records as JSON strings. Available to all tiers.

```rust
pub fn list_tip_anchors_json(&self, resolved: bool) -> Result<String, Error>
```

---

## Snapshot / Temporal

### `Store::snapshot`

Frame IDs active at the given timestamp (microseconds).

```rust
pub fn snapshot(&self, at_timestamp_us: i64) -> Option<Snapshot>
```

### `Store::snapshot_search`

Brute-force cosine search within a snapshot.

```rust
pub fn snapshot_search( &self, snapshot: &Snapshot, query_embedding: &[f32], top_k: usize, ) -> Result<String, Error>
```

### `Snapshot::frame_ids`

The frame IDs visible in a snapshot.

```rust
pub fn frame_ids(&self) -> Vec<String>
```

### `Snapshot::timestamp`

The point in time a snapshot was taken at, in microseconds since the epoch.

```rust
pub fn timestamp(&self) -> i64
```

---

## Events & Subscriptions

### `Store::subscribe`

Subscribe a callback to a store event (promote, drift, contradiction, consolidate, milestone); `on` is an alias of `subscribe`.

```rust
pub fn subscribe( &mut self, event_name: &str, callback: extern "C" fn(*const c_char, *mut c_void), user_data: *mut c_void, ) -> Result<u64, Error>
```

### `Store::unsubscribe`

Detach an event subscriber by ID.

```rust
pub fn unsubscribe(&mut self, subscriber_id: u64) -> bool
```

### `Store::subscriber_count`

Number of active subscribers.

```rust
pub fn subscriber_count(&self) -> usize
```

---

## Federation

A `Federation` routes and fans a query across many single-file stores, merging
by raw cosine. Latency is bounded by per-store eigenframe count, not total size.

### `Store::federation_vector`

Mean of all eigenframe embeddings: a single vector representing the store's semantic centre, used for federation routing.

```rust
pub fn federation_vector(&self) -> Option<Vec<f32>>
```

### `Federation::open_dir`

Open every `.sfg` store in a directory as a federation, each member named by its file stem (`physics.sfg` becomes `"physics"`); files starting with `_` or `router` are skipped.

```rust
pub fn open_dir(dir: impl AsRef<Path>) -> Result<Self, Error>
```

### `Federation::new`

Create an empty federation. Add member stores with `add_path`, or open a directory of stores with `open_dir`.

```rust
pub fn new() -> Self
```

### `Federation::add_path`

Register a store file under `name` as a member of the federation. The store must share the federation's embedding model and dimension.

```rust
pub fn add_path(&mut self, name: &str, path: impl AsRef<Path>) -> Result<(), Error>
```

### `Federation::len`

Number of member stores (`len(fed)` in Python).

```rust
pub fn len(&self) -> usize
```

### `Federation::is_empty`

Whether the federation has no member stores.

```rust
pub fn is_empty(&self) -> bool
```

### `Federation::member_names`

Member store names, in add order.

```rust
pub fn member_names(&self) -> Vec<String>
```

### `Federation::refresh`

Re-pull routing vectors for any member that promoted an eigenframe since the last refresh. Cheap: only changed members are read.

```rust
pub fn refresh(&mut self) -> bool
```

### `Federation::route`

Route a query to the members most likely to hold its answer, without searching them.

```rust
pub fn route(&self, query: &[f32], top_stores: usize) -> Result<Vec<(String, f32)>, Error>
```

### `Federation::search`

Search the federation: route the query to the best-fitting members, search each, and merge the hits into one ranked list.

```rust
pub fn search(&self, query: &[f32], top_k: usize, tau: f32, mode: RoutingMode) -> Result<Vec<FederatedHit>, Error>
```

---

## Encoding

### `Encoder::from_pretrained`

Load an encoder by model ID, downloading and caching the model files on first use.

```rust
pub fn from_pretrained(model_id: &str) -> Result<Self, Error>
```

### `Encoder::encode`

Encode one text into a normalized embedding with the loaded model. The FFI and wrapper also offer an `encode_into` variant that writes into a caller-supplied buffer.

```rust
pub fn encode(&mut self, text: &str) -> Result<Vec<f32>, Error>
```

### `Encoder::encode_into`

Encode one text into a caller-owned buffer, with no allocation.

```rust
pub fn encode_into(&mut self, text: &str, buf: &mut [f32]) -> Result<usize, Error>
```

### `Encoder::encode_batch`

Encode several texts in one batched forward pass. The FFI also offers `encode_batch_flat`, which writes all embeddings into one contiguous buffer and is the recommended batch path from C.

```rust
pub fn encode_batch(&mut self, texts: &[&str]) -> Result<Vec<Vec<f32>>, Error>
```

### `Encoder::encode_batch_json`

Encode many texts given as a JSON array of strings and return a JSON array of embeddings.

```rust
pub fn encode_batch_json(&mut self, texts_json: &str) -> Result<String, Error>
```

---

## Migration

### `migrate_store_with`

Rebuild a store into a fresh one at `dst_path`: every frame is re-committed with its identity intact (id, content, embedding, metadata, status references) and its parent chosen against the live destination, so references by id stay valid. Use it to correct an old topology or to carry a store to the current format or another `vector_precision` (`config`). Each source frame's commit time becomes its event time, so the rebuilt store's reads answer about its history; seeds in `seed_data` are created before the first frame. Watermarks are recomputed; the destination verifies.

```rust
pub fn migrate_store_with( src_path: impl AsRef<Path>, dst_path: impl AsRef<Path>, options: &MigrateOptions, ) -> Result<MigrateReport, Error>
```

### `reencode_store`

Build a fresh store from the content of an existing one, re-encoding every frame with a new model. Frames are ingested in source commit order, so the destination's topology is the one a per-frame ingest would produce.

```rust
pub fn reencode_store( src_path: impl AsRef<Path>, dst_path: impl AsRef<Path>, model_id: &str, eigen_threshold: u32, n_max: usize, pbatch: usize, ebatch: usize, ) -> Result<usize, Error>
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
