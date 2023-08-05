use std::{error::Error, path::Path};

#[allow(dead_code)]
#[derive(Debug)]
pub struct Raw<'a> {
    pub data: &'a [u8],
}

impl<'a> TryFrom<&'a [u8]> for Raw<'a> {
    type Error = Box<dyn Error>;

    fn try_from(data: &'a [u8]) -> Result<Self, Self::Error> {
        Ok(Self { data })
    }
}

impl Raw<'_> {
    pub fn dump<Q>(&self, path: Q) -> Result<(), Box<dyn Error>>
    where
        Q: AsRef<Path>,
    {
        use std::fs::write;
        write(path, self.data)?;
        Ok(())
    }
}
