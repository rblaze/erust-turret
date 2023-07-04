#![no_std]
#![deny(unsafe_code)]

use bytes::{Buf, BufMut};
use core::mem::size_of;

// Backend storage API. Originally from littlefs2 crate.
pub trait Storage {
    // Error type
    type Error;

    // Total storage size in bytes.
    fn capacity(&self) -> usize;

    // Read data from the storage device.
    // Guaranteed not to be called with off > capacity() or bufs of length > capacity() - off.
    fn read(&self, off: usize, buf: &mut [u8]) -> Result<(), Self::Error>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Error<E> {
    InvalidSignature,
    CorruptedFileSystem,
    InvalidFileIndex,
    Storage(E),
}

impl<E> From<E> for Error<E> {
    fn from(error: E) -> Self {
        Error::Storage(error)
    }
}

pub struct FileSystem<S> {
    storage: S,
    num_files: u16,
}

impl<S: Storage> FileSystem<S> {
    pub fn mount(storage: S) -> Result<Self, Error<S::Error>> {
        if storage.capacity() < size_of::<FilesystemHeader>() {
            return Err(Error::CorruptedFileSystem);
        }

        let mut buf = [0; size_of::<FilesystemHeader>()];
        storage.read(0, &mut buf)?;
        let header =
            FilesystemHeader::from_bytes(&mut buf.as_slice()).ok_or(Error::CorruptedFileSystem)?;

        if header.signature != SIGNATURE {
            return Err(Error::InvalidSignature);
        }

        if storage.capacity()
            < size_of::<FilesystemHeader>() + header.num_files as usize * size_of::<DirEntry>()
        {
            return Err(Error::CorruptedFileSystem);
        }

        Ok(FileSystem {
            storage,
            num_files: header.num_files,
        })
    }

    pub fn get_num_files(&self) -> u16 {
        self.num_files
    }

    pub fn open(&self, index: usize) -> Result<File<S>, Error<S::Error>> {
        if index >= self.num_files as usize {
            return Err(Error::InvalidFileIndex);
        }

        let mut buf = [0; size_of::<DirEntry>()];
        self.storage.read(
            size_of::<FilesystemHeader>() + index * size_of::<DirEntry>(),
            &mut buf,
        )?;

        let direntry =
            DirEntry::from_bytes(&mut buf.as_slice()).ok_or(Error::CorruptedFileSystem)?;
        if direntry.offset as usize + direntry.length as usize > self.storage.capacity() {
            return Err(Error::CorruptedFileSystem);
        }

        return Ok(File::new(&self.storage, &direntry));
    }
}

#[derive(Debug)]
pub struct File<'a, S> {
    storage: &'a S,
    file_offset: usize,
    file_size: usize,
    read_position: usize,
}

impl<'a, S: Storage> File<'a, S> {
    fn new(storage: &'a S, direntry: &DirEntry) -> Self {
        Self {
            storage,
            file_offset: direntry.offset as usize,
            file_size: direntry.length as usize,
            read_position: 0,
        }
    }

    pub fn total_size(&self) -> usize {
        self.file_size
    }

    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error<S::Error>> {
        let max_read = self.file_size - self.read_position;
        let bytes_to_read = buf.len().min(max_read);

        if bytes_to_read > 0 {
            self.storage.read(
                self.file_offset + self.read_position,
                &mut buf[..bytes_to_read],
            )?;

            self.read_position += bytes_to_read;
        }

        Ok(bytes_to_read)
    }
}

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

// Directory entry, 0 or more follow filesystem header.
pub struct DirEntry {
    pub offset: u32,
    pub length: u32,
}

impl DirEntry {
    pub fn from_bytes(reader: &mut impl Buf) -> Option<Self> {
        if reader.remaining() < size_of::<DirEntry>() {
            return None;
        }

        let offset = reader.get_u32();
        let length = reader.get_u32();

        Some(DirEntry { offset, length })
    }

    pub fn to_bytes(&self, writer: &mut impl BufMut) {
        writer.put_u32(self.offset);
        writer.put_u32(self.length);
    }
}

const _HDR_SIZE_CHECK: [u8; 10] = [0; size_of::<FilesystemHeader>()];
const _DIRENTRY_SIZE_CHECK: [u8; 8] = [0; size_of::<DirEntry>()];
