use std::{
	collections::HashMap,
	fs::File,
	io::{BufRead as _, BufReader},
	net::{IpAddr, Ipv4Addr, SocketAddr},
	str::FromStr,
};

use super::{action, dmap::DMapBuilder};

const DEFAULT_DNS_PORT: u16 = 53;

const DEFAULT_LISTEN_ADDR: SocketAddr =
	SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), DEFAULT_DNS_PORT);

pub struct Conf {
	pub listen: SocketAddr,
	pub upstream: Vec<SocketAddr>,
	pub alts: Vec<Vec<SocketAddr>>,
	pub exact_rules: HashMap<(Vec<u8>, u16), u64>,
	pub qtype_rules: Vec<(u16, u64)>,
}

impl Default for Conf {
	fn default() -> Self {
		Self {
			listen: DEFAULT_LISTEN_ADDR,
			upstream: Vec::new(),
			alts: Vec::new(),
			exact_rules: HashMap::new(),
			qtype_rules: Vec::new(),
		}
	}
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
		self.upstream.clear();
		self.upstream.extend(v);
	}

	fn domain_rule(&mut self, b: &mut DMapBuilder, args: &[&str]) {
		if args.len() < 2 {
			panic!("invalid domain rule: \"{}\"", args.join(" "));
		}
		let action = self.inner_action(&args[1..]);
		b.add_list([args[0]], action);
	}

	fn domain_list_rule(&mut self, b: &mut DMapBuilder, args: &[&str]) {
		if args.len() < 2 {
			panic!("invalid domain rule: \"{}\"", args.join(" "));
		}
		let (path, prefix) = args[0].split_once(' ').unwrap_or((args[0], ""));
		let action = self.inner_action(&args[1..]);
		if let Err(e) = b.add_file(path, prefix.as_bytes(), action) {
			panic!("error loading domain list \"{}\": {}", path, e);
		}
	}

	fn qtype_rule(&mut self, args: &[&str]) {
		if args.len() < 2 {
			panic!("invalid qtype rule: \"{}\"", args.join(" "));
		}
		let qtype = parse_qtype(args[0]);
		let action = self.inner_action(&args[1..]);
		self.qtype_rules.push((qtype, action));
	}

	fn inner_action(&mut self, args: &[&str]) -> u64 {
		match args.len() {
			0 => {
				panic!("action not specified in rule");
			}
			1 => match args[0].to_ascii_lowercase().as_str() {
				"default" => action::ACTION_DEFAULT,
				"nxdomain" => action::ACTION_NXDOMAIN,
				"notimp" => action::ACTION_NOTIMP,
				"refused" => action::ACTION_REFUSED,
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

	fn inner_alt(&mut self, args: &[&str]) -> u64 {
		if args.is_empty() {
			panic!("empty upstream spec");
		}
		let alt: Vec<SocketAddr> = args.iter().map(parse_addr).collect();
		if let Some(i) = self.alts.iter().position(|e| e == &alt) {
			action::u64_from_alt(i as u8)
		} else if self.alts.len() < u8::MAX as usize {
			self.alts.push(alt);
			action::u64_from_alt((self.alts.len() - 1) as u8)
		} else {
			panic!("too much");
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

fn parse_qtype(q: &str) -> u16 {
	match q.to_ascii_lowercase().as_str() {
		"aaaa" => 28,
		"svcb" => 64,
		"https" => 65,
		_ => u16::from_str(q).unwrap_or_else(|e| panic!("invalid query type \"{}\": {}", q, e)),
	}
}
