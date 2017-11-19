extern crate byteorder;
extern crate clap;
extern crate ffmpeg;
extern crate tempfile;

mod dfpwm;
mod rip;

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use clap::{App, Arg};
use ffmpeg::{format, media, frame};

use dfpwm::DFPWM;

// Borrowed from https://github.com/meh/rust-ffmpeg/blob/master/examples/transcode-audio.rs
fn get_filter(spec: &str, decoder: &ffmpeg::codec::decoder::Audio,
              encoder: &ffmpeg::codec::encoder::Audio) -> Result<ffmpeg::filter::Graph, ffmpeg::Error> {
    let mut filter = ffmpeg::filter::Graph::new();

    let args = format!("time_base={}:sample_rate={}:sample_fmt={}:channel_layout=0x{:x}",
                       decoder.time_base(), decoder.rate(), decoder.format().name(),
                       decoder.channel_layout().bits());

    filter.add(&ffmpeg::filter::find("abuffer").unwrap(), "in", &args)?;
    filter.add(&ffmpeg::filter::find("abuffersink").unwrap(), "out", "")?;

    {
        let mut out = filter.get("out").unwrap();

        out.set_sample_format(encoder.format());
        out.set_channel_layout(encoder.channel_layout());
        out.set_sample_rate(encoder.rate());
    }

    filter.output("in", 0)?.input("out", 0)?.parse(spec)?;
    filter.validate()?;

    if let Some(codec) = encoder.codec() {
        if !codec.capabilities().contains(ffmpeg::codec::capabilities::VARIABLE_FRAME_SIZE) {
            filter.get("out").unwrap().sink().set_frame_size(encoder.frame_size());
        }
    }

    Ok(filter)
}

struct Transcoder {
    stream: usize,
    filter: ffmpeg::filter::Graph,
    decoder: ffmpeg::codec::decoder::Audio,
    encoder: ffmpeg::codec::encoder::Audio,
}

fn get_transcoder(input_context: &mut format::context::Input,
                  output_context: &mut format::context::Output) -> Result<Transcoder, ffmpeg::Error> {
    let input = input_context.streams().best(media::Type::Audio)
        .expect("Couldn't find the best audio stream");
    let mut decoder = input.codec().decoder().audio().unwrap();
    decoder.set_parameters(input.parameters()).unwrap();

    let codec = ffmpeg::codec::encoder::find(ffmpeg::codec::Id::PCM_S8).unwrap()
        .audio().unwrap();

    let mut output = output_context.add_stream(codec).unwrap();

    let mut encoder = output.codec().encoder().audio().unwrap();

    let channel_layout = ffmpeg::channel_layout::MONO;

    // Below is borrowed from https://github.com/meh/rust-ffmpeg/blob/master/examples/transcode-audio.rs

    encoder.set_rate(48000i32);
    encoder.set_channel_layout(channel_layout);
    encoder.set_channels(channel_layout.channels());
    encoder.set_format(codec.formats().unwrap().next().unwrap());
    encoder.set_time_base((1, 48000i32));
    output.set_time_base((1, 48000i32));

    let encoder = encoder.open_as(codec).unwrap();
    output.set_parameters(&encoder);

    let filter = get_filter("anull", &decoder, &encoder).unwrap();

    Ok(Transcoder {
        decoder,
        encoder,
        filter,
        stream: input.index(),
    })
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

    let output_path = output.as_path();
    if output_path.is_dir() {
        panic!("Output is a directory.");
    }

    let mut temp_file = tempfile::NamedTempFile::new().unwrap();
    println!("Created a temporary file at {}", temp_file.path().to_str().unwrap());

    ffmpeg::init().unwrap();

    let mut input_context = format::input(&input_path).unwrap();

    {
        let temp_path = temp_file.path();
        let mut output_context = format::output_as(&temp_path, "s8").unwrap();

        let mut transcoder = get_transcoder(&mut input_context, &mut output_context).unwrap();

        println!("Transcoding to PCM (signed 8-bit)...");
        let now = Instant::now();

        output_context.write_header().unwrap();

        let in_time_base = transcoder.decoder.time_base();
        let out_time_base = output_context.stream(0).unwrap().time_base();

        let mut decoded = frame::Audio::empty();
        let mut encoded = ffmpeg::Packet::empty();

        for (stream, mut packet) in input_context.packets() {
            if stream.index() == transcoder.stream {
                packet.rescale_ts(stream.time_base(), in_time_base);

                if let Ok(true) = transcoder.decoder.decode(&packet, &mut decoded) {
                    let timestamp = decoded.timestamp();
                    decoded.set_pts(timestamp);

                    transcoder.filter.get("in").unwrap().source().add(&decoded).unwrap();

                    while let Ok(..) = transcoder.filter.get("out").unwrap().sink().frame(&mut decoded) {
                        if let Ok(true) = transcoder.encoder.encode(&decoded, &mut encoded) {
                            encoded.set_stream(0);
                            encoded.rescale_ts(in_time_base, out_time_base);
                            encoded.write_interleaved(&mut output_context).unwrap();
                        }
                    }
                }
            }
        }

        transcoder.filter.get("in").unwrap().source().flush().unwrap();

        while let Ok(..) = transcoder.filter.get("out").unwrap().sink().frame(&mut decoded) {
            if let Ok(true) = transcoder.encoder.encode(&decoded, &mut encoded) {
                encoded.set_stream(0);
                encoded.rescale_ts(in_time_base, out_time_base);
                encoded.write_interleaved(&mut output_context).unwrap();
            }
        }

        if let Ok(true) = transcoder.encoder.flush(&mut encoded) {
            encoded.set_stream(0);
            encoded.rescale_ts(in_time_base, out_time_base);
            encoded.write_interleaved(&mut output_context).unwrap();
        }

        output_context.write_trailer().unwrap();

        println!("Done (in {}.{:2} s)", now.elapsed().as_secs(), now.elapsed().subsec_nanos() / 10_000_000);
    }

    // Now we can finally convert PCM to DFPWM
    let mut pcm_bytes = Vec::<u8>::new();
    temp_file.read_to_end(&mut pcm_bytes).unwrap();
    temp_file.close().unwrap();
    let mut dfpwm_compressor = DFPWM::new();
    println!("Compressing to DFPWM...");
    let now = Instant::now();
    let mut dfpwm_bytes = Vec::<u8>::new();
    dfpwm_compressor.compress(&pcm_bytes, &mut dfpwm_bytes);
    println!("Done (in {}.{:2} s)", now.elapsed().as_secs(), now.elapsed().subsec_nanos() / 10_000_000);

    let mut out_file = File::create(output_path).unwrap();
    println!("Metadata (best stream used):");
    let stream = input_context.streams().best(media::Type::Audio).unwrap();
    let metadata = stream.metadata();
    println!("  - Title: {}", metadata.get("title").unwrap_or(""));
    println!("  - Artist: {}", metadata.get("artist").unwrap_or(""));
    println!("  - Album: {}", metadata.get("album").unwrap_or(""));
    rip::write_rip(&mut out_file, &dfpwm_bytes, &metadata);
    out_file.flush().unwrap();

    println!("Successfully converted to {}", output_path.to_str().unwrap());
}
