use anyhow::{ Context, Result };
use log::{info};

use rppal::spi::{
    Bus,
    Mode,
    SlaveSelect,
    Spi,
};

use apa102_spi::Apa102;
use local_ip_address::local_ip;

use osc_receiver::led_strip::{ LedStrip, RGB8SmartLedsWrite };
// use smart_leds::RGB8;

const MHZ: u32 = 1_000_000u32;

const PORT: u16 = 8001;

pub fn main() -> Result<()> {
    pretty_env_logger::init();


    // Print the local ip address
    if let Ok(ip_address) = local_ip() {
        println!("Listening for OSC packets at {}:{}\n", ip_address, PORT);
    } else {
        println!("Listening for OSC packets on port {}\n", PORT);
    }

    // Modes: https://en.wikipedia.org/wiki/Serial_Peripheral_Interface_Bus#Clock_polarity_and_phase
    let spi: Spi = Spi::new(
        Bus::Spi0,
        SlaveSelect::Ss0,
        12 * MHZ,
        Mode::Mode0,
    )
        .context("Unable to connect to SPI. Are you not running on a Raspberry Pi?")?;

    let mut apa102 = Apa102::new(spi);

    println!("SPI Connected");

    let mut smart_leds: Vec<&mut dyn RGB8SmartLedsWrite> = Vec::new();

    smart_leds.push(&mut apa102);

    let mut led_strips = smart_leds
        .into_iter()
        .map(LedStrip::new)
        .collect::<Vec<_>>();

    info!("Starting main loop");

    use std::net::UdpSocket;

    let socket = UdpSocket::bind(format!("0.0.0.0:{}", PORT))
        .expect("couldn't bind to address");

        let mut packet_buf = [0; 65_507];

    loop {
        let packet_size = socket.recv(&mut packet_buf).unwrap();

        let osc_packet = rosc::decoder::decode_udp(&packet_buf[..packet_size]);

        if let Ok((&[], osc_packet)) = osc_packet {
            info!("OSC Packet Received");
            // info!("OSC Packet: {}", osc_packet);
            LedStrip::update(&mut led_strips, osc_packet);
        }
    }
}
