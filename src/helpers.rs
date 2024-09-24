use std::net::IpAddr;
use std::str::FromStr;

#[must_use]
/// Checks if the address is the loopback address.
///
/// # Panics
/// `expect` calls in function can not throw as the provided IP addresses are valid.
pub fn is_loopback_address(ip: &IpAddr) -> bool {
    let loopback_v4 = IpAddr::from_str("127.0.0.1").expect("should be valid IPv4 loopback address");
    let loopback_v6 = IpAddr::from_str("::1").expect("should be valid IPv6 loopback address");

    ip.eq(&loopback_v4) || ip.eq(&loopback_v6)
}
