
use std:: {fs::File, io::stdin};

use itertools::Itertools as _;

use fstdns::{conf::Conf, dmap::{DMapBuilder, Result}};

const DEFAULT_CONF_PATH: &str = "etc/conf";

fn main() -> Result<()> {
	let args: Vec<_> = std::env::args().take(2).collect();
	let mut conf = Conf::default();
	let mut builder = DMapBuilder::default();
	if args.len() == 1 {
		conf.conf(&mut builder, File::open(DEFAULT_CONF_PATH)?);
	} else {
		if args[1] == "-" {
			conf.conf(&mut builder, stdin());
		} else {
			conf.conf(&mut builder, File::open(&args[1] as &str)?);
		}
	};

	let _dmap = builder.build();

	if conf.upstream.is_empty() {
		eprintln!("WARNING: empty upstream");
	} else {
		eprintln!("upstream: {}", conf.upstream.iter().join(", "));
	}
	Ok(())
}
