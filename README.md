# Rust Satellite

Rust Satellite is a reimplementation of [Companion Satellite](https://bitfocus.io/companion-satellite) functionality, built using Rust. The aim is to provide a natively-compiled satellite application optimized for low-resource devices.

The project originated from a weekend effort to integrate [Companion](https://bitfocus.io/companion) with a [Companion Satellite](https://bitfocus.io/companion-satellite) application on a Raspberry Pi Zero W. The Raspberry Pi Zero's compact form factor was chosen to enable a low-power, low-profile solution that could be powered via PoE (Power over Ethernet).

# Capabilities

As of now, only the Streamdeck series of devices are supported.

## rust_satellite 

`rust_satellite ` is a fully-functional version of the satellite application that establishes a direct connection to the Companion app, using its existing ASCII protocol.

## gateway

`gateway` is designed to function alongside a `leaf` application. It serves as a middleman between the Companion app and low-resource `leaf` applications. gateway communicates with Companion using the ASCII protocol and forwards pre-formatted binary data to the leaf nodes. The objective is to offload resource-intensive tasks, like image processing, to a more capable host computer, thereby minimizing the resource needs of the end leaf nodes.

## leaf

`leaf` represents a leaf node connected to a `gateway` and the actual Streamdeck hardware. With minimal processing requirements, leaf is designed to operate even on basic embedded microcontrollers.

# Academic

While most users might find `rust_satellite` sufficient for their needs, the `gateway/leaf` architecture serves as an exploratory endeavor to push the boundaries of what's possible. One aim is to run a version of the leaf application on a Teensy 4.1 microcontroller.

The academic objectives include:

- Developing a leaf project using Rust's no-std feature, as described in the [Embedded rust book](https://docs.rust-embedded.org/book/intro/no-std.html)
- Utilizing FFI (Foreign Function Interface) to leverage existing Ethernet and USB libraries for Arduino, as described in this [Blog](https://dev.to/kgrech/five-simple-steps-to-use-any-arduino-c-library-in-a-rust-project-1k78) and in repositories like [QNEthernet](https://github.com/ssilverman/QNEthernet) and [NativeEthernet](https://github.com/vjmuzik/NativeEthernet)
- Employing the [smoltcp Rust IP stack](https://github.com/smoltcp-rs/smoltcp) for networking functionalities.

# Background

This project aims to clone the functionality of Companion Satellite for low-profile computing devices like the Raspberry Pi Zero. My intended application is as a remote control for an X32 audio mixer. The setup would allow a choir director to control microphone fader positions directly from a Stream Deck Plus located on a piano. The goal is to power the satellite application with a PoE-enabled Raspberry Pi Zero, ideally concealed within the Stream Deck's housing, connected by a single cable for power and data. Alternative setups might utilize a LiPo battery for a completely wireless solution.

The original satellite application is written in NodeJS and is not recommended for use on devices with less computing power than a Raspberry Pi 4. This limitation is likely due to NodeJS's resource-intensive nature and potentially limited toolset support for lower-end Raspberry Pi models. However, I believe that a Rust-based reimplementation could efficiently run on these low-resource devices.

# Update (gateway configuration)

To follow up on the native pi zero satellite application I wrote last weekend, I have a new version that uses a gateway application running on the host to do the heavy lifting of image manipulation and formatting.  The configuration looks like `Companion <-- ascii --> Gateway <-- binary --> Leaf(pi)    The desire is to ship bits to the leaf nodes in the exact format that is needed to write to the device.

This is now the intended use.

# Cross-compiling with cross

To cross-compile the application, first install Cross using `cargo install cross`.  Then, run the following command:

```
PKG_CONFIG_PATH=/usr/lib/arm-linux-gnueabihf/pkgconfig cross build --release --target arm-unknown-linux-gnueabihf
```
