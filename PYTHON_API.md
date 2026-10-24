# SERAPH Python API Reference

> Module: `seraph` (PyO3 native extension)
> Import: `import seraph`

This reference documents the Python API for the people who **call** SERAPH from
Python. The module is a PyO3 native extension — import it as `import seraph`.
Read *Authentication & Licensing* and *Conventions* first: they apply to every
function and are not repeated per entry. Every signature below was validated
against the PyO3 bindings in `seraph-rs/src/py.rs`.

This reference covers the 0.4.0 API: one operation per question, its options passed as keyword arguments. The names 0.4.0 replaced still work through 0.x as deprecated aliases, each raising a `DeprecationWarning` that names its replacement; they are documented in the 0.3.1 release's reference, not repeated here.

---

## Authentication & Licensing

SERAPH performs no network authentication. "Authorization" is **license-tier
gating**, enforced locally at the call site. Most methods work on every tier.
The capabilities below require a **Commercial or Enterprise** license; on the
Free tier they raise `RuntimeError`.

| Capability | Methods |
|---|---|
| Encryption at rest | `StoreConfig(encryption_passphrase=...)` + `SeraphStore.create`, `SeraphStore.open_encrypted` |
| Sealing | `store.seal` |
| Time anchoring | `store.append_tip_anchor` |

**Writer identity (secure mode).** The license attests *who is licensed and at
what tier* — it does **not** carry a signing key. Per-frame writer attribution
uses a key the **calling application supplies**: pass a 32-byte Ed25519 private
seed via the `writer_seed` parameter of `SeraphStore.create` /
`SeraphStore.open` (ADR-0001). The seed stays in the caller's process; SERAPH
signs each frame with it and records only the derived public key in the store.
When `writer_seed` is provided the store is secure and every appended frame is
signed; the seed never leaves the caller, and neither SERAPH nor the license
ever sees it. Secure mode requires both a permitting tier and a supplied writer
seed.

---

## Conventions

These rules hold for all functions unless an entry says otherwise:

- **Embeddings are caller-supplied.** SERAPH is content-agnostic: pass any
  embedding as a `list[float]`. Vector length must equal the store dimension
  (`store.dim()`). The optional `Encoder` / `store.encode` helpers exist for
  convenience but are not required.
- **Model ID is fixed at genesis.** The embedding model is declared at
  `create` and permanent for the store's lifetime. Mismatched writes are
  rejected.
- **Errors raise `RuntimeError`.** Failed operations raise `RuntimeError`
  (constructors that validate string enums may raise `ValueError`). A closed
  store raises `RuntimeError("Store is closed")` on any operation.
- **Metadata is JSON.** Metadata is passed as a JSON string (e.g.
  `metadata_json='{"source":"doc1"}'`, default `"{}"`). `Frame.metadata`
  returns the deserialized `dict`; `Frame.metadata_json` returns the raw string.
- **Tau defaults.** Search defaults to `tau=0.5`. Where `tau` accepts `None`,
  it auto-resolves to the store's p95 similarity; the strings `"loose"` (p75)
  and `"strict"` (p99) and explicit floats are also accepted.
- **Content is bytes.** `content` is `bytes` (any binary payload). Frame IDs are
  strings.

---

## Table of Contents

1. [Store Lifecycle](#store-lifecycle)
2. [Encryption](#encryption)
3. [Configuration](#configuration)
4. [Ingestion](#ingestion)
5. [Search](#search)
6. [Federation](#federation)
7. [Frame Access](#frame-access)
8. [Seeds & Promotion](#seeds--promotion)
9. [Traversal & Chains](#traversal--chains)
10. [Neighborhood](#neighborhood)
11. [Warp (Steering)](#warp-steering)
12. [Analysis & Metrics](#analysis--metrics)
13. [Intelligence](#intelligence)
14. [Consolidation](#consolidation)
15. [Typed Structural Edges](#typed-structural-edges)
16. [Snapshot / Temporal](#snapshot--temporal)
17. [Provenance & Sealing](#provenance--sealing)
18. [Anchoring](#anchoring)
19. [Events & Subscriptions](#events--subscriptions)
20. [Encoding](#encoding)
21. [Batch Operations](#batch-operations)
22. [Value & Result Type Methods](#value--result-type-methods)
23. [Types Reference](#types-reference)
24. [Enums Reference](#enums-reference)

---

## Store Lifecycle

### `SeraphStore.create(path, config=None, seed_data=[], writer_seed=None, *, model_id=None, tuning=None)`

Create a new store on disk. The model and the vector precision are fixed for the store's life; `model_id` is a shortcut for a default configuration on that model.

```python
SeraphStore.create(path, config=None, seed_data=[], writer_seed=None, *, model_id=None, tuning=None) -> SeraphStore
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `path` | `str \| PathLike` | yes | -- | Filesystem path for the `.sfg` file. |
| `config` | `StoreConfig \| None` | no | `None` | Store configuration; `None` for the defaults. |
| `seed_data` | `list[tuple[str, list[float]]]` | no | `[]` | Initial seeds as (label, embedding) tuples. |
| `writer_seed` | `bytes \| None` | no | `None` | 32-byte Ed25519 private seed supplied by the application. When provided, the store is secure and every frame is signed by this key; the seed never leaves the caller. |
| `model_id` | `str \| None` | no | `None` | Embedding model, when `config` is `None`. Giving both is an error. |
| `tuning` | `Tuning \| None` | no | `None` | Runtime tuning to start with. |

**Returns:** `SeraphStore` — handle to the new store.

**Raises:** `RuntimeError` if the path is unwritable, the store already exists, the config is invalid, or (when `writer_seed` is set) the license tier does not permit secure mode. `RuntimeError` if `writer_seed` is not exactly 32 bytes.

**Example**

```python
config = seraph.StoreConfig(model_id="BAAI/bge-small-en-v1.5")
store = seraph.SeraphStore.create("./knowledge.sfg", config)
```

### `SeraphStore.open(path, writer_seed=None, *, passphrase=None, read_only=False, tuning=None)`

Open an existing store. Options supply the writer identity for frames appended in the session, the passphrase of an encrypted store, read-only access, and the runtime tuning to start with, in any combination.

```python
SeraphStore.open(path, writer_seed=None, *, passphrase=None, read_only=False, tuning=None) -> SeraphStore
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `path` | `str \| PathLike` | yes | -- | Path to an existing `.sfg` file. |
| `writer_seed` | `bytes \| None` | no | `None` | 32-byte Ed25519 private seed supplying the writer identity for frames appended in this session. Secure appends require a writer. |
| `passphrase` | `str \| None` | no | `None` | Passphrase of an encrypted store. |
| `read_only` | `bool` | no | `False` | Open for reading: a secure store's writer state is not set up, so frames appended in the session carry no writer attribution. |
| `tuning` | `Tuning \| None` | no | `None` | Runtime tuning to start with; `None` for the defaults. |

**Returns:** `SeraphStore`.

**Raises:** `RuntimeError` if the file is missing, corrupt, or **encrypted** (pass `passphrase`), or if `writer_seed` is not exactly 32 bytes.

**Example**

```python
store = seraph.SeraphStore.open("./knowledge.sfg")
```

### `store.close()`

Close the store and flush all pending writes. The store object raises on any subsequent operation.

```python
store.close() -> None
```

*No parameters.*

**Returns:** `None`.

**Raises:** Does not raise; closing an already-closed store is a no-op.

### `store.flush()`

Flush pending writes to disk without closing (`flush` on the Python surface).

```python
store.flush() -> None
```

*No parameters.*

**Returns:** `None`.

**Raises:** `RuntimeError` if the store is closed or the underlying write fails.

### `store.set_fast_mode(enabled)`

Enable or disable fast ingest mode, which skips the per-frame WAL and fsync. Disabling flushes to disk.

```python
store.set_fast_mode(enabled) -> None
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `enabled` | `bool` | yes | -- | `True` to defer WAL/fsync; `False` to restore per-write durability. |

**Returns:** `None`.

**Raises:** `RuntimeError` if the store is closed.

### `store.door_index_status()`

Whether entry-point selection currently runs through the door index, and how many eigenframes it holds.

```python
store.door_index_status() -> tuple[bool, int]
```

*No parameters.*

**Returns:** `tuple[bool, int]` — (ready, nodes).

**Raises:** `RuntimeError` if the store is closed.

### `store.vector_precision()`

The precision each frame's vector is stored at, fixed at create (`StoreConfig(vector_precision=...)`): `"codes4"` (4-bit codes, the default), `"fp32"`, `"fp16"`, `"bf16"` or `"fp64"`. Stores below format 5 report `"codes4"`.

```python
store.vector_precision() -> str
```

*No parameters.*

**Returns:** `str`.

**Raises:** `RuntimeError` if the store is closed.

### `store.resident_precision()`

The precision search scores from: `"fp32"`, `"fp16"`, `"bf16"` or `"frame"` (no resident copy). The default is set at create (`StoreConfig(resident_precision=...)`) or derived from the vector precision when unset; `set_resident_precision` changes it at runtime.

```python
store.resident_precision() -> str
```

*No parameters.*

**Returns:** `str`.

**Raises:** `RuntimeError` if the store is closed.

### `store.set_resident_precision(name)`

Change the precision search scores from and rebuild the resident copy at once. `"fp16"` and `"bf16"` halve its memory; `"frame"` keeps no resident copy, at some cost per search. Runtime only; the store's default is the one set at create.

```python
store.set_resident_precision(name) -> None
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `name` | `str` | yes | -- | `"fp32"`, `"fp16"`, `"bf16"` or `"frame"`. |

**Returns:** `None`.

**Raises:** `ValueError` for an unknown name; `RuntimeError` for a reserved one or if the store is closed.

---

## Encryption

> **Requires Commercial or Enterprise license.** Free-tier calls raise `RuntimeError`.

SERAPH supports AES-256-GCM encryption at rest. When enabled, all content
fields (content bytes, embeddings, quantized vectors, metadata) are encrypted
per-frame with a random DEK wrapped by an Argon2id-derived KEK. Structural
metadata (watermarks, parent links, timestamps) stays plaintext for keyless
chain verification.

Encryption is requested at creation time via `StoreConfig.encryption_passphrase`
(see [Configuration](#configuration)); there is no separate
`create_encrypted` method on the Python surface. Re-open encrypted stores with
[`SeraphStore.open_encrypted`](#seraphstoreopen_encryptedpath-passphrase).

```python
config = seraph.StoreConfig(encryption_passphrase="strong passphrase here")
store = seraph.SeraphStore.create("./encrypted.sfg", config)
store.put(b"sensitive content", embedding)   # encrypted transparently
```

### Notes

- Encryption is set at creation time and cannot be added later.
- The `.gidx` sidecar is not written for encrypted stores (prevents topology leaks).
- Search, chain verification, and all other operations work identically on encrypted stores.
- The passphrase derives a KEK via Argon2id; a random DEK encrypts content.

---

## Configuration

### `StoreConfig(model_id="BAAI/bge-small-en-v1.5", eigen_threshold=5, eigen_min_age_s=0.0, tau_similarity=0.5, tau_merge=0.92, max_index_elements=100000, custom_range=None, search_max_depth=None, search_visit_cap_multiplier=None, redirect_per_frame=True, secure=None, eigen_matmul_min=None, encryption_passphrase=None, search_use_stream=None, search_stream_threshold=None, search_stream_beam=None, vector_precision="codes4", resident_precision=None, *, tuning=None, edge_budget=None, parent_select=None)`

Store configuration, given to `create`. The model id and the precisions are fixed for the store's life; `tuning` holds the runtime settings the store starts with. The separate runtime arguments still work and write into `tuning`.

```python
StoreConfig(model_id="BAAI/bge-small-en-v1.5", eigen_threshold=5, eigen_min_age_s=0.0, tau_similarity=0.5, tau_merge=0.92, max_index_elements=100000, custom_range=None, search_max_depth=None, search_visit_cap_multiplier=None, redirect_per_frame=True, secure=None, eigen_matmul_min=None, encryption_passphrase=None, search_use_stream=None, search_stream_threshold=None, search_stream_beam=None, vector_precision="codes4", resident_precision=None, *, tuning=None, edge_budget=None, parent_select=None) -> StoreConfig
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `model_id` | `str` | no | `"BAAI/bge-small-en-v1.5"` | Embedding model identifier (permanent for the store). |
| `eigen_threshold` | `int` | no | `5` | In-degree threshold for eigenframe promotion. |
| `eigen_min_age_s` | `float` | no | `0.0` | Minimum age (seconds) before promotion. Persisted in the header, so an existing store keeps the value it was created with. |
| `tau_similarity` | `float` | no | `0.5` | Default similarity threshold. |
| `tau_merge` | `float` | no | `0.92` | Consolidation merge threshold. |
| `max_index_elements` | `int` | no | `100000` | Max elements in the graph index. |
| `custom_range` | `QuantizationRange \| None` | no | `None` | Custom quantization range for unknown models; `None` looks up the bundled table by `model_id`. |
| `search_max_depth` | `int \| None` | no | `None` | Deprecated: `tuning.search.max_depth` (default 50). |
| `search_visit_cap_multiplier` | `int \| None` | no | `None` | Deprecated: `tuning.search.visit_cap_multiplier` (default 20). |
| `redirect_per_frame` | `bool` | no | `True` | Emit one redirect per superseded source frame. |
| `secure` | `str \| None` | no | `None` | Security posture: `"off"`, `"standard"` (alias `"on"`), or `None` (auto-detect). Other values are an error. |
| `eigen_matmul_min` | `int \| None` | no | `None` | Deprecated: `tuning.search.eigen_matmul_min` (default 256). |
| `encryption_passphrase` | `str \| None` | no | `None` | Passphrase for AES-256-GCM encryption at rest. |
| `search_use_stream` | `bool \| None` | no | `None` | Deprecated: `tuning.search.mode`. |
| `search_stream_threshold` | `int \| None` | no | `None` | Deprecated, and has no effect. |
| `search_stream_beam` | `int \| None` | no | `None` | Deprecated: `tuning.search.beam` (default 16). |
| `vector_precision` | `str` | no | `"codes4"` | Precision each frame's vector is stored at, fixed for the store's life: `"codes4"` (4-bit codes), `"fp32"`, `"fp16"`, `"bf16"` or `"fp64"`. Read-only once constructed. |
| `resident_precision` | `str \| None` | no | `None` | Precision search scores from: `"fp32"`, `"fp16"`, `"bf16"` or `"frame"` (no resident copy). `None` derives it from `vector_precision`. Changeable at runtime with `set_resident_precision`. Read-only once constructed. |
| `tuning` | `Tuning \| None` | no | `None` | Runtime settings the store starts with; `None` for the defaults. |
| `edge_budget` | `int \| None` | no | `None` | Similarity edges per frame, fixed for the store's life (default 10). |
| `parent_select` | `str \| None` | no | `None` | Parent selection, fixed for the store's life: `"stream"` (the default) or `"exact"` (a brute-force reference for verification). |

**Returns:** `StoreConfig` — all fields are readable attributes; all but the two precisions are writable.

**Raises:** `ValueError` if `secure` is set to an unrecognized string (expected `"off"` or `"standard"`/`"on"`), or if a precision name is unknown or reserved.

### `QuantizationRange(model_id, low, high, dim)`

Custom quantization range for models not in the built-in table.

```python
QuantizationRange(model_id, low, high, dim) -> QuantizationRange
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `model_id` | `str` | yes | -- | Model identifier. |
| `low` | `float` | yes | -- | Lower bound of the embedding value range. |
| `high` | `float` | yes | -- | Upper bound of the embedding value range. |
| `dim` | `int` | yes | -- | Embedding dimensionality. |

**Returns:** `QuantizationRange` — fields .model_id, .low, .high, .dim are readable and writable.

**Raises:** `ValueError` or `RuntimeError` on an invalid argument.

### `RelationMask(slots=[])`

Bitmask over relation slots, used to scope graph traversal to specific relation types.

```python
RelationMask(slots=[]) -> RelationMask
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `slots` | `list[int]` | no | `[]` | Slot IDs (`u16`) to include in the mask. |

**Returns:** `RelationMask`.

**Raises:** `ValueError` or `RuntimeError` on an invalid argument.

Static constructors: `RelationMask.from_slots(slots)`, `RelationMask.empty()`,
`RelationMask.semantic_only()` (PARENT_OF + SIMILAR_TO),
`RelationMask.structural_only()` (empty — union user slots in as you go).
Instance API: `.slots` (getter, `list[int]`), `.contains(slot) -> bool`,
`.is_empty() -> bool`, `.union(other) -> RelationMask`,
`.intersect(other) -> RelationMask`.

---

## Ingestion

### `store.put(content, embedding, seed_term=True, *, metadata=None, occurred_at=None, within=None, promote=False)`

Ingest content with a pre-computed embedding. Options set its metadata, when the content happened (`occurred_at`), a region to place it in (`within`), whether the seed term chooses its parent, and whether it becomes a door at once (`promote`), in any combination.

```python
store.put(content, embedding, seed_term=True, *, metadata=None, occurred_at=None, within=None, promote=False) -> str
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `content` | `bytes \| str` | yes | -- | Content; a `str` is stored as UTF-8. |
| `embedding` | `list[float]` | yes | -- | Embedding vector; length must equal the store dimension. |
| `seed_term` | `bool` | no | `True` | `False` takes the raw-cosine parent for this frame (see `tuning`). |
| `metadata` | `dict \| str \| None` | no | `None` | Metadata object, or its JSON text. Anything but a JSON object is an error. |
| `occurred_at` | `int \| datetime \| None` | no | `None` | When the content happened (microseconds since the epoch, or an aware datetime). A claim, bound into the frame's watermark. `None` leaves the frame on its commit time. |
| `within` | `str \| None` | no | `None` | Place the frame inside this region: a frame ID or a seed label. Its similarity edges stay global, so ordinary search still reaches it. |
| `promote` | `bool` | no | `False` | Make the frame a door (eigenframe) at once. |

**Returns:** `str` — the new frame ID.

**Raises:** `DimensionError` for a wrong-length embedding, `StoreSealedError` on a sealed store, `ValueError` for metadata that is not a JSON object.

**Example**

```python
emb = store.encode("the cat sat on the mat")
fid = store.put(b"the cat sat on the mat", emb)
```

### `store.put_batch_incremental(items, seed_term=True)`

Bulk ingest. Each item carries the options `put` takes (metadata, event time, region, seed term, promotion) and is placed against the live store, so an item may attach to an earlier item of the same batch and the result is identical to calling `put` in a loop at a fraction of the cost. Insertion order shapes the tree, so order the batch meaningfully. Every item is checked before the first is committed; a batch that stops partway reports the frames it committed. The store stays buffered until it is synced or closed.

```python
store.put_batch_incremental(items, seed_term=True) -> list[str]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `items` | `list` | yes | -- | Items: `(content, embedding)`, `(content, embedding, metadata)`, or mappings with `content`, `embedding` and optional `metadata`, `occurred_at`, `within`, `seed_term`, `promote`. |
| `seed_term` | `bool` | no | `True` | `False` takes raw-cosine parents for every item that does not set its own. |

**Returns:** `list[str]` — frame IDs, one per item.

**Raises:** `BatchIncompleteError` (with `.committed_ids`) when the batch stops partway; `ValueError` for a malformed item, before anything is committed.

### `store.supersede(frame_id)`

Soft-forget a single frame: flips its status to Superseded in place. Creates no new frame and no redirect, and does not reparent children.

```python
store.supersede(frame_id) -> str
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `frame_id` | `str` | yes | -- | Frame ID to supersede. |

**Returns:** `str` — the same frame_id echoed back.

**Raises:** `RuntimeError` if the frame is missing, consolidation-exempt, or the store is closed.

### `store.commit_marker(kind, metadata_json=None)`

Commit an application marker: a System frame with no embedding that search never returns, durable in the store like any other frame. Use it for application records that belong with the store (a run manifest, a review note). `kind` becomes the content prefix `_{kind}:` and is echoed in the metadata as `_marker`.

```python
store.commit_marker(kind, metadata_json=None) -> str
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `kind` | `str` | yes | -- | Marker kind. Non-empty, no `:` and no whitespace. The kinds the load path interprets (`promote`, `supersede`, `seal`, `seraph_seeds`, `seraph_warp_cal`, anything starting `seraph`, case-insensitive, with or without a leading `_`) are refused. |
| `metadata_json` | `str \| dict \| None` | no | `None` | Metadata object or its JSON text. Keys the load path acts on (`_promote`, `_supersede`, any `_seraph*`) are refused, so a marker can never forge a promotion, a supersede or a registry on reopen. |

**Returns:** `str` — the marker's frame ID.

**Raises:** `ValueError` if `metadata_json` is not valid JSON; `RuntimeError` on a reserved kind, an invalid kind, a load-path metadata key, a non-object payload, a sealed store, or a closed store.

### `store.markers(kind)`

Markers of one kind, in commit order.

```python
store.markers(kind) -> list[tuple[str, str]]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `kind` | `str` | yes | -- | The kind passed to `commit_marker`. |

**Returns:** `list[tuple[str, str]]` — (frame_id, metadata_json) pairs; empty for an unknown kind or a store with no system root.

**Raises:** `RuntimeError` if the store is closed.

---

## Search

### `store.search(query, top_k=10, tau=None, *, include_superseded=False, occurred=None, committed=None, basis="or_committed", order="relevance", within=None, warp=None, trace=False, max_visits=None, beam=None)`

Search for the frames that best answer a query: a query vector, or text encoded with the store's model. Options limit which frames may fill a result slot (superseded frames, a window on when the content happened, a window on when it was committed, a region), warp the query, order the selected hits, and return the visits made. A `committed` window's end reads the store as it stood then.

```python
store.search(query, top_k=10, tau=None, *, include_superseded=False, occurred=None, committed=None, basis="or_committed", order="relevance", within=None, warp=None, trace=False, max_visits=None, beam=None) -> list[Hit]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `query` | `list[float] \| str` | yes | -- | Query vector, or text encoded with the store's model. |
| `top_k` | `int` | no | `10` | Maximum number of hits. |
| `tau` | `float \| str \| None` | no | `None` | Minimum similarity for a hit: a float, or `"store"` (the store's `tau_similarity`), `"auto"`, `"loose"`, `"strict"` (the store's background similarity). `None` is 0.5. |
| `include_superseded` | `bool` | no | `False` | Let superseded frames fill result slots. |
| `occurred` | `tuple \| None` | no | `None` | `(after, before)` on when the content happened (microseconds or aware datetimes; either end `None`). A claim; a frame without one is placed by `basis`. |
| `committed` | `tuple \| None` | no | `None` | `(after, before)` on when the frame was committed, the engine's own time, never coalesced. Its `before` end reads the store as it stood then. |
| `basis` | `str` | no | `"or_committed"` | How a frame with no event time is placed in an `occurred` window: `"or_committed"` at its commit time, `"claimed_only"` not at all. |
| `order` | `str` | no | `"relevance"` | Order of the selected hits: `"relevance"`, `"most_recent"` or `"oldest"`, by event time under the default basis. |
| `within` | `str \| None` | no | `None` | Confine the search to a region: a frame ID or a seed label. |
| `warp` | `WarpSpec \| None` | no | `None` | Warp applied to the query before the search. |
| `trace` | `bool` | no | `False` | Return a SearchResult with the visits made. |
| `max_visits` | `int \| None` | no | `None` | Visit budget for this search; `None` uses the store's tuning. |
| `beam` | `int \| None` | no | `None` | Walk width for this search; `None` uses the store's tuning. |

**Returns:** `list[Hit]` — each with .frame_id, .score, .timestamp, .occurred_at; a SearchResult (.hits, .trace, .visits) when `trace` is set.

**Raises:** `ValueError` for a malformed option, `SeraphError` when a warp or region does not resolve.

**Example**

```python
q = store.encode("feline animals")
for hit in store.search(q, top_k=5, tau=0.4):
    print(hit.frame_id, hit.score, hit.snippet)
```

### `store.warp_calibration()`

The store's resident warp calibration as a mapping from named magnitude to value, or `None` if the store has not been calibrated. Named-magnitude warps resolve against this; there are no built-in presets.

```python
store.warp_calibration() -> dict | None
```

*No parameters.*

**Returns:** `dict | None` — named magnitude to value; None if the store is uncalibrated.

**Raises:** `RuntimeError` if the store is closed.

### `store.fixup_warp()`

Refresh the store's warp calibration from its own corpus when the persisted one has drifted or none exists, and persist the result. Call from the single writer after open.

```python
store.fixup_warp() -> bool
```

*No parameters.*

**Returns:** `bool` — True if it recalibrated, False if the calibration was current or the geometry not yet developed.

**Raises:** `RuntimeError` if the store is closed.

---

## Federation

Route a query across many independent single-file stores and fan out search in
parallel. A federation holds no state of its own — point it at a directory of
`.sfg` files, or add them by path. All member stores must share the same embedding
model and dimension (enforced at add).

### `Federation.open_dir(dir)`

Open every `.sfg` store in a directory as a federation, each member named by its file stem (`physics.sfg` becomes `"physics"`); files starting with `_` or `router` are skipped.

```python
Federation.open_dir(dir) -> Federation
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `dir` | `str` | yes | -- | Directory holding the member stores. |

**Returns:** `Federation`.

**Raises:** `RuntimeError` if the store is closed or the operation fails.

### `Federation()`

Create an empty federation. Add member stores with `add_path`, or open a directory of stores with `open_dir`.

```python
Federation() -> Federation
```

*No parameters.*

**Returns:** `Federation`.

**Raises:** `ValueError` or `RuntimeError` on an invalid argument.

### `Federation.add_path(name, path)`

Register a store file under `name` as a member of the federation. The store must share the federation's embedding model and dimension.

```python
Federation.add_path(name, path) -> None
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `name` | `str` | yes | -- | Member name. |
| `path` | `str` | yes | -- | Path to the member's `.sfg` file. |

**Returns:** `None` — raises on a model or dimension mismatch.

**Raises:** `RuntimeError` if the store is closed or the operation fails.

### `Federation.member_names()`

Member store names, in add order.

```python
Federation.member_names() -> list[str]
```

*No parameters.*

**Returns:** `list[str]`.

**Raises:** `RuntimeError` if the store is closed or the operation fails.

### `Federation.refresh()`

Re-pull routing vectors for any member that promoted an eigenframe since the last refresh. Cheap: only changed members are read.

```python
Federation.refresh() -> bool
```

*No parameters.*

**Returns:** `bool` — True if the router changed.

**Raises:** `RuntimeError` if the store is closed or the operation fails.

### `Federation.route(query, top_stores)`

Route a query to the members most likely to hold its answer, without searching them.

```python
Federation.route(query, top_stores) -> list[tuple[str, float]]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `query` | `list[float]` | yes | -- | Query vector. |
| `top_stores` | `int` | yes | -- | Members to return. |

**Returns:** `list[tuple[str, float]]` — (store_name, score) pairs.

**Raises:** `RuntimeError` if the store is closed or the operation fails.

### `Federation.search(query, top_k, tau, mode="best", n=3)`

Search the federation: route the query to the best-fitting members, search each, and merge the hits into one ranked list.

```python
Federation.search(query, top_k, tau, mode="best", n=3) -> list[FederatedHit]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `query` | `list[float]` | yes | -- | Query vector. |
| `top_k` | `int` | yes | -- | Maximum number of hits. |
| `tau` | `float` | yes | -- | Minimum similarity for a hit. |
| `mode` | `str` | no | `"best"` | Routing mode: `"best"` (the best-fitting member), `"top"` (the `n` best) or `"all"`. |
| `n` | `int` | no | `3` | Members searched under `"top"`. |

**Returns:** `list[FederatedHit]` — hits tagged with the member store they came from.

**Raises:** `RuntimeError` if the store is closed or the operation fails.

---

## Frame Access

### `store.get_frame(frame_id)`

Get a frame by ID, with its full data including embedding and metadata.

```python
store.get_frame(frame_id) -> Frame | None
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `frame_id` | `str` | yes | -- | Frame ID to fetch. |

**Returns:** `Frame | None` — None if not found.

**Raises:** `RuntimeError` if the store is closed.

### `store.lifecycle(frame_id)`

The record's account of one frame: when it was committed (and the event time it claims), when it became a door and how (genesis, seed, threshold or manual promotion), and when it was retired and how (superseded, or consolidated into a frame). Each event carries its position in record order and the commit time there.

```python
store.lifecycle(frame_id) -> Lifecycle | None
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `frame_id` | `str` | yes | -- | The frame. |

**Returns:** `Lifecycle | None` — None for an unknown frame (JSON on C).

**Raises:** `RuntimeError` if the store is closed or the operation fails.

### `store.frame_ids()`

All frame IDs in insertion order.

```python
store.frame_ids() -> list[str]
```

*No parameters.*

**Returns:** `list[str]`.

**Raises:** `RuntimeError` if the store is closed.

### `store.eigenframe_ids()`

IDs of the eigenframes.

```python
store.eigenframe_ids() -> list[str]
```

*No parameters.*

**Returns:** `list[str]`.

**Raises:** `RuntimeError` if the store is closed.

### `store.eigenframes()`

Full Frame objects for all eigenframes.

```python
store.eigenframes() -> list[Frame]
```

*No parameters.*

**Returns:** `list[Frame]`.

**Raises:** `RuntimeError` if the store is closed.

### `store.children(frame_id)`

The child frames of a frame as full Frame objects. get_children returns their IDs.

```python
store.children(frame_id) -> list[Frame]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `frame_id` | `str` | yes | -- | Parent frame ID. |

**Returns:** `list[Frame]`.

**Raises:** `RuntimeError` if the store is closed.

### `store.get_children(frame_id)`

Child frame IDs of a frame.

```python
store.get_children(frame_id) -> list[str]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `frame_id` | `str` | yes | -- | Parent frame ID. |

**Returns:** `list[str]`.

**Raises:** `RuntimeError` if the store is closed.

### `store.model_id()`

The embedding model ID declared at genesis.

```python
store.model_id() -> str
```

*No parameters.*

**Returns:** `str`.

**Raises:** `RuntimeError` if the store is closed.

### `store.dim()`

Embedding dimensionality of the store.

```python
store.dim() -> int
```

*No parameters.*

**Returns:** `int`.

**Raises:** `RuntimeError` if the store is closed.

### `store.federation_vector()`

Mean of all eigenframe embeddings: a single vector representing the store's semantic centre, used for federation routing.

```python
store.federation_vector() -> list[float] | None
```

*No parameters.*

**Returns:** `list[float] | None` — None if the store has no eigenframes.

**Raises:** `RuntimeError` if the store is closed.

### `store.stats()`

Store statistics.

```python
store.stats() -> StoreStats
```

*No parameters.*

**Returns:** `StoreStats` — .total_frames, .active_frames, .eigenframes, .seeds, .redirects, .superseded, .model_id.

**Raises:** `RuntimeError` if the store is closed.

### `store.verify()`

Verify watermark-chain integrity.

```python
store.verify() -> tuple[bool, list[str]]
```

*No parameters.*

**Returns:** `tuple[bool, list[str]]` — (ok, failures).

**Raises:** `RuntimeError` if the store is closed.

---

## Seeds & Promotion

### `store.add_seed(label, embedding, metadata_json=None)`

Add a seed frame to a live store (immediately an eigenframe).

```python
store.add_seed(label, embedding, metadata_json=None) -> str
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `label` | `str` | yes | -- | Human-readable seed label. |
| `embedding` | `list[float]` | yes | -- | Seed embedding vector. |
| `metadata_json` | `str \| dict \| None` | no | `None` | Metadata object or its JSON text; anything but a JSON object is an error. |

**Returns:** `str` — the seed frame ID.

**Raises:** `RuntimeError` on dimension mismatch, a sealed store, or a closed store.

### `store.get_seeds()`

The authored seed list: `label -> frame_id` bindings in authored order. A binding may name any frame, not only a Seed. When no list has been authored, the store's Seed frames are returned instead; `seeds_are_authored()` tells the two apart.

```python
store.get_seeds() -> list[SeedEntry]
```

*No parameters.*

**Returns:** `list[SeedEntry]` — each with .label and .frame_id, in authored order.

**Raises:** `RuntimeError` if the store is closed.

**Example**

```python
for s in store.get_seeds():
    print(s.label, s.frame_id)
```

### `store.seeds_are_authored()`

Whether the seed list comes from an authored registry rather than being derived from Seed-status frames. An authored empty registry is a real state and is authoritative; it does not fall back to the derived list.

```python
store.seeds_are_authored() -> bool
```

*No parameters.*

**Returns:** `bool` — True if authored, False if derived.

**Raises:** `RuntimeError` if the store is closed.

### `store.resolve_seed(label)`

Resolve a seed label to its frame ID. Labels are unique within a store's seed registry.

```python
store.resolve_seed(label) -> str | None
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `label` | `str` | yes | -- | Registered seed label. |

**Returns:** `str | None` — the frame ID; None if the label is not registered.

**Raises:** `RuntimeError` if the store is closed.

### `store.set_seeds(entries)`

Replace the authored seed list. Removing a label removes it from the list only; the frame keeps its status.

```python
store.set_seeds(entries) -> str
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `entries` | `list[SeedEntry]` | yes | -- | The complete new registry. |

**Returns:** `str` — the snapshot sentinel's frame ID.

**Raises:** `RuntimeError` on an empty or duplicated label, a `frame_id` absent
from the store, a sealed store, or a closed store.

**Example**

```python
store.set_seeds([seraph.SeedEntry("evidence", fid)])
```

### `store.register_seed(label, frame_id)`

Bind a label to a frame in the seed registry, appending a new registry snapshot. Rebinding an existing label updates it in place and keeps its position.

```python
store.register_seed(label, frame_id) -> str
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `label` | `str` | yes | -- | Seed label; unique within the registry. |
| `frame_id` | `str` | yes | -- | Frame the label names. |

**Returns:** `str` — the new registry snapshot's frame ID.

**Raises:** `RuntimeError` on an empty label, an unknown `frame_id`, a sealed
store, or a closed store.

### `store.unregister_seed(label)`

Drop a label from the seed registry, appending a new registry snapshot. The frame it named is untouched.

```python
store.unregister_seed(label) -> str
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `label` | `str` | yes | -- | Registered seed label. |

**Returns:** `str` — the new registry snapshot's frame ID; an error if the label is not registered.

**Raises:** `RuntimeError` if the label is not registered, on a sealed store, or
on a closed store.

### `SeedEntry(label, frame_id)`

One authored `label -> frame_id` binding in the seed registry. Returned by `get_seeds` and accepted by `set_seeds`.

```python
SeedEntry(label, frame_id) -> SeedEntry
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `label` | `str` | yes | -- | Seed label; unique within a registry. |
| `frame_id` | `str` | yes | -- | Frame the label names; need not be Seed-status. |

**Returns:** `SeedEntry` — read-only getters .label and .frame_id; supports == and repr().

**Raises:** `ValueError` or `RuntimeError` on an invalid argument.

### `store.promote(frame_id)`

Promote an active frame to eigenframe unconditionally. Promotion is monotonic.

```python
store.promote(frame_id) -> None
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `frame_id` | `str` | yes | -- | Frame ID to promote. |

**Returns:** `None`.

**Raises:** `RuntimeError` if the frame is missing or the store is closed.

---

## Traversal & Chains

### `store.path(from_id, to_id)`

Shortest path between two frames over parent-child links.

```python
store.path(from_id, to_id) -> list[Frame]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `from_id` | `str` | yes | -- | Start frame ID. |
| `to_id` | `str` | yes | -- | End frame ID. |

**Returns:** `list[Frame]` — ordered from_id to to_id, empty if unreachable.

**Raises:** `RuntimeError` if the store is closed.

### `store.chain(a, b, include_superseded=False, *, forward=False, warp=None, fit=None, as_of=None)`

The shortest route between two endpoints over committed edges, each step weighted by how similar its two frames are. Each end is a frame ID or a query vector, which stands for the frames that fit it best. With `forward`, every step runs forward in record order: how the start led to the end. For a frame's own ancestry use `lineage`.

```python
store.chain(a, b, include_superseded=False, *, forward=False, warp=None, fit=None, as_of=None) -> Chain
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `a` | `str \| list[float]` | yes | -- | Start: a frame ID or a query vector. |
| `b` | `str \| list[float]` | yes | -- | End: a frame ID or a query vector. |
| `include_superseded` | `bool` | no | `False` | Let superseded frames be endpoints and steps. |
| `forward` | `bool` | no | `False` | Every step forward in record order (the start must precede the end). |
| `warp` | `WarpSpec \| None` | no | `None` | Warp applied to query endpoints. |
| `fit` | `tuple[int, float] \| None` | no | `None` | `(top_k, tau)`: the search results that may stand for a query endpoint. `None` takes the nearest frame for a route and the best-fitting pair of the top 10 at 0.5 for a forward route. |
| `as_of` | `int \| datetime \| None` | no | `None` | Read the record as it stood at this commit time: frames committed later are neither endpoints nor steps. |

**Returns:** `Chain`.

**Raises:** `RuntimeError` if the store is closed.

### `store.provenance(frame_id, tau=None, include_superseded=False, *, warp=None, as_of=None)`

What `frame_id` was built on: a route back in record order through the frames it attached to when it was committed (its parent and the similarity neighbours it chose), stopping when the next step is no more related than the frame's own background. A warp steers which strand is followed; `as_of` reads the record as it stood. `reached` is `True` when at least one step was taken.

```python
store.provenance(frame_id, tau=None, include_superseded=False, *, warp=None, as_of=None) -> Chain
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `frame_id` | `str` | yes | -- | Start frame. |
| `tau` | `float \| str \| None` | no | `None` | Stop floor: a float or a tau name; `None` for the frame's own background. |
| `include_superseded` | `bool` | no | `False` | Let superseded frames be steps. |
| `warp` | `WarpSpec \| None` | no | `None` | Steers which strand is followed; a step may not move against it. |
| `as_of` | `int \| datetime \| None` | no | `None` | Read the record as it stood at this commit time: later frames are not steps. |

**Returns:** `Chain` — start frame first.

**Raises:** `RuntimeError` if the store is closed.

### `store.influence(frame_id, tau=None, include_superseded=False, *, warp=None, as_of=None)`

What was built on `frame_id`: a route forward in record order through the frames that attached to it when they were committed, stopping when the next step is no more related than the frame's own background. A warp steers which strand is followed; `as_of` reads the record as it stood. `reached` is `True` when at least one step was taken.

```python
store.influence(frame_id, tau=None, include_superseded=False, *, warp=None, as_of=None) -> Chain
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `frame_id` | `str` | yes | -- | Start frame. |
| `tau` | `float \| str \| None` | no | `None` | Stop floor: a float or a tau name; `None` for the frame's own background. |
| `include_superseded` | `bool` | no | `False` | Let superseded frames be steps. |
| `warp` | `WarpSpec \| None` | no | `None` | Steers which strand is followed; a step may not move against it. |
| `as_of` | `int \| datetime \| None` | no | `None` | Read the record as it stood at this commit time: later frames are not steps. |

**Returns:** `Chain` — start frame first.

**Raises:** `RuntimeError` if the store is closed.

### `store.connections(from_id, to_id, max_routes=5, forward=False, include_superseded=False, *, as_of=None)`

The distinct ways two frames connect: up to `max_routes` routes over committed edges that share no interior frames, shortest first, each with its bridge frame (the interior frame closest to both ends). With `forward`, every route runs forward in record order: the ways `from_id` led to `to_id`.

```python
store.connections(from_id, to_id, max_routes=5, forward=False, include_superseded=False, *, as_of=None) -> list[Connection]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `from_id` | `str` | yes | -- | Start frame. |
| `to_id` | `str` | yes | -- | End frame. |
| `max_routes` | `int` | no | `5` | Most routes returned. |
| `forward` | `bool` | no | `False` | Routes forward in record order only (`from_id` must precede `to_id`). |
| `include_superseded` | `bool` | no | `False` | Let superseded frames be steps. |
| `as_of` | `int \| datetime \| None` | no | `None` | Read the record as it stood at this commit time. |

**Returns:** `list[Connection]` — empty when the frames are not joined.

**Raises:** `RuntimeError` if the store is closed.

### `store.chain_to(query, *, include_superseded=False, warp=None, as_of=None)`

The route from the door a query enters through to the frame nearest the query. On C and Rust use `chain` with an entry endpoint.

```python
store.chain_to(query, *, include_superseded=False, warp=None, as_of=None) -> Chain
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `query` | `list[float]` | yes | -- | Query vector. |
| `include_superseded` | `bool` | no | `False` | Let superseded frames be the end and steps. |
| `warp` | `WarpSpec \| None` | no | `None` | Warp applied to the query. |
| `as_of` | `int \| datetime \| None` | no | `None` | Read the record as it stood at this commit time, through the doors of that moment. |

**Returns:** `Chain`.

**Raises:** `RuntimeError` if the store is closed.

### `store.history(query_embedding, top_k=10, tau=None, anchors=3, *, warp=None, occurred=None, committed=None, basis="or_committed")`

How the store came to hold what the query finds. The top `anchors` search hits choose their lineages, and the best `top_k` frames along those lineages come back oldest first (by event time under the default basis), each with the route that reached it. The time windows limit which frames come back, as in `search`; a warp chooses which strand of the history is read.

```python
store.history(query_embedding, top_k=10, tau=None, anchors=3, *, warp=None, occurred=None, committed=None, basis="or_committed") -> History
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `query_embedding` | `list[float]` | yes | -- | Query vector. |
| `top_k` | `int` | no | `10` | Frames to return. |
| `tau` | `float \| str \| None` | no | `None` | Similarity floor for admission and results: a float or a tau name; `None` is 0.5. |
| `anchors` | `int` | no | `3` | Search hits used to choose the lineages. |
| `warp` | `WarpSpec \| None` | no | `None` | Warp applied to the query. |
| `occurred` | `tuple \| None` | no | `None` | `(after, before)` on when the content happened, as in `search`. |
| `committed` | `tuple \| None` | no | `None` | `(after, before)` on commit time, as in `search`; its `before` end reads the store as it stood. |
| `basis` | `str` | no | `"or_committed"` | How a frame with no event time is placed in an `occurred` window, as in `search`. |

**Returns:** `History` — hits carry .timestamp and .occurred_at.

**Raises:** `RuntimeError` if the store is closed.

### `store.lineage(frame_id)`

Parent chain back to genesis: [frame, parent, ..., GENESIS].

```python
store.lineage(frame_id) -> list[Frame]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `frame_id` | `str` | yes | -- | Frame ID. |

**Returns:** `list[Frame]`.

**Raises:** `RuntimeError` if the store is closed.

### `store.gradient(chain)`

Semantic gradient along an ordered list of frame IDs: one GradientPoint per step.

```python
store.gradient(chain) -> list[GradientPoint]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `chain` | `list[str]` | yes | -- | Ordered frame IDs. |

**Returns:** `list[GradientPoint]` — each with .frame_id, .similarity, .delta.

**Raises:** `RuntimeError` if the store is closed.

### `store.constitutive_score(chain)`

How much a chain says beyond a search at its midpoint: `1 - jaccard(chain_intermediates, midpoint_NN)` for an ordered frame-ID list. Values below `0.20` indicate the chain carries little beyond that search.

```python
store.constitutive_score(chain) -> float
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `chain` | `list[str]` | yes | -- | Ordered frame IDs. |

**Returns:** `float`.

**Raises:** `RuntimeError` if the store is closed.

### `store.subtree(frame_id, max_depth=None)`

All descendant frames under `frame_id`, optionally bounded by `max_depth`.

```python
store.subtree(frame_id, max_depth=None) -> list[Frame]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `frame_id` | `str` | yes | -- | Root frame ID. |
| `max_depth` | `int \| None` | no | `None` | Maximum descent depth; `None` = unbounded. |

**Returns:** `list[Frame]`.

**Raises:** `RuntimeError` if the store is closed.

### `store.subtrees(min_size=10)`

Enumerate seed-rooted subtrees with at least `min_size` descendants.

```python
store.subtrees(min_size=10) -> list[SubtreeInfo]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `min_size` | `int` | no | `10` | Minimum descendant count. |

**Returns:** `list[SubtreeInfo]` — each with .root, .root_status, .descendant_count, .eigenframe_count, .max_depth.

**Raises:** `RuntimeError` if the store is closed.

---

## Neighborhood

### `store.neighborhood(frame_id, tau=None, hops=1, *, warp=None)`

Three-population neighborhood of a frame: its consensus, geometric and lineage neighbours. A warp moves the anchor for the geometric population only.

```python
store.neighborhood(frame_id, tau=None, hops=1, *, warp=None) -> NeighborhoodResult
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `frame_id` | `str` | yes | -- | Center frame ID. |
| `tau` | `float \| str \| None` | no | `None` | `None` (the store's background p95), `"loose"` (p75), `"strict"` (p99), `"store"`, or an explicit float. |
| `hops` | `int` | no | `1` | Traversal radius in hops. |
| `warp` | `WarpSpec \| None` | no | `None` | Warp applied to the anchor for the geometric population. |

**Returns:** `NeighborhoodResult`.

**Raises:** `RuntimeError` on an invalid `tau` string or a closed store.

---

## Warp (Steering)

> **Profiles:** `profile` accepts `"routing"`, `"saturation"`, or `"bounded"`. Target pull is always uncapped; only `"bounded"` caps the suppression norm (at `bound_k`).

### `store.warp_query(query, spec, magnitude_overrides=None)`

Apply a warp specification to a query and return the warped vector, without searching.

```python
store.warp_query(query, spec, magnitude_overrides=None) -> list[float]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `query` | `list[float] \| str` | yes | -- | Query vector, or on Python a text string that is encoded first. |
| `spec` | `WarpSpec` | yes | -- | Warp specification; a JSON document on C and Rust. |
| `magnitude_overrides` | `dict[str, float] \| None` | no | `None` | Per-name overrides of the spec's named magnitudes. |

**Returns:** `list[float]` — the warped query vector.

**Raises:** `RuntimeError` if `query` is neither a string nor a `list[float]`, on unresolved spec inputs, or a closed store.

### `store.calibrate_warp(n_queries=30, n_targets=3, magnitudes=None, top_k=10, seed=42)`

Run an empirical magnitude calibration sweep against the store.

```python
store.calibrate_warp(n_queries=30, n_targets=3, magnitudes=None, top_k=10, seed=42) -> WarpCalibration
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `n_queries` | `int` | no | `30` | Number of sampled queries. |
| `n_targets` | `int` | no | `3` | Number of sampled targets. |
| `magnitudes` | `list[float] \| None` | no | `None` | Magnitudes to sweep; `None` uses defaults. |
| `top_k` | `int` | no | `10` | Hits per probe. |
| `seed` | `int` | no | `42` | RNG seed for reproducible sampling. |

**Returns:** `WarpCalibration`.

**Raises:** `RuntimeError` on calibration error or a closed store.

### `WarpSpec(targets=None, suppress=None, profile="bounded", bound_k=1.5, accumulation="reset", *, overrides=None)`

Warp specification: what to steer toward and away from, and how. Each `input` is a frame-id `str` or a `list[float]` embedding; each `magnitude` is a `float` or a named-magnitude `str` (`"action_floor"`, `"routing"`, `"sub_saturation"`, `"saturation"`), resolved against the store's warp calibration and the spec's own `overrides`. `WarpSpec.raw(targets, suppress, profile, bound_k)` builds one from embedding pairs.

```python
WarpSpec(targets=None, suppress=None, profile="bounded", bound_k=1.5, accumulation="reset", *, overrides=None) -> WarpSpec
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `targets` | `list[tuple] \| None` | no | `None` | (input, magnitude) pull pairs. |
| `suppress` | `list[tuple] \| None` | no | `None` | (input, magnitude) suppression pairs. |
| `profile` | `str` | no | `"bounded"` | `"routing"`, `"saturation"`, or `"bounded"`; anything else is an error. |
| `bound_k` | `float` | no | `1.5` | Suppression-norm cap. |
| `accumulation` | `str` | no | `"reset"` | `"reset"` or `"cumulative"`. |
| `overrides` | `dict[str, float] \| None` | no | `None` | Named-magnitude values that take priority over the store's calibration for this warp. |

**Returns:** `WarpSpec` — read-only getters .profile, .bound_k, .accumulation, .targets, .suppress; .overrides is writable.

**Raises:** `RuntimeError` on unknown profile/accumulation or malformed input/magnitude.

---

## Analysis & Metrics

### `store.density_score(frame_id)`

Local density score for a frame.

```python
store.density_score(frame_id) -> float
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `frame_id` | `str` | yes | -- | Frame ID. |

**Returns:** `float`.

**Raises:** `RuntimeError` if the store is closed.

### `store.seed_margin(frame_id)`

Seed margin: a frame's geometric distinctness relative to the corpus centroid.

```python
store.seed_margin(frame_id) -> float
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `frame_id` | `str` | yes | -- | Frame ID. |

**Returns:** `float`.

**Raises:** `RuntimeError` if the store is closed.

### `store.steerability(query_embedding, target_id)`

Warp steerability: angular displacement of a query toward a target at routing magnitude.

```python
store.steerability(query_embedding, target_id) -> float
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `query_embedding` | `list[float]` | yes | -- | Query vector. |
| `target_id` | `str` | yes | -- | Target frame ID. |

**Returns:** `float`.

**Raises:** `RuntimeError` if the store is closed.

### `store.region_health(eigenframe_id)`

Region health metrics for an eigenframe.

```python
store.region_health(eigenframe_id) -> RegionHealth
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `eigenframe_id` | `str` | yes | -- | Eigenframe ID. |

**Returns:** `RegionHealth` — .coherence, .direction, .candidate_split.

**Raises:** `RuntimeError` if the store is closed.

### `store.similarity_stats()`

Similarity distribution computed from parent-child relationships.

```python
store.similarity_stats() -> SimilarityDistribution
```

*No parameters.*

**Returns:** `SimilarityDistribution` — .count, .mean, .p75, .p90, .p95, .p99.

**Raises:** `RuntimeError` if the store is closed.

### `store.resolve_tau(tau=None)`

Resolve a tau value against the store's background similarity: `None` gives the p95, `"loose"` the p75, `"strict"` the p99, and a float passes through.

```python
store.resolve_tau(tau=None) -> float
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `tau` | `float \| str \| None` | no | `None` | `None`, `"loose"`, `"strict"`, or an explicit float. |

**Returns:** `float`.

**Raises:** `RuntimeError` on an unknown tau name or a closed store.

---

## Intelligence

### `store.drift_report(window=20, threshold=0.3)`

Scan eigenframes for semantic drift: children drifting away from their eigenframe.

```python
store.drift_report(window=20, threshold=0.3) -> list[DriftAlert]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `window` | `int` | no | `20` | Recent-frame window. |
| `threshold` | `float` | no | `0.3` | Drift trigger threshold. |

**Returns:** `list[DriftAlert]`.

**Raises:** `RuntimeError` if the store is closed.

### `store.contradictions(min_divergence=1.2)`

Detect high-divergence siblings under a shared parent.

```python
store.contradictions(min_divergence=1.2) -> list[ContradictionCandidate]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `min_divergence` | `float` | no | `1.2` | Minimum divergence to flag. |

**Returns:** `list[ContradictionCandidate]`.

**Raises:** `RuntimeError` if the store is closed.

### `store.contradictions_v2(tau=None)`

Three-population contradiction detection: geometric, lineage and consensus pairs.

```python
store.contradictions_v2(tau=None) -> ContradictionReport
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `tau` | `float \| str \| None` | no | `None` | `None` (p95), `"loose"` (p75), `"strict"` (p99), or an explicit float. |

**Returns:** `ContradictionReport` — .geometric, .lineage, .consensus (each list[ContradictionPair]).

**Raises:** `RuntimeError` on an invalid `tau` string or a closed store.

### `store.evolution(seed_frame_id)`

Concept evolution trajectory from a seed frame, in timestamp order.

```python
store.evolution(seed_frame_id) -> list[Frame]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `seed_frame_id` | `str` | yes | -- | Seed frame ID. |

**Returns:** `list[Frame]`.

**Raises:** `RuntimeError` if the store is closed.

### `store.evolution_steps(seed_frame_id)`

The evolution trajectory from a seed frame as steps, each with its timestamp, depth, similarity to the seed and a snippet.

```python
store.evolution_steps(seed_frame_id) -> list[EvolutionStep]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `seed_frame_id` | `str` | yes | -- | Seed frame ID. |

**Returns:** `list[EvolutionStep]` — each with .frame_id, .timestamp, .depth, .similarity_to_seed, .snippet.

**Raises:** `RuntimeError` if the store is closed.

---

## Consolidation

### `store.merge_candidates(tau=0.92)`

Find clusters of similar frames that could be consolidated.

```python
store.merge_candidates(tau=0.92) -> list[MergeCluster]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `tau` | `float` | no | `0.92` | Similarity threshold for candidacy. |

**Returns:** `list[MergeCluster]` — each with .frame_ids, .snippet, .count.

**Raises:** `RuntimeError` if the store is closed.

### `store.consolidate(frame_ids, *, strategy=None)`

Consolidate a specific set of frames into one frame at their centroid, carrying the most central frame's content, metadata and event time. Frames that carry different markings are not consolidated.

```python
store.consolidate(frame_ids, *, strategy=None) -> ConsolidateReport
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `frame_ids` | `list[str]` | yes | -- | Frame IDs to consolidate. |
| `strategy` | `MergeStrategy \| None` | no | `None` | Merge strategy; `None` is the geometric merge. |

**Returns:** `ConsolidateReport` — .merged_count, .new_frame_id, .redirect_frame_ids, .superseded_ids.

**Raises:** `RuntimeError` if the store is sealed/closed.

### `store.consolidate_all()`

Auto-detect and consolidate all merge candidates in one pass.

```python
store.consolidate_all() -> BulkConsolidateReport
```

*No parameters.*

**Returns:** `BulkConsolidateReport` — .clusters_merged, .total_frames_merged, .total_redirects, .total_new_frames.

**Raises:** `RuntimeError` if the store is sealed/closed.

---

## Typed Structural Edges

### `store.declare_relation(name, flags=1)`

Declare a named structural relation type. Returns the slot ID.

```python
store.declare_relation(name, flags=1) -> int
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `name` | `str` | yes | -- | Relation name (e.g. `"references"`, `"contradicts"`). |
| `flags` | `int` | no | `1` | Relation flag bitfield (see `RelationFlags`); `1` is Active. |

**Returns:** `int` — the slot ID (u16).

**Raises:** `RuntimeError` on duplicate name or a closed store.

### `store.list_relations()`

List all declared relation types.

```python
store.list_relations() -> list[RelationDef]
```

*No parameters.*

**Returns:** `list[RelationDef]` — each with .slot, .name, .flags, .builtin, .active, .deprecated.

**Raises:** `RuntimeError` if the store is closed.

### `store.assert_relation(from_id, to_id, relation, weight=1.0)`

Assert a typed edge between two frames.

```python
store.assert_relation(from_id, to_id, relation, weight=1.0) -> None
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `from_id` | `str` | yes | -- | Source frame ID. |
| `to_id` | `str` | yes | -- | Target frame ID. |
| `relation` | `str` | yes | -- | Declared relation name. |
| `weight` | `float` | no | `1.0` | Edge weight. |

**Returns:** `None`.

**Raises:** `RuntimeError` if a frame is missing, the relation is undeclared, or the store is closed.

### `store.retract_relation(from_id, to_id, relation)`

Retract a typed edge.

```python
store.retract_relation(from_id, to_id, relation) -> None
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `from_id` | `str` | yes | -- | Source frame ID. |
| `to_id` | `str` | yes | -- | Target frame ID. |
| `relation` | `str` | yes | -- | Relation name. |

**Returns:** `None`.

**Raises:** `RuntimeError` if the store is closed.

### `store.sim_neighbors(frame_id)`

Similarity neighbours of a frame: the live edge set. Includes edges other frames aimed at this one, so it is the traversable neighbourhood rather than the frame's own choice; for that, see `committed_sim_edges`.

```python
store.sim_neighbors(frame_id) -> list[tuple[str, float]]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `frame_id` | `str` | yes | -- | Frame whose neighbours to list. |

**Returns:** `list[tuple[str, float]]` — (frame_id, weight) pairs; empty for an unknown frame.

**Raises:** `RuntimeError` on a closed store.

### `store.committed_sim_edges(frame_id)`

The similarity neighbours this frame chose when it was committed, resolved to ids. Always a subset of `sim_neighbors`. `None` for a frame written before the choice was recorded.

```python
store.committed_sim_edges(frame_id) -> list[tuple[str, float]] | None
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `frame_id` | `str` | yes | -- | Frame whose committed choice to read. |

**Returns:** `list[tuple[str, float]] | None` — (frame_id, weight) pairs; None if the frame predates the recording.

**Raises:** `RuntimeError` on a closed store.

### `store.structural_neighbors(frame_id, relations, direction="outgoing")`

Find neighbours via typed edges.

```python
store.structural_neighbors(frame_id, relations, direction="outgoing") -> list[StructuralHit]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `frame_id` | `str` | yes | -- | Source frame. |
| `relations` | `list[str]` | yes | -- | Relation names to traverse. |
| `direction` | `str` | no | `"outgoing"` | `"outgoing"`, `"incoming"`, or `"both"`. |

**Returns:** `list[StructuralHit]` — each with .frame_id, .relation_slot, .weight, .direction.

**Raises:** `RuntimeError` on an invalid direction or a closed store.

### `store.structural_expand(hits, relations, direction="outgoing", score_combine="keep_input")`

Expand an existing (frame_id, score) hit list by one hop along structural relations.

```python
store.structural_expand(hits, relations, direction="outgoing", score_combine="keep_input") -> list[tuple[str, float]]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `hits` | `list[tuple[str, float]]` | yes | -- | Input (frame_id, score) hits. |
| `relations` | `list[str]` | yes | -- | Relation names to traverse. |
| `direction` | `str` | no | `"outgoing"` | `"outgoing"`, `"incoming"`, or `"both"`. |
| `score_combine` | `str` | no | `"keep_input"` | `"multiply"`, `"min"`, `"max"`, or `"keep_input"`. |

**Returns:** `list[tuple[str, float]]`.

**Raises:** `RuntimeError` on an invalid direction/score_combine or a closed store.

### `store.filter_by_relation(hits, relations, direction="outgoing", require_present=True)`

Filter a (frame_id, score) hit list to those participating in any of the given relations.

```python
store.filter_by_relation(hits, relations, direction="outgoing", require_present=True) -> list[tuple[str, float]]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `hits` | `list[tuple[str, float]]` | yes | -- | Input (frame_id, score) hits. |
| `relations` | `list[str]` | yes | -- | Relation names. |
| `direction` | `str` | no | `"outgoing"` | `"outgoing"`, `"incoming"`, or `"both"`. |
| `require_present` | `bool` | no | `True` | Keep hits participating in the relation (`True`) or exclude them. |

**Returns:** `list[tuple[str, float]]`.

**Raises:** `RuntimeError` on an invalid direction or a closed store.

### `SeraphStore.intersect_hits(a, b)`

Intersect two (frame_id, score) hit lists, keeping the lower score for each shared frame.

```python
SeraphStore.intersect_hits(a, b) -> list[tuple[str, float]]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `a` | `list[tuple[str, float]]` | yes | -- | First hit list. |
| `b` | `list[tuple[str, float]]` | yes | -- | Second hit list. |

**Returns:** `list[tuple[str, float]]`.

**Raises:** Does not raise (pure helper).

### `SeraphStore.union_hits(a, b)`

Union two (frame_id, score) hit lists, keeping the higher score for each frame.

```python
SeraphStore.union_hits(a, b) -> list[tuple[str, float]]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `a` | `list[tuple[str, float]]` | yes | -- | First hit list. |
| `b` | `list[tuple[str, float]]` | yes | -- | Second hit list. |

**Returns:** `list[tuple[str, float]]`.

**Raises:** Does not raise (pure helper).

---

## Snapshot / Temporal

### `store.snapshot(at_timestamp_us)`

Frame IDs active at the given timestamp (microseconds).

```python
store.snapshot(at_timestamp_us) -> SnapshotView
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `at_timestamp_us` | `int` | yes | -- | Point in time, microseconds since the epoch. |

**Returns:** `SnapshotView` — .frame_ids, .timestamp.

**Raises:** `RuntimeError` if the store is closed.

### `store.snapshot_search(snapshot, query_embedding, top_k)`

Brute-force cosine search within a snapshot.

```python
store.snapshot_search(snapshot, query_embedding, top_k) -> list[tuple[str, float]]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `snapshot` | `SnapshotView` | yes | -- | A snapshot view from `snapshot()`. |
| `query_embedding` | `list[float]` | yes | -- | Query vector. |
| `top_k` | `int` | yes | -- | Maximum number of results. |

**Returns:** `list[tuple[str, float]]` — (frame_id, score) tuples.

**Raises:** `RuntimeError` if the store is closed.

---

## Provenance & Sealing

### `store.is_sealed()`

Whether the store has been sealed (read-only). All tiers can open and read sealed stores.

```python
store.is_sealed() -> bool
```

*No parameters.*

**Returns:** `bool`.

**Raises:** `RuntimeError` if the store is closed.

### `store.seal(reason=None, anchor_backend=None, anchor_token=None)`

Seal the store: commit a final sentinel and refuse further writes. The seal records the order digest of every frame before it (see `order_digest`), so `verify` detects any frame reordered, removed or added before the seal, and any frame after it. To anchor the seal externally: stop writing, take `order_digest()`, anchor its 32 bytes with your backend, then seal with `anchor_backend` and `anchor_token`; the engine records the token and never contacts the backend. Requires Commercial or higher.

```python
store.seal(reason=None, anchor_backend=None, anchor_token=None) -> str
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `reason` | `str \| None` | no | `None` | Free-text reason recorded in the seal. |
| `anchor_backend` | `str \| None` | no | `None` | Anchor backend name (e.g. `"opentimestamps"`, `"rfc3161"`). |
| `anchor_token` | `bytes \| None` | no | `None` | The backend's token over the order digest. |

**Returns:** `str` — the seal sentinel frame ID.

**Raises:** `RuntimeError` if the store is already sealed or the tier does not support sealing; `ValueError` if only one of `anchor_backend` / `anchor_token` is given.

### `store.order_digest(through_frame_id=None)`

The digest to anchor: a running SHA-256 over the watermarks in commit order (`d0 = SHA256("SERAPH:ORDER:v1")`, `d_i = SHA256(d_{i-1} || watermark_i)`), through `through_frame_id` or the whole store. From format 4 a watermark binds a frame's content, lineage, id, commit time and committed edges, so the 32 bytes fix all of them and their order. Anchor them, then record the token with `append_tip_anchor` or `seal`.

```python
store.order_digest(through_frame_id=None) -> tuple[int, bytes]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `through_frame_id` | `str \| None` | no | `None` | Last frame covered; `None` for the whole log. |

**Returns:** `tuple[int, bytes]` — (frame_count, digest).

**Raises:** `RuntimeError` if the frame is not found or the store is closed.

### `store.anchor_claims()`

Every seal and tip anchor that commits to an order digest. `verify` checks each claim's digest against the log; the token is checked against the claim's `digest` by your anchor backend, directly or through `verify_attestation(anchor_verifier=...)`.

```python
store.anchor_claims() -> list[AnchorClaim]
```

*No parameters.*

**Returns:** `list[AnchorClaim]`.

**Raises:** `RuntimeError` if the store is closed.

### `store.writer_audit()`

Audit writer attribution across all frames.

```python
store.writer_audit() -> WriterAudit
```

*No parameters.*

**Returns:** `WriterAudit` — .writers, .genesis_customer_uuid, .cross_customer_writes, .unsigned_frame_count, .sealed, .seal_frame_id.

**Raises:** `RuntimeError` if the store is closed.

### `store.verify_attestation(public_key_resolver=None, anchor_verifier=None, verify_operator_identity=True)`

Full attestation audit: chain validity, genesis seed, operator identity, anchors, cross-customer writes.

```python
store.verify_attestation(public_key_resolver=None, anchor_verifier=None, verify_operator_identity=True) -> AttestationAudit
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `public_key_resolver` | `callable \| None` | no | `None` | `(signing_key_id: str) -> bytes`. Accepted for parity but not consulted by the default check. |
| `anchor_verifier` | `callable \| None` | no | `None` | `(backend: str, token: bytes, payload: bytes) -> bool`. When supplied, drives the `.anchor` field: called for the genesis anchor and for every seal and tip anchor with a token (payload: that claim's 32-byte order digest). |
| `verify_operator_identity` | `bool` | no | `True` | When `False`, skips operator-identity signature checks. |

**Returns:** `AttestationAudit`.

**Raises:** `RuntimeError` if the store is closed.

### `store.content_set_root()`

An order-free fingerprint of the store's retrievable content: sha256 over the sorted per-frame leaves `sha256(content_hash || codes)` under the header `"seraph-content-set/v1|<model_id>|<dim>|<n>"`. Two stores holding the same content under the same model give the same root regardless of commit order or graph. It ranges over every frame search can return; markers, redirects and superseded frames are excluded, and it is recomputable from the frame log alone.

```python
store.content_set_root() -> str
```

*No parameters.*

**Returns:** `str` — 64 hex characters.

**Raises:** `RuntimeError` if the store is closed.

### `store.genesis_seed()`

Raw genesis seed bytes (v2-attested stores only).

```python
store.genesis_seed() -> bytes | None
```

*No parameters.*

**Returns:** `bytes | None`.

**Raises:** `RuntimeError` if the store is closed.

### `store.genesis_attestation()`

Genesis attestation header as a JSON string.

```python
store.genesis_attestation() -> str | None
```

*No parameters.*

**Returns:** `str | None` — None for legacy stores.

**Raises:** `RuntimeError` if the store is closed.

### `seraph.migrate_store(src_path, dst_path, config=None, seed_data=[], seed_term=True)`

Rebuild a store into a fresh one at `dst_path`: every frame is re-committed with its identity intact (id, content, embedding, metadata, status references) and its parent chosen against the live destination, so references by id stay valid. Use it to correct an old topology or to carry a store to the current format or another `vector_precision` (`config`). Each source frame's commit time becomes its event time, so the rebuilt store's reads answer about its history; seeds in `seed_data` are created before the first frame. Watermarks are recomputed; the destination verifies.

```python
seraph.migrate_store(src_path, dst_path, config=None, seed_data=[], seed_term=True) -> dict[str, int]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `src_path` | `str \| PathLike` | yes | -- | Source `.sfg` path. Opened read-only. |
| `dst_path` | `str \| PathLike` | yes | -- | Destination `.sfg` path. Created fresh. |
| `config` | `StoreConfig \| None` | no | `None` | Destination configuration (its `vector_precision` re-cuts the store). `None` inherits the source's at the current format version. |
| `seed_data` | `list[tuple[str, list[float]]]` | no | `[]` | (label, embedding) pairs created as seeds on the destination before any frame is migrated. Not counted in the returned report. |
| `seed_term` | `bool` | no | `True` | `False` takes raw-cosine parents for the whole migration. |

**Returns:** `dict[str, int]` — keys content, seeds, sentinels, superseded, redirects, skipped, on_genesis.

**Raises:** `RuntimeError` if the source cannot be opened or the destination model does not match the source.

**Example**

```python
report = seraph.migrate_store("old.sfg", "new.sfg")
# {'content': 1335, 'seeds': 0, 'sentinels': 22, 'on_genesis': 1, ...}
```

### `seraph.reencode_store(src_path, dst_path, model_id, eigen_threshold, n_max=0, pbatch=2000, ebatch=16)`

Build a fresh store from the content of an existing one, re-encoding every frame with a new model. Frames are ingested in source commit order, so the destination's topology is the one a per-frame ingest would produce.

```python
seraph.reencode_store(src_path, dst_path, model_id, eigen_threshold, n_max=0, pbatch=2000, ebatch=16) -> int
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `src_path` | `str` | yes | -- | Source .sfg path. |
| `dst_path` | `str` | yes | -- | Destination .sfg path; must not exist. |
| `model_id` | `str` | yes | -- | Model to encode with; becomes the destination's model. |
| `eigen_threshold` | `int` | yes | -- | Eigenframe promotion threshold for the destination. |
| `n_max` | `int` | no | `0` | Stop after this many frames; 0 means all. |
| `pbatch` | `int` | no | `2000` | Write buffering batch size; does not affect topology. |
| `ebatch` | `int` | no | `16` | Encode batch size. |

**Returns:** `int` — frames ingested into the destination.

**Raises:** `RuntimeError` if the source cannot be opened, the destination cannot be created, or encoding fails.

---

## Anchoring

### `store.append_tip_anchor(backend_name, anchor_token, tip_frame_id, status, supersedes=None)`

Append a trusted-time anchor record to the chain tip. Requires a Commercial or Enterprise license.

```python
store.append_tip_anchor(backend_name, anchor_token, tip_frame_id, status, supersedes=None) -> str
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `backend_name` | `str` | yes | -- | Anchor backend (e.g. `"opentimestamps"`, `"rfc3161"`). |
| `anchor_token` | `bytes` | yes | -- | Proof/token bytes. |
| `tip_frame_id` | `str` | yes | -- | Frame ID this anchor covers. |
| `status` | `str` | yes | -- | `"pending"` or `"confirmed"`. |
| `supersedes` | `str \| None` | no | `None` | Frame ID of a previous anchor this one supersedes. |

**Returns:** `str` — the anchor frame ID.

**Raises:** `RuntimeError` on Free tier or a closed store.

### `store.list_tip_anchors(resolved=False)`

List tip anchor records as JSON strings. Available to all tiers.

```python
store.list_tip_anchors(resolved=False) -> list[str]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `resolved` | `bool` | no | `False` | Return only resolved anchors. |

**Returns:** `list[str]` — JSON strings.

**Raises:** `RuntimeError` if the store is closed.

---

## Events & Subscriptions

### `store.subscribe(event_name, callback)`

Alias for on: subscribe a callback to a store event. The callback receives the matching event payload object.

```python
store.subscribe(event_name, callback) -> int
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `event_name` | `str` | yes | -- | Event name to subscribe to. |
| `callback` | `Callable` | yes | -- | Called with the event payload object. |

**Returns:** `int` — subscriber ID, for unsubscribe.

**Raises:** `RuntimeError` if the store is closed. Callback exceptions are caught and printed to stderr; they do not interrupt the engine.

| Event | Payload | Payload fields |
|---|---|---|
| `"promote"` | `PromoteEvent` | `.frame_id`, `.in_degree`, `.trigger` (`"threshold"` \| `"manual"`) |
| `"contradiction"` | `ContradictionEvent` | `.new_frame`, `.contradicted_pairs`, `.population` |
| `"drift"` | `DriftEvent` | `.eigenframe_id`, `.coherence`, `.direction` |
| `"consolidation"` | `ConsolidateEvent` | `.merged_count`, `.new_frame_id`, `.redirect_frame_ids`, `.superseded_ids` |
| `"milestone"` | `MilestoneEvent` | `.kind`, `.detail` |
| `"*"` | any of the above | — |

### `store.on(event_name, callback)`

Subscribe a callback to a store event (promote, drift, contradiction, consolidate, milestone); `on` is an alias of `subscribe`.

```python
store.on(event_name, callback) -> int
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `event_name` | `str` | yes | -- | Event name. |
| `callback` | `callable` | yes | -- | Callable receiving one payload object. |

**Returns:** `int` — subscriber ID.

**Raises:** `RuntimeError` if the store is closed.

### `store.unsubscribe(subscriber_id)`

Detach an event subscriber by ID.

```python
store.unsubscribe(subscriber_id) -> bool
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `subscriber_id` | `int` | yes | -- | ID returned by subscribe. |

**Returns:** `bool` — whether the subscriber was present.

**Raises:** `RuntimeError` if the store is closed.

### `store.subscriber_count()`

Number of active subscribers.

```python
store.subscriber_count() -> int
```

*No parameters.*

**Returns:** `int`.

**Raises:** `RuntimeError` if the store is closed.

---

## Encoding

### `store.encode(text)`

Encode text with the store's own model into a normalized embedding. The encoder loads on first use and is kept for later calls.

```python
store.encode(text) -> list[float]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `text` | `str` | yes | -- | Text to encode. |

**Returns:** `list[float]`.

**Raises:** `RuntimeError` if encoder init/encode fails or the store is closed.

### `store.encode_batch(texts)`

Encode many texts with the store's own model in one batched pass.

```python
store.encode_batch(texts) -> list[list[float]]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `texts` | `list[str]` | yes | -- | Texts to encode. |

**Returns:** `list[list[float]]`.

**Raises:** `RuntimeError` if encoder init/encode fails or the store is closed.

### `store.quantize_query(embedding)`

The query vector exactly as the store scores it: the embedding after a round trip through the store's vector precision. Use it to reproduce scores outside the engine.

```python
store.quantize_query(embedding) -> list[float]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `embedding` | `list[float]` | yes | -- | Query vector of the store's dimension. |

**Returns:** `list[float]`.

**Raises:** `RuntimeError` if the store is closed.

### `store.encode_codes(embedding)`

The packed 4-bit codes the store would persist for an embedding: the bytes a frame's codes carry and content_set_root hashes. Lets a caller build a leaf for content it has not ingested.

```python
store.encode_codes(embedding) -> bytes
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `embedding` | `list[float]` | yes | -- | Vector of the store's dimension. |

**Returns:** `bytes` — empty on a dimension mismatch or a store that keeps no 4-bit codes.

**Raises:** `RuntimeError` if the store is closed.

### `Encoder(model_dir)`

A standalone encoder loaded from a local model directory. `from_pretrained` loads one by model ID instead.

```python
Encoder(model_dir) -> Encoder
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `model_dir` | `str` | yes | -- | Path to a local model directory. |

**Returns:** `Encoder`.

**Raises:** `RuntimeError` if the model cannot be loaded.

### `Encoder.from_pretrained(model_id)`

Load an encoder by model ID, downloading and caching the model files on first use.

```python
Encoder.from_pretrained(model_id) -> Encoder
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `model_id` | `str` | yes | -- | HuggingFace model ID. |

**Returns:** `Encoder`.

**Raises:** `RuntimeError` if the model cannot be downloaded/loaded.

---

## Batch Operations

### `store.begin_batch()`

Begin a buffered write batch. Writes skip the per-frame WAL and fsync until commit, which flushes once.

```python
store.begin_batch() -> None
```

*No parameters.*

**Returns:** `None`.

**Raises:** `RuntimeError` if the store is closed.

### `store.commit()`

End a buffered write batch: flush every buffered write to disk and restore per-write durability.

```python
store.commit() -> None
```

*No parameters.*

**Returns:** `None`.

**Raises:** `RuntimeError` if the store is closed or the flush fails.

---

## Value & Result Type Methods

These auxiliary value/result types carry callable helper methods in addition to
their fields (full field lists in [Types Reference](#types-reference)). Every
method below is validated against `py.rs`.

### `Frame`

Status predicates, each taking no arguments and returning `bool`:
`is_active_content()`, `is_eigenframe()`, `is_redirect()`, `is_consolidation_exempt()`.

### `RelationDef`

Flag predicates, each taking no arguments and returning `bool`:
`is_active()`, `is_deprecated()`, `is_bidirectional()`, `provenance_required()`.

### `RelationMask`

Set-style operations over typed-edge relation slots. Instance methods:
`contains(slot)` → `bool`, `is_empty()` → `bool`, `union(other)` → `RelationMask`,
`intersect(other)` → `RelationMask`. Static constructors:
`from_slots(slots)`, `empty()`, `semantic_only()`, `structural_only()` → `RelationMask`.
See also the [`RelationMask(slots=[])`](#relationmaskslots) constructor.

### `NeighborhoodResult`

Accessors over the three-population neighborhood result:
`union()` → `list[str]` (all member frame IDs),
`ranked(weight_consensus, weight_geometric, weight_lineage)` → `list[str]`
(dedup'd, weight-sorted), `divergence()` → `dict`.

### `Encoder`

`encode(text)` → `list[float]`, `encode_batch(texts)` → `list[list[float]]`.
See the [`Encoder(model_dir)`](#encodermodel_dir) constructor and
[`Encoder.from_pretrained(model_id)`](#encoderfrom_pretrainedmodel_id) for construction.

### `WarpCalibration`

`summary()` → `str` (human-readable calibration summary),
`as_named_magnitudes()` → `dict` (the calibrated named-magnitude mapping).

---

## Types Reference

All types below are registered PyO3 classes (verified against `py.rs` getters).

| Type | Key Fields / Members |
|------|------|
| `Frame` | `.id`, `.parent_id`, `.status`, `.content`, `.embedding`, `.metadata` (dict), `.metadata_json` (str), `.watermark`, `.content_hash`, `.codes` (bytes; packed 4-bit codes, empty on pre-v3 frames), `.in_degree`, `.model_id`, `.timestamp`, `.status_ref`, `.status_sources`, `.customer_uuid`, `.writer_signature`, `.snippet`; methods `.is_active_content()`, `.is_consolidation_exempt()`, `.is_eigenframe()`, `.is_redirect()` |
| `Hit` | `.frame_id`, `.score`, `.confidence`, `.path_length`, `.snippet`, `.content`, `.frame` |
| `TraceEntry` | `.frame_id`, `.depth`, `.score`, `.visit_order`, `.source`, `.parent_id` |
| `History` | `.hits` (list of `HistoryHit`, oldest first), `.anchors` (search hits that chose the trees), `.doors` (the frames the lineages were entered at), `.visits` |
| `HistoryHit` | `.frame_id`, `.score` (raw cosine), `.timestamp` (commit time, us since epoch), `.depth` (steps from its door), `.path` (route from the door, door first) |
| `Chain` | `.frames`, `.frame_ids`, `.primitive`, `.segments`, `.coherence`, `.reached` (`False` when no route was found within the visit budget; the path then ends at the nearest frame reached); `.primitive` is `"geodesic"` (`"geodesic_forward"` for a forward chain) |
| `Segment` | `.frames`, `.frame_ids`, `.primitive`, `.transition_reason` |
| `ChainCoherence` | `.mean_gradient`, `.min_gradient`, `.fragmentation_events`, `.pass_strict`, `.pass_loose`, `.stretch` (route length over the direct distance between its ends; 1 = direct) |
| `StoreConfig` | (constructor fields above; all readable/writable) |
| `StoreStats` | `.total_frames`, `.active_frames`, `.eigenframes`, `.seeds`, `.redirects`, `.superseded`, `.model_id` |
| `QuantizationRange` | `.model_id`, `.low`, `.high`, `.dim` |
| `SearchOpts` | `.top_k`, `.tau_similarity`, `.max_depth`, `.include_superseded` |
| `RelationMask` | `.slots`; `.contains()`, `.is_empty()`, `.union()`, `.intersect()`; statics `.from_slots()`, `.empty()`, `.semantic_only()`, `.structural_only()` |
| `TypedEdgeView` | `.from_hash`, `.to_hash`, `.slot`, `.weight`, `.asserted_by_wm`, `.asserted_by` (alias), `.edge_flags`, `.edge_offset` |
| `NeighborhoodResult` | `.consensus`, `.geometric`, `.lineage`, `.tau_used`, `.hops_used`; methods `.union()`, `.ranked()`, `.divergence()` |
| `RegionHealth` | `.coherence`, `.direction`, `.candidate_split` |
| `SimilarityDistribution` | `.count`, `.mean`, `.p75`, `.p90`, `.p95`, `.p99` |
| `SnapshotView` | `.frame_ids`, `.timestamp` |
| `SeedEntry` | `.label`, `.frame_id` |
| `SubtreeInfo` | `.root`, `.root_status`, `.descendant_count`, `.eigenframe_count`, `.max_depth` |
| `DriftAlert` | `.frame_id`, `.frame_snippet`, `.mean_child_similarity`, `.drift_score`, `.child_count` |
| `ContradictionCandidate` | `.frame_a_id`, `.frame_b_id`, `.shared_parent_id`, `.divergence` |
| `ContradictionPair` | `.frame_a_id`, `.frame_b_id`, `.cosine`, `.shared_parent`, `.content_divergence` |
| `ContradictionReport` | `.geometric`, `.lineage`, `.consensus` (each `list[ContradictionPair]`) |
| `EvolutionStep` | `.frame_id`, `.timestamp`, `.depth`, `.similarity_to_seed`, `.snippet` |
| `MergeCluster` | `.frame_ids`, `.snippet`, `.count` |
| `ConsolidateReport` | `.merged_count`, `.new_frame_id`, `.redirect_frame_ids`, `.superseded_ids` |
| `BulkConsolidateReport` | `.clusters_merged`, `.total_frames_merged`, `.total_redirects`, `.total_new_frames` |
| `RelationDef` | `.slot`, `.name`, `.flags`, `.builtin`, `.active`, `.deprecated`; methods `.is_active()`, `.is_deprecated()`, `.is_bidirectional()`, `.provenance_required()` |
| `StructuralHit` | `.frame_id`, `.relation_slot`, `.weight`, `.direction` |
| `GradientPoint` | `.frame_id`, `.similarity`, `.delta` |
| `WriterAudit` | `.writers`, `.genesis_customer_uuid`, `.cross_customer_writes`, `.unsigned_frame_count`, `.sealed`, `.seal_frame_id` |
| `AttestationAudit` | `.chain_valid`, `.genesis_seed_matches`, `.anchor`, `.secure_mode`, `.sealed`, `.writers`, `.cross_customer_writes`, `.unsigned_frame_count`, `.invalid_signature_count`, `.post_seal_frame_count`, `.all_ok`, `.failures` |
| `Connection` | `.chain` (`Chain`), `.bridge` (frame id or `None` for a direct edge) |
| `AnchorClaim` | `.kind` (`"seal"` \| `"tip"`), `.sentinel_id`, `.frame_count`, `.digest` (32 bytes the token must bind), `.backend`, `.token` |
| `WarpSpec` | `.profile`, `.bound_k`, `.accumulation` (read-only getters) |
| `WarpCalibration` | `.dim`, `.model_id`, `.n_queries`, `.n_targets`, `.sweep`, `.action_floor`, `.routing`, `.sub_saturation`, `.saturation`, `.recommended_bound_k`; methods `.summary()`, `.as_named_magnitudes()` |
| `WarpCalibrationPoint` | `.magnitude`, `.mean_jaccard`, `.std_jaccard`, `.mean_hits`, `.zero_result_pct` |
| `Encoder` | `.encode()`, `.encode_batch()`, `.dim`, `.model_id`, `.modality`; static `.from_pretrained()` |
| `PromoteEvent` | `.frame_id`, `.in_degree`, `.trigger` |
| `ContradictionEvent` | `.new_frame`, `.contradicted_pairs`, `.population` |
| `DriftEvent` | `.eigenframe_id`, `.coherence`, `.direction` |
| `ConsolidateEvent` | `.merged_count`, `.new_frame_id`, `.redirect_frame_ids`, `.superseded_ids` |
| `MilestoneEvent` | `.kind`, `.detail` |

> Module-level constant: `seraph.NAMED_MAGNITUDES` — a `dict[str, float]` of bundled named magnitudes (`"action_floor"`, `"routing"`, `"sub_saturation"`, `"saturation"`).

---

## Enums Reference

| Enum | Values | Notes / methods |
|------|--------|------|
| `FrameStatus` | `Active`, `Eigenframe`, `Superseded`, `Redirect`, `Genesis`, `Seed`, `System` | Methods: `.is_consolidation_exempt()`, `.is_eigenframe()`, `.is_redirect()` |
| `Direction` | `Outgoing`, `Incoming`, `Both` | — |
| `ScoreCombine` | `Multiply`, `Min`, `Max`, `KeepInput` | — |
| `MergeStrategy` | `Geometric`, `Semantic` | `Semantic` raises — no bundled LLM synthesis |
| `RelationFlags` | `Active` (0x01), `Deprecated` (0x02), `Bidirectional` (0x04), `AsymmetricInverse` (0x08) | Bit values for `declare_relation(name, flags)` |
| `StoreTier` | `GPU` (0), `CPU` (1) | Exposed for parity; runtime behavior is CPU |

---

Copyright (c) 2025-2026 Tree D Interactive LLC. All rights reserved.
