use std::{
    error::Error,
    fmt::Write,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use serialport::SerialPort;
use tracing::{error, info, trace, warn};
use triple_buffer::{Input, triple_buffer};

pub use triple_buffer::Output;

#[derive(Clone, Copy, Default)]
pub struct Reading {
    pub a_x_g: f32,
    pub a_y_g: f32,
    pub a_z_g: f32,
    pub valid: bool,
}

impl Reading {
    pub fn debug_format(&self) -> String {
        let mut s = String::with_capacity(21 * 3 + 9);
        writeln!(&mut s, "Scaled A_x = {:+1.5}g", self.a_x_g).unwrap();
        writeln!(&mut s, "Scaled A_y = {:+1.5}g", self.a_y_g).unwrap();
        writeln!(&mut s, "Scaled A_Z = {:+1.5}g", self.a_z_g).unwrap();

        let valid = if self.valid { "[valid]" } else { "[invalid]" };
        write!(&mut s, "{valid}").unwrap();

        s
    }
}

pub fn run(device_name: String, stop: Arc<AtomicBool>) -> (Output<Reading>, JoinHandle<()>) {
    let (mut buf_input, buf_output) = triple_buffer::<Reading>(&Default::default());

    let handle = thread::Builder::new()
        .spawn(move || {
            let baud_rate = 1_000_000_u32;
            while !stop.load(Ordering::Relaxed) {
                let mut port = match try_connect(&device_name, baud_rate) {
                    Ok(p) => p,
                    Err(e) => {
                        buf_input.input_buffer_mut().valid = false;
                        buf_input.publish();

                        error!("Failed to open device {}: {}", device_name, e);
                        error!("Retrying in 2 seconds...");

                        thread::sleep(Duration::from_secs(2));
                        continue;
                    }
                };

                info!("Reading from device {}.", device_name);
                receive(&mut *port, &mut buf_input, &stop);
            }
        })
        .unwrap();

    (buf_output, handle)
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
    let mut string_rep = String::with_capacity(bytes.len() * 5);
    for byte in bytes {
        write!(&mut string_rep, "{:#04X} ", byte).unwrap();
    }
    string_rep
}

fn receive(port: &mut dyn SerialPort, buf_input: &mut Input<Reading>, stop: &AtomicBool) {
    let mut serial_buf: Vec<u8> = vec![0; 1024];
    let mut read_end = 0;

    enum ReadState {
        Started,
        NotStarted,
        FirstStopSeen,
    }

    let mut reader_state = ReadState::NotStarted;
    let mut current_valid: Vec<u8> = Vec::with_capacity(12);

    while !stop.load(Ordering::Relaxed) {
        match port.read(&mut serial_buf) {
            Ok(t) => {
                read_end += t;
                let s = byte_string(&serial_buf[..t]);
                trace!("Current buffer: {s}");
            }
            Err(e) => {
                error!("Error reading data: {}", e);
                buf_input.input_buffer_mut().valid = false;
                buf_input.publish();
                break;
            }
        }

        // Simple state machine to read a stream of our data packets from
        // a serial port. See comments in the board C code for details.
        //
        // Currently used for USB ACM comm port, but can be used for more
        // general serial connections.
        //
        // TODO: IF device starts before reader, sometimes gets stuck in
        //       a bad state for ~10 read cycles.

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
                            buf_input.input_buffer_mut().valid = false;
                            buf_input.publish();
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
                        trace!("Received packet: {s}");
                        if let Some(reading) = process_packet(&current_valid) {
                            let s = reading.debug_format();
                            trace!("Processed packet:\n{s}");
                            buf_input.write(reading);
                        } else {
                            warn!(
                                "Invalid packet length: {}. Packet was dropped.",
                                current_valid.len()
                            );
                            buf_input.input_buffer_mut().valid = false;
                            buf_input.publish();
                        }
                    } else {
                        warn!("Invalid stop character received. Dropping current packet.");
                        buf_input.input_buffer_mut().valid = false;
                        buf_input.publish();
                    }
                    reader_state = ReadState::NotStarted;
                }
            }
        }

        read_end = 0;
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
        valid: true,
    })
}
