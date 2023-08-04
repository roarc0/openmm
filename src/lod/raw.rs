use std::{error::Error, path::Path};

#[allow(dead_code)]
#[derive(Debug)]
pub struct Raw {
    pub data: Vec<u8>,
}

impl TryFrom<Vec<u8>> for Raw {
    type Error = Box<dyn Error>;

    fn try_from(data: Vec<u8>) -> Result<Self, Self::Error> {
        Ok(Self { data })
    }
}

impl TryFrom<&[u8]> for Raw {
    type Error = Box<dyn Error>;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        Ok(Self {
            data: data.to_vec(),
        })
    }
}

impl Raw {
    pub fn dump<Q>(&self, path: Q) -> Result<(), Box<dyn Error>>
    where
        Q: AsRef<Path>,
    {
        use std::fs::write;
        write(path, &self.data)?;
        Ok(())
    }
}
