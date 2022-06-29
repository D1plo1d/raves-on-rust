A pure Rust OSC LED Receiver for APA102 LEDs.

## Useage

### Raspberry Pi

To build and upload the OSC LED receiver to a Pi:

1. `cd ./pi_osc_receiver`
2. `cargo build --target armv7-unknown-linux-gnueabihf && scp ./target/armv7-unknown-linux-gnueabihf/debug/micro-tree-rust $PI:~/`

Then once your ssh'd into the pi you can run the LED controller with:

`RUST_LOG="micro_tree_rust=trace" ./micro-tree-rust`

### Testing the LEDs without Wifi

The `led_tester` program allows you to use an ESP32-C3-DevKitM-1 to turn on the LEDs so that you can test that your solders are good without needing a Raspberry Pi or wifi.
