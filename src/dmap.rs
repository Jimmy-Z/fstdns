// domain map using fst, a demonstration

use std::{cmp::Ordering, time::Instant};

use fst::{
	Result,
	raw::{Builder, Fst, Output},
};

pub struct DMap<D>(Fst<D>);

const MAX_FQDN_LEN: usize = 253;

impl<D: AsRef<[u8]>> DMap<D> {
	pub fn get(&self, n: &str) -> Option<u64> {
		let b = n.as_bytes();
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
					eprintln!(
						"found match for \"{}\", {} at pos {} ({})",
						n,
						last_match.unwrap(),
						i,
						&n[i..] // str slicing is byte positioned
					);
				}
			} else {
				break;
			}
		}
		last_match
	}
}

pub struct DMapBuilder {
	files: Vec<(Vec<u8>, Vec<u8>, u64)>,
	lists: Vec<(Vec<Vec<u8>>, u64)>,
	count_hint: usize,
}

impl DMapBuilder {
	pub fn new() -> Self {
		Self {
			files: Vec::new(),
			lists: Vec::new(),
			count_hint: 0,
		}
	}
	pub fn add_file(&mut self, path: &str, prefix: &[u8], v: u64) -> Result<()> {
		let t0 = Instant::now();
		let file = std::fs::read(path)?;
		eprintln!(
			"loaded \"{}\", {} bytes, in {}ms",
			path,
			file.len(),
			t0.elapsed().as_secs_f32() * 1000f32
		);
		self.count_hint += file.len() / MAX_FQDN_LEN;
		self.files.push((file, prefix.to_vec(), v));
		Ok(())
	}

	pub fn add_list(&mut self, list: impl IntoIterator<Item = impl AsRef<[u8]>>, v: u64) {
		let list: Vec<_> = list.into_iter().map(|e| e.as_ref().to_vec()).collect();
		self.count_hint += list.len();
		self.lists.push((list, v));
	}

	pub fn build(self) -> Result<DMap<Vec<u8>>> {
		let mut bytes = 0;
		// we have to sort them, not storing them all in memory is HARD
		// to prevent memory fragmentation from lots of String alloc
		// we read them in whole and use &str instead
		let t0 = Instant::now();
		let mut kv: Vec<(&[u8], u64)> = Vec::with_capacity(self.count_hint);
		for (file, prefix, v) in self.files.iter() {
			for mut line in file.split(is_newline) {
				line = line.trim_ascii();
				if line.is_empty() || line[0] == b'#' {
					continue;
				}
				if !prefix.is_empty() {
					if line.len() < prefix.len() || &line[..prefix.len()] != prefix {
						eprintln!(
							"unexpected line, no prefix ({}): \"{}\"",
							str::from_utf8(&prefix).unwrap(),
							str::from_utf8(line).unwrap()
						);
					}
					line = &line[prefix.len()..];
				}
				if line.len() > MAX_FQDN_LEN {
					eprintln!(
						"unexpected line, too long ({}): \"{}\"",
						line.len(),
						str::from_utf8(line).unwrap()
					);
					continue;
				}
				bytes += line.len();
				kv.push((line, *v));
			}
		}

		for list in self.lists.iter() {
			for k in list.0.iter() {
				bytes += k.len();
				kv.push((k as &[u8], list.1));
			}
		}

		let t1 = Instant::now();
		bytes += kv.len(); // counting delimiters
		eprintln!(
			"parsed {} domains, {} bytes, in {:.1}ms",
			kv.len(),
			bytes,
			t1.duration_since(t0).as_secs_f32() * 1000f32
		);

		kv.sort_by(|&a, &b| rev_cmp(a.0, b.0));
		let t2 = Instant::now();
		eprintln!(
			"sorted in {:.1}ms",
			t2.duration_since(t1).as_secs_f32() * 1000f32
		);

		let mut b = Builder::memory();
		let mut rev = Vec::with_capacity(MAX_FQDN_LEN);
		for (k, v) in kv.into_iter() {
			rev.clear();
			rev.extend(k.iter().rev().copied());
			b.insert(&rev, v)?;
		}
		let t = b.into_fst();
		let t3 = Instant::now();
		eprintln!(
			"built fst: {} bytes, ratio: {:.1}%, in {:.1}ms",
			t.size(),
			t.size() as f32 * 100f32 / bytes as f32,
			t3.duration_since(t2).as_secs_f32() * 1000f32
		);
		Ok(DMap(t))
	}
}

#[inline]
fn is_newline(b: &u8) -> bool {
	*b == b'\n'
}

#[inline]
fn rev_cmp(a: &[u8], b: &[u8]) -> Ordering {
	a.iter().rev().cmp(b.iter().rev())
}

#[cfg(test)]
mod tests {
	use crate::dmap::DMapBuilder;

	use super::*;

	#[test]
	fn test_match() {
		let mut b = DMapBuilder::new();
		b.add_list(&[b"com"], 0);
		b.add_list(&[b"example.com"], 1);
		let m = b.build().unwrap();

		assert_eq!(m.get("com"), Some(0));
		assert_eq!(m.get("net"), None);
		assert_eq!(m.get("a.com"), Some(0));
		assert_eq!(m.get("acom"), None);
		assert_eq!(m.get("example.com"), Some(1));
		assert_eq!(m.get("sub.example.com"), Some(1));
		assert_eq!(m.get("notsubexample.com"), Some(0));
	}

	#[test]
	fn test_build() {
		let mut b = DMapBuilder::new();
		b.add_file("lst/domainswild", b"*.", 0).unwrap();
		let _ = b.build().unwrap();
	}
}
