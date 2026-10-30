use std::net::{IpAddr, Ipv4Addr};

use actix_web::HttpRequest;

/// Request facts the handlers and extractors share.
pub(crate) trait RequestExt {
    /// The IP of the connected peer, or loopback when the transport reports none (as in test
    /// requests), so a missing address shares one limiter bucket instead of bypassing it.
    fn peer_ip(&self) -> IpAddr;
}

impl RequestExt for HttpRequest {
    fn peer_ip(&self) -> IpAddr {
        self.peer_addr()
            .map(|addr| addr.ip())
            .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST))
    }
}
