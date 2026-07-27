use std::io::{BufRead as _, Result};

const DEFAULT_CONF_PATH: &str = "conf";

fn main() -> Result<()> {
	let args: Vec<_> = std::env::args().into_iter().take(2).collect();
	let mut conf = Conf::new();
	if args.len() == 1 {
		conf.parse(std::fs::File::open(DEFAULT_CONF_PATH)?)?;
	} else {
		if args[1] == "-" {
			conf.parse(std::io::stdin())?;
		} else {
			conf.parse(std::fs::File::open(&args[1] as &str)?)?;
		}
	};
	Ok(())
}

struct Conf {}

impl Conf {
	fn new() -> Self {
		Self {}
	}

	fn parse<R: std::io::Read>(&mut self, f: R) -> Result<()> {
		let mut r = std::io::BufReader::new(f);

		let mut l = String::with_capacity(0x100);
		while r.read_line(&mut l)? > 0 {
			let (mut o, v) = l.split_once("=").unwrap_or((&l, ""));

			l.clear();
		}
		unimplemented!()
	}
}
