
use std::io;
use std::io::Read;
use std::fs::File;
use byteorder::{BigEndian, ReadBytesExt};

pub struct BootloaderRow {
    pub array_id: u8,
    pub row_number: u16,
    pub data: Vec<u8>,
    pub checksum: u8,
}

impl BootloaderRow {
    fn read<T: io::BufRead>(reader: &mut T) -> Result<BootloaderRow, io::Error> {
        let column = reader.read_u8()?;
        if column != b':' {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "expected :"));
        }

        let content = read_hex_line(reader)?;
        let mut content_slice = content.as_slice();

        let array_id = content_slice.read_u8()?;
        let row_number = content_slice.read_u16::<BigEndian>()?;
        let length = content_slice.read_u16::<BigEndian>()?;
        let mut data = vec![0u8; length as usize];
        content_slice.read_exact(data.as_mut_slice())?;

        let checksum = content_slice.read_u8()?;
        Ok(BootloaderRow{
            array_id: array_id,
            row_number: row_number,
            data: data,
            checksum: checksum,
        })
    }
}

pub struct BootloaderData {
    pub silicon_id: u32,
    pub silicon_rev: u8,
    pub checksum_kind: u8,
    pub rows: Vec<BootloaderRow>,
}

impl BootloaderData {
    fn read_bootloader_info<T: io::BufRead>(reader: &mut T) -> Result<(u32, u8, u8), io::Error> {
        let header = read_hex_line(reader)?;
        println!("Header row size: {}", header.len());

        let mut header_slice = header.as_slice();

        let silicon_id = header_slice.read_u32::<BigEndian>()?;
        let silicon_rev = header_slice.read_u8()?;
        let checksum_type = header_slice.read_u8()?;

        Ok((silicon_id, silicon_rev, checksum_type))
    }

    pub fn from_file(filename: String) -> Result<BootloaderData, io::Error> {
        let f = File::open(filename)?;
        let reader = io::BufReader::new(f);
        return BootloaderData::read(reader);
    }

    pub fn read<T: io::BufRead>(reader: T) -> Result<BootloaderData, io::Error> {
        let mut rows = Vec::<BootloaderRow>::new();
        let mut header_data: (u32, u8, u8) = (0, 0, 0);
        let lines_iter = reader.lines().enumerate();
        for (index, line) in lines_iter {
            if let Ok(content) = &line {
                let mut bytes = content.as_bytes();
                if index == 0 {
                    header_data = BootloaderData::read_bootloader_info(&mut bytes)?;
                } else {
                    let row = BootloaderRow::read(&mut bytes)?;
                    rows.push(row);
                }
            }
        }

        return Ok(BootloaderData {
            silicon_id: header_data.0,
            silicon_rev: header_data.1,
            checksum_kind: header_data.2,
            rows: rows,
        })
    }
}

fn read_hex_line<T: io::BufRead>(reader: &mut T) -> Result<Vec<u8>, io::Error> {
    let mut raw_data = String::new();
    reader.read_line(&mut raw_data)?;
    match hex::decode(raw_data) {
        Ok(result) => {
            return Ok(result)
        },
        _ => {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "cannot hexify string"))
        },
    }
}