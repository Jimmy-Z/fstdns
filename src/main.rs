use std::{
	fs::File,
	net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
	rc::Rc,
	time::Duration,
};

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
	info!("{}", env!("REV"));

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
	// glibc is lazy on free
	#[cfg(all(target_os = "linux", target_env = "gnu"))]
	unsafe {
		libc::malloc_trim(0x1000);
	}

	if conf.default.is_empty() {
		error!("default upstream not configured");
		return Err(().into());
	} else {
		info!("default upstream: {}", conf.default.iter().join(" "));
	}

	conf.finalize();

	let s = udp_bind(conf.listen)?;
	info!("listening on UDP {}", s.local_addr()?);

	let s = Rc::new(s);
	let c = Rc::new(conf);
	let mut buf = Vec::with_capacity(MSG_BUF_LEN_DEF);
	loop {
		buf.clear();
		let (len, addr) = s.recv_buf_from(&mut buf).await?;
		trace!("{len} bytes from {addr}:\n{:?}", Pretty(&buf[..]));
		// this is not spawn
		// since some queries are handled directly by rule in memory
		handle(&s, addr, &mut buf, &c, &dmap).await;
	}
}

fn udp_bind(addr: SocketAddr) -> Result<UdpSocket, ()> {
	use socket2::{Domain, Protocol, Socket, Type};

	let socket = Socket::new(Domain::for_address(addr), Type::DGRAM, Some(Protocol::UDP))
		.map_err(|e| error!("error creating socket: {e}"))?;
	if addr.is_ipv6() && addr.ip() == IpAddr::V6(Ipv6Addr::UNSPECIFIED) {
		socket
			.set_only_v6(false)
			.map_err(|e| warn!("error disabling IPV6_V6ONLY: {e}"))?;
	}
	socket
		.set_reuse_address(true)
		.map_err(|e| warn!("error setting SO_REUSEADDR: {e}"))?;
	socket
		.set_nonblocking(true)
		.map_err(|e| warn!("error setting socket to non-blocking: {e}"))?;
	socket
		.bind(&addr.into())
		.map_err(|e| error!("error binding udp socket: {e}"))?;
	UdpSocket::from_std(socket.into()).map_err(|e| error!("error converting socket to tokio: {e}"))
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
	trace!("{msg}");
	debug!("{q}");
	let action = 'rules: {
		// to do: chaos
		if q.qclass == QClass::CH {
			handle_chaos(s, addr, buf, conf).await;
			return;
		} else if q.qclass != QClass::IN {
			break 'rules ActionId::RCode(RCode::NOTIMP);
		}
		// exact rules
		// yeah looks quirky but the other option is to pull in hashbrown::Equivalent
		let k = (q.name, q.qtype);
		if let Some(a) = conf.exact_rules.get(&k) {
			debug!("exact rule: {a}");
			break 'rules *a;
		}
		// gives back the moved member, hopefully this gets optimized out
		q.name = k.0;
		// unqualified
		if let Some(a) = conf.unqualified_rule
			&& unqualified(q.name.as_ref())
		{
			debug!("unqualified rule: {a}");
			break 'rules a;
		}
		// qtype rules
		if let Ok(i) = conf.qtype_rules.binary_search_by_key(&q.qtype, |k| k.0) {
			let a = conf.qtype_rules[i].1;
			debug!("qtype rule: {a}");
			break 'rules a;
		}
		// domain rules
		if let Some(v) = dmap.get(q.name.as_ref()) {
			if let Ok(a) = ActionId::try_from(v) {
				debug!("domain map rule: {a}");
				break 'rules a;
			} else {
				error!("dmap returns 0x{v:x}, not a valid action");
				break 'rules ActionId::RCode(RCode::SERVFAIL);
			}
		}
		// runtime rules
		if let Some(a) = conf.rt_name_rules.borrow().get(&q.name) {
			debug!("runtime rule: {a}");
			break 'rules *a;
		}
		debug!("default");
		ActionId::Default
	};
	if let Some(upstream_id) = action.to_upstream_id() {
		let s = s.clone();
		let b = buf.clone();
		let c = conf.clone(); // conf is required for addr rules
		tokio::task::spawn_local(handle_upstream(s, addr, b, upstream_id, c));
	} else {
		handle_local_action(&mut msg, action, conf);
		if let Err(e) = s.send_to(buf, addr).await {
			warn!("error sending response to {addr}: {e}");
		}
	}
}

async fn handle_chaos(s: &Rc<UdpSocket>, addr: SocketAddr, buf: &mut Vec<u8>, conf: &Rc<Conf>) {
	let mut msg = Msg::try_from(&mut *buf).unwrap();
	let q = msg.get_query().unwrap();
	if q.name.as_ref() == b"runtime.rules" && q.qtype == QType::TXT {
		let mut ans = Vec::new();
		for (k, v) in conf.rt_name_rules.borrow().iter() {
			let k = str::from_utf8(k.as_ref()).unwrap();
			let v = &format!("{v}");
			ans.push(Answer {
				qtype: QType::TXT,
				qclass: QClass::CH,
				ttl: 42,
				rdata: RData::Raw(CVec63::txt(&[k, v])),
			});
		}
		msg.answer(&ans);
	} else {
		msg.deny(RCode::NOTIMP);
	}
	if let Err(e) = s.send_to(buf, addr).await {
		warn!("error sending response to {addr}: {e}");
	}
}

fn handle_local_action(msg: &mut Msg, action: ActionId, conf: &Rc<Conf>) {
	match action {
		ActionId::RCode(c) => {
			msg.deny(c);
		}
		ActionId::Rewrite(i) => {
			msg.answer(&conf.rewrites[i as usize]);
		}
		_ => unreachable!(),
	}
}

async fn handle_upstream(
	c: Rc<UdpSocket>,
	c_addr: SocketAddr,
	mut query: Vec<u8>,
	upstream_id: Option<u8>,
	conf: Rc<Conf>,
) -> Result<(), ()> {
	let mut answer = Vec::with_capacity(0x600);
	// to do: shuffle upstream
	handle_upstream_inner(&mut answer, &mut query, upstream_id, &conf).await;
	if answer.is_empty() {
		return Err(());
	}

	match Msg::try_from(&mut answer) {
		Ok(mut msg) => {
			// addr rule
			let _ = msg
				.get_query()
				.map_err(|e| warn!("invalid query in response: {e:?}"))?;
			let mut action = None;
			for _ in 0..msg.an_count() {
				let answer = msg
					.next_answer()
					.map_err(|e| warn!("invalid answer in response: {e:?}"))?;
				if let Some(addr) = match answer.rdata {
					RData::A(a) => Some(IpAddr::V4(a)),
					RData::AAAA(aaaa) => Some(IpAddr::V6(aaaa)),
					_ => None,
				} && let Some(a) = conf.addr_rules.get(&addr)
				{
					action = Some(*a);
					break;
				}
			}
			if let Some(action) = action {
				if let Some(upstream_id) = action.to_upstream_id() {
					// save it as a runtime rule
					{
						// had to parse it again since passing it alone is a PITA
						let n = get_q(&mut query).name;
						debug!("answer for \"{n}\" hit addr rule: {action}");
						conf.rt_name_rules.borrow_mut().insert(n, action);
					}
					answer.clear();
					handle_upstream_inner(&mut answer, &mut query, upstream_id, &conf).await;
				} else {
					error!("action {action} for answer addr condition is not implemented yet");
					return Ok(());
				}
			}
		}
		Err(MsgError::Truncated) => {
			// this is normal
		}
		Err(e) => {
			warn!(
				"invalid header in response for {}: {e:?}",
				get_q(&mut query)
			);
		}
	}

	if !answer.is_empty() {
		c.send_to(&answer, c_addr)
			.await
			.map_err(|e| error!("error sending answer back to client: {e}"))?;
	}
	Ok(())
}

// this is used to re-retrieve info on previously parsed buf
fn get_q(buf: &mut Vec<u8>) -> Query {
	let mut msg = Msg::try_from(buf).unwrap();
	msg.get_query().unwrap()
}

async fn handle_upstream_inner(
	answer: &mut Vec<u8>,
	query: &mut Vec<u8>,
	upstream_id: Option<u8>,
	conf: &Rc<Conf>,
) {
	let upstream = match upstream_id {
		Some(i) => &conf.alts[i as usize],
		None => &conf.default,
	};
	// to do: upstream stats
	// to do: choice based on stats
	for &u in upstream.iter() {
		let r = handle_upstream_inner_inner(answer, query, u, conf.timeout).await;
		match r {
			Ok(()) => break,
			Err(true) => continue,
			Err(false) => break,
		}
	}
}

const DEFAULT_BIND_V4: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0);
const DEFAULT_BIND_V6: SocketAddr = SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0);

// the error indicates should retry (next address) or not
async fn handle_upstream_inner_inner(
	answer: &mut Vec<u8>,
	query: &mut Vec<u8>,
	upstream: SocketAddr,
	timeout: Duration,
) -> Result<(), bool> {
	let u = UdpSocket::bind(match upstream {
		SocketAddr::V4(_) => DEFAULT_BIND_V4,
		SocketAddr::V6(_) => DEFAULT_BIND_V6,
	})
	.await
	.map_err(|e| {
		error!("error binding udp socket for upstream: {e}");
		false
	})?;
	u.connect(upstream).await.map_err(|e| {
		error!("error connecting to upstream: {e}");
		false
	})?;
	u.send(query).await.map_err(|e| {
		error!("error sending request to upstream: {e}");
		false
	})?;

	match tokio::time::timeout(timeout, u.recv_buf(answer)).await {
		Ok(Ok(len)) => trace!("{len} bytes from upstream"),
		Ok(Err(e)) => {
			warn!("error receiving response from upstream: {e}");
			return Err(true);
		}
		Err(e) => {
			warn!("timeout waiting upstream for {}: {e}", get_q(query));
			return Err(true);
		}
	}
	Ok(())
}

fn unqualified(n: &[u8]) -> bool {
	!n.contains(&b'.')
}
