//! Shared pending probe tracking.
//!
//! This module provides a shared map of pending probes that both the engine
//! and receiver can access. The engine inserts entries before sending probes,
//! and the receiver removes them when responses arrive.

use parking_lot::RwLock;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Instant;

use crate::state::ProbeId;

/// A probe that has been sent and is awaiting a response
#[derive(Debug, Clone)]
pub struct PendingProbe {
    pub sent_at: Instant,
    pub target: IpAddr,
    /// Flow ID for Paris/Dublin traceroute ECMP detection (0 for single-flow mode)
    pub flow_id: u8,
    /// Original source port for NAT detection (UDP/TCP only, None for ICMP)
    pub original_src_port: Option<u16>,
    /// Packet size for PMTUD correlation (only set during PMTUD phase)
    pub packet_size: Option<u16>,
}

/// Key for pending probe lookup: (ProbeId, flow_id, target, is_pmtud)
///
/// Flow ID is included in the key because multi-flow mode sends the same ProbeId
/// for each flow per tick. Without flow_id in the key, entries would overwrite
/// each other, causing incorrect flow attribution.
///
/// Target is included to support multiple simultaneous targets - each target
/// has independent probe sequences.
///
/// is_pmtud distinguishes PMTUD probes from normal probes, preventing collision
/// when both use the same ProbeId (e.g., when dest discovered at tick N and
/// PMTUD seq wraps to N).
pub type PendingKey = (ProbeId, u8, IpAddr, bool);

/// Thread-safe map of pending probes keyed by (ProbeId, flow_id, target, is_pmtud)
pub type PendingMap = Arc<RwLock<HashMap<PendingKey, PendingProbe>>>;

/// Create a new empty pending map
pub fn new_pending_map() -> PendingMap {
    Arc::new(RwLock::new(HashMap::new()))
}
