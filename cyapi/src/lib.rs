extern crate serialport;
extern crate num_enum;
extern crate byteorder;
extern crate hex;
extern crate log;

mod cyacd;
pub use cyacd::{DataRecord, ApplicationData};

use std::io;
use std::time::Duration;
use std::cmp::min;
use std::thread::sleep;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use num_enum::{IntoPrimitive, FromPrimitive};
use log::{debug, warn};

const START_BYTE: u8 = 0x01;
const END_BYTE: u8 = 0x17;
const SLEEP_TIME_BETWEEN_RETRIES: Duration = Duration::from_millis(400);
pub const MAX_DATA_LENGTH: usize = 64-7;


#[derive(IntoPrimitive, FromPrimitive, Clone, Copy, Debug, PartialEq)]
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
    pub fn marshal<T: io::Write>(&self, writer: &mut T) -> Result<(), io::Error> {
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

pub struct UpdateSession {
    serial: std::boxed::Box<dyn serialport::SerialPort>,
    silicon_id: u32,
}

fn retry<F>(max_iterations: u16, mut function: F) -> Result<(), io::Error>
where F: FnMut(u16) -> Result<(), io::Error>
{
    let mut last_error = io::Error::new(io::ErrorKind::Other, "cannot try executing a function 0 times");
    
    if max_iterations < 1 {
        return Err(last_error);
    }

    for i in 0..max_iterations {
        match function(i) {
            Ok(val) => return Ok(val),
            Err(error) => {
                last_error = error;
                sleep(SLEEP_TIME_BETWEEN_RETRIES);
            },
        }
    }

    return Err(last_error);
}

impl UpdateSession{
    pub fn new(serial: String) -> Result<UpdateSession, io::Error> {
        let mut port = serialport::new(serial, 115_200)
            .timeout(Duration::from_millis(100)).parity(serialport::Parity::None)
            .open().expect("Failed to open port");

        let start_session = BootloaderCommand {
            command_code: CommandCode::EnterBootloader,
            data: vec![],
        };
        start_session.marshal(&mut port)?;
        let reply = BootloaderCommand::read(&mut port, 5)?;
        let silicon_id = reply.data.as_slice().read_u32::<LittleEndian>()?;

        Ok(UpdateSession{
            serial: port,
            silicon_id: silicon_id,
        })
    }

    pub fn update(&mut self, update_file_path: String, max_iterations_on_error: u16) -> Result<(), io::Error> {
        let app_data = ApplicationData::from_file(update_file_path)?;

        if app_data.silicon_id != self.silicon_id {
            return Err(io::Error::new(io::ErrorKind::InvalidData, format!("update_file for another chip: {:#0x}", app_data.silicon_id)));
        }

        for row in app_data.rows {

            debug!("Programming row number {}. array id: {}, data size: {}", row.row_number, row.array_id, row.data.len());
            let chunk_count = row.data.len() / MAX_DATA_LENGTH;

            for i in 0..chunk_count+1 {
                let index = i*MAX_DATA_LENGTH;
                let slice = &row.data[index..min(row.data.len(), index + MAX_DATA_LENGTH)];
                let mut data = vec![];

                let command = if i == chunk_count {
                    data.write_u8(row.array_id)?;
                    data.write_u16::<LittleEndian>(row.row_number)?;
                    data.extend(slice);

                    BootloaderCommand{
                        command_code: CommandCode::ProgramRow,
                        data: data,
                    }
                } else {
                    data.extend(slice);

                    BootloaderCommand {
                        command_code: CommandCode::SendData,
                        data: data,
                    }
                };

                if let Err(error) = retry(max_iterations_on_error, |iteration: u16| {
                    command.marshal(&mut self.serial)?;
                    let reply = BootloaderCommand::unmarshal(&mut self.serial)?;
                    if reply.command_code != CommandCode::Success {
                        warn!("failed writing update chunk for the {} time: {:?}", iteration+1, reply.command_code);
                        return Err(io::Error::new(io::ErrorKind::Other, format!("failed writing update data {:?} for the {} time", reply.command_code, iteration+1)));
                    }
                    Ok(())
                }) {
                    return Err(error);
                }
            }
        }

        Ok(())
    }
}

impl Drop for UpdateSession {
    fn drop(&mut self) {
        let end_session = BootloaderCommand {
            command_code: CommandCode::ExitBootloader,
            data: vec![],
        };

        if let Err(error) = end_session.marshal(&mut self.serial) {
            println!("Failed closing bootloader session: {}", error);
        }
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