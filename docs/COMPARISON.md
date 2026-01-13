# Comparison with Similar Tools

How ttl compares to other traceroute and network diagnostic tools.

## Feature Matrix

| Feature | ttl | [Trippy](https://trippy.rs/) | [MTR](https://github.com/traviscross/mtr) | [NextTrace](https://github.com/nxtrace/NTrace-core) |
|---------|:---:|:---:|:---:|:---:|
| **Protocols** |||||
| ICMP | :white_check_mark: | :white_check_mark: | :white_check_mark: | :white_check_mark: |
| UDP | :white_check_mark: | :white_check_mark: | :white_check_mark: | :white_check_mark: |
| TCP | :white_check_mark: | :white_check_mark: | :white_check_mark: | :white_check_mark: |
| **Statistics** |||||
| Loss % | :white_check_mark: | :white_check_mark: | :white_check_mark: | :white_check_mark: |
| Min/Avg/Max RTT | :white_check_mark: | :white_check_mark: | :white_check_mark: | :white_check_mark: |
| Jitter | :white_check_mark: | :white_check_mark: | :x: | :x: |
| Std deviation | :white_check_mark: | :white_check_mark: | :white_check_mark: | :x: |
| **Enrichment** |||||
| Reverse DNS | :white_check_mark: | :white_check_mark: | :white_check_mark: | :white_check_mark: |
| ASN lookup | :white_check_mark: | :white_check_mark: | :x: | :white_check_mark: |
| GeoIP | :white_check_mark: | :white_check_mark: | :x: | :white_check_mark: |
| MPLS labels | :white_check_mark: | :white_check_mark: | :white_check_mark: | :x: |
| IX detection | :white_check_mark: | :x: | :x: | :x: |
| **ECMP** |||||
| Multi-path detection | :white_check_mark: | :white_check_mark: | :white_check_mark: | :white_check_mark: |
| Paris traceroute | :white_check_mark: | :white_check_mark: | :x: | :x: |
| **TUI** |||||
| Interactive | :white_check_mark: | :white_check_mark: | :white_check_mark: | :x: |
| Themes | :white_check_mark: | :white_check_mark: | :x: | :x: |
| Theme persistence | :white_check_mark: | :white_check_mark: | :x: | :x: |
| Sparklines/charts | :white_check_mark: | :white_check_mark: | :x: | :x: |
| World map | :x: | :white_check_mark: | :x: | :white_check_mark: |
| **Export** |||||
| JSON | :white_check_mark: | :white_check_mark: | :white_check_mark: | :white_check_mark: |
| CSV | :white_check_mark: | :white_check_mark: | :white_check_mark: | :x: |
| Session replay | :white_check_mark: | :x: | :x: | :x: |
| **Advanced** |||||
| Multiple targets | :white_check_mark: | :white_check_mark: | :x: | :x: |
| PMTUD | :white_check_mark: | :x: | :x: | :x: |
| NAT detection | :white_check_mark: | :x: | :x: | :x: |
| Rate limit detection | :white_check_mark: | :x: | :x: | :x: |

:white_check_mark: = supported | :x: = not supported

## When to Use Each Tool

### Use ttl when you need:
- Path MTU Discovery (PMTUD)
- NAT detection along the path
- Internet Exchange (IX) point identification
- Session replay for historical analysis
- Multiple simultaneous targets
- ICMP rate limit detection

### Use Trippy when you need:
- World map visualization
- More mature/stable tool
- Wider platform support

### Use MTR when you need:
- Available by default on most systems
- Simple, well-known interface
- Lightweight resource usage

### Use NextTrace when you need:
- China-optimized IP geolocation
- Multiple geolocation database support
- Map visualization

## Platform Support

| Platform | ttl | Trippy | MTR | NextTrace |
|----------|:---:|:------:|:---:|:---------:|
| Linux | :white_check_mark: | :white_check_mark: | :white_check_mark: | :white_check_mark: |
| macOS | :white_check_mark: | :white_check_mark: | :white_check_mark: | :white_check_mark: |
| Windows | :x: | :white_check_mark: | :white_check_mark: | :white_check_mark: |
| BSD | :x: | :white_check_mark: | :white_check_mark: | :x: |
