//! Internet Exchange (IX) detection via PeeringDB
//!
//! Identifies when a hop is at an Internet Exchange point by matching
//! IP addresses against IX peering LAN prefixes from PeeringDB.

use anyhow::{anyhow, Result};
use ipnetwork::IpNetwork;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio_util::sync::CancellationToken;

use crate::state::IxInfo;
use crate::trace::SessionMap;

/// PeeringDB API response wrapper
#[derive(Debug, Deserialize)]
struct PdbResponse<T> {
    data: Vec<T>,
}

/// IX record from PeeringDB /api/ix
#[derive(Debug, Deserialize)]
struct PdbIx {
    id: u32,
    name: String,
    city: Option<String>,
    country: Option<String>,
}

/// IX LAN record from PeeringDB /api/ixlan
#[derive(Debug, Deserialize)]
struct PdbIxlan {
    id: u32,
    ix_id: u32,
}

/// IX prefix record from PeeringDB /api/ixpfx
#[derive(Debug, Deserialize)]
struct PdbIxpfx {
    ixlan_id: u32,
    prefix: String,
}

/// Cached IX data for fast lookups
#[derive(Debug, Clone, Serialize, Deserialize)]
struct IxCacheEntry {
    name: String,
    city: Option<String>,
    country: Option<String>,
}

/// Cached prefix to IX mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PrefixCacheEntry {
    prefix: String, // Store as string for serialization
    ix_name: String,
    ix_city: Option<String>,
    ix_country: Option<String>,
}

/// Serializable cache format
#[derive(Debug, Serialize, Deserialize)]
struct IxCache {
    version: u32,
    fetched_at: u64, // Unix timestamp
    prefixes: Vec<PrefixCacheEntry>,
}

impl IxCache {
    const VERSION: u32 = 1;
    const MAX_AGE_SECS: u64 = 24 * 60 * 60; // 24 hours

    fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now - self.fetched_at > Self::MAX_AGE_SECS
    }
}

/// In-memory prefix entry for fast lookup
struct PrefixEntry {
    network: IpNetwork,
    info: IxInfo,
}

/// IX lookup via PeeringDB prefix matching
pub struct IxLookup {
    /// Parsed prefixes for lookup (populated from cache or API)
    prefixes: RwLock<Vec<PrefixEntry>>,
    /// Cache file path
    cache_path: PathBuf,
    /// Whether data has been loaded
    loaded: RwLock<bool>,
    /// Per-IP result cache (to avoid repeated lookups)
    ip_cache: RwLock<HashMap<IpAddr, Option<IxInfo>>>,
    /// IP cache TTL
    ip_cache_ttl: Duration,
    /// Timestamps for IP cache entries
    ip_cache_times: RwLock<HashMap<IpAddr, Instant>>,
}

impl IxLookup {
    /// Create a new IX lookup instance
    pub fn new() -> Result<Self> {
        // Use standard cache directory
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ttl")
            .join("peeringdb");

        // Create cache directory if needed
        fs::create_dir_all(&cache_dir)?;

        let cache_path = cache_dir.join("ix_cache.json");

        Ok(Self {
            prefixes: RwLock::new(Vec::new()),
            cache_path,
            loaded: RwLock::new(false),
            ip_cache: RwLock::new(HashMap::new()),
            ip_cache_ttl: Duration::from_secs(3600), // 1 hour for IP results
            ip_cache_times: RwLock::new(HashMap::new()),
        })
    }

    /// Lookup IX info for an IP address
    ///
    /// Lazily loads PeeringDB data on first lookup.
    pub async fn lookup(&self, ip: IpAddr) -> Option<IxInfo> {
        // Check IP cache first
        {
            let ip_cache = self.ip_cache.read();
            let ip_times = self.ip_cache_times.read();
            if let (Some(result), Some(time)) = (ip_cache.get(&ip), ip_times.get(&ip)) {
                if time.elapsed() < self.ip_cache_ttl {
                    return result.clone();
                }
            }
        }

        // Ensure data is loaded
        if !*self.loaded.read() {
            if let Err(e) = self.load_data().await {
                eprintln!("Failed to load IX data: {}", e);
                return None;
            }
        }

        // Search prefixes for matching network
        let result = {
            let prefixes = self.prefixes.read();
            prefixes
                .iter()
                .find(|entry| entry.network.contains(ip))
                .map(|entry| entry.info.clone())
        };

        // Cache result
        {
            let mut ip_cache = self.ip_cache.write();
            let mut ip_times = self.ip_cache_times.write();
            ip_cache.insert(ip, result.clone());
            ip_times.insert(ip, Instant::now());
        }

        result
    }

    /// Load IX data from cache or API
    async fn load_data(&self) -> Result<()> {
        // Try loading from cache first
        if let Ok(cache) = self.load_cache() {
            if !cache.is_expired() {
                self.populate_from_cache(&cache)?;
                *self.loaded.write() = true;
                return Ok(());
            }
        }

        // Fetch from API
        match self.fetch_from_api().await {
            Ok(cache) => {
                // Save to disk
                if let Err(e) = self.save_cache(&cache) {
                    eprintln!("Warning: failed to save IX cache: {}", e);
                }
                self.populate_from_cache(&cache)?;
                *self.loaded.write() = true;
                Ok(())
            }
            Err(e) => {
                // If API fails, try to use expired cache as fallback
                if let Ok(cache) = self.load_cache() {
                    eprintln!("Warning: using expired IX cache (API error: {})", e);
                    self.populate_from_cache(&cache)?;
                    *self.loaded.write() = true;
                    return Ok(());
                }
                Err(e)
            }
        }
    }

    /// Load cache from disk
    fn load_cache(&self) -> Result<IxCache> {
        let data = fs::read_to_string(&self.cache_path)?;
        let cache: IxCache = serde_json::from_str(&data)?;
        if cache.version != IxCache::VERSION {
            return Err(anyhow!("cache version mismatch"));
        }
        Ok(cache)
    }

    /// Save cache to disk
    fn save_cache(&self, cache: &IxCache) -> Result<()> {
        let data = serde_json::to_string_pretty(cache)?;
        fs::write(&self.cache_path, data)?;
        Ok(())
    }

    /// Populate prefixes from cache
    fn populate_from_cache(&self, cache: &IxCache) -> Result<()> {
        let mut entries = Vec::with_capacity(cache.prefixes.len());

        for p in &cache.prefixes {
            if let Ok(network) = p.prefix.parse::<IpNetwork>() {
                entries.push(PrefixEntry {
                    network,
                    info: IxInfo {
                        name: p.ix_name.clone(),
                        city: p.ix_city.clone(),
                        country: p.ix_country.clone(),
                    },
                });
            }
        }

        *self.prefixes.write() = entries;
        Ok(())
    }

    /// Fetch IX data from PeeringDB API
    async fn fetch_from_api(&self) -> Result<IxCache> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;

        // Fetch all three endpoints in parallel
        let (ix_result, ixlan_result, ixpfx_result) = tokio::join!(
            self.fetch_ix(&client),
            self.fetch_ixlan(&client),
            self.fetch_ixpfx(&client),
        );

        let ix_data = ix_result?;
        let ixlan_data = ixlan_result?;
        let ixpfx_data = ixpfx_result?;

        // Build lookup maps
        // ixlan_id -> ix_id
        let ixlan_to_ix: HashMap<u32, u32> = ixlan_data
            .iter()
            .map(|lan| (lan.id, lan.ix_id))
            .collect();

        // ix_id -> IX info
        let ix_info: HashMap<u32, IxCacheEntry> = ix_data
            .iter()
            .map(|ix| {
                (
                    ix.id,
                    IxCacheEntry {
                        name: ix.name.clone(),
                        city: ix.city.clone(),
                        country: ix.country.clone(),
                    },
                )
            })
            .collect();

        // Build prefix cache entries
        let mut prefixes = Vec::with_capacity(ixpfx_data.len());
        for pfx in ixpfx_data {
            if let Some(&ix_id) = ixlan_to_ix.get(&pfx.ixlan_id) {
                if let Some(ix) = ix_info.get(&ix_id) {
                    prefixes.push(PrefixCacheEntry {
                        prefix: pfx.prefix,
                        ix_name: ix.name.clone(),
                        ix_city: ix.city.clone(),
                        ix_country: ix.country.clone(),
                    });
                }
            }
        }

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Ok(IxCache {
            version: IxCache::VERSION,
            fetched_at: now,
            prefixes,
        })
    }

    /// Fetch IX data from API
    async fn fetch_ix(&self, client: &reqwest::Client) -> Result<Vec<PdbIx>> {
        let url = "https://www.peeringdb.com/api/ix";
        let resp: PdbResponse<PdbIx> = client.get(url).send().await?.json().await?;
        Ok(resp.data)
    }

    /// Fetch IXLAN data from API
    async fn fetch_ixlan(&self, client: &reqwest::Client) -> Result<Vec<PdbIxlan>> {
        let url = "https://www.peeringdb.com/api/ixlan";
        let resp: PdbResponse<PdbIxlan> = client.get(url).send().await?.json().await?;
        Ok(resp.data)
    }

    /// Fetch IX prefix data from API
    async fn fetch_ixpfx(&self, client: &reqwest::Client) -> Result<Vec<PdbIxpfx>> {
        let url = "https://www.peeringdb.com/api/ixpfx";
        let resp: PdbResponse<PdbIxpfx> = client.get(url).send().await?.json().await?;
        Ok(resp.data)
    }

    /// Get the number of prefixes loaded
    pub fn prefix_count(&self) -> usize {
        self.prefixes.read().len()
    }
}

/// Maximum concurrent IX lookups
const MAX_CONCURRENT_LOOKUPS: usize = 10;

/// Background IX lookup worker that updates session state
pub async fn run_ix_worker(
    ix_lookup: Arc<IxLookup>,
    sessions: SessionMap,
    cancel: CancellationToken,
) {
    let mut interval = tokio::time::interval(Duration::from_millis(500));

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                break;
            }
            _ = interval.tick() => {
                // Collect IPs that need IX lookup from all sessions
                let ips_to_lookup: Vec<IpAddr> = {
                    let sessions = sessions.read();
                    sessions.values()
                        .flat_map(|state| {
                            let session = state.read();
                            session.hops.iter()
                                .flat_map(|hop| hop.responders.values())
                                .filter(|stats| stats.ix.is_none())
                                .map(|stats| stats.ip)
                                .collect::<Vec<_>>()
                        })
                        .collect()
                };

                if ips_to_lookup.is_empty() {
                    continue;
                }

                // Perform parallel IX lookups (limited batch size)
                let batch: Vec<IpAddr> = ips_to_lookup
                    .into_iter()
                    .take(MAX_CONCURRENT_LOOKUPS)
                    .collect();

                // Spawn concurrent lookups
                let futures: Vec<_> = batch
                    .iter()
                    .map(|&ip| {
                        let ix = ix_lookup.clone();
                        async move { (ip, ix.lookup(ip).await) }
                    })
                    .collect();

                // Wait for all lookups to complete
                let results = futures::future::join_all(futures).await;

                // Update all sessions with results
                let sessions = sessions.read();
                for (ip, ix_info) in results {
                    if let Some(ix_info) = ix_info {
                        for state in sessions.values() {
                            let mut session = state.write();
                            for hop in &mut session.hops {
                                if let Some(stats) = hop.responders.get_mut(&ip) {
                                    stats.ix = Some(ix_info.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn test_prefix_matching() {
        // Test IpNetwork contains check
        let network: IpNetwork = "206.223.115.0/24".parse().unwrap();
        let inside = IpAddr::V4(Ipv4Addr::new(206, 223, 115, 100));
        let outside = IpAddr::V4(Ipv4Addr::new(206, 223, 116, 100));

        assert!(network.contains(inside));
        assert!(!network.contains(outside));
    }

    #[test]
    fn test_ix_cache_expiry() {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Fresh cache
        let fresh = IxCache {
            version: IxCache::VERSION,
            fetched_at: now,
            prefixes: vec![],
        };
        assert!(!fresh.is_expired());

        // Expired cache (25 hours old)
        let old = IxCache {
            version: IxCache::VERSION,
            fetched_at: now - 25 * 60 * 60,
            prefixes: vec![],
        };
        assert!(old.is_expired());
    }
}
