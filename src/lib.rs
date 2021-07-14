extern crate serialport;
extern crate num_enum;
extern crate byteorder;
extern crate hex;

mod cyacd;
pub use cyacd::{DataRecord, ApplicationData};

use std::io;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use num_enum::{IntoPrimitive, FromPrimitive};

const START_BYTE: u8 = 0x01;
const END_BYTE: u8 = 0x17;
pub const MAX_DATA_LENGTH: usize = 64-7-9;


#[derive(IntoPrimitive, FromPrimitive, Clone, Copy, Debug)]
#[repr(u8)]
pub enum CommandCode {
    Success = 0x00,
    VerificationError = 0x02,
    LengthError = 0x03,
    DataError = 0x04,
    CommandError = 0x05,
    DeviceError = 0x06,
    VersionError = 0x07,
    ChecksumError = 0x08,
    FlashArrayError = 0x09,
    RowError = 0x0a,
    AppError = 0x0c,
    ActiveError = 0x0d,
    UnknownError = 0x0f,
    #[num_enum(default)]
    VerifyChecksum = 0x31,
    GetFlashSize = 0x32,
    GetAppStatus = 0x33,
    EraseRow = 0x34,
    SyncBootloader = 0x35,
    SetActiveApp = 0x36,
    SendData = 0x37,
    EnterBootloader = 0x38,
    ProgramRow = 0x39,
    VerifyRow = 0x3a,
    ExitBootloader = 0x3b,  
}


pub struct BootloaderCommand {
    pub command_code: CommandCode,
    pub data: Vec::<u8>,
}

impl BootloaderCommand {
    pub fn write<T: io::Write>(&self, writer: &mut T) -> Result<(), io::Error> {
        let mut buffer = vec![];
        buffer.write_u8(START_BYTE)?;
        buffer.write_u8(u8::from(self.command_code))?;
        buffer.write_u16::<LittleEndian>(self.data.len() as u16)?;
        writer.write_all(buffer.as_slice())?;
        writer.write_all(self.data.as_slice())?;
        writer.write_u16::<LittleEndian>(checksum( &[buffer.as_slice(), self.data.as_slice()].concat()))?;
        writer.write_u8(END_BYTE)?;
        Ok(())
    }

    pub fn unmarshal<T: io::Read>(reader: &mut T) -> Result<BootloaderCommand, io::Error> {
        let start = reader.read_u8()?;
        if start != START_BYTE {
            return Err(io::Error::new(io::ErrorKind::InvalidData, format!("start byte does not match {:#0x}", start)));
        }

        let command = reader.read_u8()?;

        let length = reader.read_u16::<LittleEndian>()?;
        let mut data = vec![0u8; length as usize];
        reader.read_exact(data.as_mut_slice())?;

        let checksum = reader.read_u16::<LittleEndian>()?;
        // TODO: check checksum

        let end = reader.read_u8()?;
        if end != END_BYTE {
            return Err(io::Error::new(io::ErrorKind::InvalidData, format!("end byte does not match: {:#0x}", end)));
        }

        return Ok(BootloaderCommand{
            command_code: CommandCode::from(command),
            data: data,
        });
    }

    pub fn read<T: io::Read>(reader: &mut T, attempts: u8) -> Result<BootloaderCommand, io::Error> {
        for _ in 0..attempts {
            match BootloaderCommand::unmarshal(reader) {
                Ok(result) => return Ok(result),
                Err(error) => println!("{:?}", error),
            }
        }
        return Err(io::Error::new(io::ErrorKind::InvalidData, format!("failed reading a meaningful message after {} attempts", attempts)))
    }
}

pub fn checksum(data: &[u8]) -> u16 {
    let sum: usize =
        data
        .iter()
        .fold(0, |acc, &value| acc.wrapping_add(value as usize));

    let lsb = (sum & 0xFFFF) as u16;
    let checksum = (0 as u16).wrapping_sub(lsb);
    checksum
}