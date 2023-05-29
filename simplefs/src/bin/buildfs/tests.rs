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

    fn read(&self, off: usize, buf: &mut [u8]) -> Result<(), Self::Error> {
        if off + buf.len() > self.bytes.len() {
            return Err(RamStorageError::OutOfBoundsAccess);
        }

        Ok(buf.copy_from_slice(&self.bytes[off..off + buf.len()]))
    }

    fn capacity(&self) -> usize {
        self.bytes.len()
    }
}

fn read_full_file(fs: &FileSystem<RamStorage>, index: usize) -> Vec<u8> {
    let mut file = fs.open(index).expect("file open");
    let mut buf = Vec::new();
    buf.resize(file.total_size(), 0);

    let bytes_read = file.read(&mut buf).expect("read");
    assert_eq!(bytes_read, buf.len());
    return buf;
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
        let status = fs.open(0).expect_err("open non-existent file");
        assert_eq!(status, Error::InvalidFileIndex);
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
        let buf = read_full_file(&fs, 0);
        assert_eq!(filedata, buf);
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
    // TODO restrict file sizes by CAPACITY or check for errors
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

        // Check that file contents are read correctly
        files.iter().enumerate().all(|(i, file)| {
            let buf = read_full_file(&fs, i);
            file.data == buf
        })
    })
    .expect("filesystem mount")
}
}
