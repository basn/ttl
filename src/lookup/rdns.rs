use anyhow::Result;
use hickory_resolver::config::{ResolverConfig, ResolverOpts};
use hickory_resolver::TokioAsyncResolver;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

use crate::state::Session;

/// DNS cache entry
struct CacheEntry {
    hostname: Option<String>,
    cached_at: Instant,
}

/// DNS lookup worker with caching
pub struct DnsLookup {
    resolver: TokioAsyncResolver,
    cache: RwLock<HashMap<IpAddr, CacheEntry>>,
    cache_ttl: Duration,
}

impl DnsLookup {
    pub async fn new() -> Result<Self> {
        let resolver = TokioAsyncResolver::tokio(ResolverConfig::default(), ResolverOpts::default());

        Ok(Self {
            resolver,
            cache: RwLock::new(HashMap::new()),
            cache_ttl: Duration::from_secs(3600), // 1 hour
        })
    }

    /// Lookup reverse DNS for an IP, using cache
    pub async fn reverse_lookup(&self, ip: IpAddr) -> Option<String> {
        // Check cache first
        {
            let cache = self.cache.read();
            if let Some(entry) = cache.get(&ip) {
                if entry.cached_at.elapsed() < self.cache_ttl {
                    return entry.hostname.clone();
                }
            }
        }

        // Perform lookup
        let hostname = match self.resolver.reverse_lookup(ip).await {
            Ok(lookup) => lookup.iter().next().map(|name| {
                let s = name.to_string();
                // Remove trailing dot
                s.trim_end_matches('.').to_string()
            }),
            Err(_) => None,
        };

        // Cache result
        {
            let mut cache = self.cache.write();
            cache.insert(
                ip,
                CacheEntry {
                    hostname: hostname.clone(),
                    cached_at: Instant::now(),
                },
            );
        }

        hostname
    }
}

/// Background DNS lookup worker that updates session state
pub async fn run_dns_worker(
    dns: Arc<DnsLookup>,
    state: Arc<RwLock<Session>>,
    cancel: CancellationToken,
) {
    let mut interval = tokio::time::interval(Duration::from_millis(500));

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                break;
            }
            _ = interval.tick() => {
                // Collect IPs that need lookup
                let ips_to_lookup: Vec<IpAddr> = {
                    let state = state.read();
                    state.hops.iter()
                        .flat_map(|hop| hop.responders.values())
                        .filter(|stats| stats.hostname.is_none())
                        .map(|stats| stats.ip)
                        .collect()
                };

                // Perform lookups (limited batch size)
                for ip in ips_to_lookup.into_iter().take(10) {
                    if cancel.is_cancelled() {
                        break;
                    }

                    if let Some(hostname) = dns.reverse_lookup(ip).await {
                        let mut state = state.write();
                        for hop in &mut state.hops {
                            if let Some(stats) = hop.responders.get_mut(&ip) {
                                stats.hostname = Some(hostname.clone());
                            }
                        }
                    }
                }
            }
        }
    }
}
