use std::net::Ipv4Addr;

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
const INVALID_OCTETS: [u8; 4] = [255; 4];
pub const INVALID_A: Ipv4Addr = Ipv4Addr::from_octets(INVALID_OCTETS);

pub const ACTION_DEFAULT: u64 = u64_from_op([0, 0, 0, 0]);
pub const ACTION_NXDOMAIN: u64 = u64_from_op([0, 0, 0, 3]);
pub const ACTION_NOTIMP: u64 = u64_from_op([0, 0, 0, 4]);
pub const ACTION_REFUSED: u64 = u64_from_op([0, 0, 0, 5]);

const fn u64_from_tuple(t: ([u8; 4], [u8; 4])) -> u64 {
	let ([a, b, c, d], [e, f, g, h]) = t;
	u64::from_be_bytes([a, b, c, d, e, f, g, h])
}

const fn u64_from_op(op: [u8; 4]) -> u64 {
	u64_from_tuple((INVALID_OCTETS, op))
}

pub fn u64_from_alt(i: u8) -> u64 {
	u64_from_op([0, 0, 1, i])
}

pub fn u64_from_rewrite(i: u8) -> u64 {
	u64_from_op([0, 0, 2, i])
}

pub const fn u64_from_a(a: &[Ipv4Addr]) -> u64 {
	match a.len() {
		1 => u64_from_tuple((a[0].octets(), INVALID_OCTETS)),
		2 => u64_from_tuple((a[0].octets(), a[1].octets())),
		_ => unreachable!(),
	}
}

pub fn u64_to_a(u: u64) -> [Ipv4Addr; 1] {
	let [a, b, c, d, _, _, _, _] = u.to_be_bytes();
	[Ipv4Addr::from_octets([a, b, c, d])]
}

pub fn u64_to_2a(u: u64) -> [Ipv4Addr; 2] {
	let [a, b, c, d, e, f, g, h] = u.to_be_bytes();
	[
		Ipv4Addr::from_octets([a, b, c, d]),
		Ipv4Addr::from_octets([e, f, g, h]),
	]
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Action {
	Default,
	NxDomain,
	NotImp,
	Refused,
	AltUp(u8),
	Rewrite(u8),
	RewriteA,
	Rewrite2A,
	Unk,
}

const P0_MASK: u64 = 0xffffffff_00000000;
const P0_INVALID_CHK: u64 = u64_from_tuple((INVALID_OCTETS, [0; 4]));
const P1_MASK: u64 = 0x00000000_ffffffff;
const P1_INVALID_CHK: u64 = u64_from_tuple(([0; 4], INVALID_OCTETS));

impl From<u64> for Action {
	fn from(v: u64) -> Self {
		match v {
			ACTION_DEFAULT => return Action::Default,
			ACTION_NXDOMAIN => return Action::NxDomain,
			ACTION_NOTIMP => return Action::NotImp,
			ACTION_REFUSED => return Action::Refused,
			_ => {}
		}
		if v & P0_MASK == P0_INVALID_CHK {
			let [_, _, _, _, a, b, c, d] = v.to_be_bytes();
			match (a, b, c, d) {
				(0, 0, 1, i) => Action::AltUp(i),
				(0, 0, 2, i) => Action::Rewrite(i),
				_ => {
					eprintln!("unexpected action: {:016x}", v);
					unreachable!()
				}
			}
		} else if v & P1_MASK == P1_INVALID_CHK {
			Action::RewriteA
		} else {
			Action::Rewrite2A
		}
	}
}

#[cfg(test)]
mod tests {
	use rand::random;

	use super::*;

	#[test]
	fn test_action() {
		eprintln!("size of Action: {}", std::mem::size_of::<Action>());
		// test layout
		let a: u64 = u64_from_a(&[Ipv4Addr::new(1, 2, 3, 4), Ipv4Addr::new(5, 6, 7, 8)]);
		assert_eq!(a, 0x0102030405060708);

		let mut tests = vec![
			ACTION_DEFAULT,
			ACTION_NXDOMAIN,
			ACTION_NOTIMP,
			ACTION_REFUSED,
		];
		for i in 0u8..=0xff {
			tests.push(u64_from_alt(i));
			tests.push(u64_from_rewrite(i));
		}
		for _ in 0..0x100000 {
			tests.push(random());
		}

		eprintln!("{} tests", tests.len());
		for &a in &tests {
			let c = match Action::from(a) {
				Action::Default => ACTION_DEFAULT,
				Action::NxDomain => ACTION_NXDOMAIN,
				Action::NotImp => ACTION_NOTIMP,
				Action::Refused => ACTION_REFUSED,
				Action::AltUp(i) => u64_from_alt(i),
				Action::Rewrite(i) => u64_from_rewrite(i),
				Action::RewriteA => {
					let b = u64_to_a(a);
					u64_from_a(&b)
				}
				Action::Rewrite2A => {
					let b = u64_to_2a(a);
					u64_from_a(&b)
				}
				Action::Unk => a,
			};
			assert_eq!(a, c);
		}
	}
}
