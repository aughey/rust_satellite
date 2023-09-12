# Rust-Satellite

This is intended to be a clone in functionality of the [Companion Satellite](https://github.com/bitfocus/companion-satellite) application that is intended to be compiled and run on low-profile computers such as the Raspberry Pi Zero.

My intended use is a remote control for an X32 audio mixer.  The desired use is to have a Stream Deck Plus sitting on the piano of the choir director and give them full control over the fader positions of each microphone in their chior.  My goal is to run the satellite application on a Raspberry Pi Zero with a PoE hat so that the footprint is tiny.  Ideally the PI is packaged inside the nook of the Stream Deck with a single cable to connect power and data.  Alternate packaging might use a LiPo battery for power and be completely wireless.

Currently the satellite application is written in NodeJS, and the makers do not recommend nor support it running on anything less than a Raspberry PI 4.  I believe this is due to the heavy-weight overhead of node and the (possibly) lack of availability of the toolsets for the lower powered PI computers.  However, I also believe the computer is adequetly powered and, if written in a compiled language like Rust, will be able to perform its function in a low-resource environment.

# Building

```
PKG_CONFIG_PATH=/usr/lib/arm-linux-gnueabihf/pkgconfig cross build --release --target arm-unknown-linux-gnueabihf
```