# STM32F407 Discover Accelerometer Controller

This project allows using the LIS3DSH accelerometer on the STM32F04-DISC1
development board as an input device on a Linux computer. We're currently
using it as a controller for the camera position in
[Wgpu Grapher](https://github.com/seansovine/wgpu_grapher), as a test.

It has two parts: The STM32F407 board code and a Rust client that runs on
a Linux PC.

<p align="center" margin="20px">
	<img src="https://github.com/seansovine/page_images/blob/main/photos/STM32F407%20accelerometer%20-%2020260816_111510.jpg?raw=true"
        alt="image of MCU echo server connected to PC" width="600" style="padding-top: 10px; padding-bottom: 10px"/>
</p>

The image shows the board setup to control the grapher camera. Tilting the
board along the right-to-left axis rotates the camera around the horizontal
axis, and tilting the board along the front-to-back axis rotates the camera
around the vertical axis.

## STM32F407 board code

The LIS3DH sensor on the board is directly wired to the SPI1 peripheral of
the MCU. We have used CubeMX to configure a project that sets up this SPI
for communication with device, and also configures the on-chip USB on-the-go
peripheral as for Communication Device Class. When connected to a Linux PC
the USB connected to this peripheral will appear in Linux as a `ttyACM*`
device. The code for this is in [`board_code`](./board_code/). The code that
directly interfaces the sensor is in [`lis3dsh.h`](board_code/Core/Inc/lis3dsh.h)
and [`lis3dsh.c`](board_code/Core/Src/lis3dsh.c).

The architecture is a superloop that targets \~60 hz sampling rate from the
sensor. It encodes the sensor reading using a basic byte-stuffing protocol
with stop/start byte `0x00` and escape byte `0xFF`. The protocol is primarily
used for the case then the Linux client starts receiving with data already in
the TTY device buffer. Note that 60 hz is quite slow for all the hardware and
communication protocols involved, so bandwidth is not really an issue here.

## Linux Rust client library

There is a Rust library in [`accel_client`](./accel_client/) that connects to
a serial device `/dev/ttyACM*` using the rust serialport crate, receives the
data sent from the board, and decodes it. It runs the continuous read loop on
a background thread and shares the most recent reading with other threads using
the triple_buffer crate.
