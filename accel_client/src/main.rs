use std::{env, time::Duration};

fn main() {
    let device = env::var("ACCEL_DEVICE")
        .expect("Environment variable ACCEL_DEVICE must be set to a valid ttyACM device.");

    let baud_rate = 1_000_000;
    let mut port = match serialport::new(&device, baud_rate)
        .timeout(Duration::from_millis(1000))
        .open()
    {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to open device {}: {}", device, e);
            std::process::exit(1);
        }
    };
    println!("Reading from device {}.", device);

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
                print!("Current buffer: ");
                for byte in &serial_buf[..t] {
                    print!("{:#04X} ", byte);
                }
                println!();
            }
            Err(e) => {
                eprintln!("Error reading data: {}", e);
                break;
            }
        }

        /*
         * Simple state machine to handle reading our data packet format.
         * See comments in the board C code for details.
         *
         * Not really necessayr for usb serial, except for the case that we
         * start receiving in the middle of a message or several messages are
         * currently buffered by the OS.
         */

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
                            println!("Invalid escape sequence.");
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
                        print!("Received packet: ");
                        for byte in &current_valid {
                            print!("{:#04X} ", byte);
                        }
                        println!();
                        process_packet(&current_valid);
                    } else {
                        println!("Invalid stop character received.");
                    }
                    reader_state = ReadState::NotStarted;
                }
            }
        }
        read_end = 0;
    }
}

fn process_packet(data: &[u8]) {
    if data.len() != 6 {
        println!("Invalid packet length.");
        return;
    }

    /*
     * Calibration constants, determined by experiment.
     * TODO: Add a calibration mode that writes a file.
     */

    const G_SCALE_X: f32 = 17_700.0;
    const G_SCALE_Y: f32 = 16_500.0;
    const G_SCALE_Z: f32 = 17_300.0;

    let a_x = (((data[0] as u16) << 8) | (data[1] as u16)) as i16;
    let a_x_f = (a_x as f32) / G_SCALE_X;

    let a_y = (((data[2] as u16) << 8) | (data[3] as u16)) as i16;
    let a_y_f = (a_y as f32) / G_SCALE_Y;

    let a_z = (((data[4] as u16) << 8) | (data[5] as u16)) as i16;
    let a_z_f = (a_z as f32) / G_SCALE_Z;

    println!("Current reading:");
    println!("Scaled A_x = {:+1.5}g ", a_x_f,);
    println!("Scaled A_y = {:+1.5}g", a_y_f);
    println!("Scaled A_Z = {:+1.5}g", a_z_f);
}
