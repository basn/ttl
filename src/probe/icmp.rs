use pnet::packet::icmp::echo_request::MutableEchoRequestPacket;
use pnet::packet::icmp::{IcmpCode, IcmpTypes, checksum};
use pnet::packet::MutablePacket;

/// ICMP Echo Request packet size
pub const ICMP_HEADER_SIZE: usize = 8;
pub const ICMP_PAYLOAD_SIZE: usize = 56; // Standard ping payload
pub const ICMP_PACKET_SIZE: usize = ICMP_HEADER_SIZE + ICMP_PAYLOAD_SIZE;

/// Get process identifier for ICMP identification field
pub fn get_identifier() -> u16 {
    std::process::id() as u16
}

/// Build an ICMP Echo Request packet
pub fn build_echo_request(identifier: u16, sequence: u16) -> Vec<u8> {
    let mut buffer = vec![0u8; ICMP_PACKET_SIZE];

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
        let packet = build_echo_request(1234, 5678);
        assert_eq!(packet.len(), ICMP_PACKET_SIZE);
        assert_eq!(packet[0], 8); // Echo Request type
        assert_eq!(packet[1], 0); // Code
    }
}
