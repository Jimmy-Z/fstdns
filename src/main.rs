use std::{collections::HashMap, fs::File, net::SocketAddr, rc::Rc};

use itertools::Itertools as _;
use log::*;
use tokio::net::UdpSocket;

use dns::*;
use misc::*;

use fstdns::{
	action::ActionId,
	conf::Conf,
	dmap::{DMap, DMapBuilder},
	*,
};

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

	let dmap = dmap.build()?;

	if conf.default.is_empty() {
		error!("default upstream not configured");
		return Err(().into());
	} else {
		info!("default upstream: {}", conf.default.iter().join(" "));
	}

	let s = UdpSocket::bind(conf.listen)
		.await
		.inspect_err(|e| error!("failed to listen on {}: {e}", conf.listen))?;
	info!("listening on UDP {}", s.local_addr()?);

	let s = Rc::new(s);
	let c = Rc::new(conf);
	let mut buf = Vec::with_capacity(MSG_BUF_LEN_DEF);
	loop {
		let (len, addr) = s.recv_buf_from(&mut buf).await?;
		eprintln!("{len} bytes, {addr}, buf: \n{:?}", Pretty(&buf[..]));
		// this is not spawn
		// since some queries are handled entirely by rule in memory
		handle(&s, addr, &mut buf, &c, &dmap).await;
	}
}

async fn handle(
	s: &Rc<UdpSocket>,
	addr: SocketAddr,
	buf: &mut Vec<u8>,
	conf: &Rc<Conf>,
	dmap: &DMap<Vec<u8>>,
) {
	let msg = Msg::try_from(&mut *buf);
	if let Err(e) = msg {
		warn!(
			"error parsing message header: {e:?}\n{:?}",
			Pretty(buf.as_slice())
		);
		buf.clear();
		return;
	}
	let mut msg = msg.unwrap();
	let q = msg.get_query();
	if let Err(e) = q {
		warn!("error parsing query: {e:?}\n{:?}", Pretty(buf.as_slice()));
		buf.clear();
		return;
	}
	let q = q.unwrap();
	let action = {
		// exact rules
		if let Some(a) = conf.exact_rules.get(&(q.name, q.qtype)) {
			*a
			// } else if let Some(a) = conf.qtype_rules.get(q.qtype) {
			// 	return handle_action(s, addr, msg, buf, a, conf, dmap).await;
		} else {
			// to do: other rules
			ActionId::Default
		}
	};
	let upstream = match action {
		// to do: random or round robin
		ActionId::Default => conf.default[0],
		ActionId::Alt(i) => conf.alts[i as usize][0],
		a => {
			let rcode = match a {
				ActionId::NotImp => RCode::NOTIMP,
				ActionId::NxDomain => RCode::NXDOMAIN,
				ActionId::Refused => RCode::REFUSED,
				_ => unreachable!(),
			};
			msg.deny(rcode);
			if let Err(e) = s.send_to(buf, addr).await {
				warn!("error sending response to {addr}: {e}");
			}
			return;
		}
	};
	let s = s.clone();
	let b = buf.clone();
	let c = conf.clone();
	tokio::task::spawn_local(handle_upstream(s, b, upstream, c));
	buf.clear();
}

async fn handle_upstream(s: Rc<UdpSocket>, buf: Vec<u8>, upstream: SocketAddr, conf: Rc<Conf>) {}
