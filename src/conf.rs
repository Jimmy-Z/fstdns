use std::{
	collections::HashMap,
	fs::File,
	io::{BufRead as _, BufReader},
	net::{IpAddr, Ipv4Addr, SocketAddr},
	str::FromStr,
};

use smart_default::SmartDefault;

use dns::{Answer, CVec63, QType};

use super::{action::ActionId, dmap::DMapBuilder};

const DEFAULT_DNS_PORT: u16 = 53;

const DEFAULT_LISTEN_ADDR: SocketAddr =
	SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), DEFAULT_DNS_PORT);

#[derive(SmartDefault)]
pub struct Conf {
	#[default(DEFAULT_LISTEN_ADDR)]
	pub listen: SocketAddr,
	pub default: Vec<SocketAddr>,
	pub alts: Vec<Vec<SocketAddr>>,
	pub rewrites: Vec<Vec<Answer>>,
	pub exact_rules: HashMap<(CVec63, QType), ActionId>,
	pub unqualified_rule: Option<ActionId>,
	pub qtype_rules: Vec<(QType, ActionId)>,
	pub addr_rules: HashMap<IpAddr, ActionId>,
}

impl Conf {
	pub fn conf<R: std::io::Read>(&mut self, b: &mut DMapBuilder, f: R) {
		let mut r = BufReader::new(f);
		let mut l = String::with_capacity(0x100);
		while r.read_line(&mut l).unwrap() > 0 {
			self.conf_line(b, &l);
			l.clear();
		}
	}

	pub fn conf_line(&mut self, b: &mut DMapBuilder, mut l: &str) {
		l = l.trim();
		if l.is_empty() || l.as_bytes().first().copied() == Some(b'#') {
			return;
		}
		let args: Vec<&str> = l.split(' ').filter(|&a| !a.is_empty()).collect();
		match args[0].to_ascii_lowercase().as_str() {
			"listen" => self.listen = parse_addr(args[1]),
			"default" => self.upstream(&args[1..]),
			"resolv-conf" => {
				self.resolv_conf(&args[1..]);
			}
			"domain" => {
				self.domain_rule(b, &args[1..]);
			}
			"domain-list" => {
				self.domain_list_rule(b, &args[1..]);
			}
			"qtype" => {
				self.qtype_rule(&args[1..]);
			}
			_ => {
				panic!("unrecognized conf line: \"{}\"", l);
			}
		}
	}

	pub fn finalize(&mut self) {
		self.qtype_rules.sort_by_key(|e| e.0);
	}

	fn upstream(&mut self, args: &[&str]) {
		self.inner_upstream(args.iter().map(parse_addr));
	}

	fn resolv_conf(&mut self, args: &[&str]) {
		if args.len() != 1 {
			panic!("invalid resolv-conf: \"{}\"", args.join(" "));
		}
		let r = args[0];
		self.inner_upstream(BufReader::new(File::open(r).unwrap()).lines().filter_map(
			|l| match l {
				Ok(l) => {
					let l = l.trim();
					if l.is_empty() || l.as_bytes()[0] == b'#' {
						None
					} else if let Some((k, v)) = l.split_once(' ')
						&& k == "nameserver"
					{
						Some(parse_addr(v.trim()))
					} else {
						eprintln!("skipped line \"{}\"", l);
						None
					}
				}
				Err(e) => {
					eprintln!("error parsing \"{}\": {}", r, e);
					None
				}
			},
		));
	}

	fn inner_upstream(&mut self, v: impl Iterator<Item = SocketAddr>) {
		self.default.clear();
		self.default.extend(v);
	}

	fn domain_rule(&mut self, b: &mut DMapBuilder, args: &[&str]) {
		if args.len() < 2 {
			panic!("invalid domain rule: \"{}\"", args.join(" "));
		}
		let action = self.inner_action(&args[1..]);
		b.add_list([args[0]], action.into());
	}

	fn domain_list_rule(&mut self, b: &mut DMapBuilder, args: &[&str]) {
		if args.len() < 2 {
			panic!("invalid domain rule: \"{}\"", args.join(" "));
		}
		let (path, prefix) = args[0].split_once(' ').unwrap_or((args[0], ""));
		let action = self.inner_action(&args[1..]);
		if b.add_file(path, prefix.as_bytes(), action.into()).is_err() {
			panic!("error loading domain list \"{}\"", path);
		}
	}

	fn qtype_rule(&mut self, args: &[&str]) {
		if args.len() < 2 {
			panic!("invalid qtype rule: \"{}\"", args.join(" "));
		}
		let qtype = QType::from_str(args[0]).unwrap();
		let action = self.inner_action(&args[1..]);
		self.qtype_rules.push((qtype, action));
	}

	fn inner_action(&mut self, args: &[&str]) -> ActionId {
		match args.len() {
			0 => {
				panic!("action not specified in rule");
			}
			1 => match ActionId::from_str(args[0].to_ascii_lowercase().as_str()) {
				Ok(a) => a,
				_ => {
					panic!("invalid action \"{}\"", args[0]);
				}
			},
			_ => match args[0].to_ascii_lowercase().as_str() {
				"alt" => self.inner_alt(&args[1..]),
				_ => {
					panic!("unknown action \"{}\"", args.join(" "));
				}
			},
		}
	}

	fn inner_alt(&mut self, args: &[&str]) -> ActionId {
		if args.is_empty() {
			panic!("empty alternative upstream spec");
		}
		let mut alt: Vec<SocketAddr> = args.iter().map(parse_addr).collect();
		alt.sort();
		if let Some(i) = self.alts.iter().position(|e| e == &alt) {
			ActionId::from_alt_id(i).unwrap()
		} else {
			self.alts.push(alt);
			ActionId::from_alt_id(self.alts.len() - 1).unwrap_or_else(|| panic!("too many alts"))
		}
	}
}

fn parse_addr(a: impl AsRef<str>) -> SocketAddr {
	let a = a.as_ref();
	if let Ok(a) = SocketAddr::from_str(a) {
		a
	} else {
		match IpAddr::from_str(a) {
			Ok(a) => SocketAddr::new(a, DEFAULT_DNS_PORT),
			Err(e) => panic!("error parsing address \"{}\": {}", a, e),
		}
	}
}
