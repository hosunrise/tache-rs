//! This is a binary running in the local environment
//!
//! You have to provide all needed configuration attributes via command line parameters,
//! or you could specify a configuration file. The format of configuration file is defined
//! in mod `config`.

use std::{io::Result as IoResult, net::SocketAddr, process};

use clap::{App, Arg};
use futures::{future::Either, Future};
use log::{debug, error, info};
use tokio::runtime::Runtime;

use tache::{run, Config, ConfigType, Mode, ServerAddr, ServerConfig};

mod logging;
mod monitor;

fn main() {
    let matches = App::new("tache")
        .version(shadowsocks::VERSION)
        .about("A fast tunnel proxy that helps you bypass firewalls.")
        .arg(
            Arg::with_name("VERBOSE")
                .short("v")
                .multiple(true)
                .help("Set the level of debug"),
        )
        .arg(
            Arg::with_name("CONFIG")
                .short("c")
                .long("config")
                .takes_value(true)
                .help("Specify config file"),
        )
        .get_matches();

    let debug_level = matches.occurrences_of("VERBOSE");

    logging::init(without_time, debug_level, "sslocal");

    let mut config = match matches.value_of("CONFIG") {
        Some(config_path) => match Config::load_from_file(config_path, ConfigType::Local) {
            Ok(cfg) => cfg,
            Err(err) => {
                error!("{:?}", err);
                return;
            }
        },
        None => Config::new(ConfigType::Local),
    };

    info!("ShadowSocks {}", shadowsocks::VERSION);

    debug!("Config: {:?}", config);

    match launch_server(config) {
        Ok(()) => {}
        Err(err) => {
            error!("Server exited unexpectly with error: {}", err);
            process::exit(1);
        }
    }
}

fn launch_server(config: Config) -> IoResult<()> {
    let mut runtime = Runtime::new().expect("Creating runtime");

    let abort_signal = monitor::create_signal_monitor();
    let result = runtime.block_on(run(config).select2(abort_signal));

    runtime.shutdown_now().wait().unwrap();

    match result {
        // Server future resolved without an error. This should never happen.
        Ok(Either::A(_)) => panic!("Server exited unexpectly"),
        // Server future resolved with an error.
        Err(Either::A((err, _))) => Err(err),
        // The abort signal future resolved. Means we should just exit.
        Ok(Either::B(..)) | Err(Either::B(..)) => Ok(()),
    }
}