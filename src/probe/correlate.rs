use crate::state::{IcmpResponseType, ProbeId};
use pnet::packet::icmp::{IcmpPacket, IcmpTypes};
use pnet::packet::ipv4::Ipv4Packet;
use std::net::IpAddr;

/// Parsed ICMP response
#[derive(Debug, Clone)]
pub struct ParsedResponse {
    pub responder: IpAddr,
    pub probe_id: ProbeId,
    pub response_type: IcmpResponseType,
}

/// Parse an ICMP response and correlate it to our probe
///
/// Returns None if:
/// - Packet is malformed
/// - Packet is not a response to our probe (wrong identifier)
pub fn parse_icmp_response(
    data: &[u8],
    responder: IpAddr,
    our_identifier: u16,
) -> Option<ParsedResponse> {
    // Skip IP header (first 20 bytes typically, but check IHL)
    let ip_packet = Ipv4Packet::new(data)?;
    let ip_header_len = (ip_packet.get_header_length() as usize) * 4;

    if data.len() < ip_header_len + 8 {
        return None;
    }

    let icmp_data = &data[ip_header_len..];
    let icmp_packet = IcmpPacket::new(icmp_data)?;

    let icmp_type = icmp_packet.get_icmp_type();

    match icmp_type {
        IcmpTypes::EchoReply => {
            // Echo Reply: identifier and sequence are in bytes 4-7
            if icmp_data.len() < 8 {
                return None;
            }
            let identifier = u16::from_be_bytes([icmp_data[4], icmp_data[5]]);
            let sequence = u16::from_be_bytes([icmp_data[6], icmp_data[7]]);

            if identifier != our_identifier {
                return None;
            }

            Some(ParsedResponse {
                responder,
                probe_id: ProbeId::from_sequence(sequence),
                response_type: IcmpResponseType::EchoReply,
            })
        }
        IcmpTypes::TimeExceeded => {
            // Time Exceeded: payload contains original IP header + first 8 bytes of original ICMP
            parse_icmp_error_payload(icmp_data, responder, our_identifier, IcmpResponseType::TimeExceeded)
        }
        IcmpTypes::DestinationUnreachable => {
            // Destination Unreachable: same structure as Time Exceeded
            let code = icmp_packet.get_icmp_code().0;
            parse_icmp_error_payload(
                icmp_data,
                responder,
                our_identifier,
                IcmpResponseType::DestUnreachable(code),
            )
        }
        _ => None,
    }
}

/// Parse the payload of an ICMP error message (Time Exceeded or Dest Unreachable)
fn parse_icmp_error_payload(
    icmp_data: &[u8],
    responder: IpAddr,
    our_identifier: u16,
    response_type: IcmpResponseType,
) -> Option<ParsedResponse> {
    // ICMP error format:
    // [0-3]  ICMP header (type, code, checksum)
    // [4-7]  Unused (4 bytes)
    // [8..]  Original IP header + first 8 bytes of original ICMP

    if icmp_data.len() < 8 + 20 + 8 {
        // Need at least ICMP header + IP header + ICMP header
        return None;
    }

    let original_ip_data = &icmp_data[8..];
    let original_ip = Ipv4Packet::new(original_ip_data)?;
    let orig_ihl = (original_ip.get_header_length() as usize) * 4;

    if original_ip_data.len() < orig_ihl + 8 {
        return None;
    }

    let original_icmp_data = &original_ip_data[orig_ihl..];

    // Extract identifier and sequence from original ICMP header
    // [0]    Type (should be 8 for Echo Request)
    // [1]    Code (should be 0)
    // [2-3]  Checksum
    // [4-5]  Identifier
    // [6-7]  Sequence

    if original_icmp_data[0] != 8 {
        // Not our Echo Request
        return None;
    }

    let identifier = u16::from_be_bytes([original_icmp_data[4], original_icmp_data[5]]);
    let sequence = u16::from_be_bytes([original_icmp_data[6], original_icmp_data[7]]);

    if identifier != our_identifier {
        return None;
    }

    Some(ParsedResponse {
        responder,
        probe_id: ProbeId::from_sequence(sequence),
        response_type,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_probe_id_round_trip() {
        let original = ProbeId::new(15, 42);
        let sequence = original.to_sequence();
        let decoded = ProbeId::from_sequence(sequence);
        assert_eq!(original.ttl, decoded.ttl);
        assert_eq!(original.seq, decoded.seq);
    }
}
