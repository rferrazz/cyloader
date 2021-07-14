extern crate cyloader;
extern crate clap;
extern crate serialport;
extern crate byteorder;

use cyloader::{BootloaderData, BootloaderCommand, CommandCode, MAX_DATA_LENGTH};
use clap::{AppSettings, Clap};
use std::time::{Duration};
use byteorder::{LittleEndian, ReadBytesExt};


#[derive(Clap)]
#[clap(version = "1.0", author = "Riccardo Ferrazzo <f.riccardo87@gmail.com>")]
#[clap(setting = AppSettings::ColoredHelp)]
struct Options {
    #[clap(short, long, required = true)]
    path: String,

    #[clap(short, long, required = true)]
    serial_port: String
}

fn main() -> Result<(), std::io::Error> {
    let options: Options = Options::parse();

    let bootloader = BootloaderData::from_file(options.path).unwrap();
    println!("Silicon id: {:#0x}, rev: {:#0x}, checksum_kind: {:#0x}", bootloader.silicon_id, bootloader.silicon_rev, bootloader.checksum_kind);

    let mut port = serialport::new(options.serial_port, 115_200)
    .timeout(Duration::from_millis(100)).parity(serialport::Parity::None)
    .open().expect("Failed to open port");

    let enter_bootloader = BootloaderCommand{
        command_code: CommandCode::EnterBootloader,
        data: vec![],
    };

    enter_bootloader.write(&mut port)?;

    let reply = BootloaderCommand::read(&mut port, 100)?;
    println!("Enter bootloader reply: {:?}", reply.command_code);
    let silicon_id = reply.data.as_slice().read_u16::<LittleEndian>()?;
    assert_eq!(silicon_id, bootloader.silicon_id);

    // TODO: send update
    for row in bootloader.rows {
        if row.data.len() > MAX_DATA_LENGTH {
            let slice =  &row.data[..MAX_DATA_LENGTH];
            let data_vec = Vec::new();
            data_vec.extend(slice);
            
            let command = BootloaderCommand{
                command_code: CommandCode::SendData,
                data: data_vec,
            };
        }
    }

    let exit_bootloader = BootloaderCommand {
        command_code: CommandCode::ExitBootloader,
        data: vec![],
    };

    exit_bootloader.write(&mut port)?;

    Ok(())
}
