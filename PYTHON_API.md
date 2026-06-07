# SERAPH Python API Reference

> Module: `seraph` (PyO3 native extension)
> Import: `import seraph`

This reference documents the Python API for the people who **call** SERAPH from
Python. The module is a PyO3 native extension — import it as `import seraph`.
Read *Authentication & Licensing* and *Conventions* first: they apply to every
function and are not repeated per entry. Every signature below was validated
against the PyO3 bindings in `seraph-rs/src/py.rs`.

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

### `SeraphStore.create(path, config, seed_data=[], writer_seed=None)`

Create a new store on disk (static method).

```python
SeraphStore.create(path, config, seed_data=[], writer_seed=None) -> SeraphStore
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `path` | `str` | yes | -- | Filesystem path for the `.sfg` file. |
| `config` | `StoreConfig` | yes | -- | Store configuration. |
| `seed_data` | `list[tuple[str, list[float]]]` | no | `[]` | Initial seeds as `(label, embedding)` tuples. |
| `writer_seed` | `bytes \| None` | no | `None` | 32-byte Ed25519 private seed supplied by the application (ADR-0001). When provided, the store is secure and every frame is signed by this key; the seed never leaves the caller. |

**Returns:** `SeraphStore` — handle to the new store.

**Raises:** `RuntimeError` if the path is unwritable, the store already exists, the config is invalid, or (when `writer_seed` is set) the license tier does not permit secure mode. `RuntimeError` if `writer_seed` is not exactly 32 bytes.

**Example**

```python
config = seraph.StoreConfig(model_id="BAAI/bge-small-en-v1.5")
store = seraph.SeraphStore.create("./knowledge.sfg", config)
```

### `SeraphStore.open(path, writer_seed=None)`

Open an existing unencrypted store (static method).

```python
SeraphStore.open(path, writer_seed=None) -> SeraphStore
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `path` | `str` | yes | -- | Path to an existing `.sfg` file. |
| `writer_seed` | `bytes \| None` | no | `None` | 32-byte Ed25519 private seed supplying the writer identity for frames appended during this session (ADR-0001). `None` opens for read/verify only; secure appends require a writer. |

**Returns:** `SeraphStore`.

**Raises:** `RuntimeError` if the file is missing, corrupt, or **encrypted** (use `open_encrypted`), or if `writer_seed` is not exactly 32 bytes.

**Example**

```python
store = seraph.SeraphStore.open("./knowledge.sfg")
```

### `SeraphStore.open_encrypted(path, passphrase)`

Open an encrypted store with a passphrase (static method).

```python
SeraphStore.open_encrypted(path, passphrase) -> SeraphStore
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `path` | `str` | yes | -- | Path to an existing encrypted `.sfg` file. |
| `passphrase` | `str` | yes | -- | Decryption passphrase. |

**Returns:** `SeraphStore`.

**Raises:** `RuntimeError` on wrong passphrase, missing file, or if the store is not encrypted.

### `store.close()`

Close the store and flush all pending writes. The store object raises `RuntimeError` on any subsequent operation.

*No parameters.*

**Returns:** `None`.

**Raises:** Does not raise; closing an already-closed store is a no-op.

### `store.flush()`

Flush all buffered writes to disk without closing. (`#[pyo3(name = "flush")]`.)

*No parameters.*

**Returns:** `None`.

**Raises:** `RuntimeError` if the store is closed or the underlying write fails.

### `store.set_fast_mode(enabled)`

Enable/disable fast ingest mode (skip per-frame WAL + fsync). (`#[pyo3(name = "set_fast_mode")]`.)

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `enabled` | `bool` | yes | -- | `True` to defer WAL/fsync; `False` to restore per-write durability. |

**Returns:** `None`.

**Raises:** `RuntimeError` if the store is closed.

### `store.set_search_visit_cap_multiplier(m)`

Set the Phase-2 BFS visit-cap multiplier at runtime (`max_visited = top_k * m`, default 5). Higher values widen the search frontier at minimal latency cost.

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `m` | `int` | yes | -- | New visit-cap multiplier. |

**Returns:** `None`.

**Raises:** `RuntimeError` if the store is closed.

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

### `StoreConfig(model_id="BAAI/bge-small-en-v1.5", eigen_threshold=5, eigen_min_age_s=60.0, tau_similarity=0.5, tau_merge=0.92, max_index_elements=100000, custom_range=None, search_max_depth=50, search_visit_cap_multiplier=5, redirect_per_frame=True, secure=None, eigen_matmul_min=256, encryption_passphrase=None)`

Store configuration object.

```python
StoreConfig(...) -> StoreConfig
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `model_id` | `str` | no | `"BAAI/bge-small-en-v1.5"` | Embedding model identifier (permanent for the store). |
| `eigen_threshold` | `int` | no | `5` | In-degree threshold for eigenframe promotion. |
| `eigen_min_age_s` | `float` | no | `60.0` | Minimum age (seconds) before promotion. |
| `tau_similarity` | `float` | no | `0.5` | Default similarity threshold. |
| `tau_merge` | `float` | no | `0.92` | Consolidation merge threshold. |
| `max_index_elements` | `int` | no | `100000` | Max elements in the graph index. |
| `custom_range` | `QuantizationRange \| None` | no | `None` | Custom quantization range for unknown models; `None` looks up the bundled table by `model_id`. |
| `search_max_depth` | `int` | no | `50` | Maximum BFS depth during search. |
| `search_visit_cap_multiplier` | `int` | no | `5` | Visit-cap multiplier for search. |
| `redirect_per_frame` | `bool` | no | `True` | Emit one redirect per superseded source frame. |
| `secure` | `str \| None` | no | `None` | Security posture: `"off"`, `"standard"` (alias `"on"`), or `None` (auto-detect). |
| `eigen_matmul_min` | `int` | no | `256` | Minimum eigenframes for the matrix GEMV scan (`0` = always). |
| `encryption_passphrase` | `str \| None` | no | `None` | Passphrase for AES-256-GCM encryption at rest. |

**Returns:** `StoreConfig`. All fields are readable/writable attributes.

**Raises:** `ValueError` if `secure` is set to an unrecognized string (expected `"off"` or `"standard"`/`"on"`).

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

**Returns:** `QuantizationRange`. Fields `.model_id`, `.low`, `.high`, `.dim` are readable/writable.

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

Static constructors: `RelationMask.from_slots(slots)`, `RelationMask.empty()`,
`RelationMask.semantic_only()` (PARENT_OF + SIMILAR_TO),
`RelationMask.structural_only()` (empty — union user slots in as you go).
Instance API: `.slots` (getter, `list[int]`), `.contains(slot) -> bool`,
`.is_empty() -> bool`, `.union(other) -> RelationMask`,
`.intersect(other) -> RelationMask`.

---

## Ingestion

### `store.put(content, embedding)`

Ingest content with a pre-computed embedding.

```python
store.put(content, embedding) -> str
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `content` | `bytes` | yes | -- | Raw content bytes. |
| `embedding` | `list[float]` | yes | -- | Embedding vector; length must equal the store dimension. |

**Returns:** `str` — the new frame ID.

**Raises:** `RuntimeError` on dimension mismatch, a sealed store, or a closed store.

**Example**

```python
emb = store.encode("the cat sat on the mat")
fid = store.put(b"the cat sat on the mat", emb)
```

### `store.put_with_metadata(content, embedding, metadata_json="{}")`

Ingest content with an embedding and JSON metadata.

```python
store.put_with_metadata(content, embedding, metadata_json="{}") -> str
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `content` | `bytes` | yes | -- | Raw content bytes. |
| `embedding` | `list[float]` | yes | -- | Embedding vector. |
| `metadata_json` | `str` | no | `"{}"` | JSON object string of metadata key-value pairs. |

**Returns:** `str` — the new frame ID.

**Raises:** `RuntimeError` as `put`, plus on invalid JSON.

### `store.put_with_embedding(content, embedding)`

Alias for [`put`](#storeputcontent-embedding) (Python wheel parity).

```python
store.put_with_embedding(content, embedding) -> str
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `content` | `bytes` | yes | -- | Raw content bytes. |
| `embedding` | `list[float]` | yes | -- | Embedding vector. |

**Returns:** `str` — the new frame ID.

**Raises:** `RuntimeError` as `put`.

### `store.put_batch(items)`

Two-phase batch ingest: all items see the same pre-batch state for parent selection, eliminating first-mover advantage.

```python
store.put_batch(items) -> list[str]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `items` | `list[tuple[bytes, list[float]]]` | yes | -- | List of `(content, embedding)` tuples. |

**Returns:** `list[str]` — frame IDs, one per item.

**Raises:** `RuntimeError` if any embedding dimension mismatches, or the store is sealed/closed.

### `store.supersede(frame_id)`

Soft-forget a single frame: flips its status to `Superseded` in place (status is not part of the watermark). Creates no new frame and no redirect, and does not reparent children.

```python
store.supersede(frame_id) -> str
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `frame_id` | `str` | yes | -- | Frame ID to supersede. |

**Returns:** `str` — the same `frame_id` echoed back.

**Raises:** `RuntimeError` if the frame is missing, consolidation-exempt, or the store is closed.

---

## Search

### `store.search(query_embedding, top_k=10, tau=0.5)`

Graph-native ANN search for the nearest frames to a query embedding.

```python
store.search(query_embedding, top_k=10, tau=0.5) -> list[Hit]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `query_embedding` | `list[float]` | yes | -- | Query vector. |
| `top_k` | `int` | no | `10` | Maximum number of hits. |
| `tau` | `float` | no | `0.5` | Minimum similarity threshold. |

**Returns:** `list[Hit]` — each with `.frame_id`, `.score`, `.confidence`, `.path_length`, `.snippet`, `.content`, and `.frame` (the resolved `Frame`).

**Raises:** `RuntimeError` on dimension mismatch or a closed store.

> Plain `search()` does **not** apply tier pre-filtering. Use `search_with_opts`, `search_by_vector`, or `search_text` for tier control.

**Example**

```python
q = store.encode("feline animals")
for hit in store.search(q, top_k=5, tau=0.4):
    print(hit.frame_id, hit.score, hit.snippet)
```

### `store.search_with_opts(query_embedding, opts)`

Search using a `SearchOpts` dataclass. Honors `top_k`, `tau_similarity`, and `tier` / `tier_auto_threshold`.

```python
store.search_with_opts(query_embedding, opts) -> list[Hit]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `query_embedding` | `list[float]` | yes | -- | Query vector. |
| `opts` | `SearchOpts` | yes | -- | Search options. |

**Returns:** `list[Hit]`.

**Raises:** `RuntimeError` on dimension mismatch or a closed store.

### `store.search_by_vector(query_embedding, opts=None)`

Search by a pre-computed query vector. With `opts=None`, behaves like `search(query_embedding, 10, 0.5)`; otherwise delegates to `search_with_opts`.

```python
store.search_by_vector(query_embedding, opts=None) -> list[Hit]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `query_embedding` | `list[float]` | yes | -- | Query vector. |
| `opts` | `SearchOpts \| None` | no | `None` | Search options, or `None` for defaults. |

**Returns:** `list[Hit]`.

**Raises:** `RuntimeError` on dimension mismatch or a closed store.

### `store.search_text(query, opts=None)`

Search by text. Encodes `query` via the store's built-in encoder (lazy-loaded from `model_id`), then runs the standard search.

```python
store.search_text(query, opts=None) -> list[Hit]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `query` | `str` | yes | -- | Query text. |
| `opts` | `SearchOpts \| None` | no | `None` | Search options, or `None` for defaults (`top_k=10`, `tau=0.5`). |

**Returns:** `list[Hit]`.

**Raises:** `RuntimeError` if encoder init fails or the store is closed.

### `store.search_with_trace(query_embedding, top_k=10, tau=0.5)`

Search returning both hits and the BFS visit trace.

```python
store.search_with_trace(query_embedding, top_k=10, tau=0.5) -> tuple[list[Hit], list[TraceEntry]]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `query_embedding` | `list[float]` | yes | -- | Query vector. |
| `top_k` | `int` | no | `10` | Maximum number of hits. |
| `tau` | `float` | no | `0.5` | Minimum similarity threshold. |

**Returns:** `tuple[list[Hit], list[TraceEntry]]` — score-ranked hits and visit-order trace.

**Raises:** `RuntimeError` on dimension mismatch or a closed store.

### `store.search_with_warp(query_embedding, targets, suppress, profile="bounded", bound_k=1.5, top_k=10, tau=0.5)`

Search with warp-field steering (target pull + suppression).

```python
store.search_with_warp(query_embedding, targets, suppress, profile="bounded", bound_k=1.5, top_k=10, tau=0.5) -> list[Hit]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `query_embedding` | `list[float]` | yes | -- | Query vector. |
| `targets` | `list[tuple[list[float], float]]` | yes | -- | `(embedding, magnitude)` pull pairs. |
| `suppress` | `list[tuple[list[float], float]]` | yes | -- | `(embedding, magnitude)` suppression pairs. |
| `profile` | `str` | no | `"bounded"` | `"routing"`, `"saturation"`, or `"bounded"`. |
| `bound_k` | `float` | no | `1.5` | Suppression-norm cap (used only by `"bounded"`). |
| `top_k` | `int` | no | `10` | Maximum number of hits. |
| `tau` | `float` | no | `0.5` | Minimum similarity threshold. |

**Returns:** `list[Hit]`.

**Raises:** `RuntimeError` on unrecognized profile, dimension mismatch, or a closed store.

### `store.search_with_warp_spec(query_embedding, spec, top_k=10, tau=0.5, magnitude_overrides=None)`

Search using a `WarpSpec` object. Resolves frame-id inputs and named magnitudes against the store's corpus and the bundled `NAMED_MAGNITUDES` table.

```python
store.search_with_warp_spec(query_embedding, spec, top_k=10, tau=0.5, magnitude_overrides=None) -> list[Hit]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `query_embedding` | `list[float]` | yes | -- | Query vector. |
| `spec` | `WarpSpec` | yes | -- | Warp specification. |
| `top_k` | `int` | no | `10` | Maximum number of hits. |
| `tau` | `float` | no | `0.5` | Minimum similarity threshold. |
| `magnitude_overrides` | `dict[str, float] \| None` | no | `None` | Overrides for named magnitudes. |

**Returns:** `list[Hit]`.

**Raises:** `RuntimeError` on unresolved frame-id/magnitude, dimension mismatch, or a closed store.

### `SearchOpts(top_k=10, tau_similarity=0.5, max_depth=50, include_superseded=False, tier=Tier.AUTO, tier_auto_threshold=10000)`

Ergonomic search-options dataclass.

```python
SearchOpts(top_k=10, tau_similarity=0.5, max_depth=50, include_superseded=False, tier=Tier.AUTO, tier_auto_threshold=10000) -> SearchOpts
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `top_k` | `int` | no | `10` | Maximum results. |
| `tau_similarity` | `float` | no | `0.5` | Similarity threshold. |
| `max_depth` | `int` | no | `50` | Accepted for parity; not honored (depth comes from `StoreConfig.search_max_depth`). |
| `include_superseded` | `bool` | no | `False` | Accepted for parity; not honored (`search` never returns superseded frames). |
| `tier` | `Tier` | no | `Tier.AUTO` | Pre-filter tier (`COARSE`/`MEDIUM`/`AUTO`-above-threshold apply quantized pre-filtering; `FINE`/`NONE` skip it). |
| `tier_auto_threshold` | `int` | no | `10000` | Frame count above which `AUTO` activates tiered pre-filtering. |

**Returns:** `SearchOpts`. All fields are readable/writable attributes.

> `SearchOpts` only takes effect through `search_with_opts`, `search_by_vector`, and `search_text`. Plain `search(...)` ignores tier pre-filtering.

---

## Frame Access

### `store.get_frame(frame_id)`

Get a frame by ID (full data including embedding and metadata).

```python
store.get_frame(frame_id) -> Frame | None
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `frame_id` | `str` | yes | -- | Frame ID to fetch. |

**Returns:** `Frame | None` — `None` if not found.

**Raises:** `RuntimeError` if the store is closed.

### `store.get(frame_id)`

Alias for [`get_frame`](#storeget_frameframe_id).

```python
store.get(frame_id) -> Frame | None
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `frame_id` | `str` | yes | -- | Frame ID to fetch. |

**Returns:** `Frame | None`.

**Raises:** `RuntimeError` if the store is closed.

### `store.frame_ids()`

All frame IDs in insertion order.

*No parameters.*

**Returns:** `list[str]`.

**Raises:** `RuntimeError` if the store is closed.

### `store.eigenframe_ids()`

IDs of eigenframes only.

*No parameters.*

**Returns:** `list[str]`.

**Raises:** `RuntimeError` if the store is closed.

### `store.eigenframes()`

Full `Frame` objects for all eigenframes.

*No parameters.*

**Returns:** `list[Frame]`.

**Raises:** `RuntimeError` if the store is closed.

### `store.children(frame_id)`

Child frames of a given frame, as full `Frame` objects.

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

Child frame IDs only (lighter than `children()`).

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

*No parameters.*

**Returns:** `str`.

**Raises:** `RuntimeError` if the store is closed.

### `store.dim()`

Embedding dimensionality.

*No parameters.*

**Returns:** `int`.

**Raises:** `RuntimeError` if the store is closed.

### `store.federation_vector()`

Mean of all eigenframe embeddings — a single vector representing the store's semantic center (federation routing).

*No parameters.*

**Returns:** `list[float] | None` — `None` if the store has no eigenframes.

**Raises:** `RuntimeError` if the store is closed.

### `store.stats()`

Store statistics.

*No parameters.*

**Returns:** `StoreStats` — `.total_frames`, `.active_frames`, `.eigenframes`, `.seeds`, `.redirects`, `.superseded`, `.model_id`.

**Raises:** `RuntimeError` if the store is closed.

### `store.verify()`

Verify watermark-chain integrity.

*No parameters.*

**Returns:** `tuple[bool, list[str]]` — `(ok, failures)`.

**Raises:** `RuntimeError` if the store is closed.

---

## Seeds & Promotion

### `store.add_seed(label, embedding, metadata_json="{}")`

Add a seed frame to a live store (immediately an eigenframe).

```python
store.add_seed(label, embedding, metadata_json="{}") -> str
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `label` | `str` | yes | -- | Human-readable seed label. |
| `embedding` | `list[float]` | yes | -- | Seed embedding vector. |
| `metadata_json` | `str` | no | `"{}"` | JSON metadata object. |

**Returns:** `str` — the seed frame ID.

**Raises:** `RuntimeError` on dimension mismatch, a sealed store, or a closed store.

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

### `store.promote_content(content, embedding, metadata_json="{}")`

Ingest content and immediately promote it to eigenframe (atomic ingest + promote).

```python
store.promote_content(content, embedding, metadata_json="{}") -> str
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `content` | `bytes` | yes | -- | Content bytes. |
| `embedding` | `list[float]` | yes | -- | Embedding vector. |
| `metadata_json` | `str` | no | `"{}"` | JSON metadata object. |

**Returns:** `str` — the new eigenframe ID.

**Raises:** `RuntimeError` on dimension mismatch, a sealed store, or a closed store.

---

## Traversal & Chains

### `store.path(from_id, to_id)`

BFS path between two frames via parent-child adjacency.

```python
store.path(from_id, to_id) -> list[Frame]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `from_id` | `str` | yes | -- | Start frame ID. |
| `to_id` | `str` | yes | -- | End frame ID. |

**Returns:** `list[Frame]` — ordered `from_id → to_id`, empty if unreachable.

**Raises:** `RuntimeError` if the store is closed.

### `store.chain(from_id, to_id)`

Semantic chain between two frame IDs (parent-tree or similarity walk).

```python
store.chain(from_id, to_id) -> Chain
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `from_id` | `str` | yes | -- | Start frame ID. |
| `to_id` | `str` | yes | -- | End frame ID. |

**Returns:** `Chain`.

**Raises:** `RuntimeError` if the store is closed.

### `store.chain_to(query_embedding)`

Chain from the nearest frame to a query embedding.

```python
store.chain_to(query_embedding) -> Chain
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `query_embedding` | `list[float]` | yes | -- | Target query vector. |

**Returns:** `Chain`.

**Raises:** `RuntimeError` if the store is closed.

### `store.chain_between(emb_a, emb_b)`

Chain between two points in embedding space.

```python
store.chain_between(emb_a, emb_b) -> Chain
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `emb_a` | `list[float]` | yes | -- | First embedding. |
| `emb_b` | `list[float]` | yes | -- | Second embedding. |

**Returns:** `Chain`.

**Raises:** `RuntimeError` if the store is closed.

### `store.chain_to_with_warp(query_embedding, targets, suppress, profile="bounded", bound_k=1.5)`

Warped `chain_to`.

```python
store.chain_to_with_warp(query_embedding, targets, suppress, profile="bounded", bound_k=1.5) -> Chain
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `query_embedding` | `list[float]` | yes | -- | Target query vector. |
| `targets` | `list[tuple[list[float], float]]` | yes | -- | `(embedding, magnitude)` pull pairs. |
| `suppress` | `list[tuple[list[float], float]]` | yes | -- | `(embedding, magnitude)` suppression pairs. |
| `profile` | `str` | no | `"bounded"` | `"routing"`, `"saturation"`, or `"bounded"`. |
| `bound_k` | `float` | no | `1.5` | Suppression-norm cap. |

**Returns:** `Chain`.

**Raises:** `RuntimeError` on unrecognized profile, dimension mismatch, or a closed store.

### `store.chain_between_with_warp(emb_a, emb_b, targets, suppress, profile="bounded", bound_k=1.5)`

Warped `chain_between`.

```python
store.chain_between_with_warp(emb_a, emb_b, targets, suppress, profile="bounded", bound_k=1.5) -> Chain
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `emb_a` | `list[float]` | yes | -- | First embedding. |
| `emb_b` | `list[float]` | yes | -- | Second embedding. |
| `targets` | `list[tuple[list[float], float]]` | yes | -- | `(embedding, magnitude)` pull pairs. |
| `suppress` | `list[tuple[list[float], float]]` | yes | -- | `(embedding, magnitude)` suppression pairs. |
| `profile` | `str` | no | `"bounded"` | `"routing"`, `"saturation"`, or `"bounded"`. |
| `bound_k` | `float` | no | `1.5` | Suppression-norm cap. |

**Returns:** `Chain`.

**Raises:** `RuntimeError` on unrecognized profile, dimension mismatch, or a closed store.

### `store.lineage(frame_id)`

Parent chain back to genesis: `[frame, parent, ..., GENESIS]`.

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

Semantic gradient along an ordered list of frame IDs — one `GradientPoint` per step.

```python
store.gradient(chain) -> list[GradientPoint]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `chain` | `list[str]` | yes | -- | Ordered frame IDs. |

**Returns:** `list[GradientPoint]` — each with `.frame_id`, `.similarity`, `.delta`.

**Raises:** `RuntimeError` if the store is closed.

### `store.constitutive_score(chain)`

`1 - jaccard(chain_intermediates, midpoint_NN)` for an ordered frame-ID list. Values below `0.20` indicate the chain carries constitutive information beyond a midpoint search.

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

**Returns:** `list[SubtreeInfo]` — each with `.root`, `.root_status`, `.descendant_count`, `.eigenframe_count`, `.max_depth`.

**Raises:** `RuntimeError` if the store is closed.

---

## Neighborhood

### `store.neighborhood(frame_id, tau=None, hops=2)`

Three-population neighborhood exploration.

```python
store.neighborhood(frame_id, tau=None, hops=2) -> NeighborhoodResult
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `frame_id` | `str` | yes | -- | Center frame ID. |
| `tau` | `float \| str \| None` | no | `None` | `None` (p95), `"loose"` (p75), `"strict"` (p99), or an explicit float. |
| `hops` | `int` | no | `2` | Traversal radius in hops. |

**Returns:** `NeighborhoodResult`.

**Raises:** `RuntimeError` on an invalid `tau` string or a closed store.

### `store.neighborhood_with_warp(frame_id, targets, suppress, profile="bounded", bound_k=1.5, tau=0.5, hops=2)`

Neighborhood exploration with a warp field applied.

```python
store.neighborhood_with_warp(frame_id, targets, suppress, profile="bounded", bound_k=1.5, tau=0.5, hops=2) -> NeighborhoodResult
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `frame_id` | `str` | yes | -- | Center frame ID. |
| `targets` | `list[tuple[list[float], float]]` | yes | -- | `(embedding, magnitude)` pull pairs. |
| `suppress` | `list[tuple[list[float], float]]` | yes | -- | `(embedding, magnitude)` suppression pairs. |
| `profile` | `str` | no | `"bounded"` | `"routing"`, `"saturation"`, or `"bounded"`. |
| `bound_k` | `float` | no | `1.5` | Suppression-norm cap. |
| `tau` | `float` | no | `0.5` | Similarity threshold. |
| `hops` | `int` | no | `2` | Traversal radius. |

**Returns:** `NeighborhoodResult`.

**Raises:** `RuntimeError` on unrecognized profile, dimension mismatch, or a closed store.

> `NeighborhoodResult` fields: `.consensus`, `.geometric`, `.lineage` (each `list[str]`), `.tau_used`, `.hops_used`. Methods: `.union() -> list[str]`, `.ranked(weight_consensus=2.0, weight_geometric=1.0, weight_lineage=1.0) -> list[str]`, `.divergence() -> dict`.

---

## Warp (Steering)

> **Profiles:** `profile` accepts `"routing"`, `"saturation"`, or `"bounded"`. Target pull is always uncapped; only `"bounded"` caps the suppression norm (at `bound_k`).

### `store.apply_warp(query, targets, suppress, profile="bounded", bound_k=1.5)`

Apply a warp field to a query embedding without searching (pure vector math).

```python
store.apply_warp(query, targets, suppress, profile="bounded", bound_k=1.5) -> list[float]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `query` | `list[float]` | yes | -- | Query vector. |
| `targets` | `list[tuple[list[float], float]]` | yes | -- | `(embedding, magnitude)` pull pairs. |
| `suppress` | `list[tuple[list[float], float]]` | yes | -- | `(embedding, magnitude)` suppression pairs. |
| `profile` | `str` | no | `"bounded"` | `"routing"`, `"saturation"`, or `"bounded"`. |
| `bound_k` | `float` | no | `1.5` | Suppression-norm cap. |

**Returns:** `list[float]` — the warped query vector.

**Raises:** Nothing store-related (no handle access); profile/dimension issues are handled internally.

### `store.warp_query(query, spec, magnitude_overrides=None)`

Apply a `WarpSpec` to a query (no search) and return the warped vector. `query` is either a text string (auto-encoded) or a pre-computed embedding.

```python
store.warp_query(query, spec, magnitude_overrides=None) -> list[float]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `query` | `str \| list[float]` | yes | -- | Text (auto-encoded) or a pre-computed embedding. |
| `spec` | `WarpSpec` | yes | -- | Warp specification. |
| `magnitude_overrides` | `dict[str, float] \| None` | no | `None` | Overrides for named magnitudes. |

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

### `store.calibrate_magnitudes(n_queries=30, n_targets=3, magnitudes=None, top_k=10, seed=42)`

Alias for [`calibrate_warp`](#storecalibrate_warpn_queries30-n_targets3-magnitudesnone-top_k10-seed42) (Python wheel parity).

```python
store.calibrate_magnitudes(n_queries=30, n_targets=3, magnitudes=None, top_k=10, seed=42) -> WarpCalibration
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `n_queries` | `int` | no | `30` | Number of sampled queries. |
| `n_targets` | `int` | no | `3` | Number of sampled targets. |
| `magnitudes` | `list[float] \| None` | no | `None` | Magnitudes to sweep. |
| `top_k` | `int` | no | `10` | Hits per probe. |
| `seed` | `int` | no | `42` | RNG seed. |

**Returns:** `WarpCalibration`.

**Raises:** `RuntimeError` on calibration error or a closed store.

### `WarpSpec(targets=None, suppress=None, profile="bounded", bound_k=1.5, accumulation="reset")`

Warp specification dataclass.

```python
WarpSpec(targets=None, suppress=None, profile="bounded", bound_k=1.5, accumulation="reset") -> WarpSpec
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `targets` | `list[tuple] \| None` | no | `None` | `(input, magnitude)` pull pairs. |
| `suppress` | `list[tuple] \| None` | no | `None` | `(input, magnitude)` suppression pairs. |
| `profile` | `str` | no | `"bounded"` | `"routing"`, `"saturation"`, or `"bounded"`. |
| `bound_k` | `float` | no | `1.5` | Suppression-norm cap. |
| `accumulation` | `str` | no | `"reset"` | `"reset"` or `"cumulative"`. |

**Returns:** `WarpSpec`. Read-only getters: `.profile`, `.bound_k`, `.accumulation`.

Each `input` is a frame-id `str` or a `list[float]` embedding. Each `magnitude`
is a `float` or a named-magnitude `str` (`"action_floor"`, `"routing"`,
`"sub_saturation"`, `"saturation"`).

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

Seed margin: geometric distinctness relative to the corpus centroid.

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

Warp steerability: angular displacement at routing magnitude.

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

**Returns:** `RegionHealth` — `.coherence`, `.direction`, `.candidate_split`.

**Raises:** `RuntimeError` if the store is closed.

### `store.similarity_stats()`

Similarity distribution computed from parent-child relationships.

*No parameters.*

**Returns:** `SimilarityDistribution` — `.count`, `.mean`, `.p75`, `.p90`, `.p95`, `.p99`.

**Raises:** `RuntimeError` if the store is closed.

### `store.resolve_tau(tau=None)`

Resolve a tau value: `None` → p95, `"loose"` → p75, `"strict"` → p99, float → passthrough.

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

Scan eigenframes for semantic drift (children drifting away).

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

Three-population contradiction detection.

```python
store.contradictions_v2(tau=None) -> ContradictionReport
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `tau` | `float \| str \| None` | no | `None` | `None` (p95), `"loose"` (p75), `"strict"` (p99), or an explicit float. |

**Returns:** `ContradictionReport` — `.geometric`, `.lineage`, `.consensus` (each `list[ContradictionPair]`).

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

Rich evolution data: each step carries timestamp, depth, and similarity-to-seed.

```python
store.evolution_steps(seed_frame_id) -> list[EvolutionStep]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `seed_frame_id` | `str` | yes | -- | Seed frame ID. |

**Returns:** `list[EvolutionStep]` — each with `.frame_id`, `.timestamp`, `.depth`, `.similarity_to_seed`, `.snippet`.

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

**Returns:** `list[MergeCluster]` — each with `.frame_ids`, `.snippet`, `.count`.

**Raises:** `RuntimeError` if the store is closed.

### `store.consolidate(frame_ids)`

Consolidate a specific set of frames into one centroid frame.

```python
store.consolidate(frame_ids) -> ConsolidateReport
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `frame_ids` | `list[str]` | yes | -- | Frame IDs to consolidate. |

**Returns:** `ConsolidateReport` — `.merged_count`, `.new_frame_id`, `.redirect_frame_ids`, `.superseded_ids`.

**Raises:** `RuntimeError` if the store is sealed/closed.

### `store.consolidate_all()`

Auto-detect and consolidate all merge candidates in one pass.

*No parameters.*

**Returns:** `BulkConsolidateReport` — `.clusters_merged`, `.total_frames_merged`, `.total_redirects`, `.total_new_frames`.

**Raises:** `RuntimeError` if the store is sealed/closed.

### `store.consolidate_with_strategy(frame_ids, strategy)`

Consolidate with an explicit `MergeStrategy`.

```python
store.consolidate_with_strategy(frame_ids, strategy) -> ConsolidateReport
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `frame_ids` | `list[str]` | yes | -- | Frame IDs to consolidate. |
| `strategy` | `MergeStrategy` | yes | -- | `MergeStrategy.Geometric` (supported); `MergeStrategy.Semantic` raises. |

**Returns:** `ConsolidateReport`.

**Raises:** `RuntimeError` for `MergeStrategy.Semantic` (no bundled LLM synthesis), a sealed store, or a closed store.

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
| `flags` | `int` | no | `1` (`0x01`, Active) | Relation flag bitfield (see `RelationFlags`). |

**Returns:** `int` — the slot ID (`u16`).

**Raises:** `RuntimeError` on duplicate name or a closed store.

### `store.list_relations()`

List all declared relation types.

*No parameters.*

**Returns:** `list[RelationDef]` — each with `.slot`, `.name`, `.flags`, `.builtin`, `.active`, `.deprecated` (and methods `.is_active()`, `.is_deprecated()`, `.is_bidirectional()`, `.provenance_required()`).

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

### `store.structural_neighbors(frame_id, relations, direction="outgoing")`

Find neighbors via typed edges.

```python
store.structural_neighbors(frame_id, relations, direction="outgoing") -> list[StructuralHit]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `frame_id` | `str` | yes | -- | Source frame. |
| `relations` | `list[str]` | yes | -- | Relation names to traverse. |
| `direction` | `str` | no | `"outgoing"` | `"outgoing"`, `"incoming"`, or `"both"`. |

**Returns:** `list[StructuralHit]` — each with `.frame_id`, `.relation_slot`, `.weight`, `.direction`.

**Raises:** `RuntimeError` on an invalid direction or a closed store.

### `store.structural_expand(hits, relations, direction="outgoing", score_combine="keep_input")`

Pipeline composition: expand an existing `(frame_id, score)` hit list by one hop along structural relations.

```python
store.structural_expand(hits, relations, direction="outgoing", score_combine="keep_input") -> list[tuple[str, float]]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `hits` | `list[tuple[str, float]]` | yes | -- | Input `(frame_id, score)` hits. |
| `relations` | `list[str]` | yes | -- | Relation names to traverse. |
| `direction` | `str` | no | `"outgoing"` | `"outgoing"`, `"incoming"`, or `"both"`. |
| `score_combine` | `str` | no | `"keep_input"` | `"multiply"`, `"min"`, `"max"`, or `"keep_input"`. |

**Returns:** `list[tuple[str, float]]`.

**Raises:** `RuntimeError` on an invalid direction/score_combine or a closed store.

### `store.filter_by_relation(hits, relations, direction="outgoing", require_present=True)`

Filter a `(frame_id, score)` hit list to those participating in any of the given relations.

```python
store.filter_by_relation(hits, relations, direction="outgoing", require_present=True) -> list[tuple[str, float]]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `hits` | `list[tuple[str, float]]` | yes | -- | Input `(frame_id, score)` hits. |
| `relations` | `list[str]` | yes | -- | Relation names. |
| `direction` | `str` | no | `"outgoing"` | `"outgoing"`, `"incoming"`, or `"both"`. |
| `require_present` | `bool` | no | `True` | Keep hits participating in the relation (`True`) vs. excluding them. |

**Returns:** `list[tuple[str, float]]`.

**Raises:** `RuntimeError` on an invalid direction or a closed store.

### `SeraphStore.intersect_hits(a, b)`

Static helper: intersect two `(frame_id, score)` hit lists, keeping the min score per shared frame.

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

Static helper: union two `(frame_id, score)` hit lists, keeping the max score per frame.

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
| `at_timestamp_us` | `int` | yes | -- | Point-in-time, microseconds. |

**Returns:** `SnapshotView` — `.frame_ids`, `.timestamp`.

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

**Returns:** `list[tuple[str, float]]` — `(frame_id, score)` tuples.

**Raises:** `RuntimeError` if the store is closed.

---

## Provenance & Sealing

### `store.is_sealed()`

Whether the store has been sealed (read-only). All tiers can open and read sealed stores.

*No parameters.*

**Returns:** `bool`.

**Raises:** `RuntimeError` if the store is closed.

### `store.seal(reason=None)`

> **Requires Commercial or Enterprise license.** Free-tier calls raise `RuntimeError`.

Seal the store; no further writes are accepted.

```python
store.seal(reason=None) -> str
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `reason` | `str \| None` | no | `None` | Optional seal reason. |

**Returns:** `str` — the seal frame ID.

**Raises:** `RuntimeError` on Free tier or a closed store.

### `store.writer_audit()`

Audit writer attribution across all frames.

*No parameters.*

**Returns:** `WriterAudit` — `.writers`, `.genesis_customer_uuid`, `.cross_customer_writes`, `.unsigned_frame_count`, `.sealed`, `.seal_frame_id`.

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
| `anchor_verifier` | `callable \| None` | no | `None` | `(backend: str, token: bytes, payload: bytes) -> bool`. When supplied, drives the `.anchor` field. |
| `verify_operator_identity` | `bool` | no | `True` | When `False`, skips operator-identity signature checks. |

**Returns:** `AttestationAudit` — see [Types Reference](#types-reference).

**Raises:** `RuntimeError` if the store is closed.

### `store.genesis_seed()`

Raw genesis seed bytes (v2-attested stores only).

*No parameters.*

**Returns:** `bytes | None`.

**Raises:** `RuntimeError` if the store is closed.

### `store.genesis_attestation()`

Genesis attestation header as a JSON string.

*No parameters.*

**Returns:** `str | None` — `None` for legacy stores.

**Raises:** `RuntimeError` if the store is closed.

### `seraph.verify_chain(frames, genesis_seed=None)`

Module-level helper. Verify watermark-chain integrity across an ordered list of `Frame` objects (parent → child order).

```python
seraph.verify_chain(frames, genesis_seed=None) -> tuple[bool, str | None]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `frames` | `list[Frame]` | yes | -- | Frames in parent → child order. |
| `genesis_seed` | `bytes \| None` | no | `None` | Supply for v2-attested stores; omit to use the default `GENESIS_SEED_V1`. |

**Returns:** `tuple[bool, str | None]` — `(is_valid, first_failing_frame_id)`.

**Raises:** Does not raise (returns the failure tuple instead).

---

## Anchoring

### `store.append_tip_anchor(backend_name, anchor_token, tip_frame_id, status, supersedes=None)`

> **Requires Commercial or Enterprise license.** Free-tier calls raise `RuntimeError`.

Append a trusted-time anchor record to the chain tip.

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
| `resolved` | `bool` | no | `False` | Whether to return only resolved anchors. |

**Returns:** `list[str]` — JSON strings.

**Raises:** `RuntimeError` if the store is closed.

---

## Events & Subscriptions

### `store.subscribe(event_name, callback)`

Subscribe to substrate events. The callback is invoked synchronously with the matching payload object.

```python
store.subscribe(event_name, callback) -> int
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `event_name` | `str` | yes | -- | `"promote"`, `"contradiction"`, `"drift"`, `"consolidation"`, `"milestone"`, or `"*"`. |
| `callback` | `callable` | yes | -- | Callable receiving one payload object. |

**Returns:** `int` — subscriber ID.

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

Alias for [`subscribe`](#storesubscribeevent_name-callback).

```python
store.on(event_name, callback) -> int
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `event_name` | `str` | yes | -- | Event name (see `subscribe`). |
| `callback` | `callable` | yes | -- | Callable receiving one payload object. |

**Returns:** `int` — subscriber ID.

**Raises:** `RuntimeError` if the store is closed.

### `store.unsubscribe(subscriber_id)`

Detach a subscriber by ID.

```python
store.unsubscribe(subscriber_id) -> bool
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `subscriber_id` | `int` | yes | -- | ID returned by `subscribe`/`on`. |

**Returns:** `bool` — `True` if it was present.

**Raises:** `RuntimeError` if the store is closed.

### `store.subscriber_count()`

Number of active subscribers.

*No parameters.*

**Returns:** `int`.

**Raises:** `RuntimeError` if the store is closed.

---

## Encoding

### `store.encode(text)`

Encode text into a normalized embedding via the store's model. Lazily loads the encoder on first call.

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

Batch-encode multiple texts via the store's model (GPU-batched forward pass).

```python
store.encode_batch(texts) -> list[list[float]]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `texts` | `list[str]` | yes | -- | Texts to encode. |

**Returns:** `list[list[float]]`.

**Raises:** `RuntimeError` if encoder init/encode fails or the store is closed.

### `Encoder(model_dir)`

Standalone pure-Rust encoder from a local model directory.

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

Load an encoder from the HuggingFace Hub by model ID (downloads + caches) (static method).

```python
Encoder.from_pretrained(model_id) -> Encoder
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `model_id` | `str` | yes | -- | HuggingFace model ID. |

**Returns:** `Encoder`.

**Raises:** `RuntimeError` if the model cannot be downloaded/loaded.

**Encoder instance API:**

| Member | Type | Description |
|---|---|---|
| `.encode(text)` | `(str) -> list[float]` | Encode a single text (mean-pooled, L2-normalized). |
| `.encode_batch(texts)` | `(list[str]) -> list[list[float]]` | Batch encode. |
| `.dim` | getter `-> int` | Embedding dimensionality. |
| `.model_id` | getter `-> str \| None` | Model identifier. |
| `.modality` | getter `-> str` | `"text"`, `"image"`, `"audio"`, or `"multimodal"`. |

---

## Batch Operations

### `store.begin_batch()`

Begin a buffered write batch. Subsequent writes skip per-frame WAL + fsync until `commit()` (enables true batch mode with a single fsync at commit time).

*No parameters.*

**Returns:** `None`.

**Raises:** `RuntimeError` if the store is closed.

### `store.commit()`

Flush all buffered writes to disk and re-enable per-write fsync.

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
| `Frame` | `.id`, `.parent_id`, `.status`, `.content`, `.embedding`, `.metadata` (dict), `.metadata_json` (str), `.watermark`, `.content_hash`, `.in_degree`, `.model_id`, `.timestamp`, `.status_ref`, `.status_sources`, `.customer_uuid`, `.writer_signature`, `.snippet`; methods `.is_active_content()`, `.is_consolidation_exempt()`, `.is_eigenframe()`, `.is_redirect()` |
| `Hit` | `.frame_id`, `.score`, `.confidence`, `.path_length`, `.snippet`, `.content`, `.frame` |
| `TraceEntry` | `.frame_id`, `.depth`, `.score`, `.visit_order`, `.source`, `.parent_id` |
| `Chain` | `.frames`, `.frame_ids`, `.primitive`, `.segments`, `.coherence` |
| `Segment` | `.frames`, `.frame_ids`, `.primitive`, `.transition_reason` |
| `ChainCoherence` | `.mean_gradient`, `.min_gradient`, `.fragmentation_events`, `.pass_strict`, `.pass_loose` |
| `StoreConfig` | (constructor fields above; all readable/writable) |
| `StoreStats` | `.total_frames`, `.active_frames`, `.eigenframes`, `.seeds`, `.redirects`, `.superseded`, `.model_id` |
| `QuantizationRange` | `.model_id`, `.low`, `.high`, `.dim` |
| `SearchOpts` | `.top_k`, `.tau_similarity`, `.max_depth`, `.include_superseded`, `.tier`, `.tier_auto_threshold` |
| `RelationMask` | `.slots`; `.contains()`, `.is_empty()`, `.union()`, `.intersect()`; statics `.from_slots()`, `.empty()`, `.semantic_only()`, `.structural_only()` |
| `TypedEdgeView` | `.from_hash`, `.to_hash`, `.slot`, `.weight`, `.asserted_by_wm`, `.asserted_by` (alias), `.edge_flags`, `.edge_offset` |
| `NeighborhoodResult` | `.consensus`, `.geometric`, `.lineage`, `.tau_used`, `.hops_used`; methods `.union()`, `.ranked()`, `.divergence()` |
| `RegionHealth` | `.coherence`, `.direction`, `.candidate_split` |
| `SimilarityDistribution` | `.count`, `.mean`, `.p75`, `.p90`, `.p95`, `.p99` |
| `SnapshotView` | `.frame_ids`, `.timestamp` |
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
| `Tier` | `AUTO`, `COARSE`, `MEDIUM`, `FINE`, `NONE` | Search pre-filter tier (used via `SearchOpts`) || `Direction` | `Outgoing`, `Incoming`, `Both` | — |
| `ScoreCombine` | `Multiply`, `Min`, `Max`, `KeepInput` | — |
| `MergeStrategy` | `Geometric`, `Semantic` | `Semantic` raises — no bundled LLM synthesis |
| `RelationFlags` | `Active` (0x01), `Deprecated` (0x02), `Bidirectional` (0x04), `AsymmetricInverse` (0x08) | Bit values for `declare_relation(name, flags)` |
| `StoreTier` | `GPU` (0), `CPU` (1) | Exposed for parity; runtime behavior is CPU |

---

Copyright (c) 2025-2026 Tree D Interactive LLC. All rights reserved.
