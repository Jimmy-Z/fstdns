use std::{
	env::Args,
	io::{BufRead as _, Result},
	net::{IpAddr, Ipv4Addr, SocketAddr},
	str::FromStr,
};

use fstdns::dmap::DMapBuilder;

const DEFAULT_CONF_PATH: &str = "conf";

const DEFAULT_DNS_PORT: u16 = 53;

const DEFAULT_LISTEN_ADDR: SocketAddr =
	SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), DEFAULT_DNS_PORT);

fn main() -> Result<()> {
	let args: Vec<_> = std::env::args().into_iter().take(2).collect();
	let mut conf = Conf::new();
	let mut fstb = DMapBuilder::new();
	if args.len() == 1 {
		conf.conf(&mut fstb, std::fs::File::open(DEFAULT_CONF_PATH)?)?;
	} else {
		if args[1] == "-" {
			conf.conf(&mut fstb, std::io::stdin())?;
		} else {
			conf.conf(&mut fstb, std::fs::File::open(&args[1] as &str)?)?;
		}
	};
	let dmap = fstb.build();
	Ok(())
}

enum Action {
	AltUp(u8),
	NxDomain,
	NotImp,
	Refused,
}

struct Conf {
	listen: SocketAddr,
	qtype_rules: Vec<(u16, Action)>,
	upstream: Vec<SocketAddr>,
	alts: Vec<Vec<SocketAddr>>,
}

impl Conf {
	fn new() -> Self {
		Self {
			listen: DEFAULT_LISTEN_ADDR,
			qtype_rules: Vec::new(),
			upstream: Vec::new(),
			alts: Vec::new(),
		}
	}

	fn conf<R: std::io::Read>(&mut self, b: &mut DMapBuilder, f: R) -> Result<()> {
		let mut r = std::io::BufReader::new(f);

		let mut l = String::with_capacity(0x100);
		while r.read_line(&mut l)? > 0 {
			self.conf_line(b, &l)?;
			l.clear();
		}
		Ok(())
	}

	fn conf_line(&mut self, b: &mut DMapBuilder, mut l: &str) -> Result<()> {
		l = l.trim();
		if l.is_empty() || l.bytes().nth(0) == Some(b'#') {
			return Ok(());
		}
		let args: Vec<&str> = l.split(' ').filter(|&a| !a.is_empty()).collect();
		match args[0] {
			"listen" => self.listen = parse_addr(args[1]),
			"upstream" => {
				self.upstream
					.extend((&args[1..]).iter().map(|a| parse_addr(a.trim())));
			}
			"qtype" => {
				self.qtype_rule(b, &args[1..])?;
			}
			_ => {
				eprintln!("unrecognized conf line: \"{}\"", l);
			}
		}
		Ok(())
	}

	fn qtype_rule(&mut self, b: &mut DMapBuilder, r: &[&str]) -> Result<()> {
		unimplemented!()
	}
}

fn parse_addr(a: &str) -> SocketAddr {
	SocketAddr::from_str(a)
		.unwrap_or_else(|_| SocketAddr::new(IpAddr::from_str(a).unwrap(), DEFAULT_DNS_PORT))
}
