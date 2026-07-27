use std::net::Ipv4Addr;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Action {
	Default,
	NxDomain,
	NotImp,
	Refused,
	AltUp(u8),
	Rewrite(u8),
	RewriteA(Ipv4Addr),
	Rewrite2A([Ipv4Addr; 2]),
	Unk(u64),
}

// we want action to be encoded into u64 (for FST)
// and hold up to 2 A records, and some other things
// mathematically that's impossible
// but consider some IP we'll never want to rewrite to
// for example 255.255.255.255/32, the local broadcast address
// (call that INVALID)
//	[IPv4, IPv4] two A record
// [IPv4, INVALID] single A record
//	[INVALID, ...] leaves us an u32 for other purposes
// this is definitely over-engineering, but it's fun

// anyway, just in case, the invalid value can be easily changed
// and since it only lives in memory, shouldn't cause any problem
const INVALID_OCTETS: [u8; 4] = [255, 255, 255, 255];
pub const INVALID_A: Ipv4Addr = Ipv4Addr::from_octets(INVALID_OCTETS);
const ZERO_A: Ipv4Addr = Ipv4Addr::new(0, 0, 0, 0);

const P0_MASK: u64 = 0xffffffff_00000000;
const P0_INVALID: u64 = Action::Rewrite2A([INVALID_A, ZERO_A]).into_u64();
const P1_MASK: u64 = 0x00000000_ffffffff;
const P1_INVALID: u64 = Action::Rewrite2A([ZERO_A, INVALID_A]).into_u64();

impl From<u64> for Action {
	fn from(v: u64) -> Self {
		if v & P0_MASK == P0_INVALID {
			let [_, _, _, _, a, b, c, d] = v.to_be_bytes();
			match (a, b, c, d) {
				(0, 0, 0, 0) => Action::Default,
				(0, 0, 0, 3) => Action::NxDomain,
				(0, 0, 0, 4) => Action::NotImp,
				(0, 0, 0, 5) => Action::Refused,
				(0, 0, 1, i) => Action::AltUp(i),
				(0, 0, 2, i) => Action::Rewrite(i),
				_ => {
					eprintln!("unexpected action: {:016x}", v);
					unreachable!()
				}
			}
		} else if v & P1_MASK == P1_INVALID {
			let [a, b, c, d, _, _, _, _] = v.to_be_bytes();
			Action::RewriteA(Ipv4Addr::new(a, b, c, d))
		} else {
			let [a, b, c, d, e, f, g, h] = v.to_be_bytes();
			Action::Rewrite2A([Ipv4Addr::new(a, b, c, d), Ipv4Addr::new(e, f, g, h)])
		}
	}
}

impl Action {
	const fn into_u64(&self) -> u64 {
		let ([a, b, c, d], [e, f, g, h]) = match self {
			Self::Default => (INVALID_OCTETS, [0, 0, 0, 0]),
			Self::NxDomain => (INVALID_OCTETS, [0, 0, 0, 3]),
			Self::NotImp => (INVALID_OCTETS, [0, 0, 0, 4]),
			Self::Refused => (INVALID_OCTETS, [0, 0, 0, 5]),
			Self::AltUp(i) => (INVALID_OCTETS, [0, 0, 1, *i]),
			Self::Rewrite(i) => (INVALID_OCTETS, [0, 0, 2, *i]),
			Self::RewriteA(addr) => (addr.octets(), INVALID_OCTETS),
			Self::Rewrite2A(addrs) => (addrs[0].octets(), addrs[1].octets()),
			_ => {
				unreachable!()
			}
		};
		u64::from_be_bytes([a, b, c, d, e, f, g, h])
	}
}

impl From<Action> for u64 {
	fn from(val: Action) -> Self {
		if let Action::Unk(v) = val {
			// eprintln is not const fn, can't be called in into_64()
			eprintln!("unexpected action: {:016x}", v);
			unreachable!()
		} else {
			val.into_u64()
		}
	}
}

#[cfg(test)]
mod tests {
	use rand::random;

	use super::*;
	const INVALID_U32: u32 = u32::from_be_bytes(INVALID_OCTETS);

	#[test]
	fn test_action() {
		eprintln!("size of Action: {}", std::mem::size_of::<Action>());
		// test layout
		let a: u64 =
			Action::Rewrite2A([Ipv4Addr::new(1, 2, 3, 4), Ipv4Addr::new(5, 6, 7, 8)]).into();
		assert_eq!(a, 0x0102030405060708);

		let mut tests = vec![Action::Default, Action::NxDomain, Action::NotImp, Action::Refused];
		for i in 0u8..=0xff {
			tests.push(Action::AltUp(i));
			tests.push(Action::Rewrite(i));

			let a: u32 = random();
			if a == INVALID_U32 {
				continue;
			}
			tests.push(Action::RewriteA(Ipv4Addr::from_octets(a.to_be_bytes())));

			let a: u32 = random();
			let b: u32 = random();
			if a == INVALID_U32 || b == INVALID_U32 {
				continue;
			}
			tests.push(Action::Rewrite2A([
				Ipv4Addr::from_octets(a.to_be_bytes()),
				Ipv4Addr::from_octets(b.to_be_bytes()),
			]));
		}

		eprintln!("{} tests", tests.len());
		for &a in &tests {
			let b: u64 = a.into();
			assert_eq!(a, b.into());
		}
	}
}
