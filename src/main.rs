extern crate clap;
extern crate lewton;

mod dfpwm;

use std::fs::File;
use std::path::{Path, PathBuf};

use clap::{App, Arg};
use lewton::inside_ogg::OggStreamReader;

fn main() {
    let matches = App::new("rip-converter")
        .about("Converts an Ogg/Vorbis file to a .rip container with DFPWM codec")
        .arg(Arg::with_name("format")
             .short("i")
             .long("input")
             .value_name("FORMAT")
             .help("Forces a specific format instead of guessing by extenstion")
             .takes_value(true)
             .possible_values(&["ogg"]))
        .arg(Arg::with_name("input")
             .value_name("INPUT")
             .help("Input file")
             .required(true)
             .index(1))
        .arg(Arg::with_name("output")
             .value_name("OUTPUT")
             .help("Output file")
             .index(2))
        .get_matches();

    let input = Path::new(matches.value_of("input").unwrap());
    if !input.is_file() {
        panic!("Input file doesn't exist or is a directory.");
    }

    let output = if let Some(path) = matches.value_of("output") {
        PathBuf::from(path)
    } else {
        input.with_extension("rip")
    };

    let output = output.as_path();
    if output.is_dir() {
        panic!("Output is a directory.");
    }

    let format = matches.value_of("format")
        .unwrap_or_else(|| input.extension()
                        .map_or("ogg", |x| x.to_str().unwrap()));

    match format {
        "ogg" => {
            let f = File::open(input).unwrap();
            let mut ogg = OggStreamReader::new(f).unwrap();

            println!("Sample rate: {}", ogg.ident_hdr.audio_sample_rate);
        },
        _ => panic!("Unsupported input file format: {}", format),
    }
}
