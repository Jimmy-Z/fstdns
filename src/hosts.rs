use std::{
	collections::HashMap,
	fs::File,
	io::{BufRead, BufReader, Result},
	net::{IpAddr, Ipv4Addr, Ipv6Addr},
	str::FromStr,
};

use crate::DName;

const BLACK_LIST: &[&[u8]] = &[
	b"localhost",
	b"ip6-localhost",
	b"ip6-loopback",
	b"ip6-allnodes",
	b"ip6-allrouters",
];

type DNameMap<T> = HashMap<DName, T>;

pub fn parse_hosts(
	hosts4: &mut DNameMap<Vec<Ipv4Addr>>,
	hosts6: &mut DNameMap<Vec<Ipv6Addr>>,
	path: &str,
) -> Result<()> {
	let mut r = BufReader::new(File::open(path).unwrap());
	let mut buf = String::with_capacity(0x100);
	while {
		buf.clear();
		r.read_line(&mut buf)?
	} > 0
	{
		let l = buf.trim_ascii();
		if l.is_empty() || l.as_bytes()[0] == b'#' {
			continue;
		}
		let mut args = l.split(' ').filter(|a| !a.is_empty());
		let addr = args.next().unwrap();
		let addr = match IpAddr::from_str(addr) {
			Ok(a) => a,
			Err(_) => {
				eprintln!("error parsing line: {}", l);
				continue;
			}
		};
		for name in args {
			let name = name.as_bytes();
			if BLACK_LIST.contains(&name) {
				continue;
			}
			let name = DName::from(name);
			match addr {
				IpAddr::V4(a) => {
					let v = hosts4.entry(name).or_insert(Vec::new());
					v.push(a);
				}
				IpAddr::V6(a) => {
					let v = hosts6.entry(name).or_insert(Vec::new());
					v.push(a);
				}
			}
		}
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use std::collections::HashMap;

use super::parse_hosts;

	#[test]
	fn test_parse() {
		let mut h4 = HashMap::new();
		let mut h6 = HashMap::new();
		parse_hosts(&mut h4, &mut h6, "etc/lists/hosts").unwrap();
		for (k, v) in h4.iter() {
			eprintln!("{} {:?}", str::from_utf8(k.as_ref()).unwrap(), v);
		}
		for (k, v) in h6.iter() {
			eprintln!("{} {:?}", str::from_utf8(k.as_ref()).unwrap(), v);
		}
	}
}
