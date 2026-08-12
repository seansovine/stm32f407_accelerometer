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
    let mut current_read = 0;
    let mut current_write = 0;

    const MAX_MSG_LEN: usize = 15;

    enum ReadState {
        Started,
        NotStarted,
        FirstStopSeen,
    }
    let mut reader_state = ReadState::NotStarted;
    let mut current_valid: Vec<u8> = Vec::with_capacity(12);

    loop {
        if current_write > serial_buf.len() - MAX_MSG_LEN {
            serial_buf.copy_within(current_read..current_write, 0);
            current_write = current_write - current_read;
            current_read = 0;
        }

        match port.read(&mut serial_buf[current_write..]) {
            Ok(t) => {
                current_write += t;
                if false {
                    print!("{}", String::from_utf8_lossy(&serial_buf[..t]));
                } else if true {
                    for byte in &serial_buf[..t] {
                        print!("{:#04X} ", byte);
                    }
                    println!();
                }
            }
            Err(e) => {
                eprintln!("Error reading data: {}", e);
                break;
            }
        }

        let mut escaped = false;
        for i in current_read..current_write {
            match reader_state {
                ReadState::NotStarted => {
                    if serial_buf[i] == 0 {
                        reader_state = ReadState::Started;
                        current_valid.clear();
                    }
                }
                ReadState::Started => {
                    if !escaped && serial_buf[i] == 0xFF {
                        escaped = true;
                        continue;
                    }
                    if escaped {
                        if serial_buf[i] != 0x00 && serial_buf[i] != 0xFF {
                            println!("Invalid escape sequence.");
                            reader_state = ReadState::NotStarted;
                        } else {
                            current_valid.push(serial_buf[i]);
                            escaped = false;
                        }
                    } else {
                        if serial_buf[i] == 0x00 {
                            reader_state = ReadState::FirstStopSeen;
                        } else {
                            current_valid.push(serial_buf[i]);
                        }
                    }
                }
                ReadState::FirstStopSeen => {
                    if serial_buf[i] == 0x00 {
                        print!("Received packet: ");
                        for byte in &current_valid {
                            print!("{:#04X} ", byte);
                        }
                        println!();
                    } else {
                        println!("Invalid stop character received.");
                    }
                    current_valid.clear();
                }
            }
        }
        current_read = current_write;
    }
}
