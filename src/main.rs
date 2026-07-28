use std::{fs::File, io::stdin};

use itertools::Itertools as _;

use fstdns::{
	conf::Conf,
	dmap::{DMapBuilder, Result},
};

const DEFAULT_CONF_PATH: &str = "etc/conf";

fn main() -> Result<()> {
	let args: Vec<_> = std::env::args().take(3).collect();
	let mut conf = Conf::default();
	let mut builder = DMapBuilder::default();
	match args.len() {
		1 => conf.conf(&mut builder, File::open(DEFAULT_CONF_PATH)?),
		2 => {
			if args[1] == "-" {
				conf.conf(&mut builder, stdin());
			} else {
				conf.conf(&mut builder, File::open(&args[1] as &str)?);
			}
		}
		_ => {
			panic!("too many arguments");
		}
	}

	let _dmap = builder.build();

	if conf.default.is_empty() {
		eprintln!("WARNING: no default upstream");
	} else {
		eprintln!("default upstream: {}", conf.default.iter().join(", "));
	}
	Ok(())
}
