// short Vec to avoid allocation, in some cases

#[derive(Clone, Debug)]
pub enum SVec<T: Copy, const C: usize> {
	Int(([T; C], u8)),
	Ext(Vec<T>),
}

impl<T: Copy + Default, const C: usize> SVec<T, C> {
	// assumes v.len() <= C
	fn inner_from_slice(v: &[T]) -> Self {
		let mut a = [T::default(); C];
		(&mut a[..v.len()]).copy_from_slice(v);
		Self::Int((a, v.len() as u8))
	}
}

impl<T: Copy + Default, const C: usize> Default for SVec<T, C> {
	fn default() -> Self {
		Self::Int(([T::default(); C], 0))
	}
}

impl<T: Copy, const C: usize> AsRef<[T]> for SVec<T, C> {
	fn as_ref(&self) -> &[T] {
		match self {
			SVec::Int((a, l)) => &a[..*l as usize],
			SVec::Ext(v) => v,
		}
	}
}

impl<T: Copy + Default, const C: usize> From<&[T]> for SVec<T, C> {
	fn from(v: &[T]) -> Self {
		if v.len() < C {
			Self::inner_from_slice(v)
		} else {
			Self::Ext(v.to_vec())
		}
	}
}

impl<T: Copy + Default, const C: usize> From<Vec<T>> for SVec<T, C> {
	fn from(v: Vec<T>) -> Self {
		if v.len() < C {
			Self::inner_from_slice(&v)
		} else {
			Self::Ext(v)
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn svec_size() {}
}
