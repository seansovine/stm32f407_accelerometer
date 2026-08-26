//! Test program for accelerometer client library.

use std::{
    env,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use clap::{Arg, Command, value_parser};
use time::macros::format_description;
use tracing::{error, info};
use tracing_subscriber::{EnvFilter, fmt::time::UtcTime};

use accel_client::*;

fn main() {
    // If no "reads" arg is passed, runs continuously.
    let args = Command::new("accel_client")
        .arg(
            Arg::new("reads")
                .long("reads")
                .value_parser(value_parser!(usize)),
        )
        .get_matches();
    let num_reads = *args.get_one("reads").unwrap_or(&usize::MAX);

    // Setup logging.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_timer(UtcTime::new(format_description!(
            "[hour]:[minute]:[second].[subsecond digits:6]"
        )))
        .with_thread_ids(true)
        .init();

    // Try to get serial device to use from environment.
    let Ok(device) = env::var("ACCEL_DEVICE") else {
        error!("Environment variable ACCEL_DEVICE must be set to a valid ttyACM device.");
        std::process::exit(1);
    };
    info!("Got device from environment: {device}");

    let stop = Arc::new(AtomicBool::new(false));
    let (mut output_buf, handle) = run(device, stop.clone());

    // Read and log latest data at ~ 60 hz.
    for _ in 0..num_reads {
        if handle.is_finished() {
            break;
        }
        std::thread::sleep(Duration::from_millis(1000 / 60));
        info!("Packet in buffer: \n{}", output_buf.read().debug_format());
    }

    // Stop read thread and cleanup.
    stop.store(true, Ordering::Relaxed);
    handle.join().unwrap();
}
