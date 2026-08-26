# STM32F407 Discover Accelerometer Controller

This project uses the LIS3DSH accelerometer on the STM32F04-DISC1
development board as an input device on a Linux computer. To test it out we
have added the feature to use it as a controller for the camera position in
[Wgpu Grapher](https://github.com/seansovine/wgpu_grapher).

This project has two parts: The STM32F407 board code and a Rust client that
runs on a Linux PC.

The image below shows the board set up to control the grapher camera. Tilting
the board along the right-to-left axis rotates the camera around the horizontal
axis, and tilting the board along the front-to-back axis rotates the camera
around the vertical axis. Once the board has been programmed everything is
(mostly) plug-and-play.

<p align="center" margin="20px">
	<img src="https://github.com/seansovine/page_images/blob/main/photos/STM32F407%20accelerometer%20-%2020260816_111510.jpg?raw=true"
        alt="image of MCU echo server connected to PC" width="600" style="padding-top: 10px; padding-bottom: 10px"/>
</p>

The angles of rotation are computed using basic mechanics and trigonometry
and the fact that the acceleration due to gravity is a known quantity. The ST
[product page](https://www.st.com/content/st_com/en/products/mems-and-sensors/accelerometers/lis3dsh.html)
for the sensor has the data sheet and several application notes with further
information on its use. Angle computation from the raw data is currently done
in the Wgpu Grapher code; the client library here just presents a raw stream
of normalized data, currently with hard coded calibration for my test device.

## STM32F407 board code

The LIS3DSH sensor on the board is directly wired to the SPI1 peripheral of
the MCU. We have used CubeMX to make a CMake + C project that sets up this SPI
for communication with the sensor and also configures the on-chip USB On-the-Go
peripheral as a communication class device. When connected to a Linux PC
the USB connected to this peripheral will appear as a `/dev/ttyACMN` device
for some value of N.

The code for this part of the project is in the [`board_code`](./board_code/) subfolder.
The specific code that directly interfaces with the sensor is in
[`lis3dsh.h`](board_code/Core/Inc/lis3dsh.h) and [`lis3dsh.c`](board_code/Core/Src/lis3dsh.c).
The specific register addresses and values and the interpretation of the raw data
can be found in the device data sheet and application notes.

The architecture of the board code is a basic superloop that targets an \~60 hz rate
for sampling from the sensor. It encodes the sensor readings using a basic byte-stuffing protocol
with stop/start byte `0x00` and escape byte `0xFF`. The protocol is primarily
useful in the case when the Linux client starts receiving with data already in
the device buffer, as otherwise the device and protocols are generally reliable. But
it can also help with error recovery.

We aimed to keep most of the library independent of the specific underlying communication
protocol. For example, this may be more appropriate as a USB human input device, so we
may switch to that class in the future. Note also that 60 hz is quite slow for all the
hardware and communication protocols involved, so bandwidth is not really an issue here.

## Linux Rust client library

There is a Rust library in [`accel_client`](./accel_client/) that connects to a serial device
 `/dev/ttyACMN` using the rust `serialport` crate, and receives and decodes the data sent from
the board in a continuous loop. It runs the read loop on a background thread and shares the most
recent reading data with other threads using the `triple_buffer` crate. As long the read thread is
running it tries to connect with the specified serial device with a timeout on failure.

There is a demo program included that shows how to use the library, which runs the read
thread and logs the data periodically. This is usesful for debugging and for getting a
feel for how the sensor responds to physical inputs.
