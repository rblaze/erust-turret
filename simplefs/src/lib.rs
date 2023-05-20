#![cfg_attr(not(test), no_std)]

use bytes::{Buf, Bytes};
use core::mem::size_of;

/*
// Backend storage API. Originally from littlefs2 crate.
pub trait Storage {
    // Error type
    type Error;

    // Minimum size of block read in bytes.
    const READ_SIZE: usize;

    // Minimum size of block write in bytes.
    const WRITE_SIZE: usize;

    // Size of an erasable block in bytes, as unsigned typenum.
    // Must be a multiple of both `READ_SIZE` and `WRITE_SIZE`.
    const BLOCK_SIZE: usize;

    // Number of erasable blocks.
    // Hence storage capacity is `BLOCK_COUNT * BLOCK_SIZE`
    const BLOCK_COUNT: usize;

    // Read data from the storage device.
    // Guaranteed to be called only with bufs of length a multiple of READ_SIZE.
    fn read(&mut self, off: usize, buf: &mut [u8]) -> Result<(), Self::Error>;
    // Write data to the storage device.
    // Guaranteed to be called only with bufs of length a multiple of WRITE_SIZE.
    fn write(&mut self, off: usize, data: &[u8]) -> Result<(), Self::Error>;
    // Erase data from the storage device.
    // Guaranteed to be called only with bufs of length a multiple of BLOCK_SIZE.
    fn erase(&mut self, off: usize, len: usize) -> Result<(), Self::Error>;
}
*/

// Filesystem header, expected at storage offset 0
#[repr(packed(1))]
pub struct FilesystemHeader {
    pub signature: u64, // "SimpleFS"
    pub num_files: u16,
}

impl FilesystemHeader {
    pub fn from_bytes(mut bytes: Bytes) -> Option<Self> {
        if bytes.remaining() < size_of::<FilesystemHeader>() {
            return None;
        }

        let signature = bytes.get_u64();
        let num_files = bytes.get_u16();

        Some(FilesystemHeader {
            signature,
            num_files,
        })
    }
}

// "SimpleFS"
pub const SIGNATURE: u64 = 0x53696d706c654653;

// Directory entry, 0 or more follow filesystem header.
#[repr(packed(1))]
pub struct DirEntry {
    pub name: [u8; 16],
    pub offset: u32,
    pub length: u32,
}

impl DirEntry {
    pub fn from_bytes(mut bytes: Bytes) -> Option<Self> {
        if bytes.remaining() < size_of::<DirEntry>() {
            return None;
        }

        let mut name = [0; 16];
        bytes.copy_to_slice(&mut name);

        let offset = bytes.get_u32();
        let length = bytes.get_u32();

        Some(DirEntry {
            name,
            offset,
            length,
        })
    }
}

const _HDR_SIZE_CHECK: [u8; 10] = [0; size_of::<FilesystemHeader>()];
const _DIRENTRY_SIZE_CHECK: [u8; 24] = [0; size_of::<DirEntry>()];
