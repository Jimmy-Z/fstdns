use std::fs::File;

use itertools::Itertools as _;
use log::*;

use fstdns::{conf::Conf, dmap::DMapBuilder, *};
use misc::*;
use tokio::net::UdpSocket;

#[cfg(debug_assertions)]
const DEBUG_CONF_PATH: &str = "etc/conf";

#[tokio::main(flavor = "local")]
async fn main() -> Dummy {
	init_env_logger();
	let args: Vec<_> = std::env::args().take(3).collect();
	let mut conf = Conf::default();
	let mut dmap = DMapBuilder::default();
	match args.len() {
		1 => {
			#[cfg(debug_assertions)]
			conf.conf(
				&mut dmap,
				File::open(DEBUG_CONF_PATH)
					.map_err(|e| error!("failed to open \"{DEBUG_CONF_PATH}\": {e}"))?,
			);
			#[cfg(not(debug_assertions))]
			return Err(().into());
		}
		2 => {
			conf.conf(
				&mut dmap,
				File::open(&args[1] as &str)
					.map_err(|e| error!("failed to open \"{}\": {e}", &args[1] as &str))?,
			);
		}
		_ => {
			error!("too many arguments");
			return Err(().into());
		}
	}

	let _dmap = dmap.build()?;

	if conf.default.is_empty() {
		error!("default upstream not configured");
		return Err(().into());
	} else {
		info!("default upstream: {}", conf.default.iter().join(", "));
	}

	let s = UdpSocket::bind(conf.listen)
		.await
		.inspect_err(|e| error!("failed to listen on {}: {e}", conf.listen))?;
	info!("listening on UDP {}", s.local_addr()?);

	let mut buf = Vec::with_capacity(0x600);
	loop {
		buf.clear();
		let r = s.recv_buf_from(&mut buf).await?;
		eprintln!("r: {:?}, buf: \n{:?}", r, Pretty(&buf[..]));
	}
}
