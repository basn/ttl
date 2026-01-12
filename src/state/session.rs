use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::net::IpAddr;
use std::time::Duration;

use crate::config::Config;

/// Identifies a specific probe for correlation
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub struct ProbeId {
    pub ttl: u8,
    pub seq: u8,
}

impl ProbeId {
    pub fn new(ttl: u8, seq: u8) -> Self {
        Self { ttl, seq }
    }

    /// Encode TTL and sequence into a 16-bit value for ICMP sequence field
    pub fn to_sequence(&self) -> u16 {
        ((self.ttl as u16) << 8) | (self.seq as u16)
    }

    /// Decode from a 16-bit ICMP sequence field
    pub fn from_sequence(seq: u16) -> Self {
        Self {
            ttl: (seq >> 8) as u8,
            seq: (seq & 0xFF) as u8,
        }
    }
}

/// ICMP response type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IcmpResponseType {
    EchoReply,
    TimeExceeded,
    DestUnreachable(u8),
}

/// Result of a single probe
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ProbeResult {
    pub id: ProbeId,
    pub rtt: Option<Duration>,
    pub responder: Option<IpAddr>,
    pub icmp_type: Option<IcmpResponseType>,
}

/// ASN information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsnInfo {
    pub number: u32,
    pub name: String,
    pub prefix: Option<String>,
}

/// Geolocation information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoInfo {
    pub city: Option<String>,
    pub region: Option<String>,
    pub country: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

/// Stats for a single responder at a given TTL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponderStats {
    pub ip: IpAddr,
    pub hostname: Option<String>,
    pub asn: Option<AsnInfo>,
    pub geo: Option<GeoInfo>,

    // Counters
    pub sent: u64,
    pub received: u64,

    // Latency stats (Welford's online algorithm)
    #[serde(with = "duration_serde")]
    pub min_rtt: Duration,
    #[serde(with = "duration_serde")]
    pub max_rtt: Duration,
    pub mean_rtt: f64, // microseconds
    pub m2: f64,       // for stddev calculation

    // Jitter (RFC 3550)
    pub jitter: f64, // microseconds
    #[serde(skip)]
    pub last_rtt: Option<Duration>,

    // Rolling window for sparkline
    #[serde(skip)]
    pub recent: VecDeque<Option<Duration>>,
}

impl ResponderStats {
    pub fn new(ip: IpAddr) -> Self {
        Self {
            ip,
            hostname: None,
            asn: None,
            geo: None,
            sent: 0,
            received: 0,
            min_rtt: Duration::MAX,
            max_rtt: Duration::ZERO,
            mean_rtt: 0.0,
            m2: 0.0,
            jitter: 0.0,
            last_rtt: None,
            recent: VecDeque::with_capacity(60),
        }
    }

    /// Update stats with a new RTT sample
    pub fn record_response(&mut self, rtt: Duration) {
        self.received += 1;

        let rtt_micros = rtt.as_micros() as f64;

        // Update min/max
        if rtt < self.min_rtt {
            self.min_rtt = rtt;
        }
        if rtt > self.max_rtt {
            self.max_rtt = rtt;
        }

        // Welford's online algorithm for mean and variance
        let delta = rtt_micros - self.mean_rtt;
        self.mean_rtt += delta / self.received as f64;
        let delta2 = rtt_micros - self.mean_rtt;
        self.m2 += delta * delta2;

        // RFC 3550 jitter calculation
        if let Some(last) = self.last_rtt {
            let diff = (rtt_micros - last.as_micros() as f64).abs();
            self.jitter += (diff - self.jitter) / 16.0;
        }
        self.last_rtt = Some(rtt);

        // Rolling window
        self.recent.push_back(Some(rtt));
        if self.recent.len() > 60 {
            self.recent.pop_front();
        }
    }

    /// Record a timeout (no response)
    pub fn record_timeout(&mut self) {
        self.recent.push_back(None);
        if self.recent.len() > 60 {
            self.recent.pop_front();
        }
    }

    /// Loss percentage
    pub fn loss_pct(&self) -> f64 {
        if self.sent == 0 {
            0.0
        } else {
            (1.0 - (self.received as f64 / self.sent as f64)) * 100.0
        }
    }

    /// Average RTT
    pub fn avg_rtt(&self) -> Duration {
        Duration::from_micros(self.mean_rtt as u64)
    }

    /// Standard deviation
    pub fn stddev(&self) -> Duration {
        if self.received < 2 {
            return Duration::ZERO;
        }
        let variance = self.m2 / self.received as f64;
        Duration::from_micros(variance.sqrt() as u64)
    }

    /// Jitter
    pub fn jitter(&self) -> Duration {
        Duration::from_micros(self.jitter as u64)
    }
}

/// A single hop (TTL level) in the path
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hop {
    pub ttl: u8,
    pub sent: u64,
    pub received: u64,
    pub responders: HashMap<IpAddr, ResponderStats>,
    pub primary: Option<IpAddr>, // most frequently seen responder
}

impl Hop {
    pub fn new(ttl: u8) -> Self {
        Self {
            ttl,
            sent: 0,
            received: 0,
            responders: HashMap::new(),
            primary: None,
        }
    }

    /// Record a probe was sent for this TTL
    pub fn record_sent(&mut self) {
        self.sent += 1;
    }

    /// Record a response from a responder
    pub fn record_response(&mut self, ip: IpAddr, rtt: Duration) {
        self.received += 1;

        let stats = self
            .responders
            .entry(ip)
            .or_insert_with(|| ResponderStats::new(ip));
        stats.sent = self.sent; // sync sent count
        stats.record_response(rtt);

        self.update_primary();
    }

    /// Record a timeout
    pub fn record_timeout(&mut self) {
        // Update recent window for all responders
        for stats in self.responders.values_mut() {
            stats.record_timeout();
        }
    }

    /// Update primary responder based on response count
    pub fn update_primary(&mut self) {
        self.primary = self
            .responders
            .iter()
            .max_by_key(|(_, s)| s.received)
            .map(|(ip, _)| *ip);
    }

    /// Get primary responder stats
    pub fn primary_stats(&self) -> Option<&ResponderStats> {
        self.primary.and_then(|ip| self.responders.get(&ip))
    }

    /// Loss percentage for this hop
    pub fn loss_pct(&self) -> f64 {
        if self.sent == 0 {
            0.0
        } else {
            (1.0 - (self.received as f64 / self.sent as f64)) * 100.0
        }
    }
}

/// Target being traced
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Target {
    pub original: String,
    pub resolved: IpAddr,
    pub hostname: Option<String>,
}

impl Target {
    pub fn new(original: String, resolved: IpAddr) -> Self {
        Self {
            original,
            resolved,
            hostname: None,
        }
    }
}

/// A complete tracing session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub target: Target,
    pub started_at: DateTime<Utc>,
    pub hops: Vec<Hop>,
    pub config: Config,
    pub complete: bool,  // destination reached?
    pub total_sent: u64, // total probes sent across all hops
}

impl Session {
    pub fn new(target: Target, config: Config) -> Self {
        let max_ttl = config.max_ttl;
        let mut hops = Vec::with_capacity(max_ttl as usize);
        for ttl in 1..=max_ttl {
            hops.push(Hop::new(ttl));
        }

        Self {
            target,
            started_at: Utc::now(),
            hops,
            config,
            complete: false,
            total_sent: 0,
        }
    }

    /// Get hop by TTL (1-indexed)
    pub fn hop(&self, ttl: u8) -> Option<&Hop> {
        if ttl == 0 || ttl as usize > self.hops.len() {
            None
        } else {
            Some(&self.hops[ttl as usize - 1])
        }
    }

    /// Get mutable hop by TTL (1-indexed)
    pub fn hop_mut(&mut self, ttl: u8) -> Option<&mut Hop> {
        if ttl == 0 || ttl as usize > self.hops.len() {
            None
        } else {
            Some(&mut self.hops[ttl as usize - 1])
        }
    }

    /// Get discovered hops (those that have received at least one response)
    #[allow(dead_code)]
    pub fn discovered_hops(&self) -> impl Iterator<Item = &Hop> {
        self.hops.iter().filter(|h| h.received > 0 || h.sent > 0)
    }

    /// Get the last hop that responded
    #[allow(dead_code)]
    pub fn last_responding_hop(&self) -> Option<&Hop> {
        self.hops.iter().rev().find(|h| h.received > 0)
    }
}

/// Serde helper for Duration
mod duration_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        duration.as_micros().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let micros = u64::deserialize(deserializer)?;
        Ok(Duration::from_micros(micros))
    }
}
