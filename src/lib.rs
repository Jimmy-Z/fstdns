pub mod action;
pub mod conf;
pub mod dmap;
pub mod exact;
pub mod hosts;
pub mod svec;

pub type DName = svec::SVec<u8, 63>;
