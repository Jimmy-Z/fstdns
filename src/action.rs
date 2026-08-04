use std::{fmt::Display, str::FromStr};

use dns::RCode;

// although fst uses fixed size (u64) values
// tests show smaller values achieves better compression
// a previous design fully utilizing u64 was scrapped
#[derive(Clone, Copy, Eq, PartialEq, Default)]
pub enum ActionId {
	#[default]
	Default,
	RCode(RCode),
	Alt(u8),
	Rewrite(u8),
}

impl ActionId {
	const DEFAULT: u64 = 0;

	const DEFAULT_STR: &str = "default";
	pub const ALT_STR: &str = "alt";
	pub const REWRITE_STR: &str = "rewrite";

	// 16 different alternative upstreams ought to be enough for anybody
	const ALT_BASE: u64 = 0x10;
	const ALT_CAP: u64 = 0x20; //exclusive
	const REWRITE_BASE: u64 = 0x20;
	const REWRITE_CAP: u64 = 0x30; //exclusive

	pub fn from_alt_id(id: usize) -> Option<Self> {
		if id < (Self::ALT_CAP - Self::ALT_BASE) as usize {
			Some(Self::Alt(id as u8))
		} else {
			None
		}
	}

	pub fn from_rewrite_id(id: usize) -> Option<Self> {
		if id < (Self::REWRITE_CAP - Self::REWRITE_BASE) as usize {
			Some(Self::Rewrite(id as u8))
		} else {
			None
		}
	}
}

impl Display for ActionId {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Default => write!(f, "{}", Self::DEFAULT_STR),
			Self::RCode(c) => write!(f, "{}", c),
			Self::Alt(a) => write!(f, "{}:{}", Self::ALT_STR, a),
			Self::Rewrite(a) => write!(f, "{}:{}", Self::REWRITE_STR, a),
		}
	}
}

impl FromStr for ActionId {
	type Err = ();
	fn from_str(v: &str) -> Result<Self, Self::Err> {
		match v {
			Self::DEFAULT_STR => Ok(Self::Default),
			s => Ok(Self::RCode(RCode::from_str(s)?)),
		}
	}
}

impl From<ActionId> for u64 {
	fn from(a: ActionId) -> Self {
		match a {
			ActionId::Default => ActionId::DEFAULT,
			ActionId::RCode(c) => c.0 as u64,
			ActionId::Alt(id) => id as u64 + ActionId::ALT_BASE,
			ActionId::Rewrite(id) => id as u64 + ActionId::REWRITE_BASE,
		}
	}
}

impl TryFrom<u64> for ActionId {
	type Error = ();
	fn try_from(v: u64) -> Result<Self, Self::Error> {
		match v {
			Self::DEFAULT => Ok(Self::Default),
			1..16 => Ok(Self::RCode(RCode(v as u8))),
			Self::ALT_BASE..Self::ALT_CAP => Ok(Self::Alt((v - Self::ALT_BASE) as u8)),
			Self::REWRITE_BASE..Self::REWRITE_CAP => {
				Ok(Self::Rewrite((v - Self::REWRITE_BASE) as u8))
			}
			_ => Err(()),
		}
	}
}
