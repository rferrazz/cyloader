extern crate cyapi;
extern crate clap;
extern crate log;
extern crate env_logger;

use cyapi::UpdateSession;
use clap::{AppSettings, Clap};
use log::{info, error};


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
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"));

    let options: Options = Options::parse();
    
    let mut session = UpdateSession::new(options.serial_port)?;

    info!("update started...");
    if let Err(error) = session.update(options.path) {
        error!("...update failed with error: {}", error);
        return Err(error)
    }
    info!("...update finished successfully!");

    Ok(())
}
