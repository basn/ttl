use anyhow::Result;
use parking_lot::RwLock;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::config::Config;
use crate::probe::{build_echo_request, create_send_socket, get_identifier, send_icmp, set_ttl};
use crate::state::{ProbeId, Session};

/// Message sent when a probe is dispatched
#[derive(Debug, Clone)]
pub struct ProbeSent {
    pub id: ProbeId,
    pub sent_at: Instant,
    pub target: IpAddr,
}

/// The probe engine sends ICMP probes at configured intervals
pub struct ProbeEngine {
    config: Config,
    target: IpAddr,
    identifier: u16,
    state: Arc<RwLock<Session>>,
    probe_tx: mpsc::Sender<ProbeSent>,
    cancel: CancellationToken,
}

impl ProbeEngine {
    pub fn new(
        config: Config,
        target: IpAddr,
        state: Arc<RwLock<Session>>,
        probe_tx: mpsc::Sender<ProbeSent>,
        cancel: CancellationToken,
    ) -> Self {
        Self {
            config,
            target,
            identifier: get_identifier(),
            state,
            probe_tx,
            cancel,
        }
    }

    /// Run the probe engine
    pub async fn run(self) -> Result<()> {
        let ipv6 = self.target.is_ipv6();
        let socket = create_send_socket(ipv6)?;

        let mut seq: u8 = 0;
        let mut total_sent: u64 = 0;
        let mut interval = tokio::time::interval(self.config.interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                _ = self.cancel.cancelled() => {
                    break;
                }
                _ = interval.tick() => {
                    // Check if paused
                    {
                        let state = self.state.read();
                        if state.paused {
                            continue;
                        }
                    }

                    // Check probe count limit
                    if let Some(count) = self.config.count {
                        if total_sent >= count * self.config.max_ttl as u64 {
                            // Signal completion
                            self.cancel.cancel();
                            break;
                        }
                    }

                    // Send probes for all TTLs
                    for ttl in 1..=self.config.max_ttl {
                        // Check if we've reached the destination for this TTL
                        let should_probe = {
                            let state = self.state.read();
                            // Always probe if we haven't completed, or if TTL is at or below last responding hop
                            !state.complete || state.hop(ttl).is_some_and(|h| h.received > 0)
                        };

                        if !should_probe {
                            continue;
                        }

                        let probe_id = ProbeId::new(ttl, seq);
                        let packet = build_echo_request(self.identifier, probe_id.to_sequence());

                        // Set TTL before sending
                        if let Err(e) = set_ttl(&socket, ttl) {
                            eprintln!("Failed to set TTL {}: {}", ttl, e);
                            continue;
                        }

                        let sent_at = Instant::now();

                        if let Err(e) = send_icmp(&socket, &packet, self.target) {
                            eprintln!("Failed to send probe TTL {}: {}", ttl, e);
                            continue;
                        }

                        // Record that we sent a probe
                        {
                            let mut state = self.state.write();
                            if let Some(hop) = state.hop_mut(ttl) {
                                hop.record_sent();
                            }
                            state.total_sent += 1;
                        }

                        // Notify receiver about sent probe for correlation
                        let _ = self.probe_tx.send(ProbeSent {
                            id: probe_id,
                            sent_at,
                            target: self.target,
                        }).await;

                        total_sent += 1;
                    }

                    seq = seq.wrapping_add(1);
                }
            }
        }

        Ok(())
    }
}

/// Create interval from config
#[allow(dead_code)]
pub fn create_probe_interval(config: &Config) -> tokio::time::Interval {
    let mut interval = tokio::time::interval(config.interval);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    interval
}
