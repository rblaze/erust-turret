use crate::builder::SimpleFsBuilder;

use anyhow::Result;
use clap::Parser;
use std::fs::File;
use std::io::Read;
use std::io::Write;

mod builder;

#[cfg(test)]
mod tests;

/// Build a filesystem image from a list of files
#[derive(Parser, Debug)]
#[command(about)]
struct Args {
    /// Image file name
    #[arg(short)]
    output: std::path::PathBuf,
    files: Vec<std::path::PathBuf>,
    /// Max image size
    #[arg(short, long, default_value_t = 4*1024*1024)]
    capacity: usize,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let mut builder: SimpleFsBuilder = SimpleFsBuilder::new(args.capacity);
    for filename in args.files {
        println!("Adding file {}", filename.display());
        let mut f = File::open(filename)?;
        let mut data = Vec::new();
        f.read_to_end(&mut data)?;
        builder.add_file(data);
    }

    let bytes = builder.finalize()?;

    println!(
        "Writing image to {}, size {} bytes",
        args.output.display(),
        bytes.len()
    );
    let mut image_file = File::create(args.output)?;
    image_file.write_all(&bytes)?;

    Ok(())
}
