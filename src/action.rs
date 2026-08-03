use std::fmt::Display;

// although fst uses a fixed u64 value time
// test shows smaller values achieves better compression
// a previous design fully utilizing u64 was scrapped
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum ActionId {
	Default,
	NxDomain,
	NotImp,
	Refused,
	Alt(u8),
}

impl ActionId {
	const DEFAULT: u64 = 0;
	const NXDOMAIN: u64 = 3;
	const NOTIMP: u64 = 4;
	const REFUSED: u64 = 5;

	const DEFAULT_STR: &str = "default";
	const NXDOMAIN_STR: &str = "nxdomain";
	const NOTIMP_STR: &str = "notimp";
	const REFUSED_STR: &str = "refused";
	pub const ALT_STR: &str = "alt";

	// 16 different alternative upstreams ought to be enough for anybody
	const ALT_BASE: u64 = 0x10;
	const ALT_CAP: u64 = 0x20; //exclusive

	pub fn from_alt_id(alt_index: usize) -> Option<Self> {
		if alt_index < (Self::ALT_CAP - Self::ALT_BASE) as usize {
			Some(Self::Alt(alt_index as u8))
		} else {
			None
		}
	}
}

impl Display for ActionId {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Default => write!(f, "{}", Self::DEFAULT_STR),
			Self::NxDomain => write!(f, "{}", Self::NXDOMAIN_STR),
			Self::NotImp => write!(f, "{}", Self::NOTIMP_STR),
			Self::Refused => write!(f, "{}", Self::REFUSED_STR),
			Self::Alt(a) => write!(f, "{}:{}", Self::ALT_STR, a),
		}
	}
}

impl TryFrom<&str> for ActionId {
	type Error = ();
	fn try_from(v: &str) -> Result<Self, Self::Error> {
		match v {
			Self::DEFAULT_STR => Ok(Self::Default),
			Self::NXDOMAIN_STR => Ok(Self::NxDomain),
			Self::NOTIMP_STR => Ok(Self::NotImp),
			Self::REFUSED_STR => Ok(Self::Refused),
			_ => Err(()),
		}
	}
}

impl From<ActionId> for u64 {
	fn from(a: ActionId) -> Self {
		match a {
			ActionId::Default => ActionId::DEFAULT,
			ActionId::NxDomain => ActionId::NXDOMAIN,
			ActionId::NotImp => ActionId::NOTIMP,
			ActionId::Refused => ActionId::NOTIMP,
			ActionId::Alt(id) => id as u64 + ActionId::ALT_BASE,
		}
	}
}

impl TryFrom<u64> for ActionId {
	type Error = ();
	fn try_from(v: u64) -> Result<Self, Self::Error> {
		match v {
			Self::DEFAULT => Ok(Self::Default),
			Self::NXDOMAIN => Ok(Self::NxDomain),
			Self::NOTIMP => Ok(Self::NotImp),
			Self::REFUSED => Ok(Self::Refused),
			Self::ALT_BASE..Self::ALT_CAP => Ok(Self::Alt((v - Self::ALT_BASE) as u8)),
			_ => Err(()),
		}
	}
}
