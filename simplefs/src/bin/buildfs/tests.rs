use crate::builder::SimpleFsBuilder;
use simplefs::*;

use std::mem::size_of;

use bytes::Bytes;
use quickcheck::{quickcheck, Arbitrary, Gen};

const CAPACITY: usize = 4096 * 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RamStorageError {
    OutOfBoundsAccess,
}

#[derive(Debug)]
struct RamStorage {
    bytes: Bytes,
}

impl RamStorage {
    fn new(bytes: Bytes) -> Self {
        Self { bytes }
    }
}

impl Storage for RamStorage {
    type Error = RamStorageError;

    fn read(&mut self, off: usize, buf: &mut [u8]) -> Result<(), Self::Error> {
        if off + buf.len() > self.bytes.len() {
            return Err(RamStorageError::OutOfBoundsAccess);
        }

        Ok(buf.copy_from_slice(&self.bytes[off..off + buf.len()]))
    }

    fn capacity(&self) -> usize {
        self.bytes.len()
    }
}

#[test]
fn test_empty_fs_build() {
    let builder: SimpleFsBuilder = SimpleFsBuilder::new(CAPACITY);

    let image_bytes = builder.finalize().expect("empty fs image");
    assert_eq!(image_bytes.len(), size_of::<FilesystemHeader>());

    let header = FilesystemHeader::from_bytes(&mut image_bytes.clone()).expect("parsing fs header");
    let signature = header.signature;
    let num_files = header.num_files;
    assert_eq!(signature, simplefs::SIGNATURE);
    assert_eq!(num_files, 0);

    FileSystem::mount_and(RamStorage::new(image_bytes), |fs| {
        assert_eq!(fs.get_num_files(), 0);
    })
    .expect("filesystem mount");
}

#[test]
fn test_single_file_fs_build() {
    let filedata = vec![
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
    ];

    let mut builder: SimpleFsBuilder = SimpleFsBuilder::new(CAPACITY);
    builder.add_file(filedata.clone());

    let image_bytes = builder.finalize().expect("fs image");
    assert_eq!(
        image_bytes.len(),
        size_of::<FilesystemHeader>() + size_of::<DirEntry>() + filedata.len()
    );

    let header = FilesystemHeader::from_bytes(&mut image_bytes.clone()).expect("parsing fs header");
    let signature = header.signature;
    let num_files = header.num_files;
    assert_eq!(signature, simplefs::SIGNATURE);
    assert_eq!(num_files, 1);

    FileSystem::mount_and(RamStorage::new(image_bytes), |fs| {
        assert_eq!(fs.get_num_files(), 1);
    })
    .expect("filesystem mount");
}

#[derive(Debug, Clone)]
struct QuickCheckFileData {
    data: Vec<u8>,
}

impl Arbitrary for QuickCheckFileData {
    fn arbitrary(g: &mut Gen) -> Self {
        QuickCheckFileData {
            data: Vec::<u8>::arbitrary(g),
        }
    }
}

quickcheck! {
fn test_valid_fs_build(files: Vec<QuickCheckFileData>) -> bool {
    let mut builder: SimpleFsBuilder = SimpleFsBuilder::new(CAPACITY);

    for file in &files {
        builder.add_file(file.data.clone());
    }

    let image_bytes = match builder.finalize() {
        Ok(image_bytes) => image_bytes,
        Err(_) => return false
    };

    FileSystem::mount_and(RamStorage::new(image_bytes), |fs| {
        if fs.get_num_files() as usize != files.len() {
            return false;
        }
        return true;
    })
    .expect("filesystem mount")
}
}
