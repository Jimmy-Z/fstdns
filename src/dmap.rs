// domain map using fst, a demonstration

use fst::raw::{Fst, Output};

pub fn dmap_get<D: AsRef<[u8]>, N: AsRef<str>>(t: &Fst<D>, n: N) -> Option<u64> {
	let b = n.as_ref().as_bytes();
	let mut node = t.root();
	let mut out = Output::zero();
	let mut last_match = None;
	for i in (0..b.len()).rev() {
		if let Some(tid) = node.find_input(b[i]) {
			let trans = node.transition(tid);
			out = out.cat(trans.out);
			node = t.node(trans.addr);
			if node.is_final() && (i == 0 || b[i - 1] == b'.') {
				last_match = Some(out.cat(node.final_output()).value());
				eprintln!(
					"found match for \"{}\", {} at pos {} ({})",
					n.as_ref(),
					last_match.unwrap(),
					i,
					&n.as_ref()[i..]
				);
			}
		} else {
			break;
		}
	}
	last_match
}

#[cfg(test)]
mod tests {
	use std::{
		fs::File,
		io::{self, BufRead, BufReader},
	};

	use fst::raw::Fst;

	use crate::dmap::dmap_get;

	#[test]
	fn test_match() {
		let t = build_test_fst(&[(b"com", 0), (b"example.com", 1)]);

		assert_eq!(dmap_get(&t, "com"), Some(0));
		assert_eq!(dmap_get(&t, "net"), None);
		assert_eq!(dmap_get(&t, "a.com"), Some(0));
		assert_eq!(dmap_get(&t, "acom"), None);
		assert_eq!(dmap_get(&t, "example.com"), Some(1));
		assert_eq!(dmap_get(&t, "sub.example.com"), Some(1));
		assert_eq!(dmap_get(&t, "notsubexample.com"), Some(0));
	}

	fn build_test_fst(lst: &[(&[u8], u64)]) -> Fst<Vec<u8>> {
		let mut lst: Vec<(Vec<u8>, u64)> = lst
			.into_iter()
			.map(|e| {
				let mut k = e.0.as_ref().to_vec();
				k.reverse();
				(k, e.1)
			})
			.collect();
		lst.sort();
		Fst::from_iter_map(lst).unwrap()
	}

	#[test]
	fn test_build() {
		let _ = from_path("./lst/domainswild");
	}

	fn from_path(fname: &str) -> io::Result<Fst<Vec<u8>>> {
		let mut f = BufReader::new(File::open(fname)?);
		let mut line = Vec::with_capacity(0x100);
		let mut lst = Vec::with_capacity(0x400);
		let mut domains = 0;
		let mut bytes = 0;
		while f.read_until(b'\n', &mut line)? > 0 {
			let mut l = line.trim_ascii();
			// eprintln!("line len: {}", l.len());
			if l.len() < 4 || l[0] == b'#' {
				// eprintln!("skipped line: \"{}\"", str::from_utf8(l).unwrap());
				line.clear();
				continue;
			}
			if l[0] != b'*' || l[1] != b'.' {
				eprintln!("unexpected line: \"{}\"", str::from_utf8(l).unwrap());
				line.clear();
				continue;
			}
			l = &l[2..];
			domains += 1;
			bytes += l.len();
			let mut rev = l.to_vec();
			rev.reverse();
			lst.push(rev);
			line.clear();
		}
		println!("{}: {} domains, {} bytes", fname, domains, bytes);
		lst.sort();
		let fst = Fst::from_iter_map(lst.iter().map(|e| (e, 0))).unwrap();
		// to be honest I'm a little disappointed in compression ratio
		println!(
			"fst map: {} bytes, ratio: {:.1}%",
			fst.size(),
			fst.size() as f32 * 100f32 / (bytes + domains) as f32
		);
		// set doesn't really save space
		// let fst = Fst::from_iter_set(lst.iter()).unwrap();
		// println!("fst set: {} bytes", fst.size());
		Ok(fst)
	}
}
