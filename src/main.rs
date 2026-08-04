use std::{
	fs::File,
	net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
	rc::Rc,
};

use itertools::Itertools as _;
use log::*;
use tokio::{net::UdpSocket, time::timeout};

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

	conf.finalize();

	let s = UdpSocket::bind(conf.listen)
		.await
		.inspect_err(|e| error!("failed to listen on {}: {e}", conf.listen))?;
	info!("listening on UDP {}", s.local_addr()?);

	let s = Rc::new(s);
	let c = Rc::new(conf);
	let mut buf = Vec::with_capacity(MSG_BUF_LEN_DEF);
	loop {
		buf.clear();
		let (len, addr) = s.recv_buf_from(&mut buf).await?;
		debug!("{len} bytes from {addr}:\n{:?}", Pretty(&buf[..]));
		// this is not spawn
		// since some queries are handled directly by rule in memory
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
		return;
	}
	let mut msg = msg.unwrap();
	let q = msg.get_query();
	if let Err(e) = q {
		warn!("error parsing query: {e:?}\n{:?}", Pretty(buf.as_slice()));
		return;
	}
	let mut q = q.unwrap();
	let action = 'blk: {
		// to do: chaos
		if q.qclass != QClass::IN {
			break 'blk ActionId::RCode(RCode::NOTIMP);
		}
		// exact rules
		// yeah looks quirky but the other option is to pull in hashbrown::Equivalent
		let k = (q.name, q.qtype);
		if let Some(a) = conf.exact_rules.get(&k) {
			break 'blk *a;
		}
		// gives back the moved member, hopefully this gets optimized out
		q.name = k.0;
		// unqualified
		if let Some(a) = conf.unqualified_rule
			&& unqualified(q.name.as_ref())
		{
			break 'blk a;
		}
		// qtype rules
		if let Ok(i) = conf.qtype_rules.binary_search_by_key(&q.qtype, |k| k.0) {
			break 'blk conf.qtype_rules[i].1;
		}
		// domain rules
		if let Some(v) = dmap.get(q.name.as_ref()) {
			if let Ok(a) = ActionId::try_from(v) {
				break 'blk a;
			} else {
				error!("");
				break 'blk ActionId::RCode(RCode::SERVFAIL);
			}
		}
		ActionId::Default
	};
	let upstream = match action {
		// to do: random or round robin
		ActionId::Default => conf.default.clone(),
		ActionId::Alt(i) => conf.alts[i as usize].clone(),
		ActionId::RCode(c) => {
			msg.deny(c);
			if let Err(e) = s.send_to(buf, addr).await {
				warn!("error sending response to {addr}: {e}");
			}
			return;
		}
		ActionId::Rewrite(i) => {
			msg.answer(&conf.rewrites[i as usize]);
			if let Err(e) = s.send_to(buf, addr).await {
				warn!("error sending response to {addr}: {e}");
			}
			return;
		}
	};
	let s = s.clone();
	let b = buf.clone();
	let c = conf.clone(); // conf is required for addr rules
	tokio::task::spawn_local(handle_upstream(s, addr, b, upstream, c));
}

const DEFAULT_BIND_V4: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0);
const DEFAULT_BIND_V6: SocketAddr = SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0);

async fn handle_upstream(
	c: Rc<UdpSocket>,
	c_addr: SocketAddr,
	query: Vec<u8>,
	upstream: Vec<SocketAddr>,
	conf: Rc<Conf>,
) -> Result<(), ()> {
	// to do: choose address randomly and retry on others
	let u_addr = upstream[0];
	let u = UdpSocket::bind(match u_addr {
		SocketAddr::V4(_) => DEFAULT_BIND_V4,
		SocketAddr::V6(_) => DEFAULT_BIND_V6,
	})
	.await
	.map_err(|e| error!("error binding udp socket for upstream: {e}"))?;
	u.connect(u_addr)
		.await
		.map_err(|e| error!("error connecting to upstream: {e}"))?;
	u.send(&query)
		.await
		.map_err(|e| error!("error sending query to upstream: {e}"))?;

	let mut answer = Vec::with_capacity(MSG_BUF_LEN_DEF);
	match timeout(conf.timeout, u.recv_buf(&mut answer)).await {
		Ok(Ok(len)) => trace!("{len} bytes from upstream"),
		Ok(Err(e)) => {
			warn!("error receiving answer from upstream: {e}");
			return Err(());
		}
		Err(e) => {
			warn!("timeout waiting for upstream: {e}");
			return Err(());
		}
	}

	// to do: addr rule

	c.send_to(&answer, c_addr)
		.await
		.map_err(|e| error!("error sending answer back to client: {e}"))?;

	Ok(())
}

fn unqualified(n: &[u8]) -> bool {
	!n.contains(&b'.')
}
