extern crate clap;
extern crate ffmpeg;
extern crate tempfile;

mod dfpwm;

use std::fs::File;
use std::path::{Path, PathBuf};

use clap::{App, Arg};
use ffmpeg::{format, media, frame};

use dfpwm::DFPWM;

// Borrowed from https://github.com/meh/rust-ffmpeg/blob/master/examples/transcode-audio.rs
fn get_filter(spec: &str, decoder: &codec::decoder::Audio,
              encoder: &codec::encoder::Audio) -> Result<ffmpeg::filter::Graph, ffmpeg::Error> {
    let mut filter = ffmpeg::filter::Graph::new();

    let args = format!("time_base={}:sample_rate={}:sample_fmt={}:channel_layout=0x{:x}",
                       decoder.time_base(), decoder.rate(), decoder.format().name(),
                       decoder.channel_layout().bits());

    filter.add(&filter::find("abuffer").unwrap(), "in", &args)?;
    filter.add(&filter::find("abuffersink").unwrap(), "out", "")?;

    {
        let mut out = filter.get("out").unwrap();

        out.set_sample_format(encoder.format());
        out.set_channel_layout(encoder.channel_layout());
        out.set_sample_rate(encoder.rate());
    }

    filter.output("in", 0)?.input("out", 0)?.parse(spec)?;
    filter.validate()?;

    println!("{}", filter.dump());

    if let Some(codec) = encoder.codec() {
        if !codec.capabilities().contains(ffmpeg::codec::capabilities::VARIABLE_FRAME_SIZE) {
            filter.get("out").unwrap().sink().set_frame_size(encoder.frame_size());
        }
    }

    Ok(filter)
}

fn main() {
    let matches = App::new("rip-converter")
        .about("Converts an Ogg/Vorbis file to a .rip container with DFPWM codec")
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

    let input_path = Path::new(matches.value_of("input").unwrap());
    if !input_path.is_file() {
        panic!("Input file doesn't exist or is a directory.");
    }

    let output = if let Some(path) = matches.value_of("output") {
        PathBuf::from(path)
    } else {
        input_path.with_extension("rip")
    };

    let output = output.as_path();
    if output.is_dir() {
        panic!("Output is a directory.");
    }

    ffmpeg::init().unwrap();

    let mut input_context = format::input(&input_path).unwrap();
    let input = input_context.streams().best(media::Type::Audio)
        .expect("Couldn't find the best audio stream");
    let mut decoder = input.codec().decoder().audio().unwrap();
    decoder.set_parameters(input.parameters());

    let codec = ffmpeg::codec::encoder::find(ffmpeg::codec::Id::PCM_S8).unwrap()
        .audio().unwrap();

    let temp_file = tempfile::NamedTempFile::new();
    let temp_path = temp_file.path();
    let mut output_context = format::output_as(temp_path, "pcm_s8").unwrap();

    let mut output = output_context.add_stream(codec)
        .unwrap();
    let mut encoder = output.codec().encoder().audio().unwrap();

    let channel_layout = ffmpeg::channel_layout::MONO;

    // Below is borrowed from https://github.com/meh/rust-ffmpeg/blob/master/examples/transcode-audio.rs

    encoder.set_rate(48000u32);
    encoder.set_channel_layout(channel_layout);
    encoder.set_channels(channel_layout.channels());
    encoder.set_format(codec.formats().unwrap().next().unwrap());
    encoder.set_time_base((1, 48000u32));
    output.set_time_base((1, 48000u32));

    let mut encoder = encoder.open_as(codec).unwrap();
    output.set_params(&encoder);

    let mut filter = get_filter("anull", &decoder, &encoder).unwrap();

    output_context.set_metadata(input_context.metadata().to_owned());
    output_context.write_header().unwrap();

    let in_time_base = decoder.time_base();
    let out_time_base = octx.stream(0).unwrap().time_base();

    let mut decoded = frame::Audio::empty();
    let mut encoded = ffmpeg::Packet::empty();

    for (stream, mut packet) in input_context.packets() {
        if stream.index() == input.index() {
            packet.rescale_ts(stream.time_base(), in_time_base);

            if let Ok(true) = decoder.decode(&packet, &mut decoded) {
                let timestamp = decoded.timestamp();
                decoded.set_pts(timestamp);

                filter.get("in").unwrap().source.add(&decoded).unwrap();

                while let Ok(..) = filter.get("out").unwrap().sink().frame(&mut decoded) {
                    if let Ok(true) = encoder.encode(&decoded, &mut encoded) {
                        encoded.set_stream(0);
                        encoded.rescale_ts(in_base_time, out_base_time);
                        encoded.write_interleaved(&output_context).unwrap();
                    }
                }
            }
        }
    }

    filter.get("in").unwrap().source().flush().unwrap();

    while let Ok(..) = filter.get("out").unwrap().sink().frame(&mut decoded) {
        if let Ok(true) = encoder.encode(&decoded, &mut encoded) {
            encoded.set_stream(0);
            encoded.rescale_ts(in_time_base, out_time_base);
            encoded.write_interleaved(&mut output_context).unwrap();
        }
    }

    if let Ok(true) = encoder.flush(&mut encoded) {
        encoded.set_stream(0);
        encoded.rescale_ts(in_time_base, out_time_base);
        encoded.write_interleaved(&mut output_context).unwrap();
    }

    output_context.write_trailer().unwrap();

    // Now we can finally convert PCM to DFPWM
    let pcm_bytes = temp_file as File.bytes().collect::<Vec<u8>>();
    let mut dfpwm_compressor = DFPWM::new();
    let dfpwm_bytes = Vec::<u8>::new();
    dfpwm_compressor.compress(pcm_bytes, dfpwm_bytes, 0, 0, pcm_bytes.len());
}
