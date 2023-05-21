#![cfg_attr(not(test), no_std)]

use bytes::{Buf, BufMut, Bytes};
use core::mem::size_of;

// Backend storage API. Originally from littlefs2 crate.
pub trait Storage {
    // Error type
    type Error;

    // Total storage size in bytes.
    fn capacity(&self) -> usize;

    // Read data from the storage device.
    // Guaranteed to be called only with bufs of length a multiple of READ_SIZE.
    fn read(&mut self, off: usize, buf: &mut [u8]) -> Result<(), Self::Error>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error<S: Storage> {
    InvalidSignature,
    InconsistentData,
    FileNotFound,
    Storage(S::Error),
}

pub struct FileSystem<S> {
    storage: S,
    num_files: u16,
}

impl<S: Storage> FileSystem<S> {
    pub fn mount_and<R>(mut storage: S, f: impl Fn(&Self) -> R) -> Result<R, Error<S>> {
        if storage.capacity() < size_of::<FilesystemHeader>() {
            return Err(Error::InconsistentData);
        }

        let mut buf = [0; size_of::<FilesystemHeader>()];
        storage.read(0, &mut buf).map_err(|e| Error::Storage(e))?;
        let header =
            FilesystemHeader::from_bytes(&mut buf.as_slice()).ok_or(Error::InconsistentData)?;

        if header.signature != SIGNATURE {
            return Err(Error::InvalidSignature);
        }

        let fs = FileSystem {
            storage,
            num_files: header.num_files,
        };
        Ok(f(&fs))
    }

    pub fn get_num_files(&self) -> u16 {
        self.num_files
    }

    pub fn open_and(&self, name: &str) -> Result<File, Error<S>> {
        todo!()
    }
}

pub struct File {}

// Filesystem header, expected at storage offset 0
#[repr(packed(1))]
pub struct FilesystemHeader {
    pub signature: u64, // "SimpleFS"
    pub num_files: u16,
}

impl FilesystemHeader {
    pub fn from_bytes(reader: &mut impl Buf) -> Option<Self> {
        if reader.remaining() < size_of::<FilesystemHeader>() {
            return None;
        }

        let signature = reader.get_u64();
        let num_files = reader.get_u16();

        Some(FilesystemHeader {
            signature,
            num_files,
        })
    }

    pub fn to_bytes(&self, writer: &mut impl BufMut) {
        writer.put_u64(self.signature);
        writer.put_u16(self.num_files);
    }
}

// "SimpleFS"
pub const SIGNATURE: u64 = 0x53696d706c654653;
pub const MAX_FILE_NAME_BYTES: usize = 16;

// Directory entry, 0 or more follow filesystem header.
pub struct DirEntry {
    pub name: [u8; MAX_FILE_NAME_BYTES],
    pub offset: u32,
    pub length: u32,
}

impl DirEntry {
    pub fn from_bytes(reader: &mut impl Buf) -> Option<Self> {
        if reader.remaining() < size_of::<DirEntry>() {
            return None;
        }

        let mut name = [0; MAX_FILE_NAME_BYTES];
        reader.copy_to_slice(&mut name);

        let offset = reader.get_u32();
        let length = reader.get_u32();

        Some(DirEntry {
            name,
            offset,
            length,
        })
    }

    pub fn to_bytes(&self, writer: &mut impl BufMut) {
        writer.put_slice(&self.name);
        writer.put_u32(self.offset);
        writer.put_u32(self.length);
    }
}

const _HDR_SIZE_CHECK: [u8; 10] = [0; size_of::<FilesystemHeader>()];
const _DIRENTRY_SIZE_CHECK: [u8; 24] = [0; size_of::<DirEntry>()];
