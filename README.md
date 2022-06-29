A pure Rust [Open Sound Control](https://opensoundcontrol.stanford.edu/) receiver for controlling APA102 and SK9822 LEDs.

The raves-on-rust receiver works like a screen - it accepts an array of OSC colors addressed to `led_strips/0` and renders the colors on the LEDs. You can generate patterns however you like programatically or via a OSC controller app and send them to the raves-on-rust receiver over wifi.

Supported Platforms:

- **Raspberry Pi** - Complete!
- **ESP32C3** - Work in progress / not useable / currently blocked on UDP support from esp-wifi.

Features:

- **Super simple rendering** - the raves-on-rust receiver displays the OSC colors array exactly as it's received
- **Scalable design** - a laptop can send to just one receiver for a small project or a whole array of raves-on-rust receivers for a larger installation.

## Useage

**New to Rust?** If your new to Rust you'll want to install [rustup](https://rustup.rs/) before anything else!

### Raspberry Pi Receiver

To build and upload the OSC LED receiver to a Pi:

1. `cd ./pi_osc_receiver`
2. `cargo build --target armv7-unknown-linux-gnueabihf && scp ./target/armv7-unknown-linux-gnueabihf/debug/pi_osc_receiver $PI:~/`

Then once your ssh'd into the pi you can run the LED controller with:

`RUST_LOG="pi_osc_receiver=trace" ./pi_osc_receiver`

If you'd like to have the pi automatically start the osc receiver each time it boots there is an example SystemD service file in `./pi_osc_receiver/pi_osc_receiver.service`.

### Testing the LEDs without Wifi

The `led_tester` program allows you to use an ESP32-C3-DevKitM-1 to turn on the LEDs so that you can test that your solders are good without needing a Raspberry Pi or wifi.
