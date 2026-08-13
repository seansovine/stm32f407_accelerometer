use std::{env, error::Error, fmt::Write, thread, time::Duration};

use serialport::SerialPort;
use time::macros::format_description;
use tracing::{debug, error, info, warn};
use tracing_subscriber::{EnvFilter, fmt::time::UtcTime};

fn main() {
    // Setup logging.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_timer(UtcTime::new(format_description!(
            "[hour]:[minute]:[second].[subsecond digits:6]"
        )))
        .init();

    let Ok(device) = env::var("ACCEL_DEVICE") else {
        error!("Environment variable ACCEL_DEVICE must be set to a valid ttyACM device.");
        std::process::exit(1);
    };
    let baud_rate = 1_000_000_u32;

    loop {
        let mut port = match try_connect(&device, baud_rate) {
            Ok(p) => p,
            Err(e) => {
                error!("Failed to open device {}: {}", device, e);
                thread::sleep(Duration::from_secs(2));
                continue;
            }
        };

        info!("Reading from device {}.", device);
        receive(&mut *port);
    }
}

fn try_connect(device_name: &str, baud_rate: u32) -> Result<Box<dyn SerialPort>, Box<dyn Error>> {
    match serialport::new(device_name, baud_rate)
        .timeout(Duration::from_millis(1000))
        .open()
    {
        Ok(p) => Ok(p),
        Err(e) => Err(Box::from(e)),
    }
}

fn byte_string(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 5);
    for byte in bytes {
        write!(&mut s, "{:#04X} ", byte).unwrap();
    }
    s
}

fn receive(port: &mut dyn SerialPort) {
    let mut serial_buf: Vec<u8> = vec![0; 1024];
    let mut read_end = 0;

    enum ReadState {
        Started,
        NotStarted,
        FirstStopSeen,
    }
    let mut reader_state = ReadState::NotStarted;
    let mut current_valid: Vec<u8> = Vec::with_capacity(12);

    loop {
        match port.read(&mut serial_buf) {
            Ok(t) => {
                read_end += t;
                let s = byte_string(&serial_buf[..t]);
                debug!("Current buffer: {s}");
            }
            Err(e) => {
                error!("Error reading data: {}", e);
                break;
            }
        }

        // Simple state machine to read a stream of our data packets from
        // a serial port. See comments in the board C code for details.
        //
        // Currently used for USB ACM comm port, but can be used for more
        // general serial connections.

        let mut escaped = false;
        for &byte in &serial_buf[0..read_end] {
            match reader_state {
                ReadState::NotStarted => {
                    if byte == 0x00 {
                        reader_state = ReadState::Started;
                        current_valid.clear();
                    }
                }
                ReadState::Started => {
                    if !escaped && byte == 0xFF {
                        escaped = true;
                        continue;
                    }
                    if escaped {
                        if byte != 0x00 && byte != 0xFF {
                            warn!("Invalid escape sequence. Dropping current packet.");
                            reader_state = ReadState::NotStarted;
                        } else {
                            current_valid.push(byte);
                        }
                        escaped = false;
                    } else {
                        if byte == 0x00 {
                            reader_state = ReadState::FirstStopSeen;
                        } else {
                            current_valid.push(byte);
                        }
                    }
                }
                ReadState::FirstStopSeen => {
                    if byte == 0x00 {
                        let s = byte_string(&current_valid);
                        debug!("Received packet: {s}");
                        if let Some(reading) = process_packet(&current_valid) {
                            reading.log_to_info();
                        } else {
                            warn!(
                                "Invalid packet length: {}. Packet was dropped.",
                                current_valid.len()
                            );
                        }
                    } else {
                        warn!("Invalid stop character received. Dropping current packet.");
                    }
                    reader_state = ReadState::NotStarted;
                }
            }
        }
        read_end = 0;
    }
}

struct Reading {
    pub a_x_g: f32,
    pub a_y_g: f32,
    pub a_z_g: f32,
}

impl Reading {
    fn log_to_info(&self) {
        info!("Current reading:");
        info!("Scaled A_x = {:+1.5}g ", self.a_x_g,);
        info!("Scaled A_y = {:+1.5}g", self.a_y_g);
        info!("Scaled A_Z = {:+1.5}g", self.a_z_g);
    }
}

fn process_packet(data: &[u8]) -> Option<Reading> {
    if data.len() != 6 {
        return None;
    }

    // Calibration constants, determined by experiment.
    //
    // TODO: Add a calibration mode that writes a file.

    const G_SCALE_X: f32 = 17_700.0;
    const G_SCALE_Y: f32 = 16_500.0;
    const G_SCALE_Z: f32 = 17_300.0;

    let a_x = (((data[0] as u16) << 8) | (data[1] as u16)) as i16;
    let a_x_g = (a_x as f32) / G_SCALE_X;

    let a_y = (((data[2] as u16) << 8) | (data[3] as u16)) as i16;
    let a_y_g = (a_y as f32) / G_SCALE_Y;

    let a_z = (((data[4] as u16) << 8) | (data[5] as u16)) as i16;
    let a_z_g = (a_z as f32) / G_SCALE_Z;

    Some(Reading {
        a_x_g,
        a_y_g,
        a_z_g,
    })
}
