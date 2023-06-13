pub enum StorageError {
    Todo,
}

pub struct SoundStorage;

impl simplefs::Storage for SoundStorage {
    type Error = StorageError;

    fn capacity(&self) -> usize {
        todo!()
    }

    fn read(&self, off: usize, buf: &mut [u8]) -> Result<(), Self::Error> {
        todo!()
    }
}
