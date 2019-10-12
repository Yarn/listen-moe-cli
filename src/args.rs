
use clap::{Arg, App, ArgMatches};

pub fn get_matches() -> ArgMatches<'static> {
    let matches = App::new("listen-moe-cli")
        .version(env!("CARGO_PKG_VERSION"))
        .arg(Arg::with_name("volume")
            .long("volume")
            .short("l")
            .takes_value(true)
            .use_delimiter(false)
            .default_value("100")
            .validator(|v| {
                v.parse::<u32>().map(|_| ()).map_err(|_| { String::from("Not a number") })
            })
        )
        .arg(Arg::with_name("kpop")
            .long("kpop")
            .short("k")
        )
        .get_matches();
    
    matches
}

#[derive(Debug)]
pub struct Args {
    pub volume: f32,
    pub kpop: bool,
}

pub fn get_args() -> Args {
    let matches = get_matches();
    
    let volume: f32 = matches.value_of("volume").unwrap().parse::<u32>().unwrap() as f32 / 100.0;
    let kpop: bool = matches.is_present("kpop");
    
    Args {
        volume: volume,
        kpop: kpop,
    }
}
