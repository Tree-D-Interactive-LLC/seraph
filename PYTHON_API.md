# SERAPH Python API Reference

> Module: `seraph` (PyO3 native extension)
> Import: `import seraph`

---

## Table of Contents

1. [Store Lifecycle](#store-lifecycle)
2. [Encryption](#encryption)
3. [Configuration](#configuration)
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
15. [Snapshot / Temporal](#snapshot--temporal)
16. [Provenance & Sealing](#provenance--sealing)
17. [Anchoring](#anchoring)
18. [Events & Subscriptions](#events--subscriptions)
19. [Encoding](#encoding)
20. [Batch Operations](#batch-operations)
21. [Types Reference](#types-reference)
22. [Enums Reference](#enums-reference)

---

## Store Lifecycle

### `SeraphStore.create(path, config, seed_data=[])`

Create a new store on disk.

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `path` | `str` | -- | Filesystem path for the `.sfg` file |
| `config` | `StoreConfig` | -- | Store configuration |
| `seed_data` | `list[tuple[str, list[float]]]` | `[]` | Initial seeds as `(label, embedding)` tuples |

**Returns:** `SeraphStore`

```python
config = seraph.StoreConfig(model_id="BAAI/bge-small-en-v1.5")
store = seraph.SeraphStore.create("./knowledge.sfg", config)
```

### `SeraphStore.open(path)`

Open an existing (unencrypted) store.

| Param | Type | Description |
|-------|------|-------------|
| `path` | `str` | Path to existing `.sfg` file |

**Returns:** `SeraphStore`

**Raises:** `RuntimeError` if the store is encrypted (use `open_encrypted` instead).

### `SeraphStore.open_encrypted(path, passphrase)`

Open an encrypted store with a passphrase.

| Param | Type | Description |
|-------|------|-------------|
| `path` | `str` | Path to existing encrypted `.sfg` file |
| `passphrase` | `str` | Decryption passphrase |

**Returns:** `SeraphStore`

```python
store = seraph.SeraphStore.open_encrypted("./secrets.sfg", "my passphrase")
```

### `store.close()`

Close the store and flush all pending writes. The store object is unusable after this call.

### `store.sync()`

Flush pending writes to disk without closing.

---

## Encryption

> **Requires Commercial or Enterprise license.** Free-tier calls will raise `RuntimeError`.

SERAPH supports AES-256-GCM encryption at rest. When enabled, all content fields (content bytes, embeddings, quantized vectors, metadata) are encrypted per-frame. Structural metadata (watermarks, parent links, timestamps) remains plaintext for keyless chain verification.

### Creating an encrypted store

Pass `encryption_passphrase` in the config:

```python
config = seraph.StoreConfig(encryption_passphrase="strong passphrase here")
store = seraph.SeraphStore.create("./encrypted.sfg", config)

# All subsequent put() calls encrypt transparently
store.put(b"sensitive content", embedding)
```

### Opening an encrypted store

```python
store = seraph.SeraphStore.open_encrypted("./encrypted.sfg", "strong passphrase here")
# All reads decrypt transparently
```

### Notes

- Encryption is set at creation time and cannot be added later.
- The `.gidx` sidecar is not written for encrypted stores (prevents topology leaks).
- Search, chain verification, and all other operations work identically on encrypted stores.
- The passphrase derives a KEK via Argon2id (64 MiB, 3 iterations, 4 lanes). A random DEK encrypts content.

---

## Configuration

### `StoreConfig(**kwargs)`

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `model_id` | `str` | `"BAAI/bge-small-en-v1.5"` | Embedding model identifier |
| `eigen_threshold` | `int` | `5` | In-degree threshold for eigenframe promotion |
| `eigen_min_age_s` | `float` | `60.0` | Minimum age (seconds) before promotion |
| `tau_similarity` | `float` | `0.5` | Default similarity threshold |
| `tau_merge` | `float` | `0.92` | Consolidation merge threshold |
| `max_index_elements` | `int` | `100000` | Max elements in graph index |
| `custom_range` | `QuantizationRange` | `None` | Custom quantization range for unknown models |
| `search_max_depth` | `int` | `50` | Maximum BFS depth during search |
| `search_visit_cap_multiplier` | `int` | `5` | Visit cap multiplier for search |
| `redirect_per_frame` | `bool` | `True` | Emit one redirect per superseded source frame |
| `secure` | `str` | `None` | Security posture: `"off"`, `"standard"`, `"on"`, `"gov"`, or `None` (auto-detect) |
| `eigen_matmul_min` | `int` | `256` | Minimum eigenframes for matrix GEMV scan |
| `encryption_passphrase` | `str` | `None` | Passphrase for AES-256-GCM encryption at rest |

### `QuantizationRange(model_id, low, high, dim)`

Custom quantization range for models not in the built-in table.

| Param | Type | Description |
|-------|------|-------------|
| `model_id` | `str` | Model identifier |
| `low` | `float` | Lower bound of embedding value range |
| `high` | `float` | Upper bound of embedding value range |
| `dim` | `int` | Embedding dimensionality |

---

## Ingestion

### `store.put(content, embedding) -> str`

Ingest content with a pre-computed embedding.

| Param | Type | Description |
|-------|------|-------------|
| `content` | `bytes` | Raw content bytes |
| `embedding` | `list[float]` | Embedding vector (must match store dimension) |

**Returns:** Frame ID (string).

### `store.put_with_metadata(content, embedding, metadata_json="{}") -> str`

Ingest content with embedding and metadata.

| Param | Type | Description |
|-------|------|-------------|
| `content` | `bytes` | Raw content bytes |
| `embedding` | `list[float]` | Embedding vector |
| `metadata_json` | `str` | JSON string of metadata key-value pairs |

### `store.put_batch(items) -> list[str]`

Batch ingest multiple frames.

| Param | Type | Description |
|-------|------|-------------|
| `items` | `list[tuple[bytes, list[float]]]` | List of `(content, embedding)` tuples |

**Returns:** List of frame IDs.

---

## Search

### `store.search(query_embedding, top_k=10, tau=0.5) -> list[Hit]`

Semantic search by embedding vector.

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `query_embedding` | `list[float]` | -- | Query vector |
| `top_k` | `int` | `10` | Maximum results |
| `tau` | `float` | `0.5` | Similarity threshold |

**Returns:** List of `Hit` objects with `.frame_id`, `.score`, `.confidence`, `.path_length`.

### `store.search_with_opts(query_embedding, opts) -> list[Hit]`

Search with full options control.

| Param | Type | Description |
|-------|------|-------------|
| `query_embedding` | `list[float]` | Query vector |
| `opts` | `SearchOpts` | Search options |

### `store.search_text(text, top_k=10, tau=0.5) -> list[Hit]`

Search by text (uses the store's built-in encoder).

| Param | Type | Description |
|-------|------|-------------|
| `text` | `str` | Query text |
| `top_k` | `int` | Maximum results |
| `tau` | `float` | Similarity threshold |

### `store.search_with_warp(query_embedding, targets, suppress, profile, bound_k, top_k, tau) -> list[Hit]`

Search with warp field steering.

### `store.search_with_trace(query_embedding, top_k, tau, include_superseded) -> list[TraceEntry]`

Search with full traversal trace for debugging.

### `SearchOpts(top_k, tau_similarity, max_depth, include_superseded, tier, tier_auto_threshold)`

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `top_k` | `int` | -- | Maximum results |
| `tau_similarity` | `float` | -- | Similarity threshold |
| `max_depth` | `int` | -- | Maximum traversal depth |
| `include_superseded` | `bool` | -- | Include superseded frames |
| `tier` | `Tier` | -- | Pre-filter tier |
| `tier_auto_threshold` | `int` | -- | Frame count to activate tiered pre-filter |

---

## Frame Access

### `store.get_frame(frame_id) -> Frame | None`

Get a frame by ID. Returns `None` if not found.

**Frame fields:** `.id`, `.content`, `.embedding`, `.parent_id`, `.watermark`, `.status`, `.in_degree`, `.timestamp`, `.metadata`, `.customer_uuid`, `.writer_signature`

### `store.frame_ids() -> list[str]`

All frame IDs in the store.

### `store.eigenframe_ids() -> list[str]`

IDs of eigenframes only.

### `store.eigenframes() -> list[Frame]`

Full frame objects for all eigenframes.

### `store.children(frame_id) -> list[Frame]`

Child frames of a given frame.

### `store.get_children(frame_id) -> list[str]`

Child frame IDs only (lighter than `children()`).

### `store.model_id() -> str`

The embedding model ID declared at genesis.

### `store.dim() -> int`

Embedding dimensionality.

### `store.stats() -> StoreStats`

Store statistics: `.total_frames`, `.active_frames`, `.eigenframe_count`, `.seed_count`, `.superseded_count`, `.redirect_count`, `.model_id`, `.dim`.

---

## Seeds & Promotion

### `store.add_seed(label, embedding, metadata_json="{}") -> str`

Add a seed frame (immediately promoted to eigenframe status).

### `store.promote(frame_id)`

Manually promote a frame to eigenframe status.

### `store.promote_content(content, embedding, metadata_json="{}") -> str`

Ingest content and immediately promote to eigenframe.

---

## Traversal & Chains

### `store.path(from_id, to_id) -> list[Frame]`

BFS path between two frames.

### `store.chain(from_id, to_id) -> Chain`

Semantic chain between two frame IDs.

### `store.chain_to(query_embedding) -> Chain`

Chain from the nearest frame to the query point.

### `store.chain_between(emb_a, emb_b) -> Chain`

Chain between two points in embedding space.

### `store.chain_to_with_warp(query_embedding, warp_spec, ...) -> Chain`

Chain with warp field applied.

### `store.chain_between_with_warp(emb_a, emb_b, warp_spec, ...) -> Chain`

Chain between two points with warp field.

### `store.lineage(frame_id) -> list[Frame]`

Ancestry chain from a frame back to genesis.

**Chain fields:** `.frames`, `.primitive`, `.coherence` (a `ChainCoherence` with `.mean`, `.min`, `.max`, `.std`, `.segments`).

---

## Neighborhood

### `store.neighborhood(frame_id, tau=0.5, hops=2) -> NeighborhoodResult`

Explore the local neighborhood of a frame.

### `store.neighborhood_with_warp(frame_id, tau, hops, warp_spec) -> NeighborhoodResult`

Neighborhood exploration with warp field.

**NeighborhoodResult fields:** `.frames`, `.edges`, `.region_eigenframe_id`.

---

## Warp (Steering)

### `store.apply_warp(query_embedding, targets, suppress, profile, bound_k) -> list[float]`

Apply a warp field to a query embedding without searching.

### `store.warp_query(query_embedding, warp_spec) -> list[float]`

Apply a `WarpSpec` to a query embedding.

### `store.calibrate_warp(n_queries, n_targets, magnitudes, top_k, seed) -> WarpCalibration`

Calibrate warp magnitude parameters.

### `WarpSpec(targets, suppress, profile, bound_k, accumulation)`

| Param | Type | Description |
|-------|------|-------------|
| `targets` | `list[tuple]` | Target (input, magnitude) pairs |
| `suppress` | `list[tuple]` | Suppress (input, magnitude) pairs |
| `profile` | `str` | Warp profile: `"gaussian"`, `"linear"`, `"step"` |
| `bound_k` | `float` | Bound parameter |
| `accumulation` | `str` | `"reset"` or `"cumulative"` |

---

## Analysis & Metrics

### `store.density_score(frame_id) -> float`

Local density score for a frame.

### `store.seed_margin(frame_id) -> float`

Distance margin to nearest seed.

### `store.steerability(query_embedding, target_id) -> float`

Steerability score from query toward target.

### `store.region_health(eigenframe_id) -> RegionHealth`

Health metrics for an eigenframe's region.

### `store.similarity_stats() -> SimilarityStats`

Global similarity distribution statistics.

### `store.resolve_tau(tau=None) -> float`

Resolve a tau value (auto-select if `None`).

---

## Intelligence

### `store.drift_report(window, threshold) -> list[DriftAlert]`

Detect semantic drift over a sliding window.

### `store.contradictions(min_divergence) -> list[ContradictionCandidate]`

Find contradictory frame pairs.

### `store.contradictions_v2(tau=None) -> ContradictionReport`

Enhanced contradiction detection with full report.

### `store.evolution(seed_frame_id) -> list[Frame]`

Evolution trajectory from a seed frame.

### `store.evolution_steps(seed_frame_id) -> list[EvolutionStep]`

Detailed evolution steps with metrics.

---

## Consolidation

### `store.merge_candidates(tau) -> list[MergeCluster]`

Find clusters of similar frames that could be consolidated.

### `store.consolidate(frame_ids) -> ConsolidateReport`

Consolidate a specific set of frames.

### `store.consolidate_all() -> BulkConsolidateReport`

Auto-detect and consolidate all merge candidates.

### `store.consolidate_with_strategy(frame_ids, strategy, ...) -> ConsolidateReport`

Consolidate with explicit merge strategy.

---

## Typed Structural Edges

### `store.declare_relation(name, flags) -> int`

Declare a named relation type. Returns the slot ID.

| Param | Type | Description |
|-------|------|-------------|
| `name` | `str` | Relation name (e.g., `"references"`, `"contradicts"`) |
| `flags` | `int` | Relation flags (bitfield) |

### `store.list_relations() -> list[RelationDef]`

List all declared relation types.

### `store.assert_relation(from_id, to_id, relation, weight) -> None`

Assert a typed edge between two frames.

### `store.retract_relation(from_id, to_id, relation) -> None`

Retract a typed edge.

### `store.structural_neighbors(frame_id, relations, direction) -> list[StructuralHit]`

Find neighbors via typed edges.

| Param | Type | Description |
|-------|------|-------------|
| `frame_id` | `str` | Source frame |
| `relations` | `str` (JSON) | Relation filter |
| `direction` | `str` | `"outgoing"`, `"incoming"`, or `"both"` |

### `store.structural_expand(frame_id, relations, direction, hops) -> list[StructuralHit]`

Multi-hop expansion along typed edges.

---

## Snapshot / Temporal

### `store.snapshot(at_timestamp_us) -> SnapshotView`

Create a point-in-time view of the store.

### `store.snapshot_search(snapshot, query_embedding, top_k) -> list[Hit]`

Search within a snapshot.

---

## Provenance & Sealing

### `store.verify() -> tuple[bool, list[str]]`

Verify watermark chain integrity. Returns `(ok, failures)`.

### `store.is_sealed() -> bool`

Check if the store is sealed (read-only). All tiers can open and read sealed stores.

### `store.seal(reason=None) -> str`

> **Requires Commercial or Enterprise license.** Free-tier calls will raise `RuntimeError`.

Seal the store. No further writes are accepted. Returns the seal frame ID.

### `store.writer_audit() -> WriterAudit`

Audit writer attribution across all frames.

### `store.verify_attestation() -> AttestationAudit`

Full attestation audit: chain validity, genesis seed, operator identity, cross-customer writes.

### `store.genesis_seed() -> bytes | None`

Raw genesis seed bytes.

### `store.genesis_attestation() -> str | None`

Genesis attestation as JSON string.

---

## Anchoring

### `store.append_tip_anchor(backend, proof, tip_frame_id, status, metadata=None) -> str`

> **Requires Commercial or Enterprise license.** Free-tier calls will raise `RuntimeError`.

Append a timestamping anchor record.

| Param | Type | Description |
|-------|------|-------------|
| `backend` | `str` | Anchor backend name (e.g., `"opentimestamps"`, `"rfc3161"`) |
| `proof` | `bytes` | Proof/token bytes |
| `tip_frame_id` | `str` | Frame ID this anchor covers |
| `status` | `str` | `"pending"` or `"confirmed"` |
| `metadata` | `str` | Optional JSON metadata |

### `store.list_tip_anchors(resolved) -> list[str]`

List tip anchor records as JSON strings. Available to all tiers.

---

## Events & Subscriptions

### `store.on(event_name, callback) -> int`

Subscribe to store events. Returns a subscriber ID.

| Event | Payload |
|-------|---------|
| `"promote"` | `PromoteEvent` |
| `"contradiction"` | `ContradictionEvent` |
| `"drift"` | `DriftEvent` |
| `"consolidate"` | `ConsolidateEvent` |
| `"milestone"` | `MilestoneEvent` |

### `store.subscribe(event_name, callback) -> int`

Alias for `on()`.

### `store.unsubscribe(subscriber_id) -> bool`

Remove a subscription.

### `store.subscriber_count() -> int`

Number of active subscribers.

---

## Encoding

### `store.encode(text) -> list[float]`

Encode text using the store's model. Lazily loads the encoder on first call.

### `store.encode_batch(texts) -> list[list[float]]`

Batch encode multiple texts.

### `Encoder(model_dir)`

Standalone encoder from a local model directory.

### `Encoder.from_pretrained(model_id)`

Load an encoder from HuggingFace by model ID. Downloads and caches automatically.

| Method | Description |
|--------|-------------|
| `.encode(text) -> list[float]` | Encode a single text |
| `.encode_batch(texts) -> list[list[float]]` | Batch encode |
| `.dim -> int` | Embedding dimensionality |
| `.model_id -> str` | Model identifier |
| `.modality -> str` | Input modality (`"text"`, `"image"`, `"audio"`, `"multimodal"`) |

---

## Batch Operations

### `store.begin_batch()`

Begin a batch write session. Defers index updates until `commit()`.

### `store.commit()`

Commit a batch write session and update indices.

---

## Types Reference

| Type | Key Fields |
|------|------------|
| `Frame` | `.id`, `.content`, `.embedding`, `.parent_id`, `.watermark`, `.status`, `.in_degree`, `.timestamp`, `.metadata`, `.customer_uuid`, `.writer_signature` |
| `Hit` | `.frame_id`, `.score`, `.confidence`, `.path_length` |
| `Chain` | `.frames`, `.primitive`, `.coherence` |
| `ChainCoherence` | `.mean`, `.min`, `.max`, `.std`, `.segments` |
| `StoreStats` | `.total_frames`, `.active_frames`, `.eigenframe_count`, `.seed_count`, `.superseded_count`, `.redirect_count`, `.model_id`, `.dim` |
| `NeighborhoodResult` | `.frames`, `.edges`, `.region_eigenframe_id` |
| `RegionHealth` | `.eigenframe_id`, `.member_count`, `.mean_similarity`, `.min_similarity`, `.max_similarity`, `.std_similarity`, `.coherence` |
| `SimilarityStats` | `.mean`, `.std`, `.min`, `.max`, `.p25`, `.p50`, `.p75`, `.p95` |
| `SnapshotView` | `.frame_ids`, `.timestamp` |
| `DriftAlert` | `.frame_id`, `.drift_score`, `.window_start`, `.window_end` |
| `ContradictionCandidate` | `.frame_a`, `.frame_b`, `.similarity`, `.divergence` |
| `EvolutionStep` | `.frame_id`, `.distance`, `.generation` |
| `MergeCluster` | `.frame_ids`, `.centroid_id`, `.similarity` |
| `ConsolidateReport` | `.consolidated_id`, `.source_ids`, `.redirect_ids` |
| `BulkConsolidateReport` | `.reports`, `.total_consolidated`, `.total_redirects` |
| `RelationDef` | `.name`, `.slot`, `.flags` |
| `StructuralHit` | `.frame_id`, `.relation`, `.weight`, `.direction`, `.hops` |
| `TypedEdge` | `.from_id`, `.to_id`, `.relation`, `.weight`, `.slot` |
| `WriterAudit` | `.writers`, `.cross_customer_writes` |
| `AttestationAudit` | `.secure_mode`, `.chain_valid`, `.genesis_seed_matches`, `.operator_identity`, `.cross_customer_writes`, `.invalid_signature_count`, `.sealed`, `.all_ok`, `.writers`, `.failures` |
| `WarpCalibration` | `.points`, `.summary` |

---

## Enums Reference

| Enum | Values |
|------|--------|
| `FrameStatus` | `Active`, `Eigenframe`, `Superseded`, `Redirect`, `Genesis`, `Seed`, `System` |
| `Tier` | `None_`, `Coarse`, `Medium`, `Fine`, `Auto` |
| `Direction` | `Outgoing`, `Incoming`, `Both` |
| `ScoreCombine` | `Max`, `Sum`, `Mean` |

---

Copyright (c) 2025-2026 Tree D Interactive LLC. All rights reserved.
