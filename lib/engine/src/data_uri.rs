use std::{
    num::ParseIntError,
    str::{self, Utf8Error},
};

use anyhow::Result;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Unable to get header")]
    GetHeader,

    #[error("Uri parameters unsupported")]
    UriParameter,

    #[error("Base64 decode error: {0}")]
    Base64Decode(#[from] base64::DecodeError),

    #[error("Url decode error: length too short in % encoding")]
    PercentEncoding,

    #[error("Utf8 error: {0}")]
    UTF8(#[from] Utf8Error),

    #[error("Integer parsing error: {0}")]
    IntParse(#[from] ParseIntError),
}

#[derive(Debug)]
pub struct DataURI {
    header: Vec<String>,
    data: String,
    is_base_64: bool,
}

impl DataURI {
    pub fn is_data_uri(data: &str) -> bool {
        data.starts_with("data:")
    }

    pub fn new(uri: &str) -> Result<Self, Error> {
        let parts: Vec<_> = uri.splitn(2, ",").collect();

        let header = parts.get(0).ok_or(Error::GetHeader)?.to_string();
        let data = parts.get(1).ok_or(Error::GetHeader)?.to_string();

        let header: Vec<_> = header["data:".len()..]
            .split(';')
            .map(String::from)
            .collect();

        for param in &header {
            if param.contains('=') {
                return Err(Error::UriParameter);
            }
        }

        Ok(Self {
            is_base_64: header.contains(&"base64".to_owned()),
            data,
            header,
        })
    }

    pub fn get_data(&self) -> Result<Vec<u8>, Error> {
        println!("{:#?}", self);

        if self.is_base_64 {
            Self::get_base64(&self.data)
        } else {
            Self::get_url_data(&self.data)
        }
    }

    fn get_base64(data: &str) -> Result<Vec<u8>, Error> {
        Ok(base64::decode(data)?)
    }

    fn get_url_data(data: &str) -> Result<Vec<u8>, Error> {
        let mut result = vec![];

        let mut iter = data.bytes();
        loop {
            if let Some(char) = iter.next() {
                if char == b'%' {
                    let a = iter.next();
                    let b = iter.next();
                    if let (Some(a), Some(b)) = (a, b) {
                        u8::from_str_radix(str::from_utf8(&[a, b])?, 16)?;
                    } else {
                        return Err(Error::PercentEncoding);
                    }
                } else {
                    result.push(char);
                }
            } else {
                break;
            }
        }

        Ok(result)
    }
}
