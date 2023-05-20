use std::mem::size_of;

use bytes::{BufMut, Bytes, BytesMut};
use simplefs::{DirEntry, FilesystemHeader};

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
    name: String,
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

    pub fn add_file(&mut self, name: String, data: Vec<u8>) {
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

        let header = FilesystemHeader {
            signature: simplefs::SIGNATURE,
            num_files,
        };

        Self::put_header(&header, &mut writer);

        let mut current_offset = size_of::<FilesystemHeader>() + dir_size;

        for file in &self.files {
            let mut direntry = DirEntry {
                name: [0; 16],
                offset: current_offset
                    .try_into()
                    .map_err(|_| BuilderError::OutOfSpace)?,
                length: file
                    .data
                    .len()
                    .try_into()
                    .map_err(|_| BuilderError::FileTooBig)?,
            };

            if file.name.len() > direntry.name.len() {
                return Err(BuilderError::FileNameTooLong);
            }
            (&mut direntry.name)[..file.name.len()].copy_from_slice(file.name.as_bytes());

            current_offset += file.data.len();
            if current_offset > self.capacity {
                return Err(BuilderError::OutOfSpace);
            }

            Self::put_direntry(&direntry, &mut writer);
        }

        for file in &self.files {
            writer.put_slice(file.data.as_slice());
        }

        Ok(writer.freeze())
    }

    fn put_header(header: &FilesystemHeader, writer: &mut impl BufMut) {
        writer.put_u64(header.signature);
        writer.put_u16(header.num_files);
    }

    fn put_direntry(direntry: &DirEntry, writer: &mut impl BufMut) {
        writer.put_slice(&direntry.name);
        writer.put_u32(direntry.offset);
        writer.put_u32(direntry.length);
    }
}

fn main() {
    todo!()
}

#[cfg(test)]
mod tests {
    use std::mem::size_of;

    use super::*;

    const CAPACITY: usize = 4096 * 128;

    #[test]
    fn test_empty_fs_build() {
        let builder: SimpleFsBuilder = SimpleFsBuilder::new(CAPACITY);

        let image_bytes = builder.finalize().expect("empty fs image");
        assert_eq!(image_bytes.len(), size_of::<FilesystemHeader>());

        let header = FilesystemHeader::from_bytes(image_bytes).expect("parsing fs header");
        let signature = header.signature;
        let num_files = header.num_files;
        assert_eq!(signature, simplefs::SIGNATURE);
        assert_eq!(num_files, 0);
    }

    #[test]
    fn test_single_file_fs_build() {
        let filename = "foo";
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

        let header = FilesystemHeader::from_bytes(image_bytes).expect("parsing fs header");
        let signature = header.signature;
        let num_files = header.num_files;
        assert_eq!(signature, simplefs::SIGNATURE);
        assert_eq!(num_files, 1);
    }
}
