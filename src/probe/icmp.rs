use pnet::packet::MutablePacket;
use pnet::packet::icmp::echo_request::MutableEchoRequestPacket;
use pnet::packet::icmp::{IcmpCode, IcmpTypes, checksum};

/// ICMP header size (fixed)
pub const ICMP_HEADER_SIZE: usize = 8;
/// Default payload size (standard ping)
pub const DEFAULT_PAYLOAD_SIZE: usize = 56;
/// Minimum payload size (just timestamp)
pub const MIN_PAYLOAD_SIZE: usize = 8;

/// Get process identifier for ICMP identification field
pub fn get_identifier() -> u16 {
    std::process::id() as u16
}

/// Build an ICMP Echo Request packet with configurable payload size
pub fn build_echo_request(identifier: u16, sequence: u16, payload_size: usize) -> Vec<u8> {
    let payload_size = payload_size.max(MIN_PAYLOAD_SIZE);
    let packet_size = ICMP_HEADER_SIZE + payload_size;
    let mut buffer = vec![0u8; packet_size];

    let mut packet = MutableEchoRequestPacket::new(&mut buffer).unwrap();

    packet.set_icmp_type(IcmpTypes::EchoRequest);
    packet.set_icmp_code(IcmpCode::new(0));
    packet.set_identifier(identifier);
    packet.set_sequence_number(sequence);

    // Fill payload with timestamp or pattern
    let payload = packet.payload_mut();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64;

    // Put timestamp in first 8 bytes
    payload[..8].copy_from_slice(&timestamp.to_be_bytes());

    // Fill rest with pattern
    for (i, byte) in payload[8..].iter_mut().enumerate() {
        *byte = (i & 0xFF) as u8;
    }

    // Calculate checksum
    let cksum = checksum(&pnet::packet::icmp::IcmpPacket::new(&buffer).unwrap());
    let mut packet = MutableEchoRequestPacket::new(&mut buffer).unwrap();
    packet.set_checksum(cksum);

    buffer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_echo_request() {
        let packet = build_echo_request(1234, 5678, DEFAULT_PAYLOAD_SIZE);
        assert_eq!(packet.len(), ICMP_HEADER_SIZE + DEFAULT_PAYLOAD_SIZE);
        assert_eq!(packet[0], 8); // Echo Request type
        assert_eq!(packet[1], 0); // Code
    }

    #[test]
    fn test_build_echo_request_custom_size() {
        // Test larger payload
        let packet = build_echo_request(1234, 5678, 1400);
        assert_eq!(packet.len(), ICMP_HEADER_SIZE + 1400);

        // Test minimum payload
        let packet = build_echo_request(1234, 5678, 0);
        assert_eq!(packet.len(), ICMP_HEADER_SIZE + MIN_PAYLOAD_SIZE);
    }
}
