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

### `SeraphStore.create(path, config, seed_data=[], writer_seed=None)`

Create a new store on disk.

```python
SeraphStore.create(path, config, seed_data=[], writer_seed=None) -> SeraphStore
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `path` | `str` | yes | -- | Filesystem path for the `.sfg` file. |
| `config` | `StoreConfig` | yes | -- | Store configuration. |
| `seed_data` | `list[tuple[str, list[float]]]` | no | `[]` | Initial seeds as (label, embedding) tuples. |
| `writer_seed` | `bytes \| None` | no | `None` | 32-byte Ed25519 private seed supplied by the application (ADR-0001). When provided, the store is secure and every frame is signed by this key; the seed never leaves the caller. |

**Returns:** `SeraphStore` — handle to the new store.

**Raises:** `RuntimeError` if the path is unwritable, the store already exists, the config is invalid, or (when `writer_seed` is set) the license tier does not permit secure mode. `RuntimeError` if `writer_seed` is not exactly 32 bytes.

**Example**

```python
config = seraph.StoreConfig(model_id="BAAI/bge-small-en-v1.5")
store = seraph.SeraphStore.create("./knowledge.sfg", config)
```

### `SeraphStore.open(path, writer_seed=None)`

Open an existing unencrypted store.

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

Open an encrypted store with a passphrase.

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

### `store.set_search_visit_cap_multiplier(m)`

Legacy visit budget for a search as a multiple of `top_k`, used only when the absolute budget (`set_search_max_visits`) is `0`. Default `20`. Runtime only.

```python
store.set_search_visit_cap_multiplier(m) -> None
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `m` | `int` | yes | -- | Multiple of `top_k` for the legacy visit budget. |

**Returns:** `None`.

**Raises:** `RuntimeError` if the store is closed.

### `store.set_seed_term(beta)`

Weight of the seed term in parent selection. When the store has seeds, a new frame's parent is chosen by its similarity to the candidate plus `beta` times its similarity to the candidate's seed, which settles near-ties between seed regions. It affects the parent only; the stored embedding, the similarity edges and search results are unchanged. Default `0.6`; `0` disables it. Per-frame opt-out: `seed_term=False` on the put calls. Runtime only, not persisted.

```python
store.set_seed_term(beta) -> None
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `beta` | `float` | yes | -- | Term weight (`>= 0`; `0` disables). |

**Returns:** `None`.

**Raises:** `RuntimeError` if the store is closed.

### `store.seed_term()`

Current weight of the commit-time seed term (see `set_seed_term`).

```python
store.seed_term() -> float
```

*No parameters.*

**Returns:** `float`.

**Raises:** `RuntimeError` if the store is closed.

### `store.set_search_ceiling_slack(slack)`

How far past the current top-k a search keeps looking before it stops, as a cosine distance below the k-th best score. Smaller values stop sooner; larger values find more on weak-match queries at more cost. Default `0.05`; `0.10` is the completeness setting. `0` uses the multiplicative margin instead (`set_search_ceiling_margin`). Runtime only, not persisted.

```python
store.set_search_ceiling_slack(slack) -> None
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `slack` | `float` | yes | -- | Cosine slack below the k-th best; `0` = multiplicative margin. |

**Returns:** `None`.

**Raises:** `RuntimeError` if the store is closed.

### `store.set_door_index(on)`

Use the door index for entry-point selection: an index over the eigenframes that replaces the full scan once the store has `door_index_min` of them (default `4096`). Default on. Entry selection is then approximate to the index's accuracy (see `set_door_index_ef`); replay and as-of reads always use the exact scan. Turning it on builds the index at once; off drops it. Runtime only, not persisted.

```python
store.set_door_index(on) -> None
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `on` | `bool` | yes | -- | `True` to pick entry points through the door index. |

**Returns:** `None`.

**Raises:** `RuntimeError` if the store is closed.

### `store.set_door_index_ef(ef)`

Candidates the door index examines per query. Higher is closer to the exact scan at more cost per query. Default `128`. Runtime only, not persisted.

```python
store.set_door_index_ef(ef) -> None
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `ef` | `int` | yes | -- | Candidates per query (at least the entry scan's 64). |

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

### `store.set_search_ceiling_margin(m)`

Multiplicative stop line for search, used only when the slack (`set_search_ceiling_slack`) is `0`: search stops once nothing left can score above `m` times the k-th best. Default `0.85`. Runtime only, not persisted.

```python
store.set_search_ceiling_margin(m) -> None
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `m` | `float` | yes | -- | Fraction of the k-th best; `1.0` is the tight stop. |

**Returns:** `None`.

**Raises:** `RuntimeError` if the store is closed.

### `store.set_commit_ceiling_margin(m)`

Stop line of the walk that picks a new frame's parent and similarity edges, as a fraction of the best score it holds. Lower values look further for a parent and cost ingest time; higher values ingest faster with slightly worse placement. Default `0.85`. Runtime only, not persisted.

```python
store.set_commit_ceiling_margin(m) -> None
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `m` | `float` | yes | -- | Ceiling slack; `1.0` is the tight stop. |

**Returns:** `None`.

**Raises:** `RuntimeError` if the store is closed.

### `store.set_commit_ceiling_slack(slack)`

Set the stop-line slack of commit-time placement, the cosine form of commit_ceiling_margin: placement stops once the best remaining candidate is no more than the k-th best minus slack. Default 0.05, the line search stops on; 0 falls back to the multiplicative margin. Runtime only; frames store their parent and edges, so changing it never forces a migration.

```python
store.set_commit_ceiling_slack(slack) -> None
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `slack` | `float` | yes | -- | Cosine distance below the k-th best candidate at which placement stops; 0 falls back to the margin. |

**Returns:** `None`.

**Raises:** `RuntimeError` if the store is closed or the operation fails.

### `store.commit_ceiling_margin()`

Current stop line of the commit walk (see `set_commit_ceiling_margin`).

```python
store.commit_ceiling_margin() -> float
```

*No parameters.*

**Returns:** `float`.

**Raises:** `RuntimeError` if the store is closed.

### `store.commit_ceiling_slack()`

The current stop-line slack of commit-time placement (default 0.05); 0 means the multiplicative margin is in use.

```python
store.commit_ceiling_slack() -> float
```

*No parameters.*

**Returns:** `float`.

**Raises:** `RuntimeError` if the store is closed or the operation fails.

### `store.set_sim_edge_policy(routing=False, parent=True, occlusion=0.0, pool=0)`

How a new frame chooses its similarity edges. By default the top `edge_budget` content frames above `tau_similarity`, the parent included and never a Genesis or Seed frame. `occlusion > 0` drops a candidate when a nearer kept neighbour already covers it (`1.0` strict; larger keeps more); `pool` widens the candidate list at ingest cost. Runtime only, not persisted; each frame records the edges it chose.

```python
store.set_sim_edge_policy(routing=False, parent=True, occlusion=0.0, pool=0) -> None
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `routing` | `bool` | no | `False` | Allow edges to Genesis/Seed frames. |
| `parent` | `bool` | no | `True` | Allow an edge to the frame's own parent. |
| `occlusion` | `float` | no | `0.0` | `0` = top-k; `> 0` = occlusion pruning with this alpha. |
| `pool` | `int` | no | `0` | Candidates the commit walk returns; `0` = `edge_budget + 1`. |

**Returns:** `None`.

**Raises:** `RuntimeError` if the store is closed.

### `store.sim_edge_policy()`

Current similarity-edge policy (see `set_sim_edge_policy`).

```python
store.sim_edge_policy() -> tuple[bool, bool, float, int]
```

*No parameters.*

**Returns:** `tuple[bool, bool, float, int]` — (routing, parent, occlusion, pool).

**Raises:** `RuntimeError` if the store is closed.

### `store.set_search_rank_mode(mode="cosine", share=0.3, doors=5, budget=30)`

How result slots are filled by `search` and the reads built on it. `"cosine"` (the default) ranks every slot by similarity. `"two_channel"` gives the last `round(share * top_k)` slots to frames reached by a descent from the `doors` nearest eigenframes, bounded by `budget` frames, that the similarity ranking lacks. Runtime only, not persisted.

```python
store.set_search_rank_mode(mode="cosine", share=0.3, doors=5, budget=30) -> None
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `mode` | `str` | no | `"cosine"` | `"cosine"` or `"two_channel"`. |
| `share` | `float` | no | `0.3` | Fraction of `top_k` given to the descent channel (two_channel only). |
| `doors` | `int` | no | `5` | Eigenframes the descent enters at (two_channel only). |
| `budget` | `int` | no | `30` | Frames the descent admits before stopping (two_channel only). |

**Returns:** `None`.

**Raises:** `ValueError` for an unknown mode; `RuntimeError` if the store is closed.

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

### `StoreConfig(model_id="BAAI/bge-small-en-v1.5", eigen_threshold=5, eigen_min_age_s=0.0, tau_similarity=0.5, tau_merge=0.92, max_index_elements=100000, custom_range=None, search_max_depth=50, search_visit_cap_multiplier=20, redirect_per_frame=True, secure=None, eigen_matmul_min=256, encryption_passphrase=None, search_use_stream=True, search_stream_threshold=50000, search_stream_beam=16, vector_precision="codes4", resident_precision=None)`

Store configuration, given to `create`. The model id and the precisions are fixed for the store's life; the search knobs are runtime defaults.

```python
StoreConfig(model_id="BAAI/bge-small-en-v1.5", eigen_threshold=5, eigen_min_age_s=0.0, tau_similarity=0.5, tau_merge=0.92, max_index_elements=100000, custom_range=None, search_max_depth=50, search_visit_cap_multiplier=20, redirect_per_frame=True, secure=None, eigen_matmul_min=256, encryption_passphrase=None, search_use_stream=True, search_stream_threshold=50000, search_stream_beam=16, vector_precision="codes4", resident_precision=None) -> StoreConfig
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
| `search_max_depth` | `int` | no | `50` | Maximum depth of a search walk. |
| `search_visit_cap_multiplier` | `int` | no | `20` | Legacy visit budget as a multiple of `top_k`, used when `search_max_visits` is `0`. |
| `redirect_per_frame` | `bool` | no | `True` | Emit one redirect per superseded source frame. |
| `secure` | `str \| None` | no | `None` | Security posture: `"off"`, `"standard"` (alias `"on"`), or `None` (auto-detect). |
| `eigen_matmul_min` | `int` | no | `256` | Eigenframe count from which the entry scan uses the resident matrix (`0` = always). |
| `encryption_passphrase` | `str \| None` | no | `None` | Passphrase for AES-256-GCM encryption at rest. |
| `search_use_stream` | `bool` | no | `True` | Run search through the bounded walk at every store size; `False` keeps the full traversal below `search_stream_threshold`. Runtime, not persisted. |
| `search_stream_threshold` | `int` | no | `50000` | With the bounded walk switched off, the store size from which `search` uses it anyway (`0` disables). Runtime, not persisted. |
| `search_stream_beam` | `int` | no | `16` | Width of the bounded search walk (`0` = unbounded). Runtime, not persisted. |
| `vector_precision` | `str` | no | `"codes4"` | Precision each frame's vector is stored at, fixed for the store's life: `"codes4"` (4-bit codes), `"fp32"`, `"fp16"`, `"bf16"` or `"fp64"`. Read-only once constructed. |
| `resident_precision` | `str \| None` | no | `None` | Precision search scores from: `"fp32"`, `"fp16"`, `"bf16"` or `"frame"` (no resident copy). `None` derives it from `vector_precision`. Changeable at runtime with `set_resident_precision`. Read-only once constructed. |

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

### `store.put(content, embedding, seed_term=True)`

Ingest content with a pre-computed embedding.

```python
store.put(content, embedding, seed_term=True) -> str
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `content` | `bytes` | yes | -- | Raw content bytes. |
| `embedding` | `list[float]` | yes | -- | Embedding vector; length must equal the store dimension. |
| `seed_term` | `bool` | no | `True` | Choose the parent with the commit-time seed term (see `set_seed_term`). `False` takes the raw-cosine parent for this frame. Similarity edges and the stored embedding are the same either way. |

**Returns:** `str` — the new frame ID.

**Raises:** `RuntimeError` on dimension mismatch, a sealed store, or a closed store.

**Example**

```python
emb = store.encode("the cat sat on the mat")
fid = store.put(b"the cat sat on the mat", emb)
```

### `store.put_with_metadata(content, embedding, metadata_json="{}", seed_term=True)`

Ingest content with an embedding and JSON metadata.

```python
store.put_with_metadata(content, embedding, metadata_json="{}", seed_term=True) -> str
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `content` | `bytes` | yes | -- | Raw content bytes. |
| `embedding` | `list[float]` | yes | -- | Embedding vector. |
| `metadata_json` | `str` | no | `"{}"` | JSON object string of metadata key-value pairs. |
| `seed_term` | `bool` | no | `True` | `False` takes the raw-cosine parent for this frame (see `set_seed_term`). |

**Returns:** `str` — the new frame ID.

**Raises:** `RuntimeError` as `put`, plus on invalid JSON.

### `store.put_with_embedding(content, embedding, seed_term=True)`

Alias for `put` (Python wheel parity).

```python
store.put_with_embedding(content, embedding, seed_term=True) -> str
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `content` | `bytes` | yes | -- | Raw content bytes. |
| `embedding` | `list[float]` | yes | -- | Embedding vector. |
| `seed_term` | `bool` | no | `True` | `False` takes the raw-cosine parent for this frame (see `set_seed_term`). |

**Returns:** `str` — the new frame ID.

**Raises:** `RuntimeError` as `put`.

### `store.put_batch(items)`

Deprecated alias of `put_batch_incremental`; calls take the incremental path. Leaves the store buffered: call `flush()` when the load is done.

```python
store.put_batch(items) -> list[str]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `items` | `list[tuple[bytes, list[float]]]` | yes | -- | List of (content, embedding) tuples. |

**Returns:** `list[str]` — frame IDs, one per item.

**Raises:** `RuntimeError` if any embedding dimension mismatches, or the store is sealed/closed.

### `store.put_batch_incremental(items, seed_term=True)`

Bulk ingest. Each item's parent is chosen against the live store, so an item may attach to an earlier item of the same batch, and the result is identical to calling `put` in a loop at a fraction of the cost. Insertion order shapes the tree, so order the batch meaningfully. The store is left buffered: call `flush()` when the load is finished.

```python
store.put_batch_incremental(items, seed_term=True) -> list[str]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `items` | `list[tuple[bytes, list[float]]]` | yes | -- | List of (content, embedding) tuples. |
| `seed_term` | `bool` | no | `True` | `False` takes raw-cosine parents for the whole batch (see `set_seed_term`). |

**Returns:** `list[str]` — frame IDs, one per item.

**Raises:** `RuntimeError` if any embedding dimension mismatches, or the store is sealed/closed.

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

### `store.commit_marker(kind, metadata_json="{}")`

Commit an application marker: a System frame with no embedding that search never returns, durable in the store like any other frame. Use it for application records that belong with the store (a run manifest, a review note). `kind` becomes the content prefix `_{kind}:` and is echoed in the metadata as `_marker`.

```python
store.commit_marker(kind, metadata_json="{}") -> str
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `kind` | `str` | yes | -- | Marker kind. Non-empty, no `:` and no whitespace. The kinds the load path interprets (`promote`, `supersede`, `seal`, `seraph_seeds`, `seraph_warp_cal`, anything starting `seraph`, case-insensitive, with or without a leading `_`) are refused. |
| `metadata_json` | `str` | no | `"{}"` | JSON object. Keys the load path acts on (`_promote`, `_supersede`, any `_seraph*`) are refused, so a marker can never forge a promotion, a supersede or a registry on reopen. |

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

### `store.search(query_embedding, top_k=10, tau=0.5)`

Search for the frames nearest a query embedding, ranked by similarity.

```python
store.search(query_embedding, top_k=10, tau=0.5) -> list[Hit]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `query_embedding` | `list[float]` | yes | -- | Query vector. |
| `top_k` | `int` | no | `10` | Maximum number of hits. |
| `tau` | `float` | no | `0.5` | Minimum similarity for a hit. |

**Returns:** `list[Hit]` — each with .frame_id, .score, .confidence, .path_length, .snippet, .content and .frame.

**Raises:** `RuntimeError` on dimension mismatch or a closed store.

**Example**

```python
q = store.encode("feline animals")
for hit in store.search(q, top_k=5, tau=0.4):
    print(hit.frame_id, hit.score, hit.snippet)
```

### `store.search_with_opts(query_embedding, opts)`

Search with a `SearchOpts` options object: `top_k`, `tau_similarity`, `max_depth`, `include_superseded`, and the time-window fields (`order`, `after_us`, `before_us`).

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

Search by a pre-computed query vector with an optional SearchOpts. Without opts it is search with the defaults.

```python
store.search_by_vector(query_embedding, opts=None) -> list[Hit]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `query_embedding` | `list[float]` | yes | -- | Query vector. |
| `opts` | `SearchOpts \| None` | no | `None` | Search options; None uses the defaults. |

**Returns:** `list[Hit]`.

**Raises:** `RuntimeError` on dimension mismatch or a closed store.

### `store.search_text(query, opts=None)`

Encode a text query with the store's model and search in one call. The encoder loads on first use.

```python
store.search_text(query, opts=None) -> list[Hit]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `query` | `str` | yes | -- | Query text. |
| `opts` |  | no | `None` | Search options. |

**Returns:** `list[Hit]`.

**Raises:** `RuntimeError` if encoder init fails or the store is closed.

### `store.search_with_trace(query_embedding, top_k=10, tau=0.5)`

Search returning the hits and the frames visited, in visit order.

```python
store.search_with_trace(query_embedding, top_k=10, tau=0.5) -> tuple[list[Hit], list[TraceEntry]]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `query_embedding` | `list[float]` | yes | -- | Query vector. |
| `top_k` | `int` | no | `10` | Maximum number of hits. |
| `tau` | `float` | no | `0.5` | Minimum similarity for a hit. |

**Returns:** `tuple[list[Hit], list[TraceEntry]]` — score-ranked hits and the visit-order trace.

**Raises:** `RuntimeError` on dimension mismatch or a closed store.

### `store.search_with_warp(query_embedding, targets, suppress, profile="bounded", bound_k=1.5, top_k=10, tau=0.5)`

Search with warp-field steering: the query is pulled toward `targets` and away from `suppress` before the search.

```python
store.search_with_warp(query_embedding, targets, suppress, profile="bounded", bound_k=1.5, top_k=10, tau=0.5) -> list[Hit]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `query_embedding` | `list[float]` | yes | -- | Query vector. |
| `targets` | `list[tuple[list[float], float]]` | yes | -- | (embedding, magnitude) pull pairs. |
| `suppress` | `list[tuple[list[float], float]]` | yes | -- | (embedding, magnitude) suppression pairs. |
| `profile` | `str` | no | `"bounded"` | `"routing"`, `"saturation"`, or `"bounded"`. |
| `bound_k` | `float` | no | `1.5` | Suppression-norm cap (used only by `"bounded"`). |
| `top_k` | `int` | no | `10` | Maximum number of hits. |
| `tau` | `float` | no | `0.5` | Minimum similarity for a hit. |

**Returns:** `list[Hit]`.

**Raises:** `RuntimeError` on unrecognized profile, dimension mismatch, or a closed store.

### `store.search_with_warp_spec(query_embedding, spec, top_k=10, tau=0.5, magnitude_overrides=None)`

Search steered by a warp specification in one call. Frame-ID targets and named magnitudes in the spec are resolved against the store.

```python
store.search_with_warp_spec(query_embedding, spec, top_k=10, tau=0.5, magnitude_overrides=None) -> list[Hit]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `query_embedding` | `list[float]` | yes | -- | Query vector. |
| `spec` | `WarpSpec` | yes | -- | Warp specification; a JSON document on C and Rust. |
| `top_k` | `int` | no | `10` | Maximum number of hits. |
| `tau` | `float` | no | `0.5` | Minimum similarity for a hit. |
| `magnitude_overrides` | `dict[str, float] \| None` | no | `None` | Per-name overrides of the spec's named magnitudes. |

**Returns:** `list[Hit]`.

**Raises:** `RuntimeError` on unresolved frame-id/magnitude, dimension mismatch, or a closed store.

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

### `SearchOpts(top_k=10, tau_similarity=0.5, max_depth=50, include_superseded=False, order=None, after_us=None, before_us=None)`

Search options for `search_with_opts`, `search_by_vector` and `search_text`. `after_us` and `before_us` set an inclusive time window on which frames may fill result slots, and with `before_us` set the search runs over the store as it stood at that time; `order` sorts the selected results by time without changing their scores. With no window and the default order, the result matches `search`.

```python
SearchOpts(top_k=10, tau_similarity=0.5, max_depth=50, include_superseded=False, order=None, after_us=None, before_us=None) -> SearchOpts
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `top_k` | `int` | no | `10` | Maximum results. |
| `tau_similarity` | `float` | no | `0.5` | Similarity threshold. |
| `max_depth` | `int` | no | `50` | Accepted for parity; not honored (depth comes from `StoreConfig.search_max_depth`). |
| `include_superseded` | `bool` | no | `False` | Opts plainly-superseded (soft-forgotten) frames back into results. |
| `order` | `str \| None` | no | `None` | Presentation order: `"relevance"` (default), `"most_recent"` (timestamp descending), `"oldest"` (timestamp ascending). Invalid values raise `ValueError`. |
| `after_us` | `int \| None` | no | `None` | Inclusive lower bound on frame timestamp (microseconds since the epoch); `None` = unbounded. |
| `before_us` | `int \| None` | no | `None` | Inclusive upper bound on frame timestamp (microseconds since the epoch); `None` = unbounded. |

**Returns:** `SearchOpts` — all fields are readable and writable attributes.

**Raises:** `ValueError` or `RuntimeError` on an invalid argument.

> `SearchOpts` takes effect through `search_with_opts`, `search_by_vector`, and `search_text`.

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

### `store.get(frame_id)`

Alias for get_frame.

```python
store.get(frame_id) -> Frame | None
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `frame_id` | `str` | yes | -- | Frame ID to fetch. |

**Returns:** `Frame | None` — None if not found.

**Raises:** `RuntimeError` if the store is closed.

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

### `store.put_subtree(root, content, embedding, metadata_json="{}")`

Ingest content under a named subtree: the parent is chosen from inside `root`'s subtree instead of the whole store, with no similarity floor. Similarity edges are chosen globally as `put` chooses them, so the frame stays reachable by ordinary search. The placement is recorded on the frame so a migration keeps it inside the same region.

```python
store.put_subtree(root, content, embedding, metadata_json="{}") -> str
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `root` | `str` | yes | -- | Subtree root: a frame ID or a seed label. A frame ID wins if it resolves as one. |
| `content` | `bytes` | yes | -- | Content bytes. |
| `embedding` | `list[float]` | yes | -- | Embedding vector. |
| `metadata_json` | `str` | no | `"{}"` | JSON metadata object. A caller-supplied `_placed_under` is overwritten. |

**Returns:** `str` — the new frame ID.

**Raises:** `RuntimeError` on an unknown root, dimension mismatch, a sealed
store, or a closed store.

**Example**

```python
fid = store.put_subtree("evidence", b"exhibit A", emb)
```

### `store.search_subtree(root, embedding, top_k=10, tau=None)`

Search confined to a named subtree: the root is the entry point and only edges inside the subtree are followed. Ordinary `search` is unaffected and still reaches these frames.

```python
store.search_subtree(root, embedding, top_k=10, tau=None) -> list[Hit]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `root` | `str` | yes | -- | Subtree root: a frame ID or a seed label. |
| `embedding` | `list[float]` | yes | -- | Query embedding. |
| `top_k` | `int` | no | `10` | Maximum results. |
| `tau` | `float \| None` | no | `None` | Similarity threshold; `None` uses the store's configured `tau_similarity`. |

**Returns:** `list[Hit]`.

**Raises:** `RuntimeError` on an unknown root or a closed store.

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

Ingest content and immediately promote it to eigenframe (atomic ingest plus promote).

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

### `store.chain(from_id, to_id, include_superseded=False)`

The shortest route between two frames over committed edges (similarity edges and parent/child links), each step weighted by how similar its two frames are. Exact; bounded by `search_max_visits`. For a frame's own ancestry use `lineage`.

```python
store.chain(from_id, to_id, include_superseded=False) -> Chain
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `from_id` | `str` | yes | -- | Start frame ID. |
| `to_id` | `str` | yes | -- | End frame ID. |
| `include_superseded` | `bool` | no | `False` | Let superseded frames be steps. |

**Returns:** `Chain`.

**Raises:** `RuntimeError` if the store is closed.

### `store.chain_forward(from_id, to_id, include_superseded=False)`

How `from_id` led to `to_id`: the shortest route over committed edges taking only steps forward in commit order, so each step is to a frame that attached to the one before it when it was committed. `from_id` must precede `to_id`; `reached` is `False` when no such route exists. Crosses lineages, which `lineage` cannot.

```python
store.chain_forward(from_id, to_id, include_superseded=False) -> Chain
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `from_id` | `str` | yes | -- | Earlier frame ID. |
| `to_id` | `str` | yes | -- | Later frame ID. |
| `include_superseded` | `bool` | no | `False` | Let superseded frames be steps. A frame only takes an edge to a frame current when it arrived, so a superseded step is history. |

**Returns:** `Chain`.

**Raises:** `RuntimeError` if the store is closed.

### `store.chain_forward_between(emb_a, emb_b, top_k=10, tau=0.5, include_superseded=False)`

How the idea at `emb_a` led to the idea at `emb_b`. Each query's `search` results (under `top_k` and `tau`) stand for it; the best-fitting earlier/later pair that a forward route joins is chosen, and the route is the forward chain between them. `reached` is `False` when no forward route joins the two result sets.

```python
store.chain_forward_between(emb_a, emb_b, top_k=10, tau=0.5, include_superseded=False) -> Chain
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `emb_a` | `list[float]` | yes | -- | The earlier idea. |
| `emb_b` | `list[float]` | yes | -- | The later idea. |
| `top_k` | `int` | no | `10` | Search results per query that may stand for it. |
| `tau` | `float` | no | `0.5` | Similarity floor for those results. |
| `include_superseded` | `bool` | no | `False` | Let superseded frames be endpoints and steps. |

**Returns:** `Chain`.

**Raises:** `RuntimeError` if the store is closed.

### `store.provenance(frame_id, tau=None, include_superseded=False)`

What `frame_id` was built on: a route back in commit order through the frames it attached to when it was committed (its parent and the similarity neighbours it chose), stopping when the next step is no more related than the store's background. `tau` overrides that stop; `reached` is `True` when at least one step was taken.

```python
store.provenance(frame_id, tau=None, include_superseded=False) -> Chain
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `frame_id` | `str` | yes | -- | Start frame. |
| `tau` | `float \| None` | no | `None` | Stop floor; `None` for the store's background. |
| `include_superseded` | `bool` | no | `False` | Let superseded frames be steps. |

**Returns:** `Chain` — start frame first.

**Raises:** `RuntimeError` if the store is closed.

### `store.influence(frame_id, tau=None, include_superseded=False)`

What was built on `frame_id`: a route forward in commit order through the frames that attached to it when they were committed, stopping when the next step is no more related than the store's background. `tau` overrides that stop; `reached` is `True` when at least one step was taken.

```python
store.influence(frame_id, tau=None, include_superseded=False) -> Chain
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `frame_id` | `str` | yes | -- | Start frame. |
| `tau` | `float \| None` | no | `None` | Stop floor; `None` for the store's background. |
| `include_superseded` | `bool` | no | `False` | Let superseded frames be steps. |

**Returns:** `Chain` — start frame first.

**Raises:** `RuntimeError` if the store is closed.

### `store.provenance_with_warp(frame_id, targets, suppress, profile="bounded", bound_k=1.5, tau=None, include_superseded=False)`

`provenance` steered by a warp: steps are chosen, and the stop measured, against the start frame pulled toward `targets` and away from `suppress`, so the warp selects which strand is followed.

```python
store.provenance_with_warp(frame_id, targets, suppress, profile="bounded", bound_k=1.5, tau=None, include_superseded=False) -> Chain
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `frame_id` | `str` | yes | -- | Start frame. |
| `targets` | `list[tuple[list[float], float]]` | yes | -- | (embedding, magnitude) to steer toward. |
| `suppress` | `list[tuple[list[float], float]]` | yes | -- | (embedding, magnitude) to steer away from. |
| `profile` | `str` | no | `"bounded"` | Warp profile. |
| `bound_k` | `float` | no | `1.5` | Suppression-norm cap. |
| `tau` | `float \| None` | no | `None` | Stop floor; `None` for the store's background. |
| `include_superseded` | `bool` | no | `False` | Let superseded frames be steps. |

**Returns:** `Chain` — start frame first.

**Raises:** `RuntimeError` if the store is closed.

### `store.influence_with_warp(frame_id, targets, suppress, profile="bounded", bound_k=1.5, tau=None, include_superseded=False)`

`influence` steered by a warp: steps are chosen, and the stop measured, against the start frame pulled toward `targets` and away from `suppress`, so the warp selects which strand is followed.

```python
store.influence_with_warp(frame_id, targets, suppress, profile="bounded", bound_k=1.5, tau=None, include_superseded=False) -> Chain
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `frame_id` | `str` | yes | -- | Start frame. |
| `targets` | `list[tuple[list[float], float]]` | yes | -- | (embedding, magnitude) to steer toward. |
| `suppress` | `list[tuple[list[float], float]]` | yes | -- | (embedding, magnitude) to steer away from. |
| `profile` | `str` | no | `"bounded"` | Warp profile. |
| `bound_k` | `float` | no | `1.5` | Suppression-norm cap. |
| `tau` | `float \| None` | no | `None` | Stop floor; `None` for the store's background. |
| `include_superseded` | `bool` | no | `False` | Let superseded frames be steps. |

**Returns:** `Chain` — start frame first.

**Raises:** `RuntimeError` if the store is closed.

### `store.connections(from_id, to_id, max_routes=5, forward=False, include_superseded=False)`

The distinct ways two frames connect: up to `max_routes` routes over committed edges that share no interior frames, shortest first, each with its bridge frame (the interior frame closest to both ends). With `forward` set, every route runs forward in commit order: the ways `from_id` led to `to_id`.

```python
store.connections(from_id, to_id, max_routes=5, forward=False, include_superseded=False) -> list[Connection]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `from_id` | `str` | yes | -- | Start frame. |
| `to_id` | `str` | yes | -- | End frame. |
| `max_routes` | `int` | no | `5` | Most routes returned. |
| `forward` | `bool` | no | `False` | Routes forward in commit order only (`from_id` must precede `to_id`). |
| `include_superseded` | `bool` | no | `False` | Let superseded frames be steps. |

**Returns:** `list[Connection]` — empty when the frames are not joined.

**Raises:** `RuntimeError` if the store is closed.

### `store.chain_to(query_embedding)`

Chain from the frame nearest a query embedding.

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

Chain between two points in embedding space: the route between the frames nearest each embedding.

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

### `store.history(query_embedding, top_k=10, tau=0.5, anchors=3)`

How the store came to hold what the query finds. The top `anchors` search hits choose their lineages, and the best `top_k` frames along those lineages come back oldest first, each with the route that reached it. On a store ingested in time order the result reads as the development of a topic.

```python
store.history(query_embedding, top_k=10, tau=0.5, anchors=3) -> History
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `query_embedding` | `list[float]` | yes | -- | Query vector. |
| `top_k` | `int` | no | `10` | Frames to return. |
| `tau` | `float` | no | `0.5` | Similarity floor for admission and results. |
| `anchors` | `int` | no | `3` | Search hits used to choose the lineages. |

**Returns:** `History`.

**Raises:** `RuntimeError` if the store is closed.

### `store.history_with_warp(query_embedding, targets, suppress, profile="bounded", bound_k=1.5, top_k=10, tau=0.5, anchors=3)`

`history` from a warped query. The warp moves which lineages the anchors fall in, so it chooses which strand of a topic's history is read: toward a finding, away from a thread. Pass a frame's embedding as a target to steer toward that frame.

```python
store.history_with_warp(query_embedding, targets, suppress, profile="bounded", bound_k=1.5, top_k=10, tau=0.5, anchors=3) -> History
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `query_embedding` | `list[float]` | yes | -- | Query vector. |
| `targets` | `list[tuple[list[float], float]]` | yes | -- | (embedding, magnitude) pull pairs. |
| `suppress` | `list[tuple[list[float], float]]` | yes | -- | (embedding, magnitude) suppression pairs. |
| `profile` | `str` | no | `"bounded"` | `"routing"`, `"saturation"`, or `"bounded"`. |
| `bound_k` | `float` | no | `1.5` | Suppression-norm cap. |
| `top_k` | `int` | no | `10` | Frames to return. |
| `tau` | `float` | no | `0.5` | Similarity floor. |
| `anchors` | `int` | no | `3` | Search hits used to choose the lineages. |

**Returns:** `History`.

**Raises:** `RuntimeError` if the store is closed.

### `store.chain_to_with_warp(query_embedding, targets, suppress, profile="bounded", bound_k=1.5)`

`chain_to` with a warp applied to the query before the route is found.

```python
store.chain_to_with_warp(query_embedding, targets, suppress, profile="bounded", bound_k=1.5) -> Chain
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `query_embedding` | `list[float]` | yes | -- | Target query vector. |
| `targets` | `list[tuple[list[float], float]]` | yes | -- | (embedding, magnitude) pull pairs. |
| `suppress` | `list[tuple[list[float], float]]` | yes | -- | (embedding, magnitude) suppression pairs. |
| `profile` | `str` | no | `"bounded"` | `"routing"`, `"saturation"`, or `"bounded"`. |
| `bound_k` | `float` | no | `1.5` | Suppression-norm cap. |

**Returns:** `Chain`.

**Raises:** `RuntimeError` on unrecognized profile, dimension mismatch, or a closed store.

### `store.chain_between_with_warp(emb_a, emb_b, targets, suppress, profile="bounded", bound_k=1.5)`

`chain_between` with a warp applied to both endpoints before the route is found.

```python
store.chain_between_with_warp(emb_a, emb_b, targets, suppress, profile="bounded", bound_k=1.5) -> Chain
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `emb_a` | `list[float]` | yes | -- | First embedding. |
| `emb_b` | `list[float]` | yes | -- | Second embedding. |
| `targets` | `list[tuple[list[float], float]]` | yes | -- | (embedding, magnitude) pull pairs. |
| `suppress` | `list[tuple[list[float], float]]` | yes | -- | (embedding, magnitude) suppression pairs. |
| `profile` | `str` | no | `"bounded"` | `"routing"`, `"saturation"`, or `"bounded"`. |
| `bound_k` | `float` | no | `1.5` | Suppression-norm cap. |

**Returns:** `Chain`.

**Raises:** `RuntimeError` on unrecognized profile, dimension mismatch, or a closed store.

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

### `store.neighborhood(frame_id, tau=None, hops=1)`

Three-population neighborhood of a frame: its consensus, geometric and lineage neighbours.

```python
store.neighborhood(frame_id, tau=None, hops=1) -> NeighborhoodResult
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `frame_id` | `str` | yes | -- | Center frame ID. |
| `tau` | `float \| str \| None` | no | `None` | `None` (p95), `"loose"` (p75), `"strict"` (p99), or an explicit float. |
| `hops` | `int` | no | `1` | Traversal radius in hops. |

**Returns:** `NeighborhoodResult`.

**Raises:** `RuntimeError` on an invalid `tau` string or a closed store.

### `store.neighborhood_with_warp(frame_id, targets, suppress, profile="bounded", bound_k=1.5, tau=0.5, hops=1)`

`neighborhood` with a warp field applied.

```python
store.neighborhood_with_warp(frame_id, targets, suppress, profile="bounded", bound_k=1.5, tau=0.5, hops=1) -> NeighborhoodResult
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `frame_id` | `str` | yes | -- | Center frame ID. |
| `targets` | `list[tuple[list[float], float]]` | yes | -- | (embedding, magnitude) pull pairs. |
| `suppress` | `list[tuple[list[float], float]]` | yes | -- | (embedding, magnitude) suppression pairs. |
| `profile` | `str` | no | `"bounded"` | `"routing"`, `"saturation"`, or `"bounded"`. |
| `bound_k` | `float` | no | `1.5` | Suppression-norm cap. |
| `tau` | `float` | no | `0.5` | Similarity threshold. |
| `hops` | `int` | no | `1` | Traversal radius. |

**Returns:** `NeighborhoodResult`.

**Raises:** `RuntimeError` on unrecognized profile, dimension mismatch, or a closed store.

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
| `targets` | `list[tuple[list[float], float]]` | yes | -- | (embedding, magnitude) pull pairs. |
| `suppress` | `list[tuple[list[float], float]]` | yes | -- | (embedding, magnitude) suppression pairs. |
| `profile` | `str` | no | `"bounded"` | `"routing"`, `"saturation"`, or `"bounded"`. |
| `bound_k` | `float` | no | `1.5` | Suppression-norm cap. |

**Returns:** `list[float]` — the warped query vector.

**Raises:** Nothing store-related (no handle access); profile/dimension issues are handled internally.

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

### `store.calibrate_magnitudes(n_queries=30, n_targets=3, magnitudes=None, top_k=10, seed=42)`

Alias for calibrate_warp.

```python
store.calibrate_magnitudes(n_queries=30, n_targets=3, magnitudes=None, top_k=10, seed=42) -> WarpCalibration
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `n_queries` | `int` | no | `30` | Number of sampled queries. |
| `n_targets` | `int` | no | `3` | Number of sampled targets. |
| `magnitudes` | `list[float] \| None` | no | `None` | Magnitudes to sweep; None uses the defaults. |
| `top_k` | `int` | no | `10` | Hits per probe. |
| `seed` | `int` | no | `42` | RNG seed for reproducible sampling. |

**Returns:** `WarpCalibration`.

**Raises:** `RuntimeError` on calibration error or a closed store.

### `WarpSpec(targets=None, suppress=None, profile="bounded", bound_k=1.5, accumulation="reset")`

Warp specification: what to steer toward and away from, and how. Each `input` is a frame-id `str` or a `list[float]` embedding; each `magnitude` is a `float` or a named-magnitude `str` (`"action_floor"`, `"routing"`, `"sub_saturation"`, `"saturation"`), resolved against the store's warp calibration.

```python
WarpSpec(targets=None, suppress=None, profile="bounded", bound_k=1.5, accumulation="reset") -> WarpSpec
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `targets` | `list[tuple] \| None` | no | `None` | (input, magnitude) pull pairs. |
| `suppress` | `list[tuple] \| None` | no | `None` | (input, magnitude) suppression pairs. |
| `profile` | `str` | no | `"bounded"` | `"routing"`, `"saturation"`, or `"bounded"`. |
| `bound_k` | `float` | no | `1.5` | Suppression-norm cap. |
| `accumulation` | `str` | no | `"reset"` | `"reset"` or `"cumulative"`. |

**Returns:** `WarpSpec` — read-only getters .profile, .bound_k, .accumulation.

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

### `store.consolidate(frame_ids)`

Consolidate a specific set of frames into one centroid frame.

```python
store.consolidate(frame_ids) -> ConsolidateReport
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `frame_ids` | `list[str]` | yes | -- | Frame IDs to consolidate. |

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

### `seraph.verify_chain(frames, genesis_seed=None)`

Verify watermark-chain integrity across an ordered list of Frame objects, parent before child, without a store. A store checks itself with verify.

```python
seraph.verify_chain(frames, genesis_seed=None) -> tuple[bool, str | None]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `frames` | `list[Frame]` | yes | -- | Frames in parent-to-child order. |
| `genesis_seed` | `bytes \| None` | no | `None` | Genesis seed of an attested store; None uses the default seed. |

**Returns:** `tuple[bool, str | None]` — (is_valid, first failing frame ID).

**Raises:** Does not raise (returns the failure tuple instead).

### `seraph.migrate_store(src_path, dst_path, config=None)`

Rebuild a store into a fresh one at `dst_path`: every frame is re-committed with its identity intact (id, timestamp, content, embedding, metadata, status references) and its parent chosen against the live destination, so sentinels and redirects that name frames by id stay valid. Use it to correct an old topology or to carry a store to the current format or another `vector_precision` (`config`). Embeddings are carried as they are; use `reencode_store` to change them. Watermarks are recomputed, so they differ from the source; the destination verifies. Free function; the Python binding releases the GIL for the whole migration.

```python
seraph.migrate_store(src_path, dst_path, config=None) -> dict[str, int]
```

**Parameters**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `src_path` | `str` | yes | -- | Source `.sfg` path. Opened read-only. |
| `dst_path` | `str` | yes | -- | Destination `.sfg` path. Created fresh. |
| `config` | `StoreConfig \| None` | no | `None` | Destination configuration (its `vector_precision` re-cuts the store). `None` inherits the source's at the current format version. |

**Returns:** `dict[str, int]` — keys content, seeds, sentinels, superseded, redirects, skipped, on_genesis; a high on_genesis relative to content indicates a star rather than a tree.

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
| `Chain` | `.frames`, `.frame_ids`, `.primitive`, `.segments`, `.coherence`, `.reached` (`False` when no route was found within the visit budget; the path then ends at the nearest frame reached); `.primitive` is `"geodesic"` (`"geodesic_forward"` from `chain_forward`) |
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
