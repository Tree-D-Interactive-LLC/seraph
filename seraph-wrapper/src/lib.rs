// SERAPH - Copyright (c) 2025-2026 Daniel S. Brooks. All rights reserved.
// Proprietary and confidential. Licensed under the SERAPH Software License
// Agreement. Unauthorized use, copying, modification, or distribution is
// strictly prohibited. See LICENSE or https://tree-d-interactive.net for terms.

//! # seraph
//!
//! Idiomatic Rust wrapper over the compiled SERAPH engine.
//!
//! This crate provides safe, ergonomic types that internally call
//! `extern "C"` functions exported by the SERAPH cdylib
//! (`seraph.dll` / `libseraph.so`). No engine source is included —
//! this is interface glue only.
//!
//! ## Quick start
//!
//! ```no_run
//! use seraph::{Store, Encoder};
//!
//! // Create an encoder for text → embedding
//! let mut encoder = Encoder::from_pretrained("BAAI/bge-small-en-v1.5")?;
//!
//! // Create a new store
//! let mut store = Store::create("memory.sfg", None)?;
//!
//! // Ingest
//! let emb = encoder.encode("the cat sat on the mat")?;
//! let frame_id = store.put(b"the cat sat on the mat", &emb)?;
//!
//! // Search
//! let query = encoder.encode("feline")?;
//! let hits = store.search(&query, 10, 0.0)?;
//! for hit in &hits {
//!     println!("{}: score={:.3}", hit.frame_id, hit.score);
//! }
//!
//! // Verify integrity
//! let (ok, failures) = store.verify()?;
//! assert!(ok);
//! # Ok::<(), seraph::Error>(())
//! ```

use std::ffi::{c_char, c_void, CStr, CString};
use std::fmt;
use std::path::Path;
use std::ptr;

// ---------------------------------------------------------------- //
// FFI declarations — extern "C" symbols from the SERAPH cdylib
// ---------------------------------------------------------------- //

extern "C" {
    fn seraph_last_error() -> *const c_char;
    fn seraph_string_free(s: *mut c_char);
    fn seraph_bytes_free(ptr: *mut u8, len: usize);
    fn seraph_floats_free(ptr: *mut f32, len: usize);
    fn seraph_hits_free(hits: *mut FfiHit, count: usize);
    fn seraph_string_array_free(arr: *mut *mut c_char, count: usize);

    // Store lifecycle
    fn seraph_store_create(
        path: *const c_char,
        model_id: *const c_char,
        config_json: *const c_char,
    ) -> *mut c_void;
    fn seraph_store_open(path: *const c_char) -> *mut c_void;
    fn seraph_store_close(handle: *mut c_void);

    // Ingest
    fn seraph_store_put(
        handle: *mut c_void,
        content: *const u8, content_len: usize,
        embedding: *const f32, embedding_len: usize,
    ) -> *mut c_char;
    fn seraph_store_put_with_metadata(
        handle: *mut c_void,
        content: *const u8, content_len: usize,
        embedding: *const f32, embedding_len: usize,
        metadata_json: *const c_char,
    ) -> *mut c_char;

    // Search
    fn seraph_store_search(
        handle: *mut c_void,
        embedding: *const f32, embedding_len: usize,
        top_k: usize, tau: f32,
        out_hits: *mut *mut FfiHit, out_count: *mut usize,
    ) -> i32;
    fn seraph_store_search_opts(
        handle: *mut c_void,
        embedding: *const f32, embedding_len: usize,
        top_k: usize, tau: f32,
        include_superseded: bool,
        out_hits: *mut *mut FfiHit, out_count: *mut usize,
    ) -> i32;

    // Frame access
    fn seraph_store_get_content(
        handle: *mut c_void,
        frame_id: *const c_char,
        out_content: *mut *mut u8, out_len: *mut usize,
    ) -> i32;
    fn seraph_store_get_metadata(
        handle: *mut c_void,
        frame_id: *const c_char,
    ) -> *mut c_char;

    // Accessors
    fn seraph_store_model_id(handle: *mut c_void) -> *mut c_char;
    fn seraph_store_dim(handle: *mut c_void) -> usize;
    fn seraph_store_frame_ids(handle: *mut c_void) -> *mut c_char;
    fn seraph_store_eigenframe_ids(handle: *mut c_void) -> *mut c_char;

    // Stats
    fn seraph_store_stats(handle: *mut c_void, out: *mut FfiStats) -> i32;

    // Integrity
    fn seraph_store_verify(
        handle: *mut c_void,
        out_failures_json: *mut *mut c_char,
    ) -> i32;
    fn seraph_store_seal(handle: *mut c_void, reason: *const c_char) -> *mut c_char;
    fn seraph_store_is_sealed(handle: *mut c_void) -> i32;

    // Supersede
    fn seraph_store_supersede(handle: *mut c_void, frame_id: *const c_char) -> *mut c_char;

    // Sync
    fn seraph_store_sync(handle: *mut c_void) -> i32;

    // Ingest — batch, seed, promote
    fn seraph_store_put_batch(handle: *mut c_void, items_json: *const c_char) -> *mut c_char;
    fn seraph_store_add_seed(
        handle: *mut c_void, label: *const c_char,
        embedding: *const f32, embedding_len: usize,
        metadata_json: *const c_char,
    ) -> *mut c_char;
    fn seraph_store_promote(handle: *mut c_void, frame_id: *const c_char) -> i32;
    fn seraph_store_promote_content(
        handle: *mut c_void,
        content: *const u8, content_len: usize,
        embedding: *const f32, embedding_len: usize,
        metadata_json: *const c_char,
    ) -> *mut c_char;

    // Lifecycle
    fn seraph_store_begin_batch(handle: *mut c_void) -> i32;
    fn seraph_store_commit(handle: *mut c_void) -> i32;

    // Tuning
    fn seraph_store_set_search_visit_cap_multiplier(handle: *mut c_void, multiplier: u32) -> i32;

    // Frame access
    fn seraph_store_get_frame(handle: *mut c_void, frame_id: *const c_char) -> *mut c_char;
    fn seraph_store_get_embedding(
        handle: *mut c_void, frame_id: *const c_char,
        out_embedding: *mut *mut f32, out_dim: *mut usize,
    ) -> i32;

    // Graph basics
    fn seraph_store_get_children(handle: *mut c_void, frame_id: *const c_char) -> *mut c_char;
    fn seraph_store_eigenframes(handle: *mut c_void) -> *mut c_char;
    fn seraph_store_federation_vector(handle: *mut c_void, out_embedding: *mut *mut f32, out_dim: *mut usize) -> i32;
    fn seraph_store_lineage(handle: *mut c_void, frame_id: *const c_char) -> *mut c_char;
    fn seraph_store_path(handle: *mut c_void, from_id: *const c_char, to_id: *const c_char) -> *mut c_char;
    fn seraph_store_subtree(handle: *mut c_void, frame_id: *const c_char, max_depth: usize) -> *mut c_char;

    // Search variants
    fn seraph_store_search_text(
        handle: *mut c_void, query: *const c_char,
        top_k: usize, tau: f32,
        out_hits: *mut *mut FfiHit, out_count: *mut usize,
    ) -> i32;
    fn seraph_store_search_with_warp(
        handle: *mut c_void,
        query_embedding: *const f32, query_len: usize,
        targets_json: *const c_char, suppress_json: *const c_char,
        profile: *const c_char, bound_k: f32,
        top_k: usize, tau: f32,
        out_hits: *mut *mut FfiHit, out_count: *mut usize,
    ) -> i32;
    fn seraph_store_apply_warp(
        query_embedding: *const f32, query_len: usize,
        targets_json: *const c_char, suppress_json: *const c_char,
        profile: *const c_char, bound_k: f32,
        out_embedding: *mut *mut f32, out_dim: *mut usize,
    ) -> i32;

    // Neighborhood / Chain
    fn seraph_store_neighborhood(handle: *mut c_void, frame_id: *const c_char, tau: f32, hops: usize) -> *mut c_char;
    fn seraph_store_chain(handle: *mut c_void, from_id: *const c_char, to_id: *const c_char) -> *mut c_char;
    fn seraph_store_chain_to(handle: *mut c_void, query_embedding: *const f32, query_len: usize) -> *mut c_char;
    fn seraph_store_chain_between(handle: *mut c_void, emb_a: *const f32, len_a: usize, emb_b: *const f32, len_b: usize) -> *mut c_char;

    // Intelligence
    fn seraph_store_density_score(handle: *mut c_void, frame_id: *const c_char) -> f32;
    fn seraph_store_seed_margin(handle: *mut c_void, frame_id: *const c_char) -> f32;
    fn seraph_store_steerability(handle: *mut c_void, query_embedding: *const f32, query_len: usize, target_id: *const c_char) -> f32;
    fn seraph_store_region_health(handle: *mut c_void, eigenframe_id: *const c_char) -> *mut c_char;
    fn seraph_store_similarity_stats(handle: *mut c_void) -> *mut c_char;
    fn seraph_store_resolve_tau(handle: *mut c_void) -> f32;
    fn seraph_store_drift_report(handle: *mut c_void, window: usize, threshold: f32) -> *mut c_char;
    fn seraph_store_contradictions(handle: *mut c_void, min_divergence: f32) -> *mut c_char;
    fn seraph_store_evolution(handle: *mut c_void, seed_frame_id: *const c_char) -> *mut c_char;
    fn seraph_store_merge_candidates(handle: *mut c_void, tau: f32) -> *mut c_char;
    fn seraph_store_consolidate(handle: *mut c_void, frame_ids_json: *const c_char) -> *mut c_char;
    fn seraph_store_consolidate_all(handle: *mut c_void) -> *mut c_char;

    // Provenance
    fn seraph_store_writer_audit(handle: *mut c_void) -> *mut c_char;
    fn seraph_store_verify_attestation(handle: *mut c_void) -> *mut c_char;
    fn seraph_store_genesis_seed(handle: *mut c_void, out_seed: *mut *mut u8, out_len: *mut usize) -> i32;
    fn seraph_store_genesis_attestation(handle: *mut c_void) -> *mut c_char;

    // Typed edges
    fn seraph_store_declare_relation(handle: *mut c_void, name: *const c_char, flags: u32) -> i32;
    fn seraph_store_list_relations(handle: *mut c_void) -> *mut c_char;
    fn seraph_store_assert_relation(handle: *mut c_void, from_id: *const c_char, to_id: *const c_char, relation: *const c_char, weight: f32) -> i32;
    fn seraph_store_retract_relation(handle: *mut c_void, from_id: *const c_char, to_id: *const c_char, relation: *const c_char) -> i32;
    fn seraph_store_structural_neighbors(handle: *mut c_void, frame_id: *const c_char, relations_json: *const c_char, direction: *const c_char) -> *mut c_char;

    // Encoder
    fn seraph_encoder_create(model_id: *const c_char) -> *mut c_void;
    fn seraph_encoder_encode(
        handle: *mut c_void,
        text: *const c_char,
        out_embedding: *mut *mut f32, out_dim: *mut usize,
    ) -> i32;
    fn seraph_encoder_encode_batch(handle: *mut c_void, texts_json: *const c_char) -> *mut c_char;
    fn seraph_encoder_encode_batch_flat(
        handle: *mut c_void,
        texts: *const *const c_char,
        count: usize,
        out_embeddings: *mut *mut f32,
        out_dim: *mut usize,
    ) -> i32;
    fn seraph_encoder_close(handle: *mut c_void);

    // Event bus
    fn seraph_store_subscribe(
        handle: *mut c_void,
        event_name: *const c_char,
        callback: extern "C" fn(*const c_char, *mut c_void),
        user_data: *mut c_void,
    ) -> u64;
    fn seraph_store_unsubscribe(handle: *mut c_void, subscriber_id: u64) -> i32;
    fn seraph_store_subscriber_count(handle: *mut c_void) -> usize;

    // Warp calibration
    fn seraph_store_calibrate_warp(
        handle: *mut c_void,
        n_queries: usize, n_targets: usize,
        magnitudes_json: *const c_char,
        top_k: usize, seed: u64,
    ) -> *mut c_char;
    fn seraph_store_fixup_warp(handle: *mut c_void) -> i32;
    fn seraph_store_warp_calibration(handle: *mut c_void) -> *mut c_char;

    // Snapshot / temporal
    fn seraph_store_snapshot(handle: *mut c_void, at_timestamp_us: i64) -> *mut c_void;
    fn seraph_snapshot_frame_ids(snapshot: *const c_void) -> *mut c_char;
    fn seraph_snapshot_timestamp(snapshot: *const c_void) -> i64;
    fn seraph_store_snapshot_search(
        handle: *mut c_void, snapshot: *const c_void,
        query_embedding: *const f32, query_len: usize,
        top_k: usize,
    ) -> *mut c_char;
    fn seraph_snapshot_free(snapshot: *mut c_void);

    // WarpSpec resolution
    fn seraph_store_search_with_warp_spec(
        handle: *mut c_void,
        query_embedding: *const f32, query_len: usize,
        warp_spec_json: *const c_char,
        top_k: usize, tau: f32,
        out_hits: *mut *mut FfiHit, out_count: *mut usize,
    ) -> i32;
    fn seraph_store_warp_query(
        handle: *mut c_void,
        query_embedding: *const f32, query_len: usize,
        warp_spec_json: *const c_char,
        out_embedding: *mut *mut f32, out_dim: *mut usize,
    ) -> i32;

    // Zero-copy accommodations
    fn seraph_store_region_health_struct(
        handle: *mut c_void, eigenframe_id: *const c_char,
        out: *mut FfiRegionHealth,
    ) -> i32;
    fn seraph_store_similarity_stats_struct(
        handle: *mut c_void,
        out: *mut FfiSimilarityStats,
    ) -> i32;
    fn seraph_store_get_embedding_into(
        handle: *mut c_void, frame_id: *const c_char,
        buf: *mut f32, buf_len: usize, out_written: *mut usize,
    ) -> i32;
    fn seraph_encoder_encode_into(
        handle: *mut c_void, text: *const c_char,
        buf: *mut f32, buf_len: usize, out_written: *mut usize,
    ) -> i32;
    fn seraph_store_borrow_frame(
        handle: *mut c_void, frame_id: *const c_char,
    ) -> *mut FfiFrameView;
    fn seraph_store_release_frame(view: *mut FfiFrameView);

    // Writer identity (Standard-mode attribution) + encrypted stores
    fn seraph_store_create_with_writer(
        path: *const c_char, model_id: *const c_char,
        config_json: *const c_char, writer_seed: *const u8,
    ) -> *mut c_void;
    fn seraph_store_open_with_writer(path: *const c_char, writer_seed: *const u8) -> *mut c_void;
    fn seraph_store_create_encrypted(
        path: *const c_char, model_id: *const c_char, config_json: *const c_char,
        passphrase: *const u8, passphrase_len: usize,
    ) -> *mut c_void;
    fn seraph_store_open_encrypted(
        path: *const c_char, passphrase: *const u8, passphrase_len: usize,
    ) -> *mut c_void;

    // Time anchoring (tip-anchor sentinels)
    fn seraph_store_append_tip_anchor(
        handle: *mut c_void, backend: *const c_char,
        token: *const u8, token_len: usize,
        tip_frame_id: *const c_char, status: *const c_char, supersedes: *const c_char,
    ) -> *mut c_char;
    fn seraph_store_list_tip_anchors(handle: *mut c_void, resolved: i32) -> *mut c_char;

    // Chain analytics — gradient + constitutive score
    fn seraph_store_gradient(handle: *mut c_void, chain_json: *const c_char) -> *mut c_char;
    fn seraph_store_constitutive_score(handle: *mut c_void, chain_json: *const c_char) -> f32;

    // Hit-list pipeline — filter + structural expand
    fn seraph_store_filter_by_relation(
        handle: *mut c_void, hits_json: *const c_char,
        relations_json: *const c_char, direction: *const c_char, require_present: bool,
    ) -> *mut c_char;
    fn seraph_store_structural_expand(
        handle: *mut c_void, hits_json: *const c_char,
        relations_json: *const c_char, direction: *const c_char, score_combine: *const c_char,
    ) -> *mut c_char;

    // Re-encode pipeline (v2/v1 source -> fresh v3 destination)
    fn seraph_reencode_store(
        src_path: *const c_char, dst_path: *const c_char, model_id: *const c_char,
        eigen_threshold: u32, n_max: usize, pbatch: usize, ebatch: usize, out_n: *mut usize,
    ) -> i32;
}

// FFI repr(C) types matching the engine's ffi.rs
#[repr(C)]
struct FfiHit {
    frame_id: *mut c_char,
    score: f32,
    confidence: f32,
    path_length: u32,
}

#[repr(C)]
struct FfiStats {
    total_frames: usize,
    active_frames: usize,
    eigenframes: usize,
    superseded: usize,
    redirects: usize,
}

#[repr(C)]
struct FfiRegionHealth {
    coherence: f32,
    direction: u8,
    candidate_split: u8,
}

#[repr(C)]
struct FfiSimilarityStats {
    count: usize,
    mean: f32,
    p75: f32,
    p90: f32,
    p95: f32,
    p99: f32,
}

#[repr(C)]
struct FfiFrameView {
    id: *const c_char,
    parent_id: *const c_char,
    embedding: *const f32,
    embedding_dim: usize,
    content: *const u8,
    content_len: usize,
    in_degree: u32,
    status: u8,
    timestamp: i64,
    _frame_box: *const c_void,
    _id_cstr: *const c_void,
    _parent_cstr: *const c_void,
}

// ---------------------------------------------------------------- //
// Error type
// ---------------------------------------------------------------- //

/// Error returned by SERAPH operations.
#[derive(Debug)]
pub struct Error {
    message: String,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

fn last_error() -> Error {
    let msg = unsafe {
        let p = seraph_last_error();
        if p.is_null() {
            "unknown error".to_string()
        } else {
            CStr::from_ptr(p).to_string_lossy().into_owned()
        }
    };
    Error { message: msg }
}

// ---------------------------------------------------------------- //
// Helper: C string conversions
// ---------------------------------------------------------------- //

fn to_cstring(s: &str) -> CString {
    CString::new(s).expect("string contains interior NUL")
}

fn path_to_cstring(p: &Path) -> CString {
    to_cstring(&p.to_string_lossy())
}

/// Take ownership of a C string, convert to Rust String, free the C memory.
unsafe fn take_cstring(p: *mut c_char) -> String {
    let s = unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
    unsafe { seraph_string_free(p) };
    s
}

// ---------------------------------------------------------------- //
// Hit
// ---------------------------------------------------------------- //

/// A search result.
#[derive(Debug, Clone)]
pub struct Hit {
    pub frame_id: String,
    pub score: f32,
    pub confidence: f32,
    pub path_length: u32,
}

// ---------------------------------------------------------------- //
// Stats
// ---------------------------------------------------------------- //

/// Store statistics.
#[derive(Debug, Clone)]
pub struct Stats {
    pub total_frames: usize,
    pub active_frames: usize,
    pub eigenframes: usize,
    pub superseded: usize,
    pub redirects: usize,
}

// ---------------------------------------------------------------- //
// RegionHealth / SimilarityStats — zero-copy struct returns
// ---------------------------------------------------------------- //

/// Region health for an eigenframe (zero-copy, no JSON).
#[derive(Debug, Clone)]
pub struct RegionHealth {
    pub coherence: f32,
    pub direction: RegionDirection,
    pub candidate_split: bool,
}

/// Direction of a region's coherence trend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionDirection {
    Stable,
    Consolidating,
    Fragmenting,
}

/// Parent-child similarity distribution (zero-copy, no JSON).
#[derive(Debug, Clone)]
pub struct SimilarityStats {
    pub count: usize,
    pub mean: f32,
    pub p75: f32,
    pub p90: f32,
    pub p95: f32,
    pub p99: f32,
}

/// A borrowed view of a frame — zero-copy read from engine memory.
///
/// Created via [`Store::borrow_frame`]. All data is valid until the
/// `FrameView` is dropped. Do not hold across store mutations.
pub struct FrameView {
    ptr: *mut FfiFrameView,
}

impl FrameView {
    /// Frame ID.
    pub fn id(&self) -> &str {
        unsafe {
            let v = &*self.ptr;
            CStr::from_ptr(v.id).to_str().unwrap_or("")
        }
    }

    /// Parent frame ID, or None for root frames.
    pub fn parent_id(&self) -> Option<&str> {
        unsafe {
            let v = &*self.ptr;
            if v.parent_id.is_null() { None }
            else { CStr::from_ptr(v.parent_id).to_str().ok() }
        }
    }

    /// Embedding slice (points into engine memory).
    pub fn embedding(&self) -> &[f32] {
        unsafe {
            let v = &*self.ptr;
            std::slice::from_raw_parts(v.embedding, v.embedding_dim)
        }
    }

    /// Content bytes (points into engine memory).
    pub fn content(&self) -> &[u8] {
        unsafe {
            let v = &*self.ptr;
            std::slice::from_raw_parts(v.content, v.content_len)
        }
    }

    /// In-degree (number of children pointing to this frame).
    pub fn in_degree(&self) -> u32 {
        unsafe { (*self.ptr).in_degree }
    }

    /// Frame status as a numeric code.
    /// 0=Genesis, 1=Seed, 2=Active, 3=Eigenframe, 4=Superseded, 5=Redirect, 6=System.
    pub fn status(&self) -> u8 {
        unsafe { (*self.ptr).status }
    }

    /// Frame timestamp (microseconds since epoch).
    pub fn timestamp(&self) -> i64 {
        unsafe { (*self.ptr).timestamp }
    }
}

impl Drop for FrameView {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe { seraph_store_release_frame(self.ptr) };
            self.ptr = ptr::null_mut();
        }
    }
}

// ---------------------------------------------------------------- //
// StoreConfig (builder)
// ---------------------------------------------------------------- //

/// Configuration for creating a new SERAPH store.
///
/// Use the builder pattern:
/// ```no_run
/// use seraph::StoreConfig;
/// let config = StoreConfig::new()
///     .model_id("BAAI/bge-small-en-v1.5")
///     .eigen_threshold(5)
///     .tau_similarity(0.5);
/// ```
#[derive(Debug, Clone, Default)]
pub struct StoreConfig {
    model_id: Option<String>,
    json_overrides: serde_json::Map<String, serde_json::Value>,
}

impl StoreConfig {
    pub fn new() -> Self { Self::default() }

    pub fn model_id(mut self, id: &str) -> Self {
        self.model_id = Some(id.to_string());
        self
    }

    pub fn eigen_threshold(mut self, v: u32) -> Self {
        self.json_overrides.insert("eigen_threshold".into(), v.into());
        self
    }

    pub fn eigen_min_age_s(mut self, v: f64) -> Self {
        self.json_overrides.insert("eigen_min_age_s".into(), serde_json::Value::from(v));
        self
    }

    pub fn tau_similarity(mut self, v: f32) -> Self {
        self.json_overrides.insert("tau_similarity".into(), serde_json::Value::from(v));
        self
    }

    pub fn tau_merge(mut self, v: f32) -> Self {
        self.json_overrides.insert("tau_merge".into(), serde_json::Value::from(v));
        self
    }

    pub fn search_max_depth(mut self, v: u32) -> Self {
        self.json_overrides.insert("search_max_depth".into(), v.into());
        self
    }

    pub fn search_visit_cap_multiplier(mut self, v: u32) -> Self {
        self.json_overrides.insert("search_visit_cap_multiplier".into(), v.into());
        self
    }

    pub fn eigen_matmul_min(mut self, v: usize) -> Self {
        self.json_overrides.insert("eigen_matmul_min".into(), (v as u64).into());
        self
    }

    fn to_json_cstring(&self) -> Option<CString> {
        if self.json_overrides.is_empty() {
            None
        } else {
            let v = serde_json::Value::Object(self.json_overrides.clone());
            CString::new(v.to_string()).ok()
        }
    }
}

// ---------------------------------------------------------------- //
// Store
// ---------------------------------------------------------------- //

/// A SERAPH store handle.
///
/// This is the main entry point for interacting with SERAPH. It wraps
/// an opaque handle to the compiled engine via C-ABI FFI.
pub struct Store {
    handle: *mut c_void,
}

// SAFETY: The underlying SeraphStore is internally synchronized where
// needed (Mutex on mutable state). The C-ABI handle is a stable pointer.
unsafe impl Send for Store {}

impl Drop for Store {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe { seraph_store_close(self.handle) };
            self.handle = ptr::null_mut();
        }
    }
}

impl Store {
    /// Create a new SERAPH store at the given path.
    ///
    /// Pass `None` for config to use defaults (model: `BAAI/bge-small-en-v1.5`).
    pub fn create(path: impl AsRef<Path>, config: Option<StoreConfig>) -> Result<Self, Error> {
        let path_c = path_to_cstring(path.as_ref());
        let config = config.unwrap_or_default();

        let model_c = config.model_id.as_deref().map(to_cstring);
        let json_c = config.to_json_cstring();

        let handle = unsafe {
            seraph_store_create(
                path_c.as_ptr(),
                model_c.as_ref().map_or(ptr::null(), |c| c.as_ptr()),
                json_c.as_ref().map_or(ptr::null(), |c| c.as_ptr()),
            )
        };

        if handle.is_null() {
            Err(last_error())
        } else {
            Ok(Store { handle })
        }
    }

    /// Open an existing SERAPH store.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path_c = path_to_cstring(path.as_ref());
        let handle = unsafe { seraph_store_open(path_c.as_ptr()) };
        if handle.is_null() {
            Err(last_error())
        } else {
            Ok(Store { handle })
        }
    }

    /// Create a store that signs frames with an app-supplied writer identity
    /// (Standard-mode writer attribution). `writer_seed` is a 32-byte Ed25519
    /// private seed.
    pub fn create_with_writer(
        path: impl AsRef<Path>,
        config: Option<StoreConfig>,
        writer_seed: &[u8; 32],
    ) -> Result<Self, Error> {
        let path_c = path_to_cstring(path.as_ref());
        let config = config.unwrap_or_default();
        let model_c = config.model_id.as_deref().map(to_cstring);
        let json_c = config.to_json_cstring();

        let handle = unsafe {
            seraph_store_create_with_writer(
                path_c.as_ptr(),
                model_c.as_ref().map_or(ptr::null(), |c| c.as_ptr()),
                json_c.as_ref().map_or(ptr::null(), |c| c.as_ptr()),
                writer_seed.as_ptr(),
            )
        };
        if handle.is_null() { Err(last_error()) } else { Ok(Store { handle }) }
    }

    /// Open a store with an app-supplied writer identity for signing frames
    /// appended during this session. `writer_seed` is a 32-byte Ed25519 seed.
    pub fn open_with_writer(path: impl AsRef<Path>, writer_seed: &[u8; 32]) -> Result<Self, Error> {
        let path_c = path_to_cstring(path.as_ref());
        let handle = unsafe { seraph_store_open_with_writer(path_c.as_ptr(), writer_seed.as_ptr()) };
        if handle.is_null() { Err(last_error()) } else { Ok(Store { handle }) }
    }

    /// Create an encrypted store. The passphrase derives an AES-256-GCM key via
    /// Argon2id; content is encrypted at rest while structural metadata stays
    /// plaintext for keyless chain verification.
    pub fn create_encrypted(
        path: impl AsRef<Path>,
        config: Option<StoreConfig>,
        passphrase: &str,
    ) -> Result<Self, Error> {
        let path_c = path_to_cstring(path.as_ref());
        let config = config.unwrap_or_default();
        let model_c = config.model_id.as_deref().map(to_cstring);
        let json_c = config.to_json_cstring();
        let pass = passphrase.as_bytes();

        let handle = unsafe {
            seraph_store_create_encrypted(
                path_c.as_ptr(),
                model_c.as_ref().map_or(ptr::null(), |c| c.as_ptr()),
                json_c.as_ref().map_or(ptr::null(), |c| c.as_ptr()),
                pass.as_ptr(), pass.len(),
            )
        };
        if handle.is_null() { Err(last_error()) } else { Ok(Store { handle }) }
    }

    /// Open an encrypted store with its passphrase.
    pub fn open_encrypted(path: impl AsRef<Path>, passphrase: &str) -> Result<Self, Error> {
        let path_c = path_to_cstring(path.as_ref());
        let pass = passphrase.as_bytes();
        let handle = unsafe {
            seraph_store_open_encrypted(path_c.as_ptr(), pass.as_ptr(), pass.len())
        };
        if handle.is_null() { Err(last_error()) } else { Ok(Store { handle }) }
    }

    /// Insert a frame with raw content bytes and a pre-computed embedding.
    pub fn put(&mut self, content: &[u8], embedding: &[f32]) -> Result<String, Error> {
        let id = unsafe {
            seraph_store_put(
                self.handle,
                content.as_ptr(), content.len(),
                embedding.as_ptr(), embedding.len(),
            )
        };
        if id.is_null() {
            Err(last_error())
        } else {
            Ok(unsafe { take_cstring(id) })
        }
    }

    /// Insert a frame with content, embedding, and JSON metadata.
    pub fn put_with_metadata(
        &mut self,
        content: &[u8],
        embedding: &[f32],
        metadata_json: &str,
    ) -> Result<String, Error> {
        let meta_c = to_cstring(metadata_json);
        let id = unsafe {
            seraph_store_put_with_metadata(
                self.handle,
                content.as_ptr(), content.len(),
                embedding.as_ptr(), embedding.len(),
                meta_c.as_ptr(),
            )
        };
        if id.is_null() {
            Err(last_error())
        } else {
            Ok(unsafe { take_cstring(id) })
        }
    }

    /// Search with a query embedding.
    ///
    /// Returns up to `top_k` hits above the similarity threshold `tau`.
    /// Set `tau` to `0.0` to return all results regardless of similarity.
    pub fn search(&self, query_embedding: &[f32], top_k: usize, tau: f32) -> Result<Vec<Hit>, Error> {
        let mut hits_ptr: *mut FfiHit = ptr::null_mut();
        let mut count: usize = 0;

        let rc = unsafe {
            seraph_store_search(
                self.handle,
                query_embedding.as_ptr(), query_embedding.len(),
                top_k, tau,
                &mut hits_ptr, &mut count,
            )
        };

        if rc != 0 {
            return Err(last_error());
        }

        Ok(take_hits(hits_ptr, count))
    }

    /// Search with options. Same as [`search`](Self::search) plus
    /// `include_superseded`: when `true`, plainly-superseded (soft-forgotten)
    /// frames still resident in the graph are opted back into results.
    /// Consolidated sources resolve through their redirect and are not
    /// resurfaced. `include_superseded = false` matches [`search`](Self::search).
    pub fn search_opts(
        &self,
        query_embedding: &[f32],
        top_k: usize,
        tau: f32,
        include_superseded: bool,
    ) -> Result<Vec<Hit>, Error> {
        let mut hits_ptr: *mut FfiHit = ptr::null_mut();
        let mut count: usize = 0;

        let rc = unsafe {
            seraph_store_search_opts(
                self.handle,
                query_embedding.as_ptr(), query_embedding.len(),
                top_k, tau,
                include_superseded,
                &mut hits_ptr, &mut count,
            )
        };

        if rc != 0 {
            return Err(last_error());
        }

        Ok(take_hits(hits_ptr, count))
    }

    /// Get a frame's content bytes by ID.
    pub fn get_content(&self, frame_id: &str) -> Result<Option<Vec<u8>>, Error> {
        let fid_c = to_cstring(frame_id);
        let mut out_ptr: *mut u8 = ptr::null_mut();
        let mut out_len: usize = 0;

        let rc = unsafe {
            seraph_store_get_content(
                self.handle, fid_c.as_ptr(),
                &mut out_ptr, &mut out_len,
            )
        };

        match rc {
            0 => {
                let bytes = unsafe { std::slice::from_raw_parts(out_ptr, out_len) }.to_vec();
                unsafe { seraph_bytes_free(out_ptr, out_len) };
                Ok(Some(bytes))
            }
            1 => Ok(None), // not found
            _ => Err(last_error()),
        }
    }

    /// Get a frame's metadata as a JSON string.
    pub fn get_metadata(&self, frame_id: &str) -> Option<String> {
        let fid_c = to_cstring(frame_id);
        let p = unsafe { seraph_store_get_metadata(self.handle, fid_c.as_ptr()) };
        if p.is_null() {
            None
        } else {
            Some(unsafe { take_cstring(p) })
        }
    }

    /// Get the embedding model ID.
    pub fn model_id(&self) -> String {
        let p = unsafe { seraph_store_model_id(self.handle) };
        if p.is_null() { String::new() } else { unsafe { take_cstring(p) } }
    }

    /// Get the embedding dimension.
    pub fn dim(&self) -> usize {
        unsafe { seraph_store_dim(self.handle) }
    }

    /// Get all frame IDs.
    pub fn frame_ids(&self) -> Vec<String> {
        let p = unsafe { seraph_store_frame_ids(self.handle) };
        if p.is_null() { return Vec::new(); }
        let json = unsafe { take_cstring(p) };
        serde_json::from_str(&json).unwrap_or_default()
    }

    /// Get eigenframe IDs.
    pub fn eigenframe_ids(&self) -> Vec<String> {
        let p = unsafe { seraph_store_eigenframe_ids(self.handle) };
        if p.is_null() { return Vec::new(); }
        let json = unsafe { take_cstring(p) };
        serde_json::from_str(&json).unwrap_or_default()
    }

    /// Get store statistics.
    pub fn stats(&self) -> Result<Stats, Error> {
        let mut ffi = FfiStats {
            total_frames: 0, active_frames: 0, eigenframes: 0,
            superseded: 0, redirects: 0,
        };
        let rc = unsafe { seraph_store_stats(self.handle, &mut ffi) };
        if rc != 0 { return Err(last_error()); }
        Ok(Stats {
            total_frames: ffi.total_frames,
            active_frames: ffi.active_frames,
            eigenframes: ffi.eigenframes,
            superseded: ffi.superseded,
            redirects: ffi.redirects,
        })
    }

    /// Verify watermark chain integrity.
    ///
    /// Returns `(ok, failure_frame_ids)`.
    pub fn verify(&self) -> Result<(bool, Vec<String>), Error> {
        let mut failures_ptr: *mut c_char = ptr::null_mut();
        let rc = unsafe {
            seraph_store_verify(self.handle, &mut failures_ptr)
        };
        match rc {
            -1 => Err(last_error()),
            _ => {
                let failures: Vec<String> = if !failures_ptr.is_null() {
                    let json = unsafe { take_cstring(failures_ptr) };
                    serde_json::from_str(&json).unwrap_or_default()
                } else {
                    Vec::new()
                };
                Ok((rc == 0, failures))
            }
        }
    }

    /// Seal the store (makes it immutable).
    pub fn seal(&mut self, reason: Option<&str>) -> Result<String, Error> {
        let reason_c = reason.map(to_cstring);
        let id = unsafe {
            seraph_store_seal(
                self.handle,
                reason_c.as_ref().map_or(ptr::null(), |c| c.as_ptr()),
            )
        };
        if id.is_null() { Err(last_error()) } else { Ok(unsafe { take_cstring(id) }) }
    }

    /// Check if the store is sealed.
    pub fn is_sealed(&self) -> bool {
        unsafe { seraph_store_is_sealed(self.handle) == 1 }
    }

    /// Mark a frame as superseded (soft forget — status flipped in place).
    ///
    /// Does not create a new frame or redirect; returns the same frame's ID
    /// echoed back. Exempt frames (Genesis/Seed/Redirect/System) error.
    pub fn supersede(&mut self, frame_id: &str) -> Result<String, Error> {
        let fid_c = to_cstring(frame_id);
        let id = unsafe { seraph_store_supersede(self.handle, fid_c.as_ptr()) };
        if id.is_null() { Err(last_error()) } else { Ok(unsafe { take_cstring(id) }) }
    }

    /// Flush pending writes to disk.
    pub fn sync(&mut self) -> Result<(), Error> {
        let rc = unsafe { seraph_store_sync(self.handle) };
        if rc == 0 { Ok(()) } else { Err(last_error()) }
    }

    // ---------------------------------------------------------------- //
    // Time anchoring
    // ---------------------------------------------------------------- //

    /// Append a tip-anchor sentinel frame. `token` is the anchor proof bytes
    /// (may be empty). `supersedes` optionally names an anchor this one replaces.
    /// Returns the anchor frame's ID.
    pub fn append_tip_anchor(
        &mut self,
        backend: &str,
        token: &[u8],
        tip_frame_id: &str,
        status: &str,
        supersedes: Option<&str>,
    ) -> Result<String, Error> {
        let backend_c = to_cstring(backend);
        let tip_c = to_cstring(tip_frame_id);
        let status_c = to_cstring(status);
        let supersedes_c = supersedes.map(to_cstring);
        let id = unsafe {
            seraph_store_append_tip_anchor(
                self.handle,
                backend_c.as_ptr(),
                token.as_ptr(), token.len(),
                tip_c.as_ptr(), status_c.as_ptr(),
                supersedes_c.as_ref().map_or(ptr::null(), |c| c.as_ptr()),
            )
        };
        if id.is_null() { Err(last_error()) } else { Ok(unsafe { take_cstring(id) }) }
    }

    /// List tip-anchor records as a JSON array. `resolved` filters to anchors
    /// whose proof has resolved.
    pub fn list_tip_anchors_json(&self, resolved: bool) -> Result<String, Error> {
        let p = unsafe { seraph_store_list_tip_anchors(self.handle, resolved as i32) };
        if p.is_null() { Err(last_error()) } else { Ok(unsafe { take_cstring(p) }) }
    }

    // ---------------------------------------------------------------- //
    // Ingest — batch, seed, promote
    // ---------------------------------------------------------------- //

    /// Batch insert content/embedding pairs. Returns frame IDs.
    ///
    /// `items_json` is a JSON array of `{"content": "...", "embedding": [...]}` or
    /// `{"content_b64": "...", "embedding": [...]}`.
    pub fn put_batch_json(&mut self, items_json: &str) -> Result<Vec<String>, Error> {
        let json_c = to_cstring(items_json);
        let p = unsafe { seraph_store_put_batch(self.handle, json_c.as_ptr()) };
        if p.is_null() { return Err(last_error()); }
        let json = unsafe { take_cstring(p) };
        Ok(serde_json::from_str(&json).unwrap_or_default())
    }

    /// Add a seed to a live store.
    pub fn add_seed(&mut self, label: &str, embedding: &[f32], metadata_json: Option<&str>) -> Result<String, Error> {
        let label_c = to_cstring(label);
        let meta_c = metadata_json.map(to_cstring);
        let id = unsafe {
            seraph_store_add_seed(
                self.handle, label_c.as_ptr(),
                embedding.as_ptr(), embedding.len(),
                meta_c.as_ref().map_or(ptr::null(), |c| c.as_ptr()),
            )
        };
        if id.is_null() { Err(last_error()) } else { Ok(unsafe { take_cstring(id) }) }
    }

    /// Promote an active frame to eigenframe.
    pub fn promote(&mut self, frame_id: &str) -> Result<(), Error> {
        let fid_c = to_cstring(frame_id);
        let rc = unsafe { seraph_store_promote(self.handle, fid_c.as_ptr()) };
        if rc == 0 { Ok(()) } else { Err(last_error()) }
    }

    /// Ingest content directly as eigenframe (atomic ingest + promote).
    pub fn promote_content(
        &mut self, content: &[u8], embedding: &[f32], metadata_json: Option<&str>,
    ) -> Result<String, Error> {
        let meta_c = metadata_json.map(to_cstring);
        let id = unsafe {
            seraph_store_promote_content(
                self.handle,
                content.as_ptr(), content.len(),
                embedding.as_ptr(), embedding.len(),
                meta_c.as_ref().map_or(ptr::null(), |c| c.as_ptr()),
            )
        };
        if id.is_null() { Err(last_error()) } else { Ok(unsafe { take_cstring(id) }) }
    }

    // ---------------------------------------------------------------- //
    // Lifecycle — batch mode
    // ---------------------------------------------------------------- //

    /// Enable buffered write mode (skip per-frame WAL + fsync).
    pub fn begin_batch(&mut self) -> Result<(), Error> {
        let rc = unsafe { seraph_store_begin_batch(self.handle) };
        if rc == 0 { Ok(()) } else { Err(last_error()) }
    }

    /// Flush buffered writes and re-enable per-write fsync.
    pub fn commit(&mut self) -> Result<(), Error> {
        let rc = unsafe { seraph_store_commit(self.handle) };
        if rc == 0 { Ok(()) } else { Err(last_error()) }
    }

    /// Set the visit cap multiplier for Phase-2 BFS.
    /// `max_visited = top_k * multiplier`. Default is 5.
    pub fn set_search_visit_cap_multiplier(&mut self, multiplier: u32) -> Result<(), Error> {
        let rc = unsafe { seraph_store_set_search_visit_cap_multiplier(self.handle, multiplier) };
        if rc == 0 { Ok(()) } else { Err(last_error()) }
    }

    // ---------------------------------------------------------------- //
    // Frame access
    // ---------------------------------------------------------------- //

    /// Get a frame as a JSON string with all properties.
    pub fn get_frame_json(&self, frame_id: &str) -> Option<String> {
        let fid_c = to_cstring(frame_id);
        let p = unsafe { seraph_store_get_frame(self.handle, fid_c.as_ptr()) };
        if p.is_null() { None } else { Some(unsafe { take_cstring(p) }) }
    }

    /// Get a frame's embedding.
    pub fn get_embedding(&self, frame_id: &str) -> Result<Option<Vec<f32>>, Error> {
        let fid_c = to_cstring(frame_id);
        let mut out_ptr: *mut f32 = ptr::null_mut();
        let mut out_dim: usize = 0;
        let rc = unsafe {
            seraph_store_get_embedding(self.handle, fid_c.as_ptr(), &mut out_ptr, &mut out_dim)
        };
        match rc {
            0 => {
                let emb = unsafe { std::slice::from_raw_parts(out_ptr, out_dim) }.to_vec();
                unsafe { seraph_floats_free(out_ptr, out_dim) };
                Ok(Some(emb))
            }
            1 => Ok(None),
            _ => Err(last_error()),
        }
    }

    // ---------------------------------------------------------------- //
    // Graph basics
    // ---------------------------------------------------------------- //

    /// Get children of a frame.
    pub fn get_children(&self, frame_id: &str) -> Vec<String> {
        let fid_c = to_cstring(frame_id);
        let p = unsafe { seraph_store_get_children(self.handle, fid_c.as_ptr()) };
        if p.is_null() { return Vec::new(); }
        let json = unsafe { take_cstring(p) };
        serde_json::from_str(&json).unwrap_or_default()
    }

    /// Get eigenframes as JSON array of frame objects.
    pub fn eigenframes_json(&self) -> String {
        let p = unsafe { seraph_store_eigenframes(self.handle) };
        if p.is_null() { "[]".to_string() } else { unsafe { take_cstring(p) } }
    }

    /// Mean of eigenframe embeddings — one vector for federation routing.
    /// Returns `None` if the store has no eigenframes.
    pub fn federation_vector(&self) -> Option<Vec<f32>> {
        let mut ptr: *mut f32 = ptr::null_mut();
        let mut dim: usize = 0;
        let rc = unsafe { seraph_store_federation_vector(self.handle, &mut ptr, &mut dim) };
        if rc != 0 || ptr.is_null() || dim == 0 {
            return None;
        }
        let vec = unsafe { Vec::from_raw_parts(ptr, dim, dim) };
        Some(vec)
    }

    /// Walk parent chain back to genesis.
    pub fn lineage(&self, frame_id: &str) -> Vec<String> {
        let fid_c = to_cstring(frame_id);
        let p = unsafe { seraph_store_lineage(self.handle, fid_c.as_ptr()) };
        if p.is_null() { return Vec::new(); }
        let json = unsafe { take_cstring(p) };
        serde_json::from_str(&json).unwrap_or_default()
    }

    /// BFS path between two frames.
    pub fn path(&self, from_id: &str, to_id: &str) -> Vec<String> {
        let from_c = to_cstring(from_id);
        let to_c = to_cstring(to_id);
        let p = unsafe { seraph_store_path(self.handle, from_c.as_ptr(), to_c.as_ptr()) };
        if p.is_null() { return Vec::new(); }
        let json = unsafe { take_cstring(p) };
        serde_json::from_str(&json).unwrap_or_default()
    }

    /// All descendants of a frame. `max_depth` of 0 means unlimited.
    pub fn subtree(&self, frame_id: &str, max_depth: usize) -> Vec<String> {
        let fid_c = to_cstring(frame_id);
        let p = unsafe { seraph_store_subtree(self.handle, fid_c.as_ptr(), max_depth) };
        if p.is_null() { return Vec::new(); }
        let json = unsafe { take_cstring(p) };
        serde_json::from_str(&json).unwrap_or_default()
    }

    // ---------------------------------------------------------------- //
    // Search variants
    // ---------------------------------------------------------------- //

    /// Search by text query (auto-encodes via store's model).
    pub fn search_text(&self, query: &str, top_k: usize, tau: f32) -> Result<Vec<Hit>, Error> {
        let query_c = to_cstring(query);
        let mut hits_ptr: *mut FfiHit = ptr::null_mut();
        let mut count: usize = 0;
        let rc = unsafe {
            seraph_store_search_text(
                self.handle, query_c.as_ptr(),
                top_k, tau,
                &mut hits_ptr, &mut count,
            )
        };
        if rc != 0 { return Err(last_error()); }
        Ok(take_hits(hits_ptr, count))
    }

    /// Warped search.
    ///
    /// `targets_json` / `suppress_json`: JSON arrays of `{"embedding": [...], "magnitude": f32}`.
    pub fn search_with_warp(
        &self,
        query_embedding: &[f32],
        targets_json: &str,
        suppress_json: &str,
        profile: &str,
        bound_k: f32,
        top_k: usize,
        tau: f32,
    ) -> Result<Vec<Hit>, Error> {
        let targets_c = to_cstring(targets_json);
        let suppress_c = to_cstring(suppress_json);
        let profile_c = to_cstring(profile);
        let mut hits_ptr: *mut FfiHit = ptr::null_mut();
        let mut count: usize = 0;
        let rc = unsafe {
            seraph_store_search_with_warp(
                self.handle,
                query_embedding.as_ptr(), query_embedding.len(),
                targets_c.as_ptr(), suppress_c.as_ptr(),
                profile_c.as_ptr(), bound_k,
                top_k, tau,
                &mut hits_ptr, &mut count,
            )
        };
        if rc != 0 { return Err(last_error()); }
        Ok(take_hits(hits_ptr, count))
    }

    // ---------------------------------------------------------------- //
    // Neighborhood / Chain
    // ---------------------------------------------------------------- //

    /// Three-population neighborhood. Returns JSON.
    pub fn neighborhood_json(&self, frame_id: &str, tau: f32, hops: usize) -> Result<String, Error> {
        let fid_c = to_cstring(frame_id);
        let p = unsafe { seraph_store_neighborhood(self.handle, fid_c.as_ptr(), tau, hops) };
        if p.is_null() { return Err(last_error()); }
        Ok(unsafe { take_cstring(p) })
    }

    /// Chain between two frames. Returns JSON.
    pub fn chain_json(&self, from_id: &str, to_id: &str) -> Result<String, Error> {
        let from_c = to_cstring(from_id);
        let to_c = to_cstring(to_id);
        let p = unsafe { seraph_store_chain(self.handle, from_c.as_ptr(), to_c.as_ptr()) };
        if p.is_null() { return Err(last_error()); }
        Ok(unsafe { take_cstring(p) })
    }

    /// Chain to nearest frame from a query embedding. Returns JSON.
    pub fn chain_to_json(&self, query_embedding: &[f32]) -> Result<String, Error> {
        let p = unsafe {
            seraph_store_chain_to(self.handle, query_embedding.as_ptr(), query_embedding.len())
        };
        if p.is_null() { return Err(last_error()); }
        Ok(unsafe { take_cstring(p) })
    }

    /// Chain between two query embeddings. Returns JSON.
    pub fn chain_between_json(&self, emb_a: &[f32], emb_b: &[f32]) -> Result<String, Error> {
        let p = unsafe {
            seraph_store_chain_between(
                self.handle,
                emb_a.as_ptr(), emb_a.len(),
                emb_b.as_ptr(), emb_b.len(),
            )
        };
        if p.is_null() { return Err(last_error()); }
        Ok(unsafe { take_cstring(p) })
    }

    // ---------------------------------------------------------------- //
    // Intelligence
    // ---------------------------------------------------------------- //

    /// Local density score for a frame.
    pub fn density_score(&self, frame_id: &str) -> f32 {
        let fid_c = to_cstring(frame_id);
        unsafe { seraph_store_density_score(self.handle, fid_c.as_ptr()) }
    }

    /// Seed margin: geometric distinctness relative to corpus centroid.
    pub fn seed_margin(&self, frame_id: &str) -> f32 {
        let fid_c = to_cstring(frame_id);
        unsafe { seraph_store_seed_margin(self.handle, fid_c.as_ptr()) }
    }

    /// Warp steerability score.
    pub fn steerability(&self, query_embedding: &[f32], target_id: &str) -> f32 {
        let tid_c = to_cstring(target_id);
        unsafe {
            seraph_store_steerability(
                self.handle,
                query_embedding.as_ptr(), query_embedding.len(),
                tid_c.as_ptr(),
            )
        }
    }

    /// Region health for an eigenframe. Returns JSON.
    pub fn region_health_json(&self, eigenframe_id: &str) -> Option<String> {
        let eid_c = to_cstring(eigenframe_id);
        let p = unsafe { seraph_store_region_health(self.handle, eid_c.as_ptr()) };
        if p.is_null() { None } else { Some(unsafe { take_cstring(p) }) }
    }

    /// Similarity distribution. Returns JSON.
    pub fn similarity_stats_json(&self) -> Option<String> {
        let p = unsafe { seraph_store_similarity_stats(self.handle) };
        if p.is_null() { None } else { Some(unsafe { take_cstring(p) }) }
    }

    /// Resolve tau: auto-threshold from corpus statistics.
    pub fn resolve_tau(&self) -> f32 {
        unsafe { seraph_store_resolve_tau(self.handle) }
    }

    /// Drift report. Returns JSON.
    pub fn drift_report_json(&self, window: usize, threshold: f32) -> Option<String> {
        let p = unsafe { seraph_store_drift_report(self.handle, window, threshold) };
        if p.is_null() { None } else { Some(unsafe { take_cstring(p) }) }
    }

    /// Contradictions. Returns JSON.
    pub fn contradictions_json(&self, min_divergence: f32) -> Option<String> {
        let p = unsafe { seraph_store_contradictions(self.handle, min_divergence) };
        if p.is_null() { None } else { Some(unsafe { take_cstring(p) }) }
    }

    /// Evolution from a seed frame. Returns JSON.
    pub fn evolution_json(&self, seed_frame_id: &str) -> Option<String> {
        let fid_c = to_cstring(seed_frame_id);
        let p = unsafe { seraph_store_evolution(self.handle, fid_c.as_ptr()) };
        if p.is_null() { None } else { Some(unsafe { take_cstring(p) }) }
    }

    /// Merge candidates. Returns JSON.
    pub fn merge_candidates_json(&self, tau: f32) -> Option<String> {
        let p = unsafe { seraph_store_merge_candidates(self.handle, tau) };
        if p.is_null() { None } else { Some(unsafe { take_cstring(p) }) }
    }

    /// Consolidate specific frames. Returns JSON report.
    pub fn consolidate_json(&mut self, frame_ids_json: &str) -> Result<String, Error> {
        let json_c = to_cstring(frame_ids_json);
        let p = unsafe { seraph_store_consolidate(self.handle, json_c.as_ptr()) };
        if p.is_null() { Err(last_error()) } else { Ok(unsafe { take_cstring(p) }) }
    }

    /// Consolidate all duplicate clusters. Returns JSON report.
    pub fn consolidate_all_json(&mut self) -> Result<String, Error> {
        let p = unsafe { seraph_store_consolidate_all(self.handle) };
        if p.is_null() { Err(last_error()) } else { Ok(unsafe { take_cstring(p) }) }
    }

    // ---------------------------------------------------------------- //
    // Provenance
    // ---------------------------------------------------------------- //

    /// Writer activity audit. Returns JSON.
    pub fn writer_audit_json(&self) -> Option<String> {
        let p = unsafe { seraph_store_writer_audit(self.handle) };
        if p.is_null() { None } else { Some(unsafe { take_cstring(p) }) }
    }

    /// Full attestation audit. Returns JSON.
    pub fn verify_attestation_json(&self) -> Result<String, Error> {
        let p = unsafe { seraph_store_verify_attestation(self.handle) };
        if p.is_null() { Err(last_error()) } else { Ok(unsafe { take_cstring(p) }) }
    }

    /// Genesis seed bytes.
    pub fn genesis_seed(&self) -> Option<Vec<u8>> {
        let mut out_ptr: *mut u8 = ptr::null_mut();
        let mut out_len: usize = 0;
        let rc = unsafe { seraph_store_genesis_seed(self.handle, &mut out_ptr, &mut out_len) };
        if rc != 0 { return None; }
        let bytes = unsafe { std::slice::from_raw_parts(out_ptr, out_len) }.to_vec();
        unsafe { seraph_bytes_free(out_ptr, out_len) };
        Some(bytes)
    }

    /// Genesis attestation header as JSON string.
    pub fn genesis_attestation_json(&self) -> Option<String> {
        let p = unsafe { seraph_store_genesis_attestation(self.handle) };
        if p.is_null() { None } else { Some(unsafe { take_cstring(p) }) }
    }

    // ---------------------------------------------------------------- //
    // Typed edges
    // ---------------------------------------------------------------- //

    /// Declare a new structural relation type. Returns the slot number.
    pub fn declare_relation(&mut self, name: &str, flags: u32) -> Result<u16, Error> {
        let name_c = to_cstring(name);
        let rc = unsafe { seraph_store_declare_relation(self.handle, name_c.as_ptr(), flags) };
        if rc < 0 { Err(last_error()) } else { Ok(rc as u16) }
    }

    /// List all declared relations. Returns JSON.
    pub fn list_relations_json(&self) -> Option<String> {
        let p = unsafe { seraph_store_list_relations(self.handle) };
        if p.is_null() { None } else { Some(unsafe { take_cstring(p) }) }
    }

    /// Assert a structural relation between two frames.
    pub fn assert_relation(&mut self, from_id: &str, to_id: &str, relation: &str, weight: f32) -> Result<(), Error> {
        let from_c = to_cstring(from_id);
        let to_c = to_cstring(to_id);
        let rel_c = to_cstring(relation);
        let rc = unsafe {
            seraph_store_assert_relation(self.handle, from_c.as_ptr(), to_c.as_ptr(), rel_c.as_ptr(), weight)
        };
        if rc == 0 { Ok(()) } else { Err(last_error()) }
    }

    /// Retract a structural relation.
    pub fn retract_relation(&mut self, from_id: &str, to_id: &str, relation: &str) -> Result<(), Error> {
        let from_c = to_cstring(from_id);
        let to_c = to_cstring(to_id);
        let rel_c = to_cstring(relation);
        let rc = unsafe {
            seraph_store_retract_relation(self.handle, from_c.as_ptr(), to_c.as_ptr(), rel_c.as_ptr())
        };
        if rc == 0 { Ok(()) } else { Err(last_error()) }
    }

    /// Query structural neighbors. Returns JSON.
    pub fn structural_neighbors_json(
        &self, frame_id: &str, relations_json: &str, direction: &str,
    ) -> Result<String, Error> {
        let fid_c = to_cstring(frame_id);
        let rels_c = to_cstring(relations_json);
        let dir_c = to_cstring(direction);
        let p = unsafe {
            seraph_store_structural_neighbors(self.handle, fid_c.as_ptr(), rels_c.as_ptr(), dir_c.as_ptr())
        };
        if p.is_null() { Err(last_error()) } else { Ok(unsafe { take_cstring(p) }) }
    }

    // ---------------------------------------------------------------- //
    // Chain analytics + hit-list pipeline
    // ---------------------------------------------------------------- //

    /// Semantic gradient along an ordered chain of frame IDs (`chain_json` is a
    /// JSON array of id strings). Returns a JSON array of
    /// `{"frame_id","similarity","delta"}` objects.
    pub fn gradient_json(&self, chain_json: &str) -> Result<String, Error> {
        let chain_c = to_cstring(chain_json);
        let p = unsafe { seraph_store_gradient(self.handle, chain_c.as_ptr()) };
        if p.is_null() { Err(last_error()) } else { Ok(unsafe { take_cstring(p) }) }
    }

    /// Constitutive score for a chain: `1 - jaccard(chain_intermediates,
    /// midpoint_NN)`. `chain_json` is a JSON array of id strings. Returns
    /// `f32::NAN` on bad input (check the error channel).
    pub fn constitutive_score(&self, chain_json: &str) -> f32 {
        let chain_c = to_cstring(chain_json);
        unsafe { seraph_store_constitutive_score(self.handle, chain_c.as_ptr()) }
    }

    /// Filter a hit list by structural relation presence. `hits_json` is a JSON
    /// array of `{"frame_id","score"}`; `relations_json` a JSON array of relation
    /// names; `direction` is "outgoing" | "incoming" | "both". When
    /// `require_present` is true, keeps hits that HAVE a matching relation;
    /// otherwise keeps those that LACK one. Returns a JSON `{"frame_id","score"}`
    /// array.
    pub fn filter_by_relation_json(
        &self,
        hits_json: &str,
        relations_json: &str,
        direction: &str,
        require_present: bool,
    ) -> Result<String, Error> {
        let hits_c = to_cstring(hits_json);
        let rels_c = to_cstring(relations_json);
        let dir_c = to_cstring(direction);
        let p = unsafe {
            seraph_store_filter_by_relation(
                self.handle, hits_c.as_ptr(), rels_c.as_ptr(), dir_c.as_ptr(), require_present,
            )
        };
        if p.is_null() { Err(last_error()) } else { Ok(unsafe { take_cstring(p) }) }
    }

    /// Expand a hit list by one hop along structural relations. `score_combine`
    /// is "multiply" | "min" | "max" | "keep_input". Returns a JSON
    /// `{"frame_id","score"}` array.
    pub fn structural_expand_json(
        &self,
        hits_json: &str,
        relations_json: &str,
        direction: &str,
        score_combine: &str,
    ) -> Result<String, Error> {
        let hits_c = to_cstring(hits_json);
        let rels_c = to_cstring(relations_json);
        let dir_c = to_cstring(direction);
        let combine_c = to_cstring(score_combine);
        let p = unsafe {
            seraph_store_structural_expand(
                self.handle, hits_c.as_ptr(), rels_c.as_ptr(), dir_c.as_ptr(), combine_c.as_ptr(),
            )
        };
        if p.is_null() { Err(last_error()) } else { Ok(unsafe { take_cstring(p) }) }
    }

    // ---------------------------------------------------------------- //
    // Event bus
    // ---------------------------------------------------------------- //

    /// Subscribe to substrate events.
    ///
    /// `event_name`: "promote" | "contradiction" | "drift" | "consolidation" | "milestone" | "*"
    ///
    /// The callback receives the event as a JSON string and the user_data pointer.
    /// The JSON string is only valid for the duration of the callback.
    pub fn subscribe(
        &mut self,
        event_name: &str,
        callback: extern "C" fn(*const c_char, *mut c_void),
        user_data: *mut c_void,
    ) -> Result<u64, Error> {
        let name_c = to_cstring(event_name);
        let id = unsafe {
            seraph_store_subscribe(self.handle, name_c.as_ptr(), callback, user_data)
        };
        if id == 0 { Err(last_error()) } else { Ok(id) }
    }

    /// Unsubscribe from events. Returns true if the subscriber was found.
    pub fn unsubscribe(&mut self, subscriber_id: u64) -> bool {
        unsafe { seraph_store_unsubscribe(self.handle, subscriber_id) != 0 }
    }

    /// Get the number of active event subscribers.
    pub fn subscriber_count(&self) -> usize {
        unsafe { seraph_store_subscriber_count(self.handle) }
    }

    // ---------------------------------------------------------------- //
    // Warp calibration
    // ---------------------------------------------------------------- //

    /// Calibrate warp magnitudes by empirical sweep. Returns JSON.
    ///
    /// `magnitudes`: optional explicit set of magnitudes to test (null = defaults).
    pub fn calibrate_warp(
        &self,
        n_queries: usize,
        n_targets: usize,
        magnitudes: Option<&[f32]>,
        top_k: usize,
        seed: u64,
    ) -> Result<String, Error> {
        let mag_json = magnitudes.map(|m| {
            to_cstring(&serde_json::to_string(m).unwrap_or_default())
        });
        let p = unsafe {
            seraph_store_calibrate_warp(
                self.handle, n_queries, n_targets,
                mag_json.as_ref().map_or(ptr::null(), |c| c.as_ptr()),
                top_k, seed,
            )
        };
        if p.is_null() { Err(last_error()) } else { Ok(unsafe { take_cstring(p) }) }
    }

    /// Run the warp-calibration fixup (the self-tuning closed loop): sweep the
    /// store's own encoder + corpus and, if the result has drifted `> 0.1` from
    /// the persisted calibration (or none exists), persist a new calibration
    /// sentinel and adopt it. This is the "store loaded" hook — the single
    /// writer calls it once after opening its write handle.
    ///
    /// Returns `true` if it (re)calibrated, `false` if it skipped (not stale, or
    /// geometry not developed). Named-magnitude warp resolves against the result.
    pub fn fixup_warp(&mut self) -> Result<bool, Error> {
        match unsafe { seraph_store_fixup_warp(self.handle) } {
            1 => Ok(true),
            0 => Ok(false),
            _ => Err(last_error()),
        }
    }

    /// The store's resident warp calibration as a JSON object
    /// (`{"routing": f32, ...}`), or the literal `null` if the store has not
    /// been calibrated.
    pub fn warp_calibration(&self) -> Result<String, Error> {
        let p = unsafe { seraph_store_warp_calibration(self.handle) };
        if p.is_null() { Err(last_error()) } else { Ok(unsafe { take_cstring(p) }) }
    }

    // ---------------------------------------------------------------- //
    // Snapshot / temporal queries
    // ---------------------------------------------------------------- //

    /// Create a snapshot view at a given timestamp (microseconds since epoch).
    pub fn snapshot(&self, at_timestamp_us: i64) -> Option<Snapshot> {
        let p = unsafe { seraph_store_snapshot(self.handle, at_timestamp_us) };
        if p.is_null() { None } else { Some(Snapshot { handle: p, store_handle: self.handle }) }
    }

    /// Search within a snapshot. Returns JSON array of `{"frame_id", "score"}`.
    pub fn snapshot_search(
        &self, snapshot: &Snapshot, query_embedding: &[f32], top_k: usize,
    ) -> Result<String, Error> {
        let p = unsafe {
            seraph_store_snapshot_search(
                self.handle, snapshot.handle,
                query_embedding.as_ptr(), query_embedding.len(),
                top_k,
            )
        };
        if p.is_null() { Err(last_error()) } else { Ok(unsafe { take_cstring(p) }) }
    }

    // ---------------------------------------------------------------- //
    // WarpSpec resolution
    // ---------------------------------------------------------------- //

    /// Search with a WarpSpec (JSON). Supports frame ID targets and named
    /// magnitudes, which resolve against the store's own persisted calibration
    /// (no built-in presets — an uncalibrated store errors on named use; see
    /// `fixup_warp`).
    ///
    /// See `seraph_store_search_with_warp_spec` in ffi.rs for the JSON format.
    pub fn search_with_warp_spec(
        &self,
        query_embedding: &[f32],
        warp_spec_json: &str,
        top_k: usize,
        tau: f32,
    ) -> Result<Vec<Hit>, Error> {
        let spec_c = to_cstring(warp_spec_json);
        let mut hits_ptr: *mut FfiHit = ptr::null_mut();
        let mut count: usize = 0;
        let rc = unsafe {
            seraph_store_search_with_warp_spec(
                self.handle,
                query_embedding.as_ptr(), query_embedding.len(),
                spec_c.as_ptr(),
                top_k, tau,
                &mut hits_ptr, &mut count,
            )
        };
        if rc != 0 { return Err(last_error()); }
        Ok(take_hits(hits_ptr, count))
    }

    /// Apply a WarpSpec to a query embedding (pure math, no search).
    /// Requires the store because frame ID inputs need embedding lookup.
    pub fn warp_query(
        &self,
        query_embedding: &[f32],
        warp_spec_json: &str,
    ) -> Result<Vec<f32>, Error> {
        let spec_c = to_cstring(warp_spec_json);
        let mut out_ptr: *mut f32 = ptr::null_mut();
        let mut out_dim: usize = 0;
        let rc = unsafe {
            seraph_store_warp_query(
                self.handle,
                query_embedding.as_ptr(), query_embedding.len(),
                spec_c.as_ptr(),
                &mut out_ptr, &mut out_dim,
            )
        };
        if rc != 0 { return Err(last_error()); }
        let emb = unsafe { std::slice::from_raw_parts(out_ptr, out_dim) }.to_vec();
        unsafe { seraph_floats_free(out_ptr, out_dim) };
        Ok(emb)
    }

    // ---------------------------------------------------------------- //
    // Zero-copy accommodations
    // ---------------------------------------------------------------- //

    /// Region health as a typed struct (no JSON parsing).
    pub fn region_health(&self, eigenframe_id: &str) -> Result<RegionHealth, Error> {
        let eid_c = to_cstring(eigenframe_id);
        let mut out = FfiRegionHealth { coherence: 0.0, direction: 0, candidate_split: 0 };
        let rc = unsafe {
            seraph_store_region_health_struct(self.handle, eid_c.as_ptr(), &mut out)
        };
        if rc != 0 { return Err(last_error()); }
        Ok(RegionHealth {
            coherence: out.coherence,
            direction: match out.direction {
                1 => RegionDirection::Consolidating,
                2 => RegionDirection::Fragmenting,
                _ => RegionDirection::Stable,
            },
            candidate_split: out.candidate_split != 0,
        })
    }

    /// Similarity stats as a typed struct (no JSON parsing).
    pub fn similarity_stats(&self) -> Result<SimilarityStats, Error> {
        let mut out = FfiSimilarityStats { count: 0, mean: 0.0, p75: 0.0, p90: 0.0, p95: 0.0, p99: 0.0 };
        let rc = unsafe {
            seraph_store_similarity_stats_struct(self.handle, &mut out)
        };
        if rc != 0 { return Err(last_error()); }
        Ok(SimilarityStats {
            count: out.count,
            mean: out.mean,
            p75: out.p75,
            p90: out.p90,
            p95: out.p95,
            p99: out.p99,
        })
    }

    /// Copy a frame's embedding into a caller-owned buffer.
    ///
    /// Returns `Ok(Some(dim))` on success, `Ok(None)` if frame not found.
    /// Returns `Err` if the buffer is too small (the error message contains
    /// the required size).
    pub fn get_embedding_into(&self, frame_id: &str, buf: &mut [f32]) -> Result<Option<usize>, Error> {
        let fid_c = to_cstring(frame_id);
        let mut written: usize = 0;
        let rc = unsafe {
            seraph_store_get_embedding_into(
                self.handle, fid_c.as_ptr(),
                buf.as_mut_ptr(), buf.len(), &mut written,
            )
        };
        match rc {
            0 => Ok(Some(written)),
            1 => Ok(None),
            2 => Err(Error { message: format!("buffer too small: need {written}, have {}", buf.len()) }),
            _ => Err(last_error()),
        }
    }

    /// Borrow a frame for zero-copy reads.
    ///
    /// The returned `FrameView` provides direct pointers into engine memory.
    /// Drop it when done. Do not hold across store mutations.
    pub fn borrow_frame(&self, frame_id: &str) -> Option<FrameView> {
        let fid_c = to_cstring(frame_id);
        let p = unsafe { seraph_store_borrow_frame(self.handle, fid_c.as_ptr()) };
        if p.is_null() { None } else { Some(FrameView { ptr: p }) }
    }
}

// ---------------------------------------------------------------- //
// Snapshot
// ---------------------------------------------------------------- //

/// A time-sliced view of the store at a specific timestamp.
///
/// Created via [`Store::snapshot`]. The snapshot borrows the store handle
/// but holds its own opaque FFI handle that must be freed.
pub struct Snapshot {
    handle: *mut c_void,
    #[allow(dead_code)]
    store_handle: *mut c_void,
}

impl Snapshot {
    /// Frame IDs visible at this snapshot's timestamp.
    pub fn frame_ids(&self) -> Vec<String> {
        let p = unsafe { seraph_snapshot_frame_ids(self.handle) };
        if p.is_null() { return Vec::new(); }
        let json = unsafe { take_cstring(p) };
        serde_json::from_str(&json).unwrap_or_default()
    }

    /// The timestamp this snapshot was created at.
    pub fn timestamp(&self) -> i64 {
        unsafe { seraph_snapshot_timestamp(self.handle) }
    }
}

impl Drop for Snapshot {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe { seraph_snapshot_free(self.handle) };
            self.handle = ptr::null_mut();
        }
    }
}

// ---------------------------------------------------------------- //
// Encoder
// ---------------------------------------------------------------- //

/// Wraps the SERAPH candle-based encoder (bge-small, bge-base, nomic, jina-v3).
pub struct Encoder {
    handle: *mut c_void,
}

unsafe impl Send for Encoder {}

impl Drop for Encoder {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe { seraph_encoder_close(self.handle) };
            self.handle = ptr::null_mut();
        }
    }
}

impl Encoder {
    /// Load an encoder by model ID (downloads from HuggingFace if not cached).
    ///
    /// Supported models:
    /// - `BAAI/bge-small-en-v1.5`
    /// - `BAAI/bge-base-en-v1.5`
    /// - `nomic-ai/nomic-embed-text-v1.5`
    /// - `jinaai/jina-embeddings-v3`
    pub fn from_pretrained(model_id: &str) -> Result<Self, Error> {
        let mid_c = to_cstring(model_id);
        let handle = unsafe { seraph_encoder_create(mid_c.as_ptr()) };
        if handle.is_null() {
            Err(last_error())
        } else {
            Ok(Encoder { handle })
        }
    }

    /// Encode a text string to an embedding vector.
    pub fn encode(&mut self, text: &str) -> Result<Vec<f32>, Error> {
        let text_c = to_cstring(text);
        let mut out_ptr: *mut f32 = ptr::null_mut();
        let mut out_dim: usize = 0;

        let rc = unsafe {
            seraph_encoder_encode(
                self.handle, text_c.as_ptr(),
                &mut out_ptr, &mut out_dim,
            )
        };

        if rc != 0 {
            return Err(last_error());
        }

        let emb = unsafe { std::slice::from_raw_parts(out_ptr, out_dim) }.to_vec();
        unsafe { seraph_floats_free(out_ptr, out_dim) };
        Ok(emb)
    }

    /// Encode text into a caller-owned buffer (no allocation).
    ///
    /// Returns `Ok(dim)` on success. Returns `Err` if the buffer is too small.
    pub fn encode_into(&mut self, text: &str, buf: &mut [f32]) -> Result<usize, Error> {
        let text_c = to_cstring(text);
        let mut written: usize = 0;
        let rc = unsafe {
            seraph_encoder_encode_into(
                self.handle, text_c.as_ptr(),
                buf.as_mut_ptr(), buf.len(), &mut written,
            )
        };
        match rc {
            0 => Ok(written),
            2 => Err(Error { message: format!("buffer too small: need {written}, have {}", buf.len()) }),
            _ => Err(last_error()),
        }
    }

    /// Batch encode texts. Returns JSON array of embedding arrays.
    pub fn encode_batch_json(&mut self, texts_json: &str) -> Result<String, Error> {
        let json_c = to_cstring(texts_json);
        let p = unsafe { seraph_encoder_encode_batch(self.handle, json_c.as_ptr()) };
        if p.is_null() { Err(last_error()) } else { Ok(unsafe { take_cstring(p) }) }
    }

    /// Batch encode texts into embedding vectors.
    ///
    /// Uses the flat batch FFI path — a single call encodes all texts in one
    /// batched forward pass, amortising GPU kernel launch and FFI boundary cost.
    pub fn encode_batch(&mut self, texts: &[&str]) -> Result<Vec<Vec<f32>>, Error> {
        if texts.is_empty() { return Ok(Vec::new()); }

        let c_strings: Vec<CString> = texts.iter()
            .map(|s| CString::new(*s).unwrap_or_default())
            .collect();
        let c_ptrs: Vec<*const c_char> = c_strings.iter()
            .map(|cs| cs.as_ptr())
            .collect();

        let mut out_ptr: *mut f32 = ptr::null_mut();
        let mut out_dim: usize = 0;

        let rc = unsafe {
            seraph_encoder_encode_batch_flat(
                self.handle,
                c_ptrs.as_ptr(),
                texts.len(),
                &mut out_ptr,
                &mut out_dim,
            )
        };

        if rc != 0 {
            return Err(last_error());
        }

        if out_ptr.is_null() || out_dim == 0 {
            return Ok(Vec::new());
        }

        let total = texts.len() * out_dim;
        let flat = unsafe { std::slice::from_raw_parts(out_ptr, total) };
        let result: Vec<Vec<f32>> = flat.chunks_exact(out_dim)
            .map(|chunk| chunk.to_vec())
            .collect();
        unsafe { seraph_floats_free(out_ptr, total) };
        Ok(result)
    }
}

// ---------------------------------------------------------------- //
// Helper: extract hits from FFI array
// ---------------------------------------------------------------- //

/// Apply warp to a query vector (pure math, no search). Returns warped embedding.
///
/// `targets_json` / `suppress_json`: JSON arrays of `{"embedding": [...], "magnitude": f32}`.
pub fn apply_warp(
    query_embedding: &[f32],
    targets_json: &str,
    suppress_json: &str,
    profile: &str,
    bound_k: f32,
) -> Result<Vec<f32>, Error> {
    let targets_c = to_cstring(targets_json);
    let suppress_c = to_cstring(suppress_json);
    let profile_c = to_cstring(profile);
    let mut out_ptr: *mut f32 = ptr::null_mut();
    let mut out_dim: usize = 0;
    let rc = unsafe {
        seraph_store_apply_warp(
            query_embedding.as_ptr(), query_embedding.len(),
            targets_c.as_ptr(), suppress_c.as_ptr(),
            profile_c.as_ptr(), bound_k,
            &mut out_ptr, &mut out_dim,
        )
    };
    if rc != 0 { return Err(last_error()); }
    let emb = unsafe { std::slice::from_raw_parts(out_ptr, out_dim) }.to_vec();
    unsafe { seraph_floats_free(out_ptr, out_dim) };
    Ok(emb)
}

/// Re-encode a source store into a fresh v3 destination store.
///
/// Streams active-content frames from `src_path`, re-encodes them with
/// `model_id`, and bulk-ingests the results into a freshly-created v3 store at
/// `dst_path`. `n_max == 0` re-encodes all frames; `pbatch == 0` defaults to
/// 2000 (frames per batch), `ebatch == 0` to 16 (frames per encode pass).
/// Returns the number of frames ingested.
#[allow(clippy::too_many_arguments)]
pub fn reencode_store(
    src_path: impl AsRef<Path>,
    dst_path: impl AsRef<Path>,
    model_id: &str,
    eigen_threshold: u32,
    n_max: usize,
    pbatch: usize,
    ebatch: usize,
) -> Result<usize, Error> {
    let src_c = path_to_cstring(src_path.as_ref());
    let dst_c = path_to_cstring(dst_path.as_ref());
    let model_c = to_cstring(model_id);
    let mut out_n: usize = 0;
    let rc = unsafe {
        seraph_reencode_store(
            src_c.as_ptr(), dst_c.as_ptr(), model_c.as_ptr(),
            eigen_threshold, n_max, pbatch, ebatch, &mut out_n,
        )
    };
    if rc != 0 { Err(last_error()) } else { Ok(out_n) }
}

fn take_hits(hits_ptr: *mut FfiHit, count: usize) -> Vec<Hit> {
    let mut results = Vec::with_capacity(count);
    if !hits_ptr.is_null() && count > 0 {
        for i in 0..count {
            let ffi = unsafe { &*hits_ptr.add(i) };
            results.push(Hit {
                frame_id: unsafe { CStr::from_ptr(ffi.frame_id) }.to_string_lossy().into_owned(),
                score: ffi.score,
                confidence: ffi.confidence,
                path_length: ffi.path_length,
            });
        }
        unsafe { seraph_hits_free(hits_ptr, count) };
    }
    results
}

// ---------------------------------------------------------------- //
// Federation — route + parallel fan-out across many single-file stores
// ---------------------------------------------------------------- //

extern "C" {
    fn seraph_federation_open_dir(dir: *const c_char) -> *mut c_void;
    fn seraph_federation_new() -> *mut c_void;
    fn seraph_federation_add_path(handle: *mut c_void, name: *const c_char, path: *const c_char) -> i32;
    fn seraph_federation_len(handle: *mut c_void) -> usize;
    fn seraph_federation_member_names(handle: *mut c_void) -> *mut c_char;
    fn seraph_federation_refresh(handle: *mut c_void) -> i32;
    fn seraph_federation_route(handle: *mut c_void, embedding: *const f32, embedding_len: usize, top_stores: usize) -> *mut c_char;
    fn seraph_federation_search(handle: *mut c_void, embedding: *const f32, embedding_len: usize, top_k: usize, tau: f32, mode: *const c_char, n: usize) -> *mut c_char;
    fn seraph_federation_free(handle: *mut c_void);
}

/// Which member stores a federated query searches.
#[derive(Debug, Clone, Copy)]
pub enum RoutingMode {
    /// Single best router-matched store (lowest latency).
    BestMatch,
    /// Top-`n` router-matched stores, merged.
    Top(usize),
    /// Every store — exhaustive fan-out (no routing).
    All,
}

/// A federated search hit, tagged with its source store.
#[derive(Debug, Clone, Default)]
pub struct FederatedHit {
    pub store: String,
    pub frame_id: String,
    pub score: f32,
    pub confidence: f32,
    pub path_length: u32,
    pub snippet: String,
}

/// Read-side coordinator over many independent single-file stores. Routes a query
/// to the best / top-N / all member stores and fans out search in parallel.
pub struct Federation {
    handle: *mut c_void,
}

// SAFETY: the C-ABI handle is a stable pointer; the underlying engine is
// internally synchronized where needed.
unsafe impl Send for Federation {}

impl Drop for Federation {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe { seraph_federation_free(self.handle) };
            self.handle = ptr::null_mut();
        }
    }
}

impl Federation {
    /// Open every `.sfg` store in a directory (named by file stem; `_*`/`router*`
    /// skipped). All stores must share the embedding model + dimension.
    pub fn open_dir(dir: impl AsRef<Path>) -> Result<Self, Error> {
        let dir_c = path_to_cstring(dir.as_ref());
        let handle = unsafe { seraph_federation_open_dir(dir_c.as_ptr()) };
        if handle.is_null() { Err(last_error()) } else { Ok(Federation { handle }) }
    }

    /// Create an empty federation.
    pub fn new() -> Self {
        Federation { handle: unsafe { seraph_federation_new() } }
    }

    /// Register a store file under `name`.
    pub fn add_path(&mut self, name: &str, path: impl AsRef<Path>) -> Result<(), Error> {
        let name_c = to_cstring(name);
        let path_c = path_to_cstring(path.as_ref());
        let rc = unsafe { seraph_federation_add_path(self.handle, name_c.as_ptr(), path_c.as_ptr()) };
        if rc == 0 { Ok(()) } else { Err(last_error()) }
    }

    /// Number of member stores.
    pub fn len(&self) -> usize { unsafe { seraph_federation_len(self.handle) } }
    pub fn is_empty(&self) -> bool { self.len() == 0 }

    /// Member store names, in add order.
    pub fn member_names(&self) -> Vec<String> {
        let p = unsafe { seraph_federation_member_names(self.handle) };
        if p.is_null() { return Vec::new(); }
        let json = unsafe { take_cstring(p) };
        serde_json::from_str(&json).unwrap_or_default()
    }

    /// Re-pull routing vectors for members that promoted since the last refresh.
    /// Returns true if the router changed.
    pub fn refresh(&mut self) -> bool {
        unsafe { seraph_federation_refresh(self.handle) == 1 }
    }

    /// Route the query to the top `top_stores` members → `(store_name, score)`.
    pub fn route(&self, query: &[f32], top_stores: usize) -> Result<Vec<(String, f32)>, Error> {
        let p = unsafe { seraph_federation_route(self.handle, query.as_ptr(), query.len(), top_stores) };
        if p.is_null() { return Err(last_error()); }
        let json = unsafe { take_cstring(p) };
        let val: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
        Ok(val.as_array().map(|arr| arr.iter().map(|h| (
            h["store"].as_str().unwrap_or("").to_string(),
            h["score"].as_f64().unwrap_or(0.0) as f32,
        )).collect()).unwrap_or_default())
    }

    /// Federated search under a [`RoutingMode`]. Each routed store returns `top_k`;
    /// results merge by cosine and truncate to the requested `top_k`.
    pub fn search(&self, query: &[f32], top_k: usize, tau: f32, mode: RoutingMode) -> Result<Vec<FederatedHit>, Error> {
        let (mode_c, n) = match mode {
            RoutingMode::BestMatch => ("best", 0usize),
            RoutingMode::Top(n) => ("top", n),
            RoutingMode::All => ("all", 0usize),
        };
        let mode_cs = to_cstring(mode_c);
        let p = unsafe {
            seraph_federation_search(self.handle, query.as_ptr(), query.len(), top_k, tau, mode_cs.as_ptr(), n)
        };
        if p.is_null() { return Err(last_error()); }
        let json = unsafe { take_cstring(p) };
        let val: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
        Ok(val.as_array().map(|arr| arr.iter().map(|h| FederatedHit {
            store: h["store"].as_str().unwrap_or("").to_string(),
            frame_id: h["frame_id"].as_str().unwrap_or("").to_string(),
            score: h["score"].as_f64().unwrap_or(0.0) as f32,
            confidence: h["confidence"].as_f64().unwrap_or(0.0) as f32,
            path_length: h["path_length"].as_u64().unwrap_or(0) as u32,
            snippet: h["snippet"].as_str().unwrap_or("").to_string(),
        }).collect()).unwrap_or_default())
    }
}

impl Default for Federation {
    fn default() -> Self { Self::new() }
}
