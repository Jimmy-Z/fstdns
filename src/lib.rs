pub mod action;
pub mod conf;
pub mod dmap;
pub mod exact;
pub mod hosts;

// just to make ? work
// we don't generate any meaningful errors
// since every error in the conf stage is fatal
// every error in the demon stage is io
pub struct DummyError();
pub type DummyResult<T> = std::result::Result<T, DummyError>;
pub type Dummy = DummyResult<()>;

impl From<()> for DummyError {
	fn from(_: ()) -> Self {
		Self()
	}
}

impl From<fst::Error> for DummyError {
	fn from(_: fst::Error) -> Self {
		Self()
	}
}

impl From<std::io::Error> for DummyError {
	fn from(_: std::io::Error) -> Self {
		Self()
	}
}

// for what ever reason this is not allowed
// impl<T> From<T> for Error {
// 	fn from(_v: ()) -> Self {
// 		Self()
// 	}
// }

// this is also not allowed
// impl<T> Into<Error> for T {
// 	fn into(_v: ()) -> Error {
// 		Error()
// 	}
// }

impl std::fmt::Display for DummyError {
	fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		unreachable!()
	}
}

impl std::fmt::Debug for DummyError {
	fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		unreachable!()
	}
}

impl std::error::Error for DummyError {}
