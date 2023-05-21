use std::ffi::CString;
use std::mem::size_of;

use bytes::{BufMut, Bytes, BytesMut};
use simplefs::{DirEntry, FilesystemHeader, MAX_FILE_NAME_BYTES};

#[derive(Debug)]
pub enum BuilderError {
    OutOfSpace,
    TooManyFiles,
    FileTooBig,
    FileNameTooLong,
    IO(std::io::Error),
}

impl std::fmt::Display for BuilderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuilderError::OutOfSpace => write!(f, "capacity exceeded"),
            BuilderError::TooManyFiles => write!(f, "too many files"),
            BuilderError::FileTooBig => write!(f, "file too big"),
            BuilderError::FileNameTooLong => write!(f, "file name too long"),
            BuilderError::IO(ioerror) => write!(f, "{}", ioerror),
        }
    }
}
impl std::error::Error for BuilderError {}
impl From<std::io::Error> for BuilderError {
    fn from(ioerror: std::io::Error) -> Self {
        BuilderError::IO(ioerror)
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct FileInfo {
    name: CString,
    data: Vec<u8>,
}

pub struct SimpleFsBuilder {
    capacity: usize,
    files: Vec<FileInfo>,
}

impl SimpleFsBuilder {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            files: Vec::new(),
        }
    }

    pub fn add_file(&mut self, name: CString, data: Vec<u8>) {
        // TODO check for duplicate file names
        self.files.push(FileInfo { name, data })
    }

    pub fn finalize(self) -> Result<Bytes, BuilderError> {
        let num_files = self
            .files
            .len()
            .try_into()
            .map_err(|_| BuilderError::TooManyFiles)?;

        let total_file_size: usize = self.files.iter().map(|file| file.data.len()).sum();
        let dir_size = self.files.len() * size_of::<DirEntry>();

        let mut writer =
            BytesMut::with_capacity(size_of::<FilesystemHeader>() + dir_size + total_file_size);

        FilesystemHeader {
            signature: simplefs::SIGNATURE,
            num_files,
        }
        .to_bytes(&mut writer);

        let mut current_offset = size_of::<FilesystemHeader>() + dir_size;

        for file in &self.files {
            let mut direntry = DirEntry {
                name: [0; MAX_FILE_NAME_BYTES],
                offset: current_offset
                    .try_into()
                    .map_err(|_| BuilderError::OutOfSpace)?,
                length: file
                    .data
                    .len()
                    .try_into()
                    .map_err(|_| BuilderError::FileTooBig)?,
            };

            let name = file.name.as_bytes();
            if name.len() > direntry.name.len() {
                return Err(BuilderError::FileNameTooLong);
            }
            (&mut direntry.name)[..name.len()].copy_from_slice(name);

            current_offset += file.data.len();
            if current_offset > self.capacity {
                return Err(BuilderError::OutOfSpace);
            }

            direntry.to_bytes(&mut writer);
        }

        for file in &self.files {
            writer.put_slice(file.data.as_slice());
        }

        Ok(writer.freeze())
    }
}

fn main() {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use simplefs::{FileSystem, Storage};

    use std::ffi::CString;
    use std::mem::size_of;

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

        let header =
            FilesystemHeader::from_bytes(&mut image_bytes.clone()).expect("parsing fs header");
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
        let filename = CString::new("foo").unwrap();
        let filedata = vec![
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
        ];

        let mut builder: SimpleFsBuilder = SimpleFsBuilder::new(CAPACITY);
        builder.add_file(filename.to_owned(), filedata.clone());

        let image_bytes = builder.finalize().expect("fs image");
        assert_eq!(
            image_bytes.len(),
            size_of::<FilesystemHeader>() + size_of::<DirEntry>() + filedata.len()
        );

        let header =
            FilesystemHeader::from_bytes(&mut image_bytes.clone()).expect("parsing fs header");
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
    struct FileName(CString);

    impl Arbitrary for FileName {
        fn arbitrary(g: &mut Gen) -> Self {
            let size = 1 + usize::arbitrary(g) % MAX_FILE_NAME_BYTES;

            FileName(
                CString::new(
                    (0..)
                        .map(|_| u8::arbitrary(g))
                        .filter(|&c| c != b'\0')
                        .take(size)
                        .collect::<Vec<u8>>(),
                )
                .unwrap(),
            )
        }
    }

    #[derive(Debug, Clone)]
    struct QuickCheckFileData {
        name: FileName,
        data: Vec<u8>,
    }

    impl Arbitrary for QuickCheckFileData {
        fn arbitrary(g: &mut Gen) -> Self {
            QuickCheckFileData {
                name: FileName::arbitrary(g),
                data: Vec::<u8>::arbitrary(g),
            }
        }
    }

    quickcheck! {
    fn test_valid_fs_build(files: Vec<QuickCheckFileData>) -> bool {
        let mut builder: SimpleFsBuilder = SimpleFsBuilder::new(CAPACITY);

        for file in &files {
            let FileName(name) = file.name.clone();
            builder.add_file(name, file.data.clone());
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
}
