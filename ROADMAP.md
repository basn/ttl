# ttl Roadmap

## Current Status (v0.1.0)

### Core Features
- [x] ICMP Echo probing with TTL sweep
- [x] IPv4 and IPv6 support
- [x] Real-time TUI with ratatui
- [x] Hop statistics (loss, min/avg/max, stddev, jitter)
- [x] ECMP detection (multiple responders per TTL)
- [x] Reverse DNS resolution
- [x] JSON, CSV, and report export formats
- [x] Session replay from saved JSON
- [x] Pause/resume probing
- [x] Stats reset

### TUI Features
- [x] Interactive hop selection with j/k navigation
- [x] Hop detail modal view
- [x] Loss-aware sparkline visualization
- [x] Help overlay
- [x] Status bar with keybind hints

## Planned Features

### v0.2.0 - Enrichment
- [ ] ASN lookup via MaxMind GeoLite2
- [ ] Geolocation display
- [ ] IP-to-ASN mapping
- [ ] Network path visualization

### v0.3.0 - Multi-target
- [ ] Multiple simultaneous targets
- [ ] Target groups/presets
- [ ] Comparative views

### v0.4.0 - Advanced Probing
- [ ] UDP probing mode
- [ ] TCP SYN probing mode
- [ ] Custom port selection
- [ ] Paris traceroute (flow-aware)

### Future Ideas
- [ ] Historical data storage
- [ ] Alert thresholds (latency/loss)
- [ ] Web UI mode
- [ ] Prometheus metrics export
- [ ] MPLS label detection
- [ ] Path MTU discovery

## Non-Goals
- Full packet capture/analysis (use tcpdump/wireshark)
- Bandwidth testing (use iperf)
- Port scanning (use nmap)
