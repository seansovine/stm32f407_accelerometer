use std::{
    env,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use time::macros::format_description;
use tracing::{error, info};
use tracing_subscriber::{EnvFilter, fmt::time::UtcTime};

use accel_client::*;

fn main() {
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

    let Ok(device) = env::var("ACCEL_DEVICE") else {
        error!("Environment variable ACCEL_DEVICE must be set to a valid ttyACM device.");
        std::process::exit(1);
    };
    info!("Got device from environment: {device}");

    let stop = Arc::new(AtomicBool::new(false));
    let (mut output_buf, handle) = run(device, stop.clone());

    for _ in 0..200 {
        if handle.is_finished() {
            break;
        }
        std::thread::sleep(Duration::from_millis(1000 / 60));
        let s = output_buf.read().debug_format();
        info!("Packet in buffer: \n{s}");
    }
    stop.store(true, Ordering::Relaxed);
    handle.join().unwrap();
}
