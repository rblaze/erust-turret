#![deny(unsafe_code)]

use std::error::Error;
use std::fs::OpenOptions;
use std::io::{Read, Write};

use anyhow::Result;
use clap::Parser;
use crc::*;

/// Send filesystem image to the device
#[derive(Parser, Debug)]
#[command(about)]
struct Args {
    /// Serial port
    #[arg(short, default_value = "/dev/ttyACM0")]
    serial_port: std::path::PathBuf,
    /// Image file name
    image: std::path::PathBuf,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum SendError {
    InvalidAck(u8),
}

impl std::fmt::Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SendError::InvalidAck(received_ack) => {
                f.write_fmt(format_args!("InvalidAck({})", received_ack))
            }
        }
    }
}

impl Error for SendError {}

fn main() -> Result<()> {
    let args = Args::parse();

    let mut image = std::fs::read(args.image)?;

    if image.len() % 4 != 0 {
        // Image length must be a multiple of 4, STM CRC unit takes 32-bit inputs
        image.extend(vec![0; 4 - image.len() % 4]);
    }

    let mut device = OpenOptions::new()
        .read(true)
        .write(true)
        .open(args.serial_port)?;

    println!("Sending image size");
    device.write_all((image.len() as u32).to_be_bytes().as_ref())?;

    println!("Reading block size");
    let mut block_size_buf = [0; 2];
    device.read_exact(&mut block_size_buf)?;

    let block_size = u16::from_be_bytes(block_size_buf).into();
    println!("Block size: {}", block_size);

    for chunk in image.chunks(block_size) {
        let crc = Crc::<u32>::new(&CRC_32_MPEG_2).checksum(chunk);
        println!("Sending chunk of len {} with crc {:x}", chunk.len(), crc);
        device.write_all(chunk)?;
        device.write_all(crc.to_be_bytes().as_ref())?;

        println!("Reading ack");
        let mut ack = [0; 1];
        device.read_exact(&mut ack)?;

        if ack[0] != 42 {
            Err(SendError::InvalidAck(ack[0]))?;
        }
    }

    Ok(())
}
