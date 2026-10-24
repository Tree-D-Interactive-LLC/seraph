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

use std::collections::HashMap;
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
    #[allow(dead_code)] // bound for ABI coverage; no method calls it
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
    #[allow(dead_code)] // bound for ABI coverage; no method calls it
    fn seraph_store_search(
        handle: *mut c_void,
        embedding: *const f32, embedding_len: usize,
        top_k: usize, tau: f32,
        out_hits: *mut *mut FfiHit, out_count: *mut usize,
    ) -> i32;
    #[allow(dead_code)] // bound for ABI coverage; no method calls it
    fn seraph_store_search_opts(
        handle: *mut c_void,
        embedding: *const f32, embedding_len: usize,
        top_k: usize, tau: f32,
        include_superseded: bool,
        out_hits: *mut *mut FfiHit, out_count: *mut usize,
    ) -> i32;
    fn seraph_store_search_stream(
        handle: *mut c_void,
        embedding: *const f32, embedding_len: usize,
        top_k: usize, tau: f32,
        beam_width: usize, max_visits: usize,
        out_hits: *mut *mut FfiHit, out_count: *mut usize,
    ) -> i32;
    fn seraph_store_search_with_trace(
        handle: *mut c_void,
        embedding: *const f32, embedding_len: usize,
        top_k: usize, tau: f32,
    ) -> *mut c_char;
    #[allow(dead_code)] // bound for ABI coverage; no method calls it
    fn seraph_store_search_temporal(
        handle: *mut c_void,
        embedding: *const f32, embedding_len: usize,
        top_k: usize, tau: f32,
        include_superseded: i32,
        order: i32,
        after_us: u64, before_us: u64,
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
    fn seraph_store_seal_with_anchor(handle: *mut c_void, reason: *const c_char, backend: *const c_char, token: *const u8, token_len: usize) -> *mut c_char;
    fn seraph_store_order_digest(handle: *mut c_void, through_frame_id: *const c_char) -> *mut c_char;
    fn seraph_store_anchor_claims(handle: *mut c_void) -> *mut c_char;
    fn seraph_store_is_sealed(handle: *mut c_void) -> i32;

    // Supersede
    fn seraph_store_supersede(handle: *mut c_void, frame_id: *const c_char) -> *mut c_char;

    // Sync
    fn seraph_store_sync(handle: *mut c_void) -> i32;

    // Ingest — batch, seed, promote
    fn seraph_store_put_batch(handle: *mut c_void, items_json: *const c_char) -> *mut c_char;
    fn seraph_store_put_batch_incremental(handle: *mut c_void, items_json: *const c_char) -> *mut c_char;
    fn seraph_store_add_seed(
        handle: *mut c_void, label: *const c_char,
        embedding: *const f32, embedding_len: usize,
        metadata_json: *const c_char,
    ) -> *mut c_char;
    fn seraph_store_get_seeds(handle: *mut c_void) -> *mut c_char;
    fn seraph_store_seeds_are_authored(handle: *mut c_void) -> i32;
    fn seraph_store_resolve_seed(handle: *mut c_void, label: *const c_char) -> *mut c_char;
    fn seraph_store_set_seeds(handle: *mut c_void, entries_json: *const c_char) -> *mut c_char;
    fn seraph_store_register_seed(
        handle: *mut c_void, label: *const c_char, frame_id: *const c_char,
    ) -> *mut c_char;
    fn seraph_store_unregister_seed(handle: *mut c_void, label: *const c_char) -> *mut c_char;
    fn seraph_store_put_subtree(
        handle: *mut c_void, root: *const c_char,
        content: *const u8, content_len: usize,
        embedding: *const f32, embedding_len: usize,
        metadata_json: *const c_char,
    ) -> *mut c_char;
    #[allow(dead_code)] // bound for ABI coverage; no method calls it
    fn seraph_store_search_subtree(
        handle: *mut c_void, root: *const c_char,
        query: *const f32, query_len: usize,
        top_k: usize, tau: f32,
        out_hits: *mut *mut FfiHit, out_count: *mut usize,
    ) -> i32;
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
    fn seraph_store_set_search_entry_k(handle: *mut c_void, k: usize) -> i32;
    fn seraph_store_set_search_entry_gap(handle: *mut c_void, gap: f32) -> i32;
    fn seraph_store_set_search_use_stream(handle: *mut c_void, on: bool, beam: usize) -> i32;
    fn seraph_store_set_search_stream_threshold(handle: *mut c_void, n: usize) -> i32;
    fn seraph_store_set_search_rank_mode(handle: *mut c_void, mode: u8, share: f32, doors: usize, budget: usize) -> i32;
    fn seraph_store_set_seed_term(handle: *mut c_void, beta: f32) -> i32;
    fn seraph_store_set_commit_ceiling_margin(handle: *mut c_void, m: f32) -> i32;
    fn seraph_store_set_commit_ceiling_slack(handle: *mut c_void, slack: f32) -> i32;
    fn seraph_store_set_sim_edge_policy(handle: *mut c_void, routing: bool, parent: bool, occlusion: f32, pool: usize) -> i32;
    fn seraph_store_set_search_max_visits(handle: *mut c_void, v: usize) -> i32;
    fn seraph_store_set_search_ceiling_margin(handle: *mut c_void, m: f32) -> i32;
    fn seraph_store_set_search_ceiling_slack(handle: *mut c_void, slack: f32) -> i32;
    fn seraph_store_set_door_index(handle: *mut c_void, on: bool) -> i32;
    fn seraph_store_set_door_index_ef(handle: *mut c_void, ef: usize) -> i32;
    fn seraph_store_vector_precision(handle: *mut c_void) -> *mut c_char;
    fn seraph_store_resident_precision(handle: *mut c_void) -> *mut c_char;
    fn seraph_store_set_resident_precision(handle: *mut c_void, name: *const c_char) -> i32;
    fn seraph_store_set_search_arena(handle: *mut c_void, on: bool) -> i32;
    fn seraph_store_set_search_adjacency(handle: *mut c_void, on: bool) -> i32;
    fn seraph_store_set_walk_profile(handle: *mut c_void, on: bool) -> i32;
    fn seraph_store_take_walk_profile(handle: *mut c_void) -> *mut c_char;
    fn seraph_store_entry_count(handle: *mut c_void, query_embedding: *const f32, query_len: usize) -> i64;
    fn seraph_store_last_search_visits(handle: *mut c_void) -> i64;
    fn seraph_store_last_commit_visits(handle: *mut c_void) -> i64;

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
    fn seraph_store_subtrees(handle: *mut c_void, min_size: usize) -> *mut c_char;

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
    fn seraph_store_neighborhood_spec(handle: *mut c_void, frame_id: *const c_char, tau_mode: i32, tau_value: f32, hops: usize) -> *mut c_char;
    fn seraph_store_chain(handle: *mut c_void, from_id: *const c_char, to_id: *const c_char) -> *mut c_char;
    fn seraph_store_chain_opts(handle: *mut c_void, from_id: *const c_char, to_id: *const c_char, include_superseded: bool) -> *mut c_char;
    fn seraph_store_provenance(handle: *mut c_void, frame_id: *const c_char, tau: f32, include_superseded: bool) -> *mut c_char;
    fn seraph_store_influence(handle: *mut c_void, frame_id: *const c_char, tau: f32, include_superseded: bool) -> *mut c_char;
    fn seraph_store_provenance_with_warp_spec(handle: *mut c_void, frame_id: *const c_char, warp_spec_json: *const c_char, tau: f32, include_superseded: bool) -> *mut c_char;
    fn seraph_store_influence_with_warp_spec(handle: *mut c_void, frame_id: *const c_char, warp_spec_json: *const c_char, tau: f32, include_superseded: bool) -> *mut c_char;
    fn seraph_store_connections(handle: *mut c_void, from_id: *const c_char, to_id: *const c_char, max_routes: usize, forward: bool, include_superseded: bool) -> *mut c_char;
    fn seraph_store_chain_forward(handle: *mut c_void, from_id: *const c_char, to_id: *const c_char, include_superseded: bool) -> *mut c_char;
    fn seraph_store_chain_to(handle: *mut c_void, query_embedding: *const f32, query_len: usize) -> *mut c_char;
    fn seraph_store_chain_between(handle: *mut c_void, emb_a: *const f32, len_a: usize, emb_b: *const f32, len_b: usize) -> *mut c_char;
    fn seraph_store_chain_forward_between(handle: *mut c_void, emb_a: *const f32, len_a: usize, emb_b: *const f32, len_b: usize, top_k: usize, tau: f32, include_superseded: bool) -> *mut c_char;
    fn seraph_store_history(handle: *mut c_void, query_embedding: *const f32, query_len: usize, top_k: usize, tau: f32, anchors: usize) -> *mut c_char;
    fn seraph_store_history_with_warp_spec(handle: *mut c_void, query_embedding: *const f32, query_len: usize, warp_spec_json: *const c_char, top_k: usize, tau: f32, anchors: usize) -> *mut c_char;
    fn seraph_store_chain_to_with_warp_spec(handle: *mut c_void, query_embedding: *const f32, query_len: usize, warp_spec_json: *const c_char) -> *mut c_char;
    fn seraph_store_chain_between_with_warp_spec(handle: *mut c_void, emb_a: *const f32, len_a: usize, emb_b: *const f32, len_b: usize, warp_spec_json: *const c_char) -> *mut c_char;
    fn seraph_store_neighborhood_with_warp_spec(handle: *mut c_void, frame_id: *const c_char, warp_spec_json: *const c_char, tau: f32, hops: usize) -> *mut c_char;

    // Intelligence
    fn seraph_store_density_score(handle: *mut c_void, frame_id: *const c_char) -> f32;
    fn seraph_store_seed_margin(handle: *mut c_void, frame_id: *const c_char) -> f32;
    fn seraph_store_steerability(handle: *mut c_void, query_embedding: *const f32, query_len: usize, target_id: *const c_char) -> f32;
    fn seraph_store_region_health(handle: *mut c_void, eigenframe_id: *const c_char) -> *mut c_char;
    fn seraph_store_similarity_stats(handle: *mut c_void) -> *mut c_char;
    fn seraph_store_sim_neighbors(handle: *mut c_void, frame_id: *const c_char) -> *mut c_char;
    fn seraph_store_committed_sim_edges(handle: *mut c_void, frame_id: *const c_char) -> *mut c_char;

    // Content-set root, walk query, codes, application markers
    fn seraph_store_content_set_root(handle: *mut c_void) -> *mut c_char;
    fn seraph_store_quantize_query(
        handle: *mut c_void, embedding: *const f32, embedding_len: usize,
        out_query: *mut *mut f32, out_dim: *mut usize,
    ) -> i32;
    fn seraph_store_encode_codes(
        handle: *mut c_void, embedding: *const f32, embedding_len: usize,
        out_codes: *mut *mut u8, out_len: *mut usize,
    ) -> i32;
    fn seraph_store_commit_marker(handle: *mut c_void, kind: *const c_char, metadata_json: *const c_char) -> *mut c_char;
    fn seraph_store_markers(handle: *mut c_void, kind: *const c_char) -> *mut c_char;
    fn seraph_store_resolve_tau(handle: *mut c_void) -> f32;
    fn seraph_store_resolve_tau_spec(handle: *mut c_void, tau_mode: i32, tau_value: f32) -> f32;
    fn seraph_store_drift_report(handle: *mut c_void, window: usize, threshold: f32) -> *mut c_char;
    fn seraph_store_contradictions(handle: *mut c_void, min_divergence: f32) -> *mut c_char;
    fn seraph_store_contradictions_v2(handle: *mut c_void, tau_mode: i32, tau_value: f32) -> *mut c_char;
    fn seraph_store_evolution(handle: *mut c_void, seed_frame_id: *const c_char) -> *mut c_char;
    fn seraph_store_merge_candidates(handle: *mut c_void, tau: f32) -> *mut c_char;
    fn seraph_store_consolidate(handle: *mut c_void, frame_ids_json: *const c_char) -> *mut c_char;
    fn seraph_store_consolidate_all(handle: *mut c_void) -> *mut c_char;
    fn seraph_store_consolidate_with_strategy(handle: *mut c_void, frame_ids: *const *const c_char, count: usize, strategy: i32) -> *mut c_char;

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
        callback: Option<extern "C" fn(*const c_char, *mut c_void)>,
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
    #[allow(dead_code)] // bound for ABI coverage; no method calls it
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
    fn seraph_migrate_store(
        src_path: *const c_char, dst_path: *const c_char,
    ) -> *mut c_char;
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
// Public argument enums
// ---------------------------------------------------------------- //

/// Tau threshold specification, resolved against the store's
/// background-similarity distribution.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TauSpec {
    /// Auto — background p95 (the `resolve_tau` default).
    Auto,
    /// Loose — background p75.
    Loose,
    /// Strict — background p99.
    Strict,
    /// Explicit threshold, passed through unchanged.
    Value(f32),
}

impl TauSpec {
    /// FFI encoding: the (mode, value) pair `seraph_store_resolve_tau_spec`
    /// expects (0 = auto, 1 = loose, 2 = strict, 3 = explicit value).
    fn to_mode_value(self) -> (i32, f32) {
        match self {
            TauSpec::Auto => (0, 0.0),
            TauSpec::Loose => (1, 0.0),
            TauSpec::Strict => (2, 0.0),
            TauSpec::Value(v) => (3, v),
        }
    }
}

/// Content strategy for a consolidated frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeStrategy {
    /// Most-central source frame's content verbatim (geometric only).
    Geometric,
    /// LLM-synthesized content — not implemented in the Rust runtime; errors.
    Semantic,
}

/// Presentation order for `search_temporal` hits. Applied AFTER relevance
/// selection — never changes which frames are selected or their scores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TemporalOrder {
    /// Score-ranked (identical to `search` / `search_opts` output order).
    #[default]
    Relevance,
    /// Timestamp descending (newest first).
    MostRecent,
    /// Timestamp ascending (oldest first).
    Oldest,
}

impl TemporalOrder {
    /// FFI encoding: the order code `seraph_store_search_temporal` expects
    /// (0 = relevance, 1 = most_recent, 2 = oldest).
    fn to_code(self) -> i32 {
        match self {
            TemporalOrder::Relevance => 0,
            TemporalOrder::MostRecent => 1,
            TemporalOrder::Oldest => 2,
        }
    }
}

// ---------------------------------------------------------------- //
// Error type
// ---------------------------------------------------------------- //

/// Error returned by SERAPH operations. Each variant carries the engine's
/// message.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Error {
    /// The engine refused or failed the operation.
    Store(String),
    /// A named frame does not exist.
    NotFound(String),
    /// A required argument was missing.
    NullArgument(String),
    /// An argument was malformed: bad JSON, an unknown name, an out-of-range value.
    InvalidArgument(String),
    /// An embedding's dimension differs from the store's.
    Dimension(String),
    /// The store is sealed; writes are refused.
    Sealed(String),
    /// The store's format cannot hold what was asked.
    Format(String),
    /// The combination is not supported by this engine.
    Unsupported(String),
    /// The engine panicked.
    Panic(String),
    /// A caller-owned buffer is too small.
    BufferTooSmall { needed: usize, have: usize },
    /// A batch stopped partway; `committed` holds the ids it committed, in batch order.
    BatchIncomplete { committed: Vec<String>, message: String },
}

impl Error {
    /// The engine's message.
    pub fn message(&self) -> String {
        match self {
            Error::Store(m) | Error::NotFound(m) | Error::NullArgument(m) | Error::InvalidArgument(m)
            | Error::Dimension(m) | Error::Sealed(m) | Error::Format(m) | Error::Unsupported(m)
            | Error::Panic(m) => m.clone(),
            Error::BufferTooSmall { needed, have } => format!("buffer too small: need {needed}, have {have}"),
            Error::BatchIncomplete { message, .. } => message.clone(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message())
    }
}

impl std::error::Error for Error {}

fn last_error_message() -> String {
    unsafe {
        let p = seraph_last_error();
        if p.is_null() {
            "unknown error".to_string()
        } else {
            CStr::from_ptr(p).to_string_lossy().into_owned()
        }
    }
}

/// The error of a failed call to a sentinel-returning export.
fn last_error() -> Error {
    Error::Store(last_error_message())
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

/// A search result, with the frame's two times.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub frame_id: String,
    pub score: f32,
    pub confidence: f32,
    pub path_length: u32,
    /// Commit time, microseconds since the epoch: the engine's own record.
    pub timestamp: i64,
    /// When the content happened, as claimed at ingest; `None` when the frame
    /// claims no event time.
    pub occurred_at: Option<i64>,
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

    /// Store format version (default 5, precision-tagged vectors).
    pub fn format_version(mut self, v: u32) -> Self {
        self.json_overrides.insert("format_version".into(), v.into());
        self
    }

    /// What each frame persists of its vector: `"codes4"` (default), `"fp32"`,
    /// `"fp16"`, `"bf16"` or `"fp64"`. Fixed at create for the store's life.
    pub fn vector_precision(mut self, name: &str) -> Self {
        self.json_overrides.insert("vector_precision".into(), name.into());
        self
    }

    /// What the walk scores from by default: `"fp32"`, `"fp16"`, `"bf16"` or
    /// `"frame"`. Unset picks the form that scores `vector_precision` exactly at
    /// the least memory.
    pub fn resident_precision(mut self, name: &str) -> Self {
        self.json_overrides.insert("resident_precision".into(), name.into());
        self
    }

    /// Similarity edges per frame (`0` = parent tree only). Persisted.
    pub fn edge_budget(mut self, v: usize) -> Self {
        self.json_overrides.insert("edge_budget".into(), (v as u64).into());
        self
    }

    /// How the semantic parent is chosen at commit: `"stream"` (default) or
    /// `"exact"` (the brute-force oracle, for verification). Persisted.
    pub fn parent_select(mut self, name: &str) -> Self {
        self.json_overrides.insert("parent_select".into(), name.into());
        self
    }

    /// Security posture: `"off"` or `"standard"`. Unset, the license decides.
    pub fn secure(mut self, mode: &str) -> Self {
        self.json_overrides.insert("secure".into(), mode.into());
        self
    }

    /// The overrides as a JSON object, with `model_id` when one is set.
    fn to_json_object(&self) -> serde_json::Value {
        let mut m = self.json_overrides.clone();
        if let Some(id) = &self.model_id {
            m.insert("model_id".into(), id.as_str().into());
        }
        serde_json::Value::Object(m)
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
    #[deprecated(since = "0.4.0", note = "use Store::open_with with OpenOptions { writer_seed, .. }")]
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
    #[deprecated(since = "0.4.0", note = "use Store::open_with with OpenOptions { passphrase, .. }")]
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
    #[deprecated(since = "0.4.0", note = "use Store::put_with with PutOptions { metadata, .. }")]
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
    /// Returns up to `top_k` hits above the similarity threshold `tau`, each
    /// with its commit and event time. Set `tau` to `0.0` to return all
    /// results regardless of similarity. Same as
    /// [`search_with`](Self::search_with) with [`SearchOptions::top`].
    pub fn search(&self, query_embedding: &[f32], top_k: usize, tau: f32) -> Result<Vec<Hit>, Error> {
        if top_k == 0 {
            return Ok(Vec::new());
        }
        Ok(self.search_with(query_embedding, &SearchOptions::top(top_k, tau))?.hits)
    }

    /// Search with options. Same as [`search`](Self::search) plus
    /// `include_superseded`: when `true`, plainly-superseded (soft-forgotten)
    /// frames still resident in the graph are opted back into results.
    /// Consolidated sources resolve through their redirect and are not
    /// resurfaced. `include_superseded = false` matches [`search`](Self::search).
    #[deprecated(since = "0.4.0", note = "use Store::search_with with SearchOptions { include_superseded, .. }")]
    pub fn search_opts(
        &self,
        query_embedding: &[f32],
        top_k: usize,
        tau: f32,
        include_superseded: bool,
    ) -> Result<Vec<Hit>, Error> {
        if top_k == 0 {
            return Ok(Vec::new());
        }
        let options = SearchOptions { include_superseded, ..SearchOptions::top(top_k, tau) };
        Ok(self.search_with(query_embedding, &options)?.hits)
    }

    /// Search with a temporal view. Identical relevance selection to
    /// [`search_opts`](Self::search_opts), plus:
    /// - `after_us` / `before_us`: inclusive window on event time
    ///   (microseconds since epoch), `None` = unbounded; a frame that claims
    ///   no event time stands at its commit time (the default basis).
    ///   Frames outside it still route, only ineligible for result slots, so
    ///   `top_k` fills with in-window frames.
    /// - `order`: presentation order for the selected top_k set -- a pure
    ///   post-selection reorder; scores and confidences are untouched.
    #[deprecated(since = "0.4.0", note = "use Store::search_with with SearchOptions { time, order, .. }")]
    pub fn search_temporal(
        &self,
        query_embedding: &[f32],
        top_k: usize,
        tau: f32,
        include_superseded: bool,
        order: TemporalOrder,
        after_us: Option<u64>,
        before_us: Option<u64>,
    ) -> Result<Vec<Hit>, Error> {
        if top_k == 0 {
            return Ok(Vec::new());
        }
        let bound = |v: Option<u64>, what: &str| -> Result<Option<i64>, Error> {
            v.map(|t| i64::try_from(t).map_err(|_| Error::InvalidArgument(format!("{what} {t} exceeds i64::MAX"))))
                .transpose()
        };
        let options = SearchOptions {
            include_superseded,
            order,
            time: TimeFilter {
                occurred: TimeWindow { after_us: bound(after_us, "after_us")?, before_us: bound(before_us, "before_us")? },
                ..Default::default()
            },
            ..SearchOptions::top(top_k, tau)
        };
        Ok(self.search_with(query_embedding, &options)?.hits)
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

    /// What each frame of this store persists of its vector: `"codes4"`,
    /// `"fp32"`, `"fp16"`, `"bf16"` or `"fp64"` (formats below 5 are `"codes4"`).
    pub fn vector_precision(&self) -> String {
        let p = unsafe { seraph_store_vector_precision(self.handle) };
        if p.is_null() { String::new() } else { unsafe { take_cstring(p) } }
    }

    /// What the walk currently scores from: `"fp32"`, `"fp16"`, `"bf16"` or `"frame"`.
    pub fn resident_precision(&self) -> String {
        let p = unsafe { seraph_store_resident_precision(self.handle) };
        if p.is_null() { String::new() } else { unsafe { take_cstring(p) } }
    }

    /// Change what the walk scores from and rebuild the resident copy at once:
    /// `"fp32"`, `"fp16"`, `"bf16"` (a contiguous arena at that width) or
    /// `"frame"` (none; 20-40% more per walk). Runtime only.
    pub fn set_resident_precision(&mut self, name: &str) -> Result<(), Error> {
        let c = c_arg(name, "resident_precision")?;
        let rc = unsafe { seraph_store_set_resident_precision(self.handle, c.as_ptr()) };
        if rc == 0 { Ok(()) } else { Err(last_error()) }
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

    /// Seal with an external time anchor over the order digest of every frame
    /// before the seal (take it with `order_digest_json(None)` after the last write).
    pub fn seal_with_anchor(&mut self, reason: Option<&str>, backend: &str, token: &[u8]) -> Result<String, Error> {
        let reason_c = reason.map(to_cstring);
        let backend_c = to_cstring(backend);
        let id = unsafe {
            seraph_store_seal_with_anchor(
                self.handle,
                reason_c.as_ref().map_or(ptr::null(), |c| c.as_ptr()),
                backend_c.as_ptr(),
                token.as_ptr(), token.len(),
            )
        };
        if id.is_null() { Err(last_error()) } else { Ok(unsafe { take_cstring(id) }) }
    }

    /// The order digest to anchor, as JSON `{"frame_count", "digest_hex"}`, over
    /// every frame through `through_frame_id` or the whole log.
    pub fn order_digest_json(&self, through_frame_id: Option<&str>) -> Result<String, Error> {
        let through_c = through_frame_id.map(to_cstring);
        let p = unsafe {
            seraph_store_order_digest(self.handle, through_c.as_ref().map_or(ptr::null(), |c| c.as_ptr()))
        };
        if p.is_null() { return Err(last_error()); }
        Ok(unsafe { take_cstring(p) })
    }

    /// Every seal and tip anchor that commits to an order digest, as JSON.
    pub fn anchor_claims_json(&self) -> Result<String, Error> {
        let p = unsafe { seraph_store_anchor_claims(self.handle) };
        if p.is_null() { return Err(last_error()); }
        Ok(unsafe { take_cstring(p) })
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

    /// Commit an application marker: a System sentinel with no geometric
    /// footprint (never an entry point, never a parent, never searched),
    /// durable in the log. `metadata_json` must be a JSON object (`"{}"` for
    /// none). Kinds the load path interprets and the metadata keys it acts on
    /// are refused. Returns the marker's frame ID.
    pub fn commit_marker(&mut self, kind: &str, metadata_json: &str) -> Result<String, Error> {
        let kind_c = to_cstring(kind);
        let meta_c = to_cstring(metadata_json);
        let id = unsafe { seraph_store_commit_marker(self.handle, kind_c.as_ptr(), meta_c.as_ptr()) };
        if id.is_null() { Err(last_error()) } else { Ok(unsafe { take_cstring(id) }) }
    }

    /// Markers of `kind` in record order, as a JSON array of
    /// `{"frame_id", "metadata"}`.
    pub fn markers_json(&self, kind: &str) -> Option<String> {
        let kind_c = std::ffi::CString::new(kind).ok()?;
        let p = unsafe { seraph_store_markers(self.handle, kind_c.as_ptr()) };
        if p.is_null() { None } else { Some(unsafe { take_cstring(p) }) }
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
    ///
    /// Deprecated alias of [`Self::put_batch_incremental_json`]: the two-phase
    /// snapshot path this name used to run was removed from the engine (it
    /// collapsed bulk loads into a star), and the FFI symbol now takes the
    /// incremental path. Leaves the store buffered: call `sync()` when done.
    #[deprecated(since = "0.4.0", note = "use Store::put_batch")]
    pub fn put_batch_json(&mut self, items_json: &str) -> Result<Vec<String>, Error> {
        let json_c = to_cstring(items_json);
        let p = unsafe { seraph_store_put_batch(self.handle, json_c.as_ptr()) };
        if p.is_null() { return Err(last_error()); }
        let json = unsafe { take_cstring(p) };
        Ok(serde_json::from_str(&json).unwrap_or_default())
    }

    /// Batch insert that lets the batch see itself — the geometrically true
    /// batched path, and the right default for bulk loads.
    ///
    /// Same `items_json` shape as [`Self::put_batch_json`]. Parents are selected
    /// against the live store, so a real tree forms: geometry identical to
    /// calling `put` in a loop on every metric measured, at roughly 7.8x the
    /// speed because writes are buffered.
    ///
    /// Insertion-order dependent — order the batch meaningfully
    /// (most-central-first) before calling. Leaves the store in buffered mode:
    /// call `sync()` when the load is done.
    #[deprecated(since = "0.4.0", note = "use Store::put_batch")]
    pub fn put_batch_incremental_json(&mut self, items_json: &str) -> Result<Vec<String>, Error> {
        let json_c = to_cstring(items_json);
        let p = unsafe { seraph_store_put_batch_incremental(self.handle, json_c.as_ptr()) };
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

    /// The authored seed list as JSON: an array of `{label, frame_id}`.
    ///
    /// Derived from Seed-status frames when no registry has been authored —
    /// see [`Self::seeds_are_authored`].
    pub fn get_seeds(&self) -> Result<String, Error> {
        let s = unsafe { seraph_store_get_seeds(self.handle) };
        if s.is_null() { Err(last_error()) } else { Ok(unsafe { take_cstring(s) }) }
    }

    /// Whether the seed list is authored (a registry sentinel exists) rather
    /// than derived from Seed-status frames.
    pub fn seeds_are_authored(&self) -> Result<bool, Error> {
        match unsafe { seraph_store_seeds_are_authored(self.handle) } {
            -1 => Err(last_error()),
            v  => Ok(v == 1),
        }
    }

    /// Resolve a seed label to its frame id, or `None` if not registered.
    pub fn resolve_seed(&self, label: &str) -> Result<Option<String>, Error> {
        let label_c = to_cstring(label);
        let id = unsafe { seraph_store_resolve_seed(self.handle, label_c.as_ptr()) };
        if !id.is_null() {
            return Ok(Some(unsafe { take_cstring(id) }));
        }
        // Null is both "not registered" and "failed". The export clears the
        // error slot on entry and sets it only on failure, so an empty slot
        // means the label simply is not bound.
        if unsafe { seraph_last_error().is_null() } {
            Ok(None)
        } else {
            Err(last_error())
        }
    }

    /// Replace the whole registry with `entries_json`, a JSON array of
    /// `{label, frame_id}`. Returns the snapshot sentinel's frame id.
    pub fn set_seeds(&mut self, entries_json: &str) -> Result<String, Error> {
        let entries_c = to_cstring(entries_json);
        let id = unsafe { seraph_store_set_seeds(self.handle, entries_c.as_ptr()) };
        if id.is_null() { Err(last_error()) } else { Ok(unsafe { take_cstring(id) }) }
    }

    /// Bind `label` to `frame_id`, appending a new snapshot. Rebinding an
    /// existing label updates it in place.
    pub fn register_seed(&mut self, label: &str, frame_id: &str) -> Result<String, Error> {
        let label_c = to_cstring(label);
        let fid_c = to_cstring(frame_id);
        let id = unsafe {
            seraph_store_register_seed(self.handle, label_c.as_ptr(), fid_c.as_ptr())
        };
        if id.is_null() { Err(last_error()) } else { Ok(unsafe { take_cstring(id) }) }
    }

    /// Drop `label` from the authored set. The frame it named is untouched.
    pub fn unregister_seed(&mut self, label: &str) -> Result<String, Error> {
        let label_c = to_cstring(label);
        let id = unsafe { seraph_store_unregister_seed(self.handle, label_c.as_ptr()) };
        if id.is_null() { Err(last_error()) } else { Ok(unsafe { take_cstring(id) }) }
    }

    /// Ingest content into a named subtree: the parent is chosen from inside
    /// `root`'s parent-edge subtree instead of from the store at large.
    ///
    /// `root` is a frame id or a seed label. Similarity edges are still chosen
    /// globally, so a placed frame stays reachable by ordinary
    /// [`search`](Self::search) — only the parent edge is overridden. The frame
    /// is stamped so a later migration re-derives its position inside the same
    /// region rather than re-routing it.
    #[deprecated(since = "0.4.0", note = "use Store::put_with with PutOptions { within, .. }")]
    pub fn put_subtree(
        &mut self,
        root: &str,
        content: &[u8],
        embedding: &[f32],
        metadata_json: Option<&str>,
    ) -> Result<String, Error> {
        let root_c = to_cstring(root);
        let meta_c = metadata_json.map(to_cstring);
        let id = unsafe {
            seraph_store_put_subtree(
                self.handle, root_c.as_ptr(),
                content.as_ptr(), content.len(),
                embedding.as_ptr(), embedding.len(),
                meta_c.as_ref().map_or(ptr::null(), |c| c.as_ptr()),
            )
        };
        if id.is_null() { Err(last_error()) } else { Ok(unsafe { take_cstring(id) }) }
    }

    /// Search confined to a named subtree: entry is the root (no Phase-1
    /// eigenframe scan), and the traversal follows parent edges plus only
    /// those similarity edges whose target is inside the subtree.
    ///
    /// `root` is a frame id or a seed label. `tau` of `None` uses the store's
    /// configured `tau_similarity`. Ordinary [`search`](Self::search) is
    /// unaffected and still reaches frames inside the subtree.
    #[deprecated(since = "0.4.0", note = "use Store::search_with with SearchOptions { within, .. }")]
    pub fn search_subtree(
        &self,
        root: &str,
        query: &[f32],
        top_k: usize,
        tau: Option<f32>,
    ) -> Result<Vec<Hit>, Error> {
        if top_k == 0 {
            return Ok(Vec::new());
        }
        let options = SearchOptions {
            top_k,
            tau: tau.map_or(Tau::Store, Tau::Value),
            within: Some(root.to_string()),
            ..Default::default()
        };
        Ok(self.search_with(query, &options)?.hits)
    }

    /// Promote an active frame to eigenframe.
    pub fn promote(&mut self, frame_id: &str) -> Result<(), Error> {
        let fid_c = to_cstring(frame_id);
        let rc = unsafe { seraph_store_promote(self.handle, fid_c.as_ptr()) };
        if rc == 0 { Ok(()) } else { Err(last_error()) }
    }

    /// Ingest content directly as eigenframe (atomic ingest + promote).
    #[deprecated(since = "0.4.0", note = "use Store::put_with with PutOptions { promote: true, .. }")]
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
    #[deprecated(since = "0.4.0", note = "use Store::set_tuning (search.visit_cap_multiplier)")]
    pub fn set_search_visit_cap_multiplier(&mut self, multiplier: u32) -> Result<(), Error> {
        let rc = unsafe { seraph_store_set_search_visit_cap_multiplier(self.handle, multiplier) };
        if rc == 0 { Ok(()) } else { Err(last_error()) }
    }

    /// Fix the Phase-1 entry rank floor (`0` restores the default).
    #[deprecated(since = "0.4.0", note = "use Store::set_tuning (search.entry_k)")]
    pub fn set_search_entry_k(&mut self, k: usize) -> Result<(), Error> {
        let rc = unsafe { seraph_store_set_search_entry_k(self.handle, k) };
        if rc == 0 { Ok(()) } else { Err(last_error()) }
    }

    /// Relative-gap Phase-1 entry breadth (`0` falls back to entry_k).
    #[deprecated(since = "0.4.0", note = "use Store::set_tuning (search.entry_gap)")]
    pub fn set_search_entry_gap(&mut self, gap: f32) -> Result<(), Error> {
        let rc = unsafe { seraph_store_set_search_entry_gap(self.handle, gap) };
        if rc == 0 { Ok(()) } else { Err(last_error()) }
    }

    /// EXPERIMENTAL: route the trace-free read path through the gradient-beam
    /// stream. `beam` of 0 means unbounded.
    #[deprecated(since = "0.4.0", note = "use Store::set_tuning (search.mode, search.beam)")]
    pub fn set_search_use_stream(&mut self, on: bool, beam: usize) -> Result<(), Error> {
        let rc = unsafe { seraph_store_set_search_use_stream(self.handle, on, beam) };
        if rc == 0 { Ok(()) } else { Err(last_error()) }
    }

    /// Weight of the commit-time seed term (default 0.6; `0.0` disables it).
    /// Runtime only: parents are stored on frames, so no migration is involved.
    #[deprecated(since = "0.4.0", note = "use Store::set_tuning (commit.seed_term)")]
    pub fn set_seed_term(&mut self, beta: f32) -> Result<(), Error> {
        let rc = unsafe { seraph_store_set_seed_term(self.handle, beta) };
        if rc == 0 { Ok(()) } else { Err(last_error()) }
    }

    /// Ceiling slack of the commit-time walk that picks each new frame's parent
    /// and similarity edges. Runtime only: frames store both.
    #[deprecated(since = "0.4.0", note = "use Store::set_tuning (commit.ceiling_margin)")]
    pub fn set_commit_ceiling_margin(&mut self, m: f32) -> Result<(), Error> {
        let rc = unsafe { seraph_store_set_commit_ceiling_margin(self.handle, m) };
        if rc == 0 { Ok(()) } else { Err(last_error()) }
    }

    /// Stop-line slack of the commit-time walk, the cosine form of the margin:
    /// placement stops once the best remaining candidate is no more than the k-th
    /// best minus `slack`. Default 0.05, the line search stops on; `0` falls back
    /// to the multiplicative margin.
    #[deprecated(since = "0.4.0", note = "use Store::set_tuning (commit.ceiling_slack)")]
    pub fn set_commit_ceiling_slack(&mut self, slack: f32) -> Result<(), Error> {
        let rc = unsafe { seraph_store_set_commit_ceiling_slack(self.handle, slack) };
        if rc == 0 { Ok(()) } else { Err(last_error()) }
    }

    /// How each committing frame chooses its similarity edges: `routing` allows
    /// edges to Genesis/Seed frames (default false), `parent` the edge to its own
    /// parent (default true), `occlusion > 0` prunes redundant candidates (`0` =
    /// top-k), `pool` widens the candidate list (`0` = `edge_budget + 1`).
    #[deprecated(since = "0.4.0", note = "use Store::set_tuning (commit.sim_edge_policy)")]
    pub fn set_sim_edge_policy(&mut self, routing: bool, parent: bool, occlusion: f32, pool: usize) -> Result<(), Error> {
        let rc = unsafe { seraph_store_set_sim_edge_policy(self.handle, routing, parent, occlusion, pool) };
        if rc == 0 { Ok(()) } else { Err(last_error()) }
    }

    /// Result-slot policy for `search`: `None` = cosine (default);
    /// `Some((share, doors, budget))` = two-channel, the last `round(share * top_k)`
    /// slots filled by a confined descent's best hits the cosine list lacks.
    #[deprecated(since = "0.4.0", note = "use Store::set_tuning (search.rank_mode)")]
    pub fn set_search_rank_mode(&mut self, two_channel: Option<(f32, usize, usize)>) -> Result<(), Error> {
        let (mode, (share, doors, budget)) = match two_channel {
            None => (0u8, (0.0, 0, 0)),
            Some(p) => (1u8, p),
        };
        let rc = unsafe { seraph_store_set_search_rank_mode(self.handle, mode, share, doors, budget) };
        if rc == 0 { Ok(()) } else { Err(last_error()) }
    }

    /// No effect: the walk is chosen by the tuning's search mode alone.
    #[deprecated(since = "0.4.0", note = "has no effect; Store::set_tuning (search.mode) chooses the walk")]
    pub fn set_search_stream_threshold(&mut self, n: usize) -> Result<(), Error> {
        let rc = unsafe { seraph_store_set_search_stream_threshold(self.handle, n) };
        if rc == 0 { Ok(()) } else { Err(last_error()) }
    }

    /// Absolute Phase-2 visit budget, decoupled from top_k (`0` falls back to
    /// `top_k * multiplier`).
    #[deprecated(since = "0.4.0", note = "use Store::set_tuning (search.max_visits)")]
    pub fn set_search_max_visits(&mut self, v: usize) -> Result<(), Error> {
        let rc = unsafe { seraph_store_set_search_max_visits(self.handle, v) };
        if rc == 0 { Ok(()) } else { Err(last_error()) }
    }

    /// Stream ceiling slack margin (values below 1.0 keep the beam exploring
    /// past the k-th-best score).
    #[deprecated(since = "0.4.0", note = "use Store::set_tuning (search.ceiling_margin)")]
    pub fn set_search_ceiling_margin(&mut self, m: f32) -> Result<(), Error> {
        let rc = unsafe { seraph_store_set_search_ceiling_margin(self.handle, m) };
        if rc == 0 { Ok(()) } else { Err(last_error()) }
    }

    /// Search stop-line slack: search stops once the best frontier score is no
    /// more than the k-th best minus `slack` (default 0.05; 0.10 is the
    /// completeness setting); 0 falls back to the multiplicative margin.
    #[deprecated(since = "0.4.0", note = "use Store::set_tuning (search.ceiling_slack)")]
    pub fn set_search_ceiling_slack(&mut self, slack: f32) -> Result<(), Error> {
        let rc = unsafe { seraph_store_set_search_ceiling_slack(self.handle, slack) };
        if rc == 0 { Ok(()) } else { Err(last_error()) }
    }

    /// Phase 1 through the door index, a navigable small-world graph over the
    /// eigenframes (default on). On builds the index at once when the store has
    /// enough eigenframes; off drops it and Phase 1 scans.
    #[deprecated(since = "0.4.0", note = "use Store::set_tuning (search.door_index)")]
    pub fn set_door_index(&mut self, on: bool) -> Result<(), Error> {
        let rc = unsafe { seraph_store_set_door_index(self.handle, on) };
        if rc == 0 { Ok(()) } else { Err(last_error()) }
    }

    /// Candidates the door index examines per query (default 128): higher is
    /// closer to the exact scan, at more distance evaluations.
    #[deprecated(since = "0.4.0", note = "use Store::set_tuning (search.door_index_ef)")]
    pub fn set_door_index_ef(&mut self, ef: usize) -> Result<(), Error> {
        let rc = unsafe { seraph_store_set_door_index_ef(self.handle, ef) };
        if rc == 0 { Ok(()) } else { Err(last_error()) }
    }

    /// Toggle the Phase-2 contiguous pre-normalized embedding arena. Default on.
    /// Off, the walk takes the same exact dot from each frame's embedding and a
    /// stored inverse norm, N*D f32 less resident, at 20-40% more per search.
    /// Results are unchanged either way.
    pub fn set_search_arena(&mut self, on: bool) -> Result<(), Error> {
        let rc = unsafe { seraph_store_set_search_arena(self.handle, on) };
        if rc == 0 { Ok(()) } else { Err(last_error()) }
    }

    /// When on, the Phase-2 flood reads the pre-resolved search adjacency.
    /// Results are unchanged either way.
    #[deprecated(since = "0.4.0", note = "use Store::set_tuning (search.adjacency)")]
    pub fn set_search_adjacency(&mut self, on: bool) -> Result<(), Error> {
        let rc = unsafe { seraph_store_set_search_adjacency(self.handle, on) };
        if rc == 0 { Ok(()) } else { Err(last_error()) }
    }

    /// DIAGNOSTIC: gate Phase-2 walk profiling (thread-local, zero-overhead off).
    pub fn set_walk_profile(&self, on: bool) -> Result<(), Error> {
        let rc = unsafe { seraph_store_set_walk_profile(self.handle, on) };
        if rc == 0 { Ok(()) } else { Err(last_error()) }
    }

    /// DIAGNOSTIC: read + reset the walk-profile accumulators. Returns JSON
    /// `{"resolve_ns", "visited_ns", "score_ns", "push_ns", "visit_count"}`.
    pub fn take_walk_profile_json(&self) -> Result<String, Error> {
        let p = unsafe { seraph_store_take_walk_profile(self.handle) };
        if p.is_null() { Err(last_error()) } else { Ok(unsafe { take_cstring(p) }) }
    }

    /// DIAGNOSTIC: number of Phase-1 entry points selected for a query.
    pub fn entry_count(&self, query_embedding: &[f32]) -> Result<usize, Error> {
        let n = unsafe {
            seraph_store_entry_count(self.handle, query_embedding.as_ptr(), query_embedding.len())
        };
        if n < 0 { Err(last_error()) } else { Ok(n as usize) }
    }

    /// DIAGNOSTIC: nodes visited by the last streamed search (ops cost).
    pub fn last_search_visits(&self) -> Result<usize, Error> {
        let n = unsafe { seraph_store_last_search_visits(self.handle) };
        if n < 0 { Err(last_error()) } else { Ok(n as usize) }
    }

    /// DIAGNOSTIC: frames examined to place the last frame committed (ops cost).
    pub fn last_commit_visits(&self) -> Result<usize, Error> {
        let n = unsafe { seraph_store_last_commit_visits(self.handle) };
        if n < 0 { Err(last_error()) } else { Ok(n as usize) }
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

    /// The query the engine walks with: `embedding` after a round trip
    /// through the store's write quantizer (identity on pre-v3 stores).
    pub fn quantize_query(&self, embedding: &[f32]) -> Result<Vec<f32>, Error> {
        let mut out_ptr: *mut f32 = ptr::null_mut();
        let mut out_dim: usize = 0;
        let rc = unsafe {
            seraph_store_quantize_query(self.handle, embedding.as_ptr(), embedding.len(), &mut out_ptr, &mut out_dim)
        };
        if rc != 0 { return Err(last_error()); }
        let q = unsafe { std::slice::from_raw_parts(out_ptr, out_dim) }.to_vec();
        unsafe { seraph_floats_free(out_ptr, out_dim) };
        Ok(q)
    }

    /// The packed 4-bit codes the store would persist for `embedding` — the
    /// bytes behind `codes_hex` in the frame JSON. Empty on a pre-v3 store or
    /// a dimension mismatch.
    pub fn encode_codes(&self, embedding: &[f32]) -> Result<Vec<u8>, Error> {
        let mut out_ptr: *mut u8 = ptr::null_mut();
        let mut out_len: usize = 0;
        let rc = unsafe {
            seraph_store_encode_codes(self.handle, embedding.as_ptr(), embedding.len(), &mut out_ptr, &mut out_len)
        };
        if rc != 0 { return Err(last_error()); }
        if out_ptr.is_null() || out_len == 0 { return Ok(Vec::new()); }
        let codes = unsafe { std::slice::from_raw_parts(out_ptr, out_len) }.to_vec();
        unsafe { seraph_bytes_free(out_ptr, out_len) };
        Ok(codes)
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
    #[deprecated(since = "0.4.0", note = "use Store::encode, then Store::search_with")]
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
        take_hits(self, hits_ptr, count)
    }

    /// Warped search.
    ///
    /// `targets_json` / `suppress_json`: JSON arrays of `{"embedding": [...], "magnitude": f32}`.
    #[deprecated(since = "0.4.0", note = "use Store::search_with with a compiled Warp")]
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
        take_hits(self, hits_ptr, count)
    }

    // ---------------------------------------------------------------- //
    // Neighborhood / Chain
    // ---------------------------------------------------------------- //

    /// Three-population neighborhood. Returns JSON.
    #[deprecated(since = "0.4.0", note = "use Store::neighborhood_with")]
    pub fn neighborhood_json(&self, frame_id: &str, tau: f32, hops: usize) -> Result<String, Error> {
        let fid_c = to_cstring(frame_id);
        let p = unsafe { seraph_store_neighborhood(self.handle, fid_c.as_ptr(), tau, hops) };
        if p.is_null() { return Err(last_error()); }
        Ok(unsafe { take_cstring(p) })
    }

    /// Three-population neighborhood with tau given as a [`TauSpec`]. Returns JSON.
    #[deprecated(since = "0.4.0", note = "use Store::neighborhood_with with a Tau rule")]
    pub fn neighborhood_spec_json(
        &self,
        frame_id: &str,
        tau: TauSpec,
        hops: usize,
    ) -> Result<String, Error> {
        let fid_c = to_cstring(frame_id);
        let (mode, value) = tau.to_mode_value();
        let p = unsafe {
            seraph_store_neighborhood_spec(self.handle, fid_c.as_ptr(), mode, value, hops)
        };
        if p.is_null() { return Err(last_error()); }
        Ok(unsafe { take_cstring(p) })
    }

    /// Chain between two frames. Returns JSON.
    #[deprecated(since = "0.4.0", note = "use Store::chain_with(Endpoint::Frame, Endpoint::Frame, ..)")]
    pub fn chain_json(&self, from_id: &str, to_id: &str) -> Result<String, Error> {
        let from_c = to_cstring(from_id);
        let to_c = to_cstring(to_id);
        let p = unsafe { seraph_store_chain(self.handle, from_c.as_ptr(), to_c.as_ptr()) };
        if p.is_null() { return Err(last_error()); }
        Ok(unsafe { take_cstring(p) })
    }

    /// `chain_json` with `include_superseded`: superseded frames may be steps.
    #[deprecated(since = "0.4.0", note = "use Store::chain_with with RouteOptions { include_superseded, .. }")]
    pub fn chain_opts_json(&self, from_id: &str, to_id: &str, include_superseded: bool) -> Result<String, Error> {
        let from_c = to_cstring(from_id);
        let to_c = to_cstring(to_id);
        let p = unsafe { seraph_store_chain_opts(self.handle, from_c.as_ptr(), to_c.as_ptr(), include_superseded) };
        if p.is_null() { return Err(last_error()); }
        Ok(unsafe { take_cstring(p) })
    }

    /// Provenance walk from `frame_id` as chain JSON (`tau: None` = the store's background).
    #[deprecated(since = "0.4.0", note = "use Store::provenance_with")]
    pub fn provenance_json(&self, frame_id: &str, tau: Option<f32>, include_superseded: bool) -> Result<String, Error> {
        let fid = to_cstring(frame_id);
        let p = unsafe { seraph_store_provenance(self.handle, fid.as_ptr(), tau.unwrap_or(f32::NAN), include_superseded) };
        if p.is_null() { return Err(last_error()); }
        Ok(unsafe { take_cstring(p) })
    }

    /// Influence walk from `frame_id` as chain JSON (`tau: None` = the store's background).
    #[deprecated(since = "0.4.0", note = "use Store::influence_with")]
    pub fn influence_json(&self, frame_id: &str, tau: Option<f32>, include_superseded: bool) -> Result<String, Error> {
        let fid = to_cstring(frame_id);
        let p = unsafe { seraph_store_influence(self.handle, fid.as_ptr(), tau.unwrap_or(f32::NAN), include_superseded) };
        if p.is_null() { return Err(last_error()); }
        Ok(unsafe { take_cstring(p) })
    }

    /// Provenance walk steered by a warp spec (JSON), as chain JSON.
    #[deprecated(since = "0.4.0", note = "use Store::provenance_with with a compiled Warp")]
    pub fn provenance_with_warp_spec_json(&self, frame_id: &str, warp_spec_json: &str, tau: Option<f32>, include_superseded: bool) -> Result<String, Error> {
        let fid = to_cstring(frame_id);
        let spec = to_cstring(warp_spec_json);
        let p = unsafe { seraph_store_provenance_with_warp_spec(self.handle, fid.as_ptr(), spec.as_ptr(), tau.unwrap_or(f32::NAN), include_superseded) };
        if p.is_null() { return Err(last_error()); }
        Ok(unsafe { take_cstring(p) })
    }

    /// Influence walk steered by a warp spec (JSON), as chain JSON.
    #[deprecated(since = "0.4.0", note = "use Store::influence_with with a compiled Warp")]
    pub fn influence_with_warp_spec_json(&self, frame_id: &str, warp_spec_json: &str, tau: Option<f32>, include_superseded: bool) -> Result<String, Error> {
        let fid = to_cstring(frame_id);
        let spec = to_cstring(warp_spec_json);
        let p = unsafe { seraph_store_influence_with_warp_spec(self.handle, fid.as_ptr(), spec.as_ptr(), tau.unwrap_or(f32::NAN), include_superseded) };
        if p.is_null() { return Err(last_error()); }
        Ok(unsafe { take_cstring(p) })
    }

    /// The distinct ways two frames connect, as JSON `[{"chain", "bridge"}]`.
    #[deprecated(since = "0.4.0", note = "use Store::connections_with")]
    pub fn connections_json(&self, from_id: &str, to_id: &str, max_routes: usize, forward: bool, include_superseded: bool) -> Result<String, Error> {
        let from_c = to_cstring(from_id);
        let to_c = to_cstring(to_id);
        let p = unsafe { seraph_store_connections(self.handle, from_c.as_ptr(), to_c.as_ptr(), max_routes, forward, include_superseded) };
        if p.is_null() { return Err(last_error()); }
        Ok(unsafe { take_cstring(p) })
    }

    /// How `from_id` led to `to_id`, as JSON: the geodesic over committed edges
    /// restricted to steps forward in record order (`from_id` must precede `to_id`).
    /// `include_superseded` admits superseded frames as steps.
    #[deprecated(since = "0.4.0", note = "use Store::chain_with with RouteOptions { forward: true, .. }")]
    pub fn chain_forward_json(&self, from_id: &str, to_id: &str, include_superseded: bool) -> Result<String, Error> {
        let from_c = to_cstring(from_id);
        let to_c = to_cstring(to_id);
        let p = unsafe { seraph_store_chain_forward(self.handle, from_c.as_ptr(), to_c.as_ptr(), include_superseded) };
        if p.is_null() { return Err(last_error()); }
        Ok(unsafe { take_cstring(p) })
    }

    /// Chain to nearest frame from a query embedding. Returns JSON.
    #[deprecated(since = "0.4.0", note = "use Store::chain_with(Endpoint::Entry, Endpoint::Query(q), ..)")]
    pub fn chain_to_json(&self, query_embedding: &[f32]) -> Result<String, Error> {
        let p = unsafe {
            seraph_store_chain_to(self.handle, query_embedding.as_ptr(), query_embedding.len())
        };
        if p.is_null() { return Err(last_error()); }
        Ok(unsafe { take_cstring(p) })
    }

    /// Chain between two query embeddings. Returns JSON.
    #[deprecated(since = "0.4.0", note = "use Store::chain_with(Endpoint::Query(a), Endpoint::Query(b), ..)")]
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

    /// How the idea at `emb_a` led to the idea at `emb_b`, as JSON: the forward
    /// geodesic between the best-fitting pair of the two queries' search results
    /// that a forward route joins. `include_superseded` admits superseded frames
    /// as endpoints and steps.
    #[deprecated(since = "0.4.0", note = "use Store::chain_with with RouteOptions { forward: true, fit, .. }")]
    pub fn chain_forward_between_json(&self, emb_a: &[f32], emb_b: &[f32], top_k: usize, tau: f32, include_superseded: bool) -> Result<String, Error> {
        let p = unsafe {
            seraph_store_chain_forward_between(
                self.handle,
                emb_a.as_ptr(), emb_a.len(),
                emb_b.as_ptr(), emb_b.len(),
                top_k, tau, include_superseded,
            )
        };
        if p.is_null() { return Err(last_error()); }
        Ok(unsafe { take_cstring(p) })
    }

    /// History read: search locates the answer, its top `anchors` hits choose the
    /// trees, a parent-tree walk returns the best `top_k` frames oldest first with
    /// their routes. Returns JSON (`hits`, `anchors`, `doors`, `visits`).
    #[deprecated(since = "0.4.0", note = "use Store::history_with")]
    pub fn history_json(&self, query_embedding: &[f32], top_k: usize, tau: f32, anchors: usize) -> Result<String, Error> {
        let p = unsafe {
            seraph_store_history(self.handle, query_embedding.as_ptr(), query_embedding.len(), top_k, tau, anchors)
        };
        if p.is_null() { return Err(last_error()); }
        Ok(unsafe { take_cstring(p) })
    }

    /// History read from a warped query (`warp_spec_json` as for the warp-spec search). Returns JSON.
    #[deprecated(since = "0.4.0", note = "use Store::history_with with a compiled Warp")]
    pub fn history_with_warp_spec_json(&self, query_embedding: &[f32], warp_spec_json: &str, top_k: usize, tau: f32,
                                       anchors: usize) -> Result<String, Error> {
        let spec_c = to_cstring(warp_spec_json);
        let p = unsafe {
            seraph_store_history_with_warp_spec(self.handle, query_embedding.as_ptr(), query_embedding.len(),
                                                spec_c.as_ptr(), top_k, tau, anchors)
        };
        if p.is_null() { return Err(last_error()); }
        Ok(unsafe { take_cstring(p) })
    }

    /// `chain_to_json` from a warped query (`warp_spec_json` as for the warp-spec search). Returns JSON.
    #[deprecated(since = "0.4.0", note = "use Store::chain_with with a compiled Warp")]
    pub fn chain_to_with_warp_spec_json(&self, query_embedding: &[f32], warp_spec_json: &str) -> Result<String, Error> {
        let spec_c = to_cstring(warp_spec_json);
        let p = unsafe {
            seraph_store_chain_to_with_warp_spec(self.handle, query_embedding.as_ptr(), query_embedding.len(), spec_c.as_ptr())
        };
        if p.is_null() { return Err(last_error()); }
        Ok(unsafe { take_cstring(p) })
    }

    /// `chain_between_json` with both endpoints warped (`warp_spec_json` as for the warp-spec search). Returns JSON.
    #[deprecated(since = "0.4.0", note = "use Store::chain_with with a compiled Warp")]
    pub fn chain_between_with_warp_spec_json(&self, emb_a: &[f32], emb_b: &[f32], warp_spec_json: &str) -> Result<String, Error> {
        let spec_c = to_cstring(warp_spec_json);
        let p = unsafe {
            seraph_store_chain_between_with_warp_spec(self.handle, emb_a.as_ptr(), emb_a.len(), emb_b.as_ptr(), emb_b.len(),
                                                      spec_c.as_ptr())
        };
        if p.is_null() { return Err(last_error()); }
        Ok(unsafe { take_cstring(p) })
    }

    /// `neighborhood_json` under a warp (`warp_spec_json` as for the warp-spec search). Returns JSON.
    #[deprecated(since = "0.4.0", note = "use Store::neighborhood_with with a compiled Warp")]
    pub fn neighborhood_with_warp_spec_json(&self, frame_id: &str, warp_spec_json: &str, tau: f32, hops: usize) -> Result<String, Error> {
        let fid_c = to_cstring(frame_id);
        let spec_c = to_cstring(warp_spec_json);
        let p = unsafe {
            seraph_store_neighborhood_with_warp_spec(self.handle, fid_c.as_ptr(), spec_c.as_ptr(), tau, hops)
        };
        if p.is_null() { return Err(last_error()); }
        Ok(unsafe { take_cstring(p) })
    }

    /// Search through the bounded walk with an explicit beam width and visit budget
    /// (`None` = the store's `search_max_visits`); records the visit count for
    /// [`last_search_visits`](Self::last_search_visits). With the store's own beam and
    /// budget it returns the same hits as [`search`](Self::search).
    #[deprecated(since = "0.4.0", note = "use Store::search_with with SearchOptions { beam, max_visits, .. }")]
    pub fn search_stream(&self, query_embedding: &[f32], top_k: usize, tau: f32, beam_width: usize,
                         max_visits: Option<usize>) -> Result<Vec<Hit>, Error> {
        let mut hits_ptr: *mut FfiHit = ptr::null_mut();
        let mut count: usize = 0;
        let rc = unsafe {
            seraph_store_search_stream(self.handle, query_embedding.as_ptr(), query_embedding.len(), top_k, tau,
                                       beam_width, max_visits.unwrap_or(0), &mut hits_ptr, &mut count)
        };
        if rc != 0 { return Err(last_error()); }
        take_hits(self, hits_ptr, count)
    }

    /// Search returning the hits and the frames visited, in visit order, as JSON
    /// (`hits`, `trace`).
    #[deprecated(since = "0.4.0", note = "use Store::search_with with SearchOptions { trace: true, .. }")]
    pub fn search_with_trace_json(&self, query_embedding: &[f32], top_k: usize, tau: f32) -> Result<String, Error> {
        let p = unsafe {
            seraph_store_search_with_trace(self.handle, query_embedding.as_ptr(), query_embedding.len(), top_k, tau)
        };
        if p.is_null() { return Err(last_error()); }
        Ok(unsafe { take_cstring(p) })
    }

    /// Seed-rooted subtrees with at least `min_size` descendants, as a JSON array of
    /// `{"root", "root_status", "descendant_count", "eigenframe_count", "max_depth"}`.
    pub fn subtrees_json(&self, min_size: usize) -> Result<String, Error> {
        let p = unsafe { seraph_store_subtrees(self.handle, min_size) };
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

    /// Similarity neighbours of a frame — the LIVE edge set. Returns a JSON
    /// array of `{"frame_id", "weight"}`.
    ///
    /// Includes edges other frames aimed at this one, so it is the traversable
    /// neighbourhood rather than the frame's own choice — for that see
    /// [`Self::committed_sim_edges_json`].
    pub fn sim_neighbors_json(&self, frame_id: &str) -> Option<String> {
        let c = std::ffi::CString::new(frame_id).ok()?;
        let p = unsafe { seraph_store_sim_neighbors(self.handle, c.as_ptr()) };
        if p.is_null() { None } else { Some(unsafe { take_cstring(p) }) }
    }

    /// The similarity neighbours a frame CHOSE at commit time. Returns a JSON
    /// array of `{"frame_id", "weight"}`, or JSON `null` for a frame written
    /// before the choice was recorded.
    ///
    /// Provenance rather than topology: what the frame decided against the
    /// store as it stood, not what accumulated around it. Always a subset of
    /// [`Self::sim_neighbors_json`].
    pub fn committed_sim_edges_json(&self, frame_id: &str) -> Option<String> {
        let c = std::ffi::CString::new(frame_id).ok()?;
        let p = unsafe { seraph_store_committed_sim_edges(self.handle, c.as_ptr()) };
        if p.is_null() { None } else { Some(unsafe { take_cstring(p) }) }
    }

    /// Resolve tau: auto-threshold from corpus statistics.
    pub fn resolve_tau(&self) -> f32 {
        unsafe { seraph_store_resolve_tau(self.handle) }
    }

    /// Resolve tau from a [`TauSpec`] (auto p95 / loose p75 / strict p99 /
    /// explicit passthrough).
    pub fn resolve_tau_spec(&self, tau: TauSpec) -> f32 {
        let (mode, value) = tau.to_mode_value();
        unsafe { seraph_store_resolve_tau_spec(self.handle, mode, value) }
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

    /// Three-population contradiction scan (v2) at a [`TauSpec`] threshold.
    /// Returns JSON `{"geometric": [...], "lineage": [...], "consensus": [...]}`.
    pub fn contradictions_v2_json(&self, tau: TauSpec) -> Result<String, Error> {
        let (mode, value) = tau.to_mode_value();
        let p = unsafe { seraph_store_contradictions_v2(self.handle, mode, value) };
        if p.is_null() { Err(last_error()) } else { Ok(unsafe { take_cstring(p) }) }
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

    /// Consolidate specific frames under an explicit [`MergeStrategy`].
    /// Returns the same JSON report as [`consolidate_json`](Store::consolidate_json).
    #[deprecated(since = "0.4.0", note = "use Store::consolidate_json; the geometric strategy is the only one the engine runs")]
    pub fn consolidate_with_strategy_json(
        &mut self,
        frame_ids: &[&str],
        strategy: MergeStrategy,
    ) -> Result<String, Error> {
        let ids_c: Vec<CString> = frame_ids.iter().map(|s| to_cstring(s)).collect();
        let id_ptrs: Vec<*const c_char> = ids_c.iter().map(|c| c.as_ptr()).collect();
        let strat = match strategy {
            MergeStrategy::Geometric => 0,
            MergeStrategy::Semantic => 1,
        };
        let p = unsafe {
            seraph_store_consolidate_with_strategy(
                self.handle, id_ptrs.as_ptr(), id_ptrs.len(), strat,
            )
        };
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

    /// Order-free commitment to the store's retrievable content: 64 hex
    /// chars over the sorted multiset of `sha256(content_hash || codes)`
    /// leaves (scheme `seraph-content-set/v1`). Same content under the same
    /// model gives the same root regardless of record order or graph.
    pub fn content_set_root(&self) -> Result<String, Error> {
        let p = unsafe { seraph_store_content_set_root(self.handle) };
        if p.is_null() { Err(last_error()) } else { Ok(unsafe { take_cstring(p) }) }
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
            seraph_store_subscribe(self.handle, name_c.as_ptr(), Some(callback), user_data)
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
    /// (no built-in presets -- an uncalibrated store errors on named use; see
    /// `fixup_warp`). The JSON format is that of [`compile_warp`](Self::compile_warp).
    #[deprecated(since = "0.4.0", note = "use Store::search_with with a compiled Warp")]
    pub fn search_with_warp_spec(
        &self,
        query_embedding: &[f32],
        warp_spec_json: &str,
        top_k: usize,
        tau: f32,
    ) -> Result<Vec<Hit>, Error> {
        if top_k == 0 {
            return Ok(Vec::new());
        }
        let warp = self.compile_warp(warp_spec_json)?;
        let options = SearchOptions { warp: Some(&warp), ..SearchOptions::top(top_k, tau) };
        Ok(self.search_with(query_embedding, &options)?.hits)
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
            2 => Err(Error::BufferTooSmall { needed: written, have: buf.len() }),
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
            2 => Err(Error::BufferTooSmall { needed: written, have: buf.len() }),
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
#[deprecated(since = "0.4.0", note = "use Store::warp_query")]
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

/// What a [`migrate_store`] run carried across.
#[derive(Debug, Clone, Default)]
pub struct MigrateReport {
    pub content: usize,
    pub seeds: usize,
    pub sentinels: usize,
    pub superseded: usize,
    pub redirects: usize,
    pub skipped: usize,
    /// Content frames whose parent came out as genesis — a star indicator.
    pub on_genesis: usize,
}

/// Rebuild a store's topology while preserving everything else.
///
/// Re-commits every frame into a fresh destination with its identity intact —
/// same frame id, timestamp, content, embedding, metadata, `status_ref` and
/// `status_sources` — but with its parent selected against the live
/// destination. Preserving ids is what makes it a migration rather than a
/// rebuild: typed-edge sentinels, supersede sentinels and redirect targets all
/// reference frames by id, so they stay valid without remapping and
/// reconstruct on the destination's first open.
///
/// Use it to correct a topology built by an older parent-selection strategy or
/// by the two-phase batch path, and as the carrier for frame-format changes —
/// the destination is always created at the current format version.
///
/// Eigenframe status is deliberately not carried: source eigenframes are
/// committed as `Active` so the destination promotes on its own geometry.
/// Watermarks are recomputed from each frame's new parent, so they differ from
/// the source by construction; the destination chain is internally valid and
/// `verify()` passes.
///
/// Embeddings are carried verbatim — this does not re-encode. The destination
/// keeps the source's model, and a mismatched model is rejected.
#[deprecated(since = "0.4.0", note = "use migrate_store_with")]
pub fn migrate_store(
    src_path: impl AsRef<Path>,
    dst_path: impl AsRef<Path>,
) -> Result<MigrateReport, Error> {
    let src_c = path_to_cstring(src_path.as_ref());
    let dst_c = path_to_cstring(dst_path.as_ref());
    let p = unsafe { seraph_migrate_store(src_c.as_ptr(), dst_c.as_ptr()) };
    if p.is_null() { return Err(last_error()); }
    let json = unsafe { take_cstring(p) };
    let v: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
    Ok(MigrateReport::from_json(&v))
}

impl MigrateReport {
    fn from_json(v: &serde_json::Value) -> Self {
        let g = |k: &str| v.get(k).and_then(|x| x.as_u64()).unwrap_or(0) as usize;
        MigrateReport {
            content: g("content"),
            seeds: g("seeds"),
            sentinels: g("sentinels"),
            superseded: g("superseded"),
            redirects: g("redirects"),
            skipped: g("skipped"),
            on_genesis: g("on_genesis"),
        }
    }
}

/// Take a legacy hit array (which carries no times) and give each hit its
/// commit and event time from the frame's record.
fn take_hits(store: &Store, hits_ptr: *mut FfiHit, count: usize) -> Result<Vec<Hit>, Error> {
    let mut raw = Vec::with_capacity(count);
    if !hits_ptr.is_null() && count > 0 {
        for i in 0..count {
            let ffi = unsafe { &*hits_ptr.add(i) };
            raw.push((
                unsafe { CStr::from_ptr(ffi.frame_id) }.to_string_lossy().into_owned(),
                ffi.score, ffi.confidence, ffi.path_length,
            ));
        }
        unsafe { seraph_hits_free(hits_ptr, count) };
    }
    raw.into_iter().map(|(frame_id, score, confidence, path_length)| {
        let l = store.lifecycle(&frame_id)?
            .ok_or_else(|| Error::Store(format!("hit {frame_id} has no record")))?;
        Ok(Hit {
            frame_id,
            score,
            confidence,
            path_length,
            timestamp: l.committed.committed_at,
            occurred_at: l.occurred_at,
        })
    }).collect()
}

/// Take the hit array of `seraph_search`, which carries both times.
fn take_search_hits(hits_ptr: *mut FfiSearchHit, count: usize) -> Vec<Hit> {
    let mut results = Vec::with_capacity(count);
    if !hits_ptr.is_null() && count > 0 {
        for i in 0..count {
            let ffi = unsafe { &*hits_ptr.add(i) };
            results.push(Hit {
                frame_id: unsafe { CStr::from_ptr(ffi.frame_id) }.to_string_lossy().into_owned(),
                score: ffi.score,
                confidence: ffi.confidence,
                path_length: ffi.path_length,
                timestamp: ffi.committed_us,
                occurred_at: (ffi.flags & HIT_HAS_OCCURRED != 0).then_some(ffi.occurred_us),
            });
        }
        unsafe { seraph_search_hits_free(hits_ptr, count) };
    }
    results
}

// ---------------------------------------------------------------- //
// Canonical surface: one operation per question, typed options
// ---------------------------------------------------------------- //

extern "C" {
    fn seraph_abi_version() -> u32;
    fn seraph_open(path: *const c_char, options: *const FfiOpenOptions, out_store: *mut *mut c_void) -> i32;
    fn seraph_put(
        store: *mut c_void,
        content: *const u8, content_len: usize,
        embedding: *const f32, dim: usize,
        options: *const FfiPutOptions,
        out_frame_id: *mut *mut c_char,
    ) -> i32;
    fn seraph_put_batch(
        store: *mut c_void, items: *const FfiPutItem, count: usize, dim: usize,
        out_ids_json: *mut *mut c_char,
    ) -> i32;
    fn seraph_search(
        store: *const c_void, query: *const f32, dim: usize,
        options: *const FfiSearchOptions,
        out_hits: *mut *mut FfiSearchHit, out_count: *mut usize,
        out_trace_json: *mut *mut c_char, out_visits: *mut usize,
    ) -> i32;
    fn seraph_search_hits_free(hits: *mut FfiSearchHit, count: usize);
    fn seraph_lifecycle(store: *const c_void, frame_id: *const c_char, out_json: *mut *mut c_char) -> i32;
    fn seraph_chain(
        store: *const c_void, from: *const FfiEndpoint, to: *const FfiEndpoint,
        options: *const FfiRouteOptions, out_json: *mut *mut c_char,
    ) -> i32;
    fn seraph_connections(
        store: *const c_void, from: *const FfiEndpoint, to: *const FfiEndpoint,
        options: *const FfiRouteOptions, max_routes: usize, out_json: *mut *mut c_char,
    ) -> i32;
    fn seraph_provenance(
        store: *const c_void, frame_id: *const c_char,
        options: *const FfiStrandOptions, out_json: *mut *mut c_char,
    ) -> i32;
    fn seraph_influence(
        store: *const c_void, frame_id: *const c_char,
        options: *const FfiStrandOptions, out_json: *mut *mut c_char,
    ) -> i32;
    fn seraph_history(
        store: *const c_void, query: *const f32, dim: usize,
        options: *const FfiHistoryOptions, out_json: *mut *mut c_char,
    ) -> i32;
    fn seraph_neighborhood(
        store: *const c_void, frame_id: *const c_char,
        options: *const FfiNeighborhoodOptions, out_json: *mut *mut c_char,
    ) -> i32;
    fn seraph_tuning_default(out: *mut FfiTuning) -> i32;
    fn seraph_get_tuning(store: *const c_void, out: *mut FfiTuning) -> i32;
    fn seraph_set_tuning(store: *mut c_void, tuning: *const FfiTuning) -> i32;
    fn seraph_migrate(
        src_path: *const c_char, dst_path: *const c_char,
        options: *const FfiMigrateOptions, out_report_json: *mut *mut c_char,
    ) -> i32;
    fn seraph_warp_compile(store: *const c_void, spec_json: *const c_char, out_warp: *mut *mut c_void) -> i32;
    fn seraph_warp_free(warp: *mut c_void);
}

// Status codes and option constants of the C ABI.
const SERAPH_OK: i32 = 0;
const SERAPH_NOT_FOUND: i32 = 1;
const SERAPH_E_NULL_ARG: i32 = -2;
const SERAPH_E_INVALID_ARG: i32 = -3;
const SERAPH_E_DIM: i32 = -4;
const SERAPH_E_SEALED: i32 = -5;
const SERAPH_E_FORMAT: i32 = -6;
const SERAPH_E_UNSUPPORTED: i32 = -7;
const SERAPH_E_PANIC: i32 = -8;
const SERAPH_E_BATCH_INCOMPLETE: i32 = -9;

const PUT_HAS_OCCURRED_AT: u32 = 1;
const PUT_PROMOTE: u32 = 2;
const PUT_NO_SEED_TERM: u32 = 4;
const SEARCH_INCLUDE_SUPERSEDED: u32 = 1;
const SEARCH_TRACE: u32 = 2;
const TIME_HAS_OCCURRED_AFTER: u32 = 0x10;
const TIME_HAS_OCCURRED_BEFORE: u32 = 0x20;
const TIME_HAS_COMMITTED_AFTER: u32 = 0x40;
const TIME_HAS_COMMITTED_BEFORE: u32 = 0x80;
const BASIS_OR_COMMITTED: i32 = 0;
const BASIS_CLAIMED_ONLY: i32 = 1;
const HIT_HAS_OCCURRED: u32 = 1;
const ENDPOINT_FRAME: i32 = 0;
const ENDPOINT_QUERY: i32 = 1;
const ENDPOINT_ENTRY: i32 = 2;
const ROUTE_FORWARD: u32 = 1;
const ROUTE_INCLUDE_SUPERSEDED: u32 = 2;
const ROUTE_HAS_FIT: u32 = 4;
const ROUTE_HAS_AS_OF: u32 = 8;
const STRAND_INCLUDE_SUPERSEDED: u32 = 1;
const STRAND_HAS_AS_OF: u32 = 2;
const OPEN_READ_ONLY: u32 = 1;
const OPEN_ALLOW_CROSS_CUSTOMER_WRITE: u32 = 2;
const MIGRATE_NO_SEED_TERM: u32 = 1;
const TUNING_DOOR_INDEX: u32 = 1;
const TUNING_ADJACENCY: u32 = 2;
const TUNING_SIM_EDGE_ROUTING: u32 = 4;
const TUNING_SIM_EDGE_PARENT: u32 = 8;

// repr(C) mirrors of the canonical structs in the engine's ffi.rs.

#[repr(C)]
#[derive(Clone, Copy)]
struct FfiTau {
    mode: i32,
    value: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct FfiPutOptions {
    struct_size: u32,
    flags: u32,
    metadata_json: *const c_char,
    occurred_at_us: i64,
    within: *const c_char,
    marking: *const u8,
    marking_len: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct FfiPutItem {
    content: *const u8,
    content_len: usize,
    embedding: *const f32,
    options: FfiPutOptions,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct FfiSearchOptions {
    struct_size: u32,
    flags: u32,
    top_k: usize,
    tau: FfiTau,
    occurred_after_us: i64,
    occurred_before_us: i64,
    committed_after_us: i64,
    committed_before_us: i64,
    basis: i32,
    order: i32,
    within: *const c_char,
    warp: *const c_void,
    max_visits: usize,
    beam: usize,
}

#[repr(C)]
struct FfiSearchHit {
    frame_id: *mut c_char,
    score: f32,
    confidence: f32,
    path_length: u32,
    flags: u32,
    committed_us: i64,
    occurred_us: i64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct FfiEndpoint {
    kind: i32,
    frame_id: *const c_char,
    query: *const f32,
    query_len: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct FfiRouteOptions {
    struct_size: u32,
    flags: u32,
    warp: *const c_void,
    fit_top_k: usize,
    fit_tau: f32,
    reserved: u32,
    as_of_us: i64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct FfiStrandOptions {
    struct_size: u32,
    flags: u32,
    tau: FfiTau,
    warp: *const c_void,
    as_of_us: i64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct FfiHistoryOptions {
    struct_size: u32,
    flags: u32,
    top_k: usize,
    tau: FfiTau,
    anchors: usize,
    warp: *const c_void,
    occurred_after_us: i64,
    occurred_before_us: i64,
    committed_after_us: i64,
    committed_before_us: i64,
    basis: i32,
    reserved: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct FfiNeighborhoodOptions {
    struct_size: u32,
    flags: u32,
    tau: FfiTau,
    hops: usize,
    warp: *const c_void,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct FfiOpenOptions {
    struct_size: u32,
    flags: u32,
    writer_seed: *const u8,
    passphrase: *const c_char,
    tuning: *const FfiTuning,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct FfiMigrateOptions {
    struct_size: u32,
    flags: u32,
    config_json: *const c_char,
    seeds_json: *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct FfiTuning {
    struct_size: u32,
    flags: u32,
    search_mode: i32,
    rank_mode: i32,
    beam: usize,
    max_visits: usize,
    visit_cap_multiplier: u32,
    max_depth: u32,
    search_ceiling_slack: f32,
    search_ceiling_margin: f32,
    entry_k: usize,
    entry_gap: f32,
    rank_share: f32,
    rank_doors: usize,
    rank_budget: usize,
    door_index_min: usize,
    door_index_ef: usize,
    eigen_matmul_min: usize,
    eigen_gpu_min: usize,
    commit_ceiling_slack: f32,
    commit_ceiling_margin: f32,
    seed_term: f32,
    sim_edge_occlusion: f32,
    sim_edge_pool: usize,
}

#[cfg(target_pointer_width = "64")]
const _: () = {
    use std::mem::size_of;
    assert!(size_of::<FfiTau>() == 8);
    assert!(size_of::<FfiPutOptions>() == 48);
    assert!(size_of::<FfiPutItem>() == 72);
    assert!(size_of::<FfiSearchOptions>() == 96);
    assert!(size_of::<FfiSearchHit>() == 40);
    assert!(size_of::<FfiEndpoint>() == 32);
    assert!(size_of::<FfiRouteOptions>() == 40);
    assert!(size_of::<FfiStrandOptions>() == 32);
    assert!(size_of::<FfiHistoryOptions>() == 80);
    assert!(size_of::<FfiNeighborhoodOptions>() == 32);
    assert!(size_of::<FfiOpenOptions>() == 32);
    assert!(size_of::<FfiMigrateOptions>() == 24);
    assert!(size_of::<FfiTuning>() == 136);
};

/// The size of a C struct as its `struct_size` field.
fn struct_size<T>() -> u32 {
    std::mem::size_of::<T>() as u32
}

/// The version of the C ABI the linked engine implements.
pub fn abi_version() -> u32 {
    unsafe { seraph_abi_version() }
}

/// The error a canonical call's status code stands for, with the engine's message.
fn status_error(code: i32) -> Error {
    let msg = last_error_message();
    match code {
        SERAPH_NOT_FOUND => Error::NotFound(msg),
        SERAPH_E_NULL_ARG => Error::NullArgument(msg),
        SERAPH_E_INVALID_ARG => Error::InvalidArgument(msg),
        SERAPH_E_DIM => Error::Dimension(msg),
        SERAPH_E_SEALED => Error::Sealed(msg),
        SERAPH_E_FORMAT => Error::Format(msg),
        SERAPH_E_UNSUPPORTED => Error::Unsupported(msg),
        SERAPH_E_PANIC => Error::Panic(msg),
        SERAPH_E_BATCH_INCOMPLETE => Error::BatchIncomplete { committed: Vec::new(), message: msg },
        _ => Error::Store(msg),
    }
}

/// `Ok(())` for `SERAPH_OK`, the matching error otherwise.
fn check_status(code: i32) -> Result<(), Error> {
    if code == SERAPH_OK { Ok(()) } else { Err(status_error(code)) }
}

/// A C string from caller text; an interior NUL is an invalid argument.
fn c_arg(s: &str, what: &str) -> Result<CString, Error> {
    CString::new(s).map_err(|_| Error::InvalidArgument(format!("{what} contains a NUL byte")))
}

fn opt_c_arg(s: Option<&str>, what: &str) -> Result<Option<CString>, Error> {
    s.map(|s| c_arg(s, what)).transpose()
}

fn c_ptr(c: &Option<CString>) -> *const c_char {
    c.as_ref().map_or(ptr::null(), |c| c.as_ptr())
}

/// Take an engine-owned JSON string and parse it.
fn take_json(p: *mut c_char) -> Result<serde_json::Value, Error> {
    if p.is_null() {
        return Err(Error::Store("the engine returned no result".into()));
    }
    let s = unsafe { take_cstring(p) };
    serde_json::from_str(&s).map_err(|e| Error::Store(format!("malformed engine JSON: {e}")))
}

// ---------------------------------------------------------------- //
// Value types
// ---------------------------------------------------------------- //

/// A similarity threshold, or the rule that picks one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tau {
    /// This value.
    Value(f32),
    /// The store's configured `tau_similarity`.
    Store,
    /// The 95th percentile of the store's background similarity.
    Background,
    /// The 75th percentile of the store's background similarity.
    Loose,
    /// The 99th percentile of the store's background similarity.
    Strict,
    /// The 95th percentile of this read's own vector against the store's frames.
    QueryBackground,
}

impl Tau {
    fn to_ffi(self) -> FfiTau {
        let (mode, value) = match self {
            Tau::Value(v) => (1, v),
            Tau::Store => (2, 0.0),
            Tau::Background => (3, 0.0),
            Tau::Loose => (4, 0.0),
            Tau::Strict => (5, 0.0),
            Tau::QueryBackground => (6, 0.0),
        };
        FfiTau { mode, value }
    }
}

/// An inclusive window on one time axis, in microseconds since the epoch;
/// `None` leaves that end open.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TimeWindow {
    pub after_us: Option<i64>,
    pub before_us: Option<i64>,
}

/// How a frame that claims no event time meets an `occurred` window.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum EventBasis {
    /// At its commit time (the default). A store without event times then
    /// answers as it did before event time existed.
    #[default]
    OrCommitted,
    /// Not at all: only frames that claim an event time can match.
    ClaimedOnly,
}

/// Which frames a read may return, on two time axes of different standing.
/// A frame's commit time is the engine's own observation (attested); its
/// event time is when the content happened, as the caller claimed it at
/// ingest. Both windows may be set on one read.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TimeFilter {
    /// On event time. A frame that claims none is placed by `basis`: at its
    /// commit time by default.
    pub occurred: TimeWindow,
    pub basis: EventBasis,
    /// On commit time, never mixed with event time. Its `before` end reads
    /// the record as it stood then: later frames neither route nor fill slots.
    pub committed: TimeWindow,
}

impl TimeFilter {
    /// The flag bits, the four bounds and the basis code, as C takes them.
    fn to_ffi(&self) -> (u32, [i64; 4], i32) {
        let mut flags = 0;
        let mut b = [0i64; 4];
        for (i, (v, bit)) in [
            (self.occurred.after_us, TIME_HAS_OCCURRED_AFTER),
            (self.occurred.before_us, TIME_HAS_OCCURRED_BEFORE),
            (self.committed.after_us, TIME_HAS_COMMITTED_AFTER),
            (self.committed.before_us, TIME_HAS_COMMITTED_BEFORE),
        ].into_iter().enumerate() {
            if let Some(t) = v {
                flags |= bit;
                b[i] = t;
            }
        }
        let basis = match self.basis {
            EventBasis::OrCommitted => BASIS_OR_COMMITTED,
            EventBasis::ClaimedOnly => BASIS_CLAIMED_ONLY,
        };
        (flags, b, basis)
    }
}

// ---------------------------------------------------------------- //
// Ingest options
// ---------------------------------------------------------------- //

/// How one frame is committed.
#[derive(Debug, Clone, PartialEq)]
pub struct PutOptions {
    /// Frame metadata, a JSON object.
    pub metadata: serde_json::Map<String, serde_json::Value>,
    /// When the content happened, microseconds since the epoch. Bound into the
    /// watermark; needs a format-6 store.
    pub occurred_at: Option<i64>,
    /// Place the frame inside this region: a frame id or a seed label.
    pub within: Option<String>,
    /// `false` takes the raw-cosine parent for this frame.
    pub seed_term: bool,
    /// Promote the frame to an eigenframe at once.
    pub promote: bool,
    /// Gov classification marking. Gov stores only.
    pub marking: Option<Vec<u8>>,
}

impl Default for PutOptions {
    fn default() -> Self {
        PutOptions {
            metadata: serde_json::Map::new(),
            occurred_at: None,
            within: None,
            seed_term: true,
            promote: false,
            marking: None,
        }
    }
}

/// A `FfiPutOptions` and the C strings it points into.
struct PutOptionsC {
    raw: FfiPutOptions,
    _metadata: Option<CString>,
    _within: Option<CString>,
}

impl PutOptions {
    /// Options carrying `metadata` and nothing else.
    pub fn with_metadata(metadata: serde_json::Map<String, serde_json::Value>) -> Self {
        PutOptions { metadata, ..Default::default() }
    }

    fn to_ffi(&self) -> Result<PutOptionsC, Error> {
        let metadata = if self.metadata.is_empty() { None } else {
            Some(c_arg(&serde_json::Value::Object(self.metadata.clone()).to_string(), "metadata")?)
        };
        let within = opt_c_arg(self.within.as_deref(), "within")?;
        let mut flags = 0;
        if self.occurred_at.is_some() { flags |= PUT_HAS_OCCURRED_AT; }
        if self.promote { flags |= PUT_PROMOTE; }
        if !self.seed_term { flags |= PUT_NO_SEED_TERM; }
        let raw = FfiPutOptions {
            struct_size: struct_size::<FfiPutOptions>(),
            flags,
            metadata_json: c_ptr(&metadata),
            occurred_at_us: self.occurred_at.unwrap_or(0),
            within: c_ptr(&within),
            marking: self.marking.as_ref().map_or(ptr::null(), |m| m.as_ptr()),
            marking_len: self.marking.as_ref().map_or(0, |m| m.len()),
        };
        Ok(PutOptionsC { raw, _metadata: metadata, _within: within })
    }
}

/// One frame of a batch.
#[derive(Debug, Clone, PartialEq)]
pub struct PutItem {
    pub content: Vec<u8>,
    pub embedding: Vec<f32>,
    pub options: PutOptions,
}

impl From<(Vec<u8>, Vec<f32>)> for PutItem {
    fn from((content, embedding): (Vec<u8>, Vec<f32>)) -> Self {
        PutItem { content, embedding, options: PutOptions::default() }
    }
}

impl From<(Vec<u8>, Vec<f32>, PutOptions)> for PutItem {
    fn from((content, embedding, options): (Vec<u8>, Vec<f32>, PutOptions)) -> Self {
        PutItem { content, embedding, options }
    }
}

// ---------------------------------------------------------------- //
// Search options and outcome
// ---------------------------------------------------------------- //

/// One search: which frames may fill result slots, how the query is read, and
/// what comes back besides the hits.
#[derive(Debug, Clone)]
pub struct SearchOptions<'w> {
    /// Hits to return; must be at least 1.
    pub top_k: usize,
    pub tau: Tau,
    /// Superseded frames may fill result slots.
    pub include_superseded: bool,
    /// Which frames may fill result slots, on the event and commit axes.
    pub time: TimeFilter,
    /// Presentation order of the selected hits (by event time, a frame
    /// without one at its commit time).
    pub order: TemporalOrder,
    /// Confine the search to this region: a frame id or a seed label.
    pub within: Option<String>,
    /// Warp applied to the query before the search.
    pub warp: Option<&'w Warp>,
    /// Record the visits the walk made.
    pub trace: bool,
    /// Per-call visit budget (at least 1); `None` = the store's tuning.
    pub max_visits: Option<usize>,
    /// Per-call beam width (at least 1); `None` = the store's tuning.
    pub beam: Option<usize>,
}

impl Default for SearchOptions<'_> {
    fn default() -> Self {
        SearchOptions {
            top_k: 10,
            tau: Tau::Value(0.5),
            include_superseded: false,
            time: TimeFilter::default(),
            order: TemporalOrder::Relevance,
            within: None,
            warp: None,
            trace: false,
            max_visits: None,
            beam: None,
        }
    }
}

impl SearchOptions<'_> {
    /// The top `top_k` hits at the threshold `tau`.
    pub fn top(top_k: usize, tau: f32) -> Self {
        SearchOptions { top_k, tau: Tau::Value(tau), ..Default::default() }
    }
}

/// One visit of a traced walk.
#[derive(Debug, Clone, PartialEq)]
pub struct TraceEntry {
    pub frame_id: String,
    pub depth: u32,
    pub score: f32,
    pub visit_order: u32,
    /// "phase1_eigen" | "child" | "sim_edge"
    pub source: String,
    /// The frame whose neighbours were expanded to reach this one; `None` for an entry point.
    pub parent_id: Option<String>,
}

/// What a search returns: the hits (each with its commit time and event
/// time), the visits it recorded when `trace` was set, and how many frames it
/// visited.
#[derive(Debug, Clone, Default)]
pub struct SearchOutcome {
    pub hits: Vec<Hit>,
    pub trace: Vec<TraceEntry>,
    pub visits: usize,
}

// ---------------------------------------------------------------- //
// Route, strand, history and neighborhood options
// ---------------------------------------------------------------- //

/// One end of a route.
#[derive(Debug, Clone, Copy)]
pub enum Endpoint<'a> {
    /// A frame, by id.
    Frame(&'a str),
    /// The frames that fit a query vector best (see [`EndpointFit`]).
    Query(&'a [f32]),
    /// The door the other end's query enters through. Valid only as the start
    /// of a route whose end is a `Query`.
    Entry,
}

/// Which frames stand for a query endpoint: its search results under this
/// `top_k` and `tau`, the best-fitting pair a route joins.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EndpointFit {
    pub top_k: usize,
    pub tau: f32,
}

/// How a route between two endpoints is found.
#[derive(Debug, Clone, Default)]
pub struct RouteOptions<'w> {
    /// Every step forward in record order (the sequence in which frames were
    /// committed): how the start led to the end.
    pub forward: bool,
    /// Superseded frames may be endpoints and steps.
    pub include_superseded: bool,
    /// Warp applied to query endpoints.
    pub warp: Option<&'w Warp>,
    /// Endpoint fit for query endpoints. `None` uses the form's rule: the
    /// nearest frame for a route, the best-fitting pair of the top 10 at tau
    /// 0.5 for a forward route.
    pub fit: Option<EndpointFit>,
    /// The record as it stood at this commit time (microseconds since the
    /// epoch): frames committed later are neither endpoints nor steps.
    pub as_of: Option<i64>,
}

/// How a provenance or influence strand is followed.
#[derive(Debug, Clone)]
pub struct StrandOptions<'w> {
    /// Where the walk stops. Default: the frame's own background (`QueryBackground`).
    pub tau: Tau,
    pub include_superseded: bool,
    /// Steers which strand is followed; a step may not move against it.
    pub warp: Option<&'w Warp>,
    /// The record as it stood at this commit time (microseconds since the
    /// epoch): frames committed later are not steps.
    pub as_of: Option<i64>,
}

impl Default for StrandOptions<'_> {
    fn default() -> Self {
        StrandOptions { tau: Tau::QueryBackground, include_superseded: false, warp: None, as_of: None }
    }
}

/// How a history read is taken.
#[derive(Debug, Clone)]
pub struct HistoryOptions<'w> {
    /// Frames to return; at least 1.
    pub top_k: usize,
    pub tau: Tau,
    /// How many of the search's top hits choose the trees read; at least 1.
    pub anchors: usize,
    pub warp: Option<&'w Warp>,
    /// Which frames may be returned, on the event and commit axes.
    pub time: TimeFilter,
}

impl Default for HistoryOptions<'_> {
    fn default() -> Self {
        HistoryOptions { top_k: 10, tau: Tau::Value(0.5), anchors: 3, warp: None, time: TimeFilter::default() }
    }
}

/// How a three-population neighborhood is read.
#[derive(Debug, Clone)]
pub struct NeighborhoodOptions<'w> {
    pub tau: Tau,
    /// Lineage reach; at least 1.
    pub hops: usize,
    /// Warp applied to the anchor for the geometric population only.
    pub warp: Option<&'w Warp>,
}

impl Default for NeighborhoodOptions<'_> {
    fn default() -> Self {
        NeighborhoodOptions { tau: Tau::Background, hops: 1, warp: None }
    }
}

fn warp_ptr(w: Option<&Warp>) -> *const c_void {
    w.map_or(ptr::null(), |w| w.ptr as *const c_void)
}

fn at_least_one(v: usize, what: &str) -> Result<usize, Error> {
    if v == 0 { Err(Error::InvalidArgument(format!("{what} must be at least 1"))) } else { Ok(v) }
}

// ---------------------------------------------------------------- //
// Results
// ---------------------------------------------------------------- //

/// How smooth a route is.
#[derive(Debug, Clone, PartialEq)]
pub struct ChainCoherence {
    pub mean_gradient: f32,
    pub min_gradient: f32,
    pub fragmentation_events: u32,
    pub pass_strict: bool,
    pub pass_loose: bool,
    /// Arc of the route over the direct arc between its ends; NaN when undefined.
    pub stretch: f32,
}

/// A route: its frames in order and how coherent it is.
#[derive(Debug, Clone, PartialEq)]
pub struct ChainResult {
    pub frame_ids: Vec<String>,
    pub primitive: String,
    /// False when the search exhausted its budget before the target.
    pub reached: bool,
    pub coherence: ChainCoherence,
}

/// One of the distinct ways two frames connect.
#[derive(Debug, Clone, PartialEq)]
pub struct Connection {
    pub chain: ChainResult,
    /// The interior frame closest to both ends at once; `None` for a direct edge.
    pub bridge: Option<String>,
}

/// One frame in a history read.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryHit {
    pub frame_id: String,
    pub score: f32,
    /// Commit time, microseconds since the epoch.
    pub timestamp: i64,
    /// When the content happened, where the frame records it.
    pub occurred_at: Option<i64>,
    /// Steps from the door the walk entered at.
    pub depth: u32,
    /// The walk's route from that door to this frame, door first.
    pub path: Vec<String>,
}

/// A history read: the best frames a tree walk reaches, oldest first.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryResult {
    pub hits: Vec<HistoryHit>,
    /// The search hits that chose the trees.
    pub anchors: Vec<String>,
    /// The doors the walk entered at.
    pub doors: Vec<String>,
    pub visits: usize,
}

/// A three-population neighborhood.
#[derive(Debug, Clone, PartialEq)]
pub struct NeighborhoodResult {
    pub consensus: Vec<String>,
    pub geometric: Vec<String>,
    pub lineage: Vec<String>,
    pub tau_used: f32,
    pub hops_used: u32,
    /// Cosine to the anchor for the consensus and geometric frames.
    pub scores: HashMap<String, f32>,
}

fn malformed(what: &str) -> Error {
    Error::Store(format!("malformed engine JSON: {what}"))
}

fn j_str(v: &serde_json::Value, k: &str) -> Result<String, Error> {
    v.get(k).and_then(|x| x.as_str()).map(str::to_string).ok_or_else(|| malformed(k))
}

fn j_opt_str(v: &serde_json::Value, k: &str) -> Option<String> {
    v.get(k).and_then(|x| x.as_str()).map(str::to_string)
}

fn j_f32(v: &serde_json::Value, k: &str) -> f32 {
    v.get(k).and_then(|x| x.as_f64()).map_or(f32::NAN, |f| f as f32)
}

fn j_u64(v: &serde_json::Value, k: &str) -> Result<u64, Error> {
    v.get(k).and_then(|x| x.as_u64()).ok_or_else(|| malformed(k))
}

fn j_bool(v: &serde_json::Value, k: &str) -> Result<bool, Error> {
    v.get(k).and_then(|x| x.as_bool()).ok_or_else(|| malformed(k))
}

fn j_strings(v: &serde_json::Value, k: &str) -> Result<Vec<String>, Error> {
    v.get(k).and_then(|x| x.as_array()).ok_or_else(|| malformed(k))?
        .iter().map(|s| s.as_str().map(str::to_string).ok_or_else(|| malformed(k))).collect()
}

impl ChainResult {
    fn from_json(v: &serde_json::Value) -> Result<Self, Error> {
        let c = v.get("coherence").ok_or_else(|| malformed("coherence"))?;
        Ok(ChainResult {
            frame_ids: j_strings(v, "frame_ids")?,
            primitive: j_str(v, "primitive")?,
            reached: j_bool(v, "reached")?,
            coherence: ChainCoherence {
                mean_gradient: j_f32(c, "mean_gradient"),
                min_gradient: j_f32(c, "min_gradient"),
                fragmentation_events: j_u64(c, "fragmentation_events")? as u32,
                pass_strict: j_bool(c, "pass_strict")?,
                pass_loose: j_bool(c, "pass_loose")?,
                stretch: j_f32(c, "stretch"),
            },
        })
    }
}

impl HistoryResult {
    fn from_json(v: &serde_json::Value) -> Result<Self, Error> {
        let hits = v.get("hits").and_then(|x| x.as_array()).ok_or_else(|| malformed("hits"))?
            .iter().map(|h| Ok(HistoryHit {
                frame_id: j_str(h, "frame_id")?,
                score: j_f32(h, "score"),
                timestamp: h.get("timestamp").and_then(|x| x.as_i64()).ok_or_else(|| malformed("timestamp"))?,
                occurred_at: h.get("occurred_at").and_then(|x| x.as_i64()),
                depth: j_u64(h, "depth")? as u32,
                path: j_strings(h, "path")?,
            }))
            .collect::<Result<_, Error>>()?;
        Ok(HistoryResult {
            hits,
            anchors: j_strings(v, "anchors")?,
            doors: j_strings(v, "doors")?,
            visits: j_u64(v, "visits")? as usize,
        })
    }
}

impl NeighborhoodResult {
    fn from_json(v: &serde_json::Value) -> Result<Self, Error> {
        let scores = v.get("scores").and_then(|x| x.as_object())
            .map(|m| m.iter().filter_map(|(k, s)| s.as_f64().map(|f| (k.clone(), f as f32))).collect())
            .unwrap_or_default();
        Ok(NeighborhoodResult {
            consensus: j_strings(v, "consensus")?,
            geometric: j_strings(v, "geometric")?,
            lineage: j_strings(v, "lineage")?,
            tau_used: j_f32(v, "tau_used"),
            hops_used: j_u64(v, "hops_used")? as u32,
            scores,
        })
    }
}

fn trace_from_json(v: &serde_json::Value) -> Result<Vec<TraceEntry>, Error> {
    v.as_array().ok_or_else(|| malformed("trace"))?.iter().map(|t| Ok(TraceEntry {
        frame_id: j_str(t, "frame_id")?,
        depth: j_u64(t, "depth")? as u32,
        score: j_f32(t, "score"),
        visit_order: j_u64(t, "visit_order")? as u32,
        source: j_str(t, "source")?,
        parent_id: j_opt_str(t, "parent_id"),
    })).collect()
}

// ---------------------------------------------------------------- //
// Runtime tuning
// ---------------------------------------------------------------- //

/// How a search walks the graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    /// The gradient-beam stream: best-first, stopped by the score ceiling.
    Stream,
    /// The breadth-first flood, the completeness reference.
    Flood,
}

/// How `search` fills its result slots.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RankMode {
    /// Every slot by raw cosine.
    Cosine,
    /// The last `round(share * top_k)` slots go to the best frames a confined
    /// descent reaches that the cosine list lacks.
    TwoChannel { share: f32, doors: usize, budget: usize },
}

/// How a committing frame chooses its similarity edges.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SimEdgePolicy {
    /// Allow edges to routing frames (Genesis, Seed).
    pub routing: bool,
    /// Allow an edge to the frame's own parent.
    pub parent: bool,
    /// `0` = top-k; `> 0` = occlusion pruning.
    pub occlusion: f32,
    /// Candidates the commit walk returns (`0` = `edge_budget + 1`).
    pub pool: usize,
}

/// Runtime settings of the read path.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SearchTuning {
    pub mode: SearchMode,
    /// Stream frontier nodes expanded per round; `0` = unbounded.
    pub beam: usize,
    /// Visits a search may make; `0` = `top_k * visit_cap_multiplier`.
    pub max_visits: usize,
    pub visit_cap_multiplier: u32,
    pub max_depth: u32,
    /// Stop-line slack (cosine); `0` uses `ceiling_margin`.
    pub ceiling_slack: f32,
    pub ceiling_margin: f32,
    pub entry_k: usize,
    pub entry_gap: f32,
    pub rank_mode: RankMode,
    pub door_index: bool,
    pub door_index_min: usize,
    pub door_index_ef: usize,
    pub eigen_matmul_min: usize,
    pub eigen_gpu_min: usize,
    pub adjacency: bool,
}

/// Runtime settings of placement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CommitTuning {
    pub ceiling_slack: f32,
    pub ceiling_margin: f32,
    /// Weight of the seed term in parent choice; `0` disables it.
    pub seed_term: f32,
    pub sim_edge_policy: SimEdgePolicy,
}

/// What an open handle may change at runtime. Never persisted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tuning {
    pub search: SearchTuning,
    pub commit: CommitTuning,
}

impl Tuning {
    /// The engine's default runtime settings.
    pub fn engine_default() -> Result<Tuning, Error> {
        let mut raw = Tuning::blank_ffi();
        check_status(unsafe { seraph_tuning_default(&mut raw) })?;
        Tuning::from_ffi(&raw)
    }

    fn blank_ffi() -> FfiTuning {
        // SAFETY: FfiTuning holds only integers and floats.
        let mut raw: FfiTuning = unsafe { std::mem::zeroed() };
        raw.struct_size = struct_size::<FfiTuning>();
        raw
    }

    fn from_ffi(c: &FfiTuning) -> Result<Tuning, Error> {
        let mode = match c.search_mode {
            0 => SearchMode::Stream,
            1 => SearchMode::Flood,
            m => return Err(malformed(&format!("search_mode {m}"))),
        };
        let rank_mode = match c.rank_mode {
            0 => RankMode::Cosine,
            1 => RankMode::TwoChannel { share: c.rank_share, doors: c.rank_doors, budget: c.rank_budget },
            m => return Err(malformed(&format!("rank_mode {m}"))),
        };
        Ok(Tuning {
            search: SearchTuning {
                mode,
                beam: c.beam,
                max_visits: c.max_visits,
                visit_cap_multiplier: c.visit_cap_multiplier,
                max_depth: c.max_depth,
                ceiling_slack: c.search_ceiling_slack,
                ceiling_margin: c.search_ceiling_margin,
                entry_k: c.entry_k,
                entry_gap: c.entry_gap,
                rank_mode,
                door_index: c.flags & TUNING_DOOR_INDEX != 0,
                door_index_min: c.door_index_min,
                door_index_ef: c.door_index_ef,
                eigen_matmul_min: c.eigen_matmul_min,
                eigen_gpu_min: c.eigen_gpu_min,
                adjacency: c.flags & TUNING_ADJACENCY != 0,
            },
            commit: CommitTuning {
                ceiling_slack: c.commit_ceiling_slack,
                ceiling_margin: c.commit_ceiling_margin,
                seed_term: c.seed_term,
                sim_edge_policy: SimEdgePolicy {
                    routing: c.flags & TUNING_SIM_EDGE_ROUTING != 0,
                    parent: c.flags & TUNING_SIM_EDGE_PARENT != 0,
                    occlusion: c.sim_edge_occlusion,
                    pool: c.sim_edge_pool,
                },
            },
        })
    }

    fn to_ffi(&self) -> FfiTuning {
        let s = &self.search;
        let c = &self.commit;
        let mut flags = 0;
        if s.door_index { flags |= TUNING_DOOR_INDEX; }
        if s.adjacency { flags |= TUNING_ADJACENCY; }
        if c.sim_edge_policy.routing { flags |= TUNING_SIM_EDGE_ROUTING; }
        if c.sim_edge_policy.parent { flags |= TUNING_SIM_EDGE_PARENT; }
        let (rank_mode, rank_share, rank_doors, rank_budget) = match s.rank_mode {
            RankMode::Cosine => (0, 0.0, 0, 0),
            RankMode::TwoChannel { share, doors, budget } => (1, share, doors, budget),
        };
        FfiTuning {
            struct_size: struct_size::<FfiTuning>(),
            flags,
            search_mode: match s.mode { SearchMode::Stream => 0, SearchMode::Flood => 1 },
            rank_mode,
            beam: s.beam,
            max_visits: s.max_visits,
            visit_cap_multiplier: s.visit_cap_multiplier,
            max_depth: s.max_depth,
            search_ceiling_slack: s.ceiling_slack,
            search_ceiling_margin: s.ceiling_margin,
            entry_k: s.entry_k,
            entry_gap: s.entry_gap,
            rank_share,
            rank_doors,
            rank_budget,
            door_index_min: s.door_index_min,
            door_index_ef: s.door_index_ef,
            eigen_matmul_min: s.eigen_matmul_min,
            eigen_gpu_min: s.eigen_gpu_min,
            commit_ceiling_slack: c.ceiling_slack,
            commit_ceiling_margin: c.ceiling_margin,
            seed_term: c.seed_term,
            sim_edge_occlusion: c.sim_edge_policy.occlusion,
            sim_edge_pool: c.sim_edge_policy.pool,
        }
    }
}

impl Default for Tuning {
    /// The engine's default runtime settings (see [`Tuning::engine_default`]).
    fn default() -> Self {
        Tuning::engine_default().expect("seraph_tuning_default cannot fail on a well-formed struct")
    }
}

// ---------------------------------------------------------------- //
// Open and migrate options
// ---------------------------------------------------------------- //

/// How a store is opened.
#[derive(Clone, Default)]
pub struct OpenOptions {
    /// Open without writer state.
    pub read_only: bool,
    /// Let this license write into a store created by another customer.
    pub allow_cross_customer_write: bool,
    /// 32-byte Ed25519 writer seed for frames appended in this session.
    pub writer_seed: Option<[u8; 32]>,
    /// Passphrase of an encrypted store.
    pub passphrase: Option<String>,
    /// Runtime tuning to open with; `None` = the engine's defaults.
    pub tuning: Option<Tuning>,
}

impl fmt::Debug for OpenOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenOptions")
            .field("read_only", &self.read_only)
            .field("allow_cross_customer_write", &self.allow_cross_customer_write)
            .field("writer_seed", &self.writer_seed.map(|_| "<redacted>"))
            .field("passphrase", &self.passphrase.as_ref().map(|_| "<redacted>"))
            .field("tuning", &self.tuning)
            .finish()
    }
}

/// How a store is rebuilt by [`migrate_store_with`].
#[derive(Debug, Clone)]
pub struct MigrateOptions {
    /// Destination configuration over the defaults, keeping the source's
    /// model unless it names one; `None` inherits the source's configuration
    /// at the current format.
    pub config: Option<StoreConfig>,
    /// Seeds created on the destination before the first frame: (label, embedding, metadata).
    pub seeds: Vec<(String, Vec<f32>, serde_json::Map<String, serde_json::Value>)>,
    /// `false` takes raw-cosine parents for the whole migration.
    pub seed_term: bool,
}

impl Default for MigrateOptions {
    fn default() -> Self {
        MigrateOptions { config: None, seeds: Vec::new(), seed_term: true }
    }
}

// ---------------------------------------------------------------- //
// Warp
// ---------------------------------------------------------------- //

/// One target or suppression of a warp.
#[derive(Debug, Clone, PartialEq)]
pub enum WarpInput {
    /// A frame's embedding, by id.
    Frame(String),
    /// A raw vector.
    Embedding(Vec<f32>),
}

/// A warp magnitude: a value or a name resolved from the store's calibration.
#[derive(Debug, Clone, PartialEq)]
pub enum Magnitude {
    Value(f32),
    Named(String),
}

/// A warp's operating regime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarpProfile {
    Routing,
    Saturation,
    Bounded,
}

/// How repeated warp applications combine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accumulation {
    Reset,
    Cumulative,
}

/// A warp before compilation: targets pull the query toward directions,
/// suppressions push it away.
#[derive(Debug, Clone, PartialEq)]
pub struct WarpSpec {
    pub targets: Vec<(WarpInput, Magnitude)>,
    pub suppress: Vec<(WarpInput, Magnitude)>,
    pub profile: WarpProfile,
    pub bound_k: f32,
    pub accumulation: Accumulation,
    /// Named-magnitude values that take priority over the store's calibration.
    pub overrides: HashMap<String, f32>,
}

impl Default for WarpSpec {
    fn default() -> Self {
        WarpSpec {
            targets: Vec::new(),
            suppress: Vec::new(),
            profile: WarpProfile::Bounded,
            bound_k: 1.5,
            accumulation: Accumulation::Reset,
            overrides: HashMap::new(),
        }
    }
}

impl WarpSpec {
    /// The JSON form `seraph_warp_compile` takes.
    pub fn to_json(&self) -> String {
        let side = |v: &[(WarpInput, Magnitude)]| -> serde_json::Value {
            serde_json::Value::Array(v.iter().map(|(i, m)| {
                let input = match i {
                    WarpInput::Frame(id) => serde_json::Value::from(id.as_str()),
                    WarpInput::Embedding(e) => serde_json::Value::from(e.clone()),
                };
                let magnitude = match m {
                    Magnitude::Value(x) => serde_json::Value::from(*x),
                    Magnitude::Named(n) => serde_json::Value::from(n.as_str()),
                };
                serde_json::json!({ "input": input, "magnitude": magnitude })
            }).collect())
        };
        let profile = match self.profile {
            WarpProfile::Routing => "routing",
            WarpProfile::Saturation => "saturation",
            WarpProfile::Bounded => "bounded",
        };
        let accumulation = match self.accumulation {
            Accumulation::Reset => "reset",
            Accumulation::Cumulative => "cumulative",
        };
        serde_json::json!({
            "targets": side(&self.targets),
            "suppress": side(&self.suppress),
            "profile": profile,
            "bound_k": self.bound_k,
            "accumulation": accumulation,
            "magnitude_overrides": self.overrides,
        }).to_string()
    }
}

/// A compiled warp: every target and suppression resolved to a vector and a
/// magnitude once, for use in any number of reads.
pub struct Warp {
    ptr: *mut c_void,
}

// SAFETY: a compiled warp is immutable after compilation.
unsafe impl Send for Warp {}
unsafe impl Sync for Warp {}

impl fmt::Debug for Warp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Warp")
    }
}

impl Drop for Warp {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe { seraph_warp_free(self.ptr) };
            self.ptr = ptr::null_mut();
        }
    }
}

impl Warp {
    fn compile_raw(store: *const c_void, spec_json: &str) -> Result<Warp, Error> {
        let spec_c = c_arg(spec_json, "spec_json")?;
        let mut out: *mut c_void = ptr::null_mut();
        check_status(unsafe { seraph_warp_compile(store, spec_c.as_ptr(), &mut out) })?;
        Ok(Warp { ptr: out })
    }

    /// Compile a spec that names no frames and whose named magnitudes all
    /// appear in its overrides, without a store.
    pub fn compile_standalone(spec: &WarpSpec) -> Result<Warp, Error> {
        Warp::compile_raw(ptr::null(), &spec.to_json())
    }
}

// ---------------------------------------------------------------- //
// Store: canonical operations
// ---------------------------------------------------------------- //

impl Store {
    /// Open an existing store under `options`: a writer identity, a
    /// passphrase, read-only or cross-customer access, and the tuning to start
    /// with, in any combination.
    pub fn open_with(path: impl AsRef<Path>, options: &OpenOptions) -> Result<Self, Error> {
        let path_c = c_arg(&path.as_ref().to_string_lossy(), "path")?;
        let pass = opt_c_arg(options.passphrase.as_deref(), "passphrase")?;
        let tuning = options.tuning.as_ref().map(Tuning::to_ffi);
        let mut flags = 0;
        if options.read_only { flags |= OPEN_READ_ONLY; }
        if options.allow_cross_customer_write { flags |= OPEN_ALLOW_CROSS_CUSTOMER_WRITE; }
        let raw = FfiOpenOptions {
            struct_size: struct_size::<FfiOpenOptions>(),
            flags,
            writer_seed: options.writer_seed.as_ref().map_or(ptr::null(), |s| s.as_ptr()),
            passphrase: c_ptr(&pass),
            tuning: tuning.as_ref().map_or(ptr::null(), |t| t as *const FfiTuning),
        };
        let mut handle: *mut c_void = ptr::null_mut();
        check_status(unsafe { seraph_open(path_c.as_ptr(), &raw, &mut handle) })?;
        Ok(Store { handle })
    }

    /// Ingest one frame under `options`: metadata, event time, a region, the
    /// seed term, promotion and a Gov marking, in any combination.
    pub fn put_with(&mut self, content: &[u8], embedding: &[f32], options: &PutOptions) -> Result<String, Error> {
        let o = options.to_ffi()?;
        let mut id: *mut c_char = ptr::null_mut();
        check_status(unsafe {
            seraph_put(self.handle, content.as_ptr(), content.len(), embedding.as_ptr(), embedding.len(), &o.raw, &mut id)
        })?;
        Ok(unsafe { take_cstring(id) })
    }

    /// Ingest a batch. Each item's parent is chosen against the live store, so
    /// an item may attach to an earlier item of the same batch, exactly as the
    /// same [`put_with`](Self::put_with) calls would; the writes are buffered
    /// and the store is left buffered (call [`sync`](Self::sync) when the load
    /// is done). Every item is checked before the first is committed; a batch
    /// that stops partway fails with [`Error::BatchIncomplete`] carrying the
    /// ids already committed.
    pub fn put_batch(&mut self, items: &[PutItem]) -> Result<Vec<String>, Error> {
        let dim = items.first().map_or(0, |i| i.embedding.len());
        if let Some((i, it)) = items.iter().enumerate().find(|(_, it)| it.embedding.len() != dim) {
            return Err(Error::Dimension(format!(
                "items[{i}].embedding has dimension {}, items[0]'s is {dim}", it.embedding.len())));
        }
        let opts: Vec<PutOptionsC> = items.iter().map(|i| i.options.to_ffi()).collect::<Result<_, _>>()?;
        let raw: Vec<FfiPutItem> = items.iter().zip(&opts).map(|(i, o)| FfiPutItem {
            content: i.content.as_ptr(),
            content_len: i.content.len(),
            embedding: i.embedding.as_ptr(),
            options: o.raw,
        }).collect();
        let mut out: *mut c_char = ptr::null_mut();
        let rc = unsafe { seraph_put_batch(self.handle, raw.as_ptr(), raw.len(), dim, &mut out) };
        let ids = |p: *mut c_char| -> Vec<String> {
            if p.is_null() { return Vec::new(); }
            serde_json::from_str(&unsafe { take_cstring(p) }).unwrap_or_default()
        };
        match rc {
            SERAPH_OK => Ok(ids(out)),
            SERAPH_E_BATCH_INCOMPLETE => {
                let message = last_error_message();
                Err(Error::BatchIncomplete { committed: ids(out), message })
            }
            code => {
                let e = status_error(code);
                if !out.is_null() { unsafe { seraph_string_free(out) }; }
                Err(e)
            }
        }
    }

    /// Search under `options`: how many hits, at what threshold, from which
    /// frames (superseded, the event and commit windows, a region), with what
    /// warp, in what order, and whether to record the walk. Every option
    /// composes with every other. Each hit carries its commit time and its
    /// claimed event time.
    pub fn search_with(&self, query: &[f32], options: &SearchOptions) -> Result<SearchOutcome, Error> {
        let top_k = at_least_one(options.top_k, "top_k")?;
        let max_visits = options.max_visits.map(|v| at_least_one(v, "max_visits")).transpose()?.unwrap_or(0);
        let beam = options.beam.map(|v| at_least_one(v, "beam")).transpose()?.unwrap_or(0);
        let within = opt_c_arg(options.within.as_deref(), "within")?;
        let (time_flags, bounds, basis) = options.time.to_ffi();
        let mut flags = time_flags;
        if options.include_superseded { flags |= SEARCH_INCLUDE_SUPERSEDED; }
        if options.trace { flags |= SEARCH_TRACE; }
        let raw = FfiSearchOptions {
            struct_size: struct_size::<FfiSearchOptions>(),
            flags,
            top_k,
            tau: options.tau.to_ffi(),
            occurred_after_us: bounds[0],
            occurred_before_us: bounds[1],
            committed_after_us: bounds[2],
            committed_before_us: bounds[3],
            basis,
            order: options.order.to_code(),
            within: c_ptr(&within),
            warp: warp_ptr(options.warp),
            max_visits,
            beam,
        };
        let mut hits: *mut FfiSearchHit = ptr::null_mut();
        let mut count = 0usize;
        let mut trace: *mut c_char = ptr::null_mut();
        let mut visits = 0usize;
        check_status(unsafe {
            seraph_search(self.handle, query.as_ptr(), query.len(), &raw, &mut hits, &mut count, &mut trace, &mut visits)
        })?;
        let hits = take_search_hits(hits, count);
        let trace = trace_from_json(&take_json(trace)?)?;
        Ok(SearchOutcome { hits, trace, visits })
    }

    /// The route between two endpoints under `options`. A frame endpoint is
    /// that frame; a query endpoint is the frames that fit the query best,
    /// warped by `options.warp`; [`Endpoint::Entry`] as the start is the door
    /// the end query enters through. With `forward`, every step is forward in
    /// record order: how the start led to the end. With `as_of`, the route
    /// runs over the record as it stood then. A frame that does not exist is
    /// [`Error::NotFound`].
    pub fn chain_with(&self, from: Endpoint, to: Endpoint, options: &RouteOptions) -> Result<ChainResult, Error> {
        let (a, _ka) = endpoint_ffi(from)?;
        let (b, _kb) = endpoint_ffi(to)?;
        let raw = route_options_ffi(options);
        let mut out: *mut c_char = ptr::null_mut();
        check_status(unsafe { seraph_chain(self.handle, &a, &b, &raw, &mut out) })?;
        ChainResult::from_json(&take_json(out)?)
    }

    /// The distinct ways two frames connect under `options`: successive
    /// geodesics with disjoint interiors, up to `max_routes` (at least 1),
    /// stopping when a further route fails strict coherence. With `forward`,
    /// the distinct ways `from` led to `to` in record order; with `as_of`,
    /// over the record as it stood then. Both endpoints must be frames.
    pub fn connections_with(
        &self, from: Endpoint, to: Endpoint, options: &RouteOptions, max_routes: usize,
    ) -> Result<Vec<Connection>, Error> {
        let max_routes = at_least_one(max_routes, "max_routes")?;
        let (a, _ka) = endpoint_ffi(from)?;
        let (b, _kb) = endpoint_ffi(to)?;
        let raw = route_options_ffi(options);
        let mut out: *mut c_char = ptr::null_mut();
        check_status(unsafe { seraph_connections(self.handle, &a, &b, &raw, max_routes, &mut out) })?;
        take_json(out)?.as_array().ok_or_else(|| malformed("connections"))?.iter().map(|c| Ok(Connection {
            chain: ChainResult::from_json(c.get("chain").ok_or_else(|| malformed("chain"))?)?,
            bridge: j_opt_str(c, "bridge"),
        })).collect()
    }

    /// Provenance under `options`: the strand of the past `frame_id` was built
    /// on, a walk back in record order over its committed edges and parent.
    /// With `as_of`, over the record as it stood then.
    pub fn provenance_with(&self, frame_id: &str, options: &StrandOptions) -> Result<ChainResult, Error> {
        self.strand(frame_id, options, false)
    }

    /// Influence under `options`: the strand of later work built next to
    /// `frame_id`, a walk forward in record order over the frames that chose
    /// it. With `as_of`, over the record as it stood then.
    pub fn influence_with(&self, frame_id: &str, options: &StrandOptions) -> Result<ChainResult, Error> {
        self.strand(frame_id, options, true)
    }

    fn strand(&self, frame_id: &str, options: &StrandOptions, forward: bool) -> Result<ChainResult, Error> {
        let fid = c_arg(frame_id, "frame_id")?;
        let mut flags = 0;
        if options.include_superseded { flags |= STRAND_INCLUDE_SUPERSEDED; }
        if options.as_of.is_some() { flags |= STRAND_HAS_AS_OF; }
        let raw = FfiStrandOptions {
            struct_size: struct_size::<FfiStrandOptions>(),
            flags,
            tau: options.tau.to_ffi(),
            warp: warp_ptr(options.warp),
            as_of_us: options.as_of.unwrap_or(0),
        };
        let mut out: *mut c_char = ptr::null_mut();
        let rc = unsafe {
            if forward { seraph_influence(self.handle, fid.as_ptr(), &raw, &mut out) }
            else { seraph_provenance(self.handle, fid.as_ptr(), &raw, &mut out) }
        };
        check_status(rc)?;
        ChainResult::from_json(&take_json(out)?)
    }

    /// History read under `options`: search locates the answer, its top
    /// `anchors` hits choose the trees, and a walk over the parent tree
    /// returns the best `top_k` frames oldest first, each with its route and
    /// both of its times. The time filter decides which frames may be returned.
    pub fn history_with(&self, query: &[f32], options: &HistoryOptions) -> Result<HistoryResult, Error> {
        let (flags, bounds, basis) = options.time.to_ffi();
        let raw = FfiHistoryOptions {
            struct_size: struct_size::<FfiHistoryOptions>(),
            flags,
            top_k: at_least_one(options.top_k, "top_k")?,
            tau: options.tau.to_ffi(),
            anchors: at_least_one(options.anchors, "anchors")?,
            warp: warp_ptr(options.warp),
            occurred_after_us: bounds[0],
            occurred_before_us: bounds[1],
            committed_after_us: bounds[2],
            committed_before_us: bounds[3],
            basis,
            reserved: 0,
        };
        let mut out: *mut c_char = ptr::null_mut();
        check_status(unsafe { seraph_history(self.handle, query.as_ptr(), query.len(), &raw, &mut out) })?;
        HistoryResult::from_json(&take_json(out)?)
    }

    /// Three-population neighborhood around a frame under `options`.
    pub fn neighborhood_with(&self, frame_id: &str, options: &NeighborhoodOptions) -> Result<NeighborhoodResult, Error> {
        let fid = c_arg(frame_id, "frame_id")?;
        let raw = FfiNeighborhoodOptions {
            struct_size: struct_size::<FfiNeighborhoodOptions>(),
            flags: 0,
            tau: options.tau.to_ffi(),
            hops: at_least_one(options.hops, "hops")?,
            warp: warp_ptr(options.warp),
        };
        let mut out: *mut c_char = ptr::null_mut();
        check_status(unsafe { seraph_neighborhood(self.handle, fid.as_ptr(), &raw, &mut out) })?;
        NeighborhoodResult::from_json(&take_json(out)?)
    }

    /// The runtime settings this handle runs with.
    pub fn tuning(&self) -> Result<Tuning, Error> {
        let mut raw = Tuning::blank_ffi();
        check_status(unsafe { seraph_get_tuning(self.handle, &mut raw) })?;
        Tuning::from_ffi(&raw)
    }

    /// Replace the runtime settings. Takes effect on the next read or commit;
    /// never persisted.
    pub fn set_tuning(&mut self, tuning: &Tuning) -> Result<(), Error> {
        let raw = tuning.to_ffi();
        check_status(unsafe { seraph_set_tuning(self.handle, &raw) })
    }

    /// Compile a warp spec (the JSON documented on the engine's
    /// `seraph_warp_compile`) against this store: frame ids and named
    /// magnitudes resolve once, here.
    pub fn compile_warp(&self, spec_json: &str) -> Result<Warp, Error> {
        Warp::compile_raw(self.handle, spec_json)
    }

    /// [`compile_warp`](Self::compile_warp) from a typed spec.
    pub fn compile_warp_spec(&self, spec: &WarpSpec) -> Result<Warp, Error> {
        Warp::compile_raw(self.handle, &spec.to_json())
    }
}

fn endpoint_ffi(e: Endpoint) -> Result<(FfiEndpoint, Option<CString>), Error> {
    Ok(match e {
        Endpoint::Frame(id) => {
            let c = c_arg(id, "frame_id")?;
            (FfiEndpoint { kind: ENDPOINT_FRAME, frame_id: c.as_ptr(), query: ptr::null(), query_len: 0 }, Some(c))
        }
        Endpoint::Query(q) => (
            FfiEndpoint { kind: ENDPOINT_QUERY, frame_id: ptr::null(), query: q.as_ptr(), query_len: q.len() },
            None,
        ),
        Endpoint::Entry => (
            FfiEndpoint { kind: ENDPOINT_ENTRY, frame_id: ptr::null(), query: ptr::null(), query_len: 0 },
            None,
        ),
    })
}

fn route_options_ffi(o: &RouteOptions) -> FfiRouteOptions {
    let mut flags = 0;
    if o.forward { flags |= ROUTE_FORWARD; }
    if o.include_superseded { flags |= ROUTE_INCLUDE_SUPERSEDED; }
    if o.fit.is_some() { flags |= ROUTE_HAS_FIT; }
    if o.as_of.is_some() { flags |= ROUTE_HAS_AS_OF; }
    FfiRouteOptions {
        struct_size: struct_size::<FfiRouteOptions>(),
        flags,
        warp: warp_ptr(o.warp),
        fit_top_k: o.fit.map_or(0, |f| f.top_k),
        fit_tau: o.fit.map_or(0.0, |f| f.tau),
        reserved: 0,
        as_of_us: o.as_of.unwrap_or(0),
    }
}

/// Rebuild the store at `src_path` into a fresh store at `dst_path` under
/// `options`: frames keep their identity and record order, and each takes its
/// parent against the live destination (see [`migrate_store`]).
pub fn migrate_store_with(
    src_path: impl AsRef<Path>,
    dst_path: impl AsRef<Path>,
    options: &MigrateOptions,
) -> Result<MigrateReport, Error> {
    let src_c = c_arg(&src_path.as_ref().to_string_lossy(), "src_path")?;
    let dst_c = c_arg(&dst_path.as_ref().to_string_lossy(), "dst_path")?;
    let config = options.config.as_ref().map(|c| c_arg(&c.to_json_object().to_string(), "config")).transpose()?;
    let seeds = if options.seeds.is_empty() { None } else {
        let arr: Vec<serde_json::Value> = options.seeds.iter().map(|(label, emb, meta)| serde_json::json!({
            "label": label, "embedding": emb, "metadata": meta,
        })).collect();
        Some(c_arg(&serde_json::Value::Array(arr).to_string(), "seeds")?)
    };
    let raw = FfiMigrateOptions {
        struct_size: struct_size::<FfiMigrateOptions>(),
        flags: if options.seed_term { 0 } else { MIGRATE_NO_SEED_TERM },
        config_json: c_ptr(&config),
        seeds_json: c_ptr(&seeds),
    };
    let mut out: *mut c_char = ptr::null_mut();
    check_status(unsafe { seraph_migrate(src_c.as_ptr(), dst_c.as_ptr(), &raw, &mut out) })?;
    Ok(MigrateReport::from_json(&take_json(out)?))
}


// ---------------------------------------------------------------- //
// Lifecycle
// ---------------------------------------------------------------- //

/// A record event: its position in record order (the sequence in which frames
/// were committed) and the commit time there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordEvent {
    pub position: u32,
    pub committed_at: i64,
}

/// How a frame became a door.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoorCause {
    Genesis,
    Seed,
    /// Its in-degree reached the eigenframe threshold.
    Threshold,
    /// It was promoted by hand.
    Manual,
}

/// How a frame was retired from the active record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Retirement {
    /// Soft-forgotten (`supersede`).
    Superseded,
    /// Consolidated into the frame named.
    Consolidated { into: String },
}

/// The record's account of one frame (see [`Store::lifecycle`]).
#[derive(Debug, Clone, PartialEq)]
pub struct FrameLifecycle {
    pub frame_id: String,
    /// When it was committed.
    pub committed: RecordEvent,
    /// When the content happened, as claimed at ingest.
    pub occurred_at: Option<i64>,
    /// When and how it became a door.
    pub door: Option<(RecordEvent, DoorCause)>,
    /// When and how it was retired.
    pub retired: Option<(RecordEvent, Retirement)>,
}

impl FrameLifecycle {
    fn from_json(v: &serde_json::Value) -> Result<Self, Error> {
        let event = |e: &serde_json::Value| -> Result<RecordEvent, Error> {
            Ok(RecordEvent {
                position: j_u64(e, "position")? as u32,
                committed_at: e.get("committed_at").and_then(|x| x.as_i64()).ok_or_else(|| malformed("committed_at"))?,
            })
        };
        let door = match v.get("door") {
            Some(d) if !d.is_null() => {
                let cause = match j_str(d, "cause")?.as_str() {
                    "genesis" => DoorCause::Genesis,
                    "seed" => DoorCause::Seed,
                    "threshold" => DoorCause::Threshold,
                    "manual" => DoorCause::Manual,
                    other => return Err(malformed(&format!("door cause {other}"))),
                };
                Some((event(d)?, cause))
            }
            _ => None,
        };
        let retired = match v.get("retired") {
            Some(r) if !r.is_null() => {
                let how = match j_str(r, "how")?.as_str() {
                    "superseded" => Retirement::Superseded,
                    "consolidated" => Retirement::Consolidated { into: j_str(r, "into")? },
                    other => return Err(malformed(&format!("retirement {other}"))),
                };
                Some((event(r)?, how))
            }
            _ => None,
        };
        Ok(FrameLifecycle {
            frame_id: j_str(v, "frame_id")?,
            committed: event(v.get("committed").ok_or_else(|| malformed("committed"))?)?,
            occurred_at: v.get("occurred_at").and_then(|x| x.as_i64()),
            door,
            retired,
        })
    }
}

impl Store {
    /// The record's account of one frame: when it was committed (with the
    /// event time it claims), when it became a door and how, and when it was
    /// retired and how, each with its position in record order and commit
    /// time. `None` when the frame does not exist.
    pub fn lifecycle(&self, frame_id: &str) -> Result<Option<FrameLifecycle>, Error> {
        let fid = c_arg(frame_id, "frame_id")?;
        let mut out: *mut c_char = ptr::null_mut();
        match unsafe { seraph_lifecycle(self.handle, fid.as_ptr(), &mut out) } {
            SERAPH_OK => Ok(Some(FrameLifecycle::from_json(&take_json(out)?)?)),
            SERAPH_NOT_FOUND => Ok(None),
            code => Err(status_error(code)),
        }
    }
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
