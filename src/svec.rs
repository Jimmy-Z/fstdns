// short Vec to avoid allocation, in some cases

use std::num::NonZeroU8;

// beware, since the usage of NonZero, len is internally presented +1
const MAX_CAP: usize = NonZeroU8::MAX.get() as usize - 1;

#[derive(Clone, Debug, Hash)]
pub enum SVec<T: Copy, const C: usize> {
	Int(([T; C], NonZeroU8)),
	Ext(Vec<T>),
}

impl<T: Copy + Default, const C: usize> SVec<T, C> {
	fn inner_from_slice(v: &[T]) -> Self {
		#[cfg(debug_assertions)]
		assert!(v.len() <= C);
		let mut a = [T::default(); C];
		a[..v.len()].copy_from_slice(v);
		Self::Int((a, unsafe { NonZeroU8::new_unchecked(v.len() as u8 + 1) }))
	}

	pub fn len(&self) -> usize {
		match self {
			SVec::Int((_, l)) => l.get() as usize - 1,
			SVec::Ext(v) => v.len(),
		}
	}

	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}

	pub fn capacity(&self) -> usize {
		match self {
			SVec::Int(_) => C,
			SVec::Ext(v) => v.capacity(),
		}
	}

	pub fn push(&mut self, t: T) {
		match self {
			Self::Int(a) => {
				let len = a.1.get() - 1;
				if (len as usize) < C {
					a.0[len as usize] = t;
					a.1 = unsafe { NonZeroU8::new_unchecked(len + 2) };
				} else {
					let mut v = Vec::with_capacity(len as usize + 1);
					v.extend_from_slice(&a.0[..len as usize]);
					v.push(t);
					*self = Self::Ext(v);
				}
			}
			Self::Ext(v) => v.push(t),
		}
	}
	pub fn extend_from_slice(&mut self, s: &[T]) {
		match self {
			Self::Int(a) => {
				let len = a.1.get() - 1;
				if (len as usize) + s.len() <= C {
					a.0[len as usize..len as usize + s.len()].copy_from_slice(s);
					a.1 = unsafe { NonZeroU8::new_unchecked(len + s.len() as u8 + 1) };
				} else {
					let mut v = Vec::with_capacity(len as usize + 1);
					v.extend_from_slice(&a.0[..len as usize]);
					v.extend_from_slice(s);
					*self = Self::Ext(v);
				}
			}
			Self::Ext(v) => v.extend_from_slice(s),
		}
	}
}

// impl<T: Copy + Default, const C: usize> Default for SVec<T, C> {
// 	fn default() -> Self {
// 		#[cfg(debug_assertions)]
// 		assert!(C <= NonZeroU8::MAX.get() as usize - 1);
// 		Self::Int(([T::default(); C], unsafe { NonZeroU8::new_unchecked(1) }))
// 	}
// }

impl<T: Copy, const C: usize> AsRef<[T]> for SVec<T, C> {
	fn as_ref(&self) -> &[T] {
		match self {
			SVec::Int((a, l)) => &a[..(*l).get() as usize - 1],
			SVec::Ext(v) => v,
		}
	}
}

impl<T: Copy + Default, const C: usize> From<&[T]> for SVec<T, C> {
	fn from(v: &[T]) -> Self {
		#[cfg(debug_assertions)]
		assert!(C <= MAX_CAP);
		if v.len() <= C {
			Self::inner_from_slice(v)
		} else {
			Self::Ext(v.to_vec())
		}
	}
}

impl<T: Copy + Default, const C: usize> From<Vec<T>> for SVec<T, C> {
	fn from(v: Vec<T>) -> Self {
		#[cfg(debug_assertions)]
		assert!(C <= MAX_CAP);
		if v.len() <= C {
			Self::inner_from_slice(&v)
		} else {
			Self::Ext(v)
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::{
		fs::File,
		io::{BufRead, BufReader},
		mem::size_of,
		num::NonZeroU8,
	};

	#[test]
	fn svec_size() {
		println!("Option<u8>: {}", size_of::<Option<u8>>());
		println!("Option<NonZeroU8>: {}", size_of::<Option<NonZeroU8>>());
		println!("Vec<u8>: {}", size_of::<Vec<u8>>());
		println!("Option<Vec<u8>>: {}", size_of::<Option<Vec<u8>>>());

		// compact printing for many const-generic sizes
		macro_rules! print_svec_sizes {
			($ty:ty; $($n:expr),+ $(,)?) => {
				$(println!(
					concat!("SVec<", stringify!($ty), ", ", stringify!($n), ">: {}"),
					size_of::<SVec<$ty, $n>>()
				);)+
			};
		}

		print_svec_sizes!(u8; 15, 16, 23, 31, 35, 39, 47, 55, 63);
	}

	// tough choice
	// === etc/lists/queries-dedupe ===
	// 3517 total names
	//  31,    2198,  62.5%
	//  39,    2716,  77.2%
	//  47,    3130,  89.0%
	//  55,    3353,  95.3%
	//  63,    3485,  99.1%
	// === etc/lists/queries ===
	// 506810 total names
	//  31,  266896,  52.7%
	//  39,  384498,  75.9%
	//  47,  488979,  96.5%
	//  55,  501117,  98.9%
	//  63,  506018,  99.8%
	#[test]
	#[ignore]
	fn query_len_stats() {
		for &(p, t) in &[
			("etc/lists/queries-dedupe", true),
			("etc/lists/queries", false),
		] {
			eprintln!("=== {} ===", p);
			inner_query_len_stats(p, t);
		}
	}

	fn inner_query_len_stats(path: &str, test_svec: bool) {
		let mut stats = [0; 0x100];
		let mut buf = Vec::with_capacity(0x100);
		let mut r = BufReader::new(File::open(path).unwrap());
		let mut total = 0;
		while r.read_until(b'\n', &mut buf).unwrap() > 0 {
			stats[buf.len()] += 1;
			total += 1;
			if test_svec {
				const TEST_SVEC_C: usize = 63;
				let a: SVec<u8, TEST_SVEC_C> = SVec::from(buf.as_ref());
				match &a {
					SVec::Int(_) => assert!(buf.len() <= TEST_SVEC_C),
					SVec::Ext(_) => assert!(buf.len() > TEST_SVEC_C),
				}
				assert_eq!(a.as_ref(), &buf);
			}
			buf.clear();
		}
		eprintln!("{} total names", total);
		let mut acc = 0;
		for (i, c) in stats.iter().enumerate() {
			acc += *c;
			if [15usize, 31, 39, 47, 55, 63].contains(&i) {
				println!(
					"{:>3}, {:>7}, {:>5.1}%",
					i,
					acc,
					acc as f32 * 100f32 / total as f32
				)
			}
		}
	}
}
