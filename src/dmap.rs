// domain map using fst, a demonstration

use std::{cmp::Ordering, collections::BTreeMap, time::Instant};

use fst::raw::{Builder, Fst, Output};
use log::*;

use misc::Pretty;

use super::*;

pub struct DMap<D>(Fst<D>);

const MAX_FQDN_LEN: usize = 253;

impl<D: AsRef<[u8]>> DMap<D> {
	pub fn get(&self, b: &[u8]) -> Option<u64> {
		let mut node = self.0.root();
		let mut out = Output::zero();
		let mut last_match = None;
		for i in (0..b.len()).rev() {
			if let Some(tid) = node.find_input(b[i]) {
				let trans = node.transition(tid);
				out = out.cat(trans.out);
				node = self.0.node(trans.addr);
				if node.is_final() && (i == 0 || b[i - 1] == b'.') {
					last_match = Some(out.cat(node.final_output()).value());
					trace!(
						"found match for \"{}\", {} at pos {} ({})",
						Pretty(b),
						last_match.unwrap(),
						i,
						Pretty(&b[i..])
					);
				}
			} else {
				break;
			}
		}
		last_match
	}
}

#[derive(Default)]
pub struct DMapBuilder {
	files: Vec<(Vec<u8>, Vec<u8>, u64)>,
	lists: Vec<(Vec<Vec<u8>>, u64)>,
}

impl DMapBuilder {
	pub fn add_file(&mut self, path: &str, prefix: &[u8], v: u64) -> Dummy {
		let t0 = Instant::now();
		let file =
			std::fs::read(path).map_err(|e| error!("failed to read file \"{path}\": {e}"))?;
		info!(
			"loaded \"{}\", {} bytes, in {}ms",
			path,
			file.len(),
			t0.elapsed().as_secs_f32() * 1000f32
		);
		self.files.push((file, prefix.to_vec(), v));
		Ok(())
	}

	pub fn add_list(&mut self, list: impl IntoIterator<Item = impl AsRef<[u8]>>, v: u64) {
		let list: Vec<_> = list.into_iter().map(|e| e.as_ref().to_vec()).collect();
		self.lists.push((list, v));
	}

	pub fn build(self) -> DummyResult<DMap<Vec<u8>>> {
		let mut bytes = 0;
		// we have to sort them, not storing them all in memory is HARD
		// to prevent memory fragmentation from lots of String alloc
		// we read them in whole and use &str instead
		let t0 = Instant::now();
		let mut kv: BTreeMap<RevOrd, u64> = BTreeMap::new();
		for (file, prefix, v) in self.files.iter() {
			for mut line in file.split(is_newline) {
				line = line.trim_ascii();
				if line.is_empty() || line[0] == b'#' {
					continue;
				}
				if !prefix.is_empty() {
					if line.len() < prefix.len() || &line[..prefix.len()] != prefix {
						error!(
							"unexpected line, no prefix ({}): \"{}\"",
							str::from_utf8(prefix).unwrap(),
							str::from_utf8(line).unwrap()
						);
					}
					line = &line[prefix.len()..];
				}
				if line.len() > MAX_FQDN_LEN {
					error!(
						"unexpected line, too long ({}): \"{}\"",
						line.len(),
						str::from_utf8(line).unwrap()
					);
					continue;
				}
				bytes += line.len();
				kv.insert(RevOrd(line), *v);
			}
		}

		for list in self.lists.iter() {
			for k in list.0.iter() {
				bytes += k.len();
				kv.insert(RevOrd(k as &[u8]), list.1);
			}
		}

		let t1 = Instant::now();
		bytes += kv.len(); // counting delimiters
		info!(
			"parsed {} domains, {} bytes, in {:.1}ms",
			kv.len(),
			bytes,
			t1.duration_since(t0).as_secs_f32() * 1000f32
		);

		let mut b = Builder::memory();
		let mut rev = Vec::with_capacity(MAX_FQDN_LEN);
		for (&k, &v) in &kv {
			rev.clear();
			rev.extend(k.0.iter().rev().copied());
			b.insert(&rev, v)
				.map_err(|e| error!("fst insertion error on key \"{}\": {e}", Pretty(k.0)))?;
		}
		let t = b.into_fst();
		let t2 = Instant::now();
		info!(
			"built fst: {} bytes, ratio: {:.1}%, in {:.1}ms",
			t.size(),
			t.size() as f32 * 100f32 / bytes as f32,
			t2.duration_since(t1).as_secs_f32() * 1000f32
		);
		Ok(DMap(t))
	}
}

#[inline]
fn is_newline(b: &u8) -> bool {
	*b == b'\n'
}

// so we don't have to reverse the keys before inserting
#[derive(Clone, Copy, PartialEq, Eq)]
struct RevOrd<'a>(&'a [u8]);

impl<'a> PartialOrd for RevOrd<'a> {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}

impl<'a> Ord for RevOrd<'a> {
	fn cmp(&self, other: &Self) -> Ordering {
		self.0.iter().rev().cmp(other.0.iter().rev())
	}
}

#[cfg(test)]
mod tests {
	use std::{
		collections::HashMap,
		hash::{Hash as _, Hasher},
		io::BufRead as _,
	};

	use crate::dmap::DMapBuilder;

	use super::*;

	#[test]
	fn test_match() {
		let mut b = DMapBuilder::default();
		b.add_list([b"com"], 0);
		b.add_list([b"example.com"], 1);
		let m = b.build().unwrap();

		assert_eq!(m.get(b"com"), Some(0));
		assert_eq!(m.get(b"net"), None);
		assert_eq!(m.get(b"a.com"), Some(0));
		assert_eq!(m.get(b"acom"), None);
		assert_eq!(m.get(b"example.com"), Some(1));
		assert_eq!(m.get(b"sub.example.com"), Some(1));
		assert_eq!(m.get(b"notsubexample.com"), Some(0));
	}

	const DOMAIN_LST_FILE: &str = "etc/lists/domainswild";
	const DOMAIN_LST_PRE: &[u8] = b"*.";
	const QUERY_LST_FILE: &str = "etc/lists/queries-dedupe";

	#[test]
	#[ignore]
	fn test_build() {
		let mut b = DMapBuilder::default();
		mem();
		b.add_file(DOMAIN_LST_FILE, DOMAIN_LST_PRE, 0).unwrap();
		mem();
		let m = b.build().unwrap();
		mem();
		eprintln!("fst: {:.2} MB", m.0.size() as f32 / 1048576.0);
	}

	fn mem() {
		eprintln!(
			"mem: {:.2} MB",
			memory_stats::memory_stats().unwrap().physical_mem as f32 / 1048576.0
		);
	}

	// you probably want to test this with a release build since it's slow
	// cargo test --release test_match_lst_macro -- --ignored --no-capture
	#[test]
	#[ignore]
	fn test_match_lst_macro() {
		for &m in &[
			0xffffffffffffffff,
			0xffffffff,
			0x00ffffff,
			0x0000ffff,
			0x00000fff,
			0x000000ff,
			0x0000ff00,
			0x00ff0000,
			0xff000000,
			0x000000ff,
			0x0000000f,
			0x00000001,
		] {
			eprintln!("=== test with mask {m:x} ===");
			test_match_lst_with_mask(m);
		}
	}

	// test a list using hashmap as control
	fn test_match_lst_with_mask(mask: u64) {
		// build fst and control
		let mut b = DMapBuilder::default();
		let mut h = HashMap::new();
		let mut l = Vec::with_capacity(MAX_FQDN_LEN + 2);
		let mut r = std::io::BufReader::new(std::fs::File::open(DOMAIN_LST_FILE).unwrap());
		let mut c: usize = 0;
		while r.read_until(b'\n', &mut l).unwrap() > 0 {
			let mut n = l.trim_ascii();
			if n.is_empty() || n[0] == b'#' {
				trace!("skipped empty line / comment: {}", unsafe {
					str::from_utf8_unchecked(n)
				});
				l.clear();
				continue;
			}
			if &n[0..DOMAIN_LST_PRE.len()] != DOMAIN_LST_PRE {
				trace!("unexpected line: {:?}", unsafe {
					str::from_utf8_unchecked(n)
				});
				l.clear();
				continue;
			}
			n = &n[DOMAIN_LST_PRE.len()..];
			let v = {
				// derive v
				let mut h = std::hash::DefaultHasher::new();
				n.hash(&mut h);
				h.finish()
			} & mask;
			b.add_list([n], v);
			h.insert(n.to_vec(), v);
			c += 1;
			l.clear();
		}
		eprintln!("{} domains loaded", c);
		let t = b.build().unwrap();

		// test
		r = std::io::BufReader::new(std::fs::File::open(QUERY_LST_FILE).unwrap());
		c = 0;
		let mut hits: usize = 0;
		while r.read_until(b'\n', &mut l).unwrap() > 0 {
			let n = l.trim_ascii();
			if n.is_empty() {
				l.clear();
				continue;
			}
			let expected = subdomain_get(&h, n);
			assert_eq!(t.get(n), expected);
			if expected.is_some() {
				hits += 1;
			}
			c += 1;
			l.clear();
		}
		eprintln!("{} tests conducted, {} hits", c, hits);
	}

	fn subdomain_get(h: &HashMap<Vec<u8>, u64>, mut n: &[u8]) -> Option<u64> {
		loop {
			if let Some(r) = h.get(n) {
				return Some(*r);
			}
			let p = n.iter().position(|b| *b == b'.')?;
			n = &n[p + 1..]
		}
	}
}
