use std::net::{Ipv4Addr, Ipv6Addr};

use super::DName;

// since ip6 ptr name is so fucking long, 32 (digits) * 2 + 8 (ip6.arpa) = 72 chars
// 5.c.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.d.0.0.3.c.5.9.0.e.0.4.2.ip6.arpa
pub enum Exact {
	Q((DName, u16)),
	Ptr4(Ipv4Addr),
	Ptr6(Ipv6Addr),
}

#[cfg(test)]
mod tests {
	use std::{
		mem::size_of,
		net::{Ipv4Addr, Ipv6Addr},
	};

	#[test]
	fn test() {
		eprintln!("size of Ipv4Addr: {}", size_of::<Ipv4Addr>());
		eprintln!("size of Ipv6Addr: {}", size_of::<Ipv6Addr>());
	}
}
