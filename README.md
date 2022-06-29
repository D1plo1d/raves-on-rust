A pure Rust [Open Sound Control](https://opensoundcontrol.stanford.edu/) receiver for controlling APA102 and SK9822 LEDs.

The raves-on-rust receiver works like a screen - it accepts an array of OSC colors addressed to `led_strips/0` and then displays those colors using the addressable LED strip. You can generate patterns however you like - programatically or via a OSC controller app and send them to the raves-on-rust receiver over wifi.

Supported Platforms:

- **Raspberry Pi** - Complete!
- **ESP32C3** - Work in progress

Features:

- **Super simple rendering** - the raves-on-rust receiver displays OSC colors arrays exactly as it receives them so a receiver never needs to be re-compiled to show fancy new patterns. Any pattern you want can be displayed by just sending OSC packets over wifi.
- **Scalable design** - a laptop can send to just one receiver for a small project or a whole array of raves-on-rust receivers for a larger installation.
- **Build in Rust** - because who wouldn't want their LEDs running [the most loved language in the world?](https://www.reddit.com/r/rust/comments/owll2j/rust_is_the_most_loved_language_six_years_in_a/)

## Useage

**New to Rust?** You'll want to install [rustup](https://rustup.rs/) before anything else!

### Raspberry Pi Receiver

#### Wiring

First wire your LEDs to your Raspberry Pi by following the wiring instructions at: https://pimylifeup.com/raspberry-pi-led-strip-apa102/

#### Installation

To build and upload the OSC LED receiver to a Pi:

1. `cd ./pi_osc_receiver`
2. `cargo build --target armv7-unknown-linux-gnueabihf && scp ./target/armv7-unknown-linux-gnueabihf/debug/pi_osc_receiver $PI:~/`
3. Then ssh into the pi and run the LED controller with:
  `RUST_LOG="pi_osc_receiver=trace" ./pi_osc_receiver`
4. That's it! Now you can send OSC packets to your pi and your LEDs should light up!

If you'd like to have the pi automatically start the osc receiver each time it boots there is an example SystemD service file in `./pi_osc_receiver/pi_osc_receiver.service`.

### Testing the LEDs without Wifi

The `led_tester` program allows you to use an ESP32-C3-DevKitM-1 to turn on the LEDs so that you can test that your solders are good without needing a Raspberry Pi or wifi.
