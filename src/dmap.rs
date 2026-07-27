// domain map using fst, a demonstration

use std::{cmp::Ordering, time::Instant};

use fst::{raw::{Builder, Fst, Output}, Result};

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

// implement a proper builder style api would require massive effort on lifetime annotation
impl DMap<Vec<u8>> {
	pub fn new(
		file_list: &[(&str, Option<&[u8]>, u64)],
		plain_lists: &[(&[&[u8]], u64)],
		capacity: usize,
	) -> Result<Self> {
		let mut bytes = 0;
		// we have to sort them, not storing them all in memory is HARD
		// to prevent memory fragmentation from lots of String alloc
		// we read them in whole and use &str instead
		let t0 = Instant::now();
		let files: Vec<Vec<u8>> = file_list
			.iter()
			.map(|e| std::fs::read(e.0))
			.collect::<std::io::Result<Vec<_>>>()?;
		let mut kv: Vec<(&[u8], u64)> = Vec::with_capacity(capacity);
		for i in 0..file_list.len() {
			let prefix = file_list[i].1;
			let v = file_list[i].2;
			for mut line in files[i].split(is_newline) {
				line = line.trim_ascii();
				if line.is_empty() || line[0] == b'#' {
					continue;
				}
				if let Some(prefix) = prefix {
					if line.len() < prefix.len() || &line[..prefix.len()] != prefix {
						eprintln!(
							"unexpected line, no prefix ({}): \"{}\"",
							str::from_utf8(prefix).unwrap(),
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
				kv.push((line, v));
			}
		}

		for &list in plain_lists {
			for &k in list.0 {
				bytes += k.len();
				kv.push((k, list.1));
			}
		}

		let t1 = Instant::now();
		bytes += kv.len(); // counting delimiters
		eprintln!(
			"loaded {} domains, {} bytes, in {:.1}ms",
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
			rev.resize(0, 0);
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
	use super::DMap;

	#[test]
	fn test_match() {
		let m = DMap::new(&[], &[(&[b"com"], 0), (&[b"example.com"], 1)], 0x10).unwrap();

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
		let _ = DMap::new(&[("./lst/domainswild", Some(b"*."), 0)], &[], 0x1000);
	}
}
