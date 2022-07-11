use alloc::vec::Vec;
use anyhow::Result;
use core::convert::Infallible;
use core::default::Default;
use core::iter::Iterator;
#[cfg(feature = "std")]
use core::marker::{Send, Sync};
use core::option::Option::*;
use core::result::Result::*;
use embedded_hal::blocking::spi::Write;
use embedded_hal::spi::FullDuplex;
use log::{info, trace, warn};
use rosc::{OscPacket, OscType};
use smart_leds::{SmartLedsWrite, RGB8};

// pub const STRIP_LENGTH: usize = 450;
pub const STRIP_LENGTH: usize = 450;

type LedStripData = [RGB8; STRIP_LENGTH];

pub trait RGB8SmartLedsWrite {
    fn write_rgb8(&mut self, iterator: &mut dyn Iterator<Item = RGB8>) -> Result<()>;
}

#[cfg(feature = "std")]
impl<SPI, E> RGB8SmartLedsWrite for apa102_spi::Apa102<SPI>
where
    SPI: Write<u8, Error = E>,
    E: std::error::Error + Send + Sync + 'static,
{
    fn write_rgb8(&mut self, iterator: &mut dyn Iterator<Item = RGB8>) -> Result<()> {
        use anyhow::Context;

        self.write(iterator).context("Writing to LED SPI port")
    }
}

impl<SPI> RGB8SmartLedsWrite for apa102_spi::Apa102<SPI>
where
    SPI: Write<u8, Error = Infallible>,
{
    fn write_rgb8(&mut self, iterator: &mut dyn Iterator<Item = RGB8>) -> Result<()> {
        Ok(self.write(iterator).unwrap())
    }
}

impl<SPI> RGB8SmartLedsWrite for ws2812_spi::Ws2812<SPI>
where
    SPI: FullDuplex<u8, Error = Infallible>,
{
    fn write_rgb8(&mut self, iterator: &mut dyn Iterator<Item = RGB8>) -> Result<()> {
        Ok(self.write(iterator).unwrap())
    }
}

pub struct LedStrip<'a> {
    pub smart_led: &'a mut dyn RGB8SmartLedsWrite,
    pub data: LedStripData,
}

impl<'a> LedStrip<'a> {
    pub fn new(smart_led: &'a mut dyn RGB8SmartLedsWrite) -> Self {
        Self {
            smart_led,
            data: [RGB8::default(); STRIP_LENGTH],
        }
    }

    pub fn update(led_strips: &mut Vec<LedStrip>, osc_packet: OscPacket) -> () {
        receive_osc_packet(
            osc_packet,
            led_strips.iter_mut().map(|led_strip| &mut led_strip.data),
        );

        for led_strip in led_strips.iter_mut() {
            // // This seems to fix Store Prohibited errors on the esp32
            // delay::Delay::new().delay_us(100u32);

            led_strip
                .smart_led
                .write_rgb8(&mut led_strip.data.iter().cloned())
                .unwrap();
        }

        trace!("LED Strips ({:?}) updated", &led_strips.len());
    }
}

fn receive_osc_packet<'a, I>(
    packet: OscPacket,
    mut strips: I,
    // tx: &mut esp32_hal::serial::Tx<esp32::UART0>,
) where
    I: Iterator<Item = &'a mut LedStripData>,
{
    use rosc::{OscMessage, OscType::Float};

    let (addr, args) = match &packet {
        OscPacket::Message(OscMessage { addr, args }) => (
            addr.trim_start_matches('/').split('/').collect::<Vec<_>>(),
            args,
        ),
        _ => {
            // info!("Unsupported packet received: {:?}", packet);
            warn!("Unsupported packet received");
            return;
        }
    };

    match (&addr[..], &args[..]) {
        (["led_strips", led_strip_index], input) => {
            let _led_strip_index: usize = match led_strip_index.parse() {
                Ok(led_strip_index) => led_strip_index,
                Err(_err) => {
                    warn!("Invalid led_strip_index: {:?}", led_strip_index);
                    return;
                }
            };

            let mut current_strip = None;

            for osc_type in input.into_iter() {
                match osc_type {
                    OscType::Color(c) => {
                        // If the current LED strip ends before the next index i then reset the index and go to
                        // the next LED strip
                        if current_strip
                            .as_ref()
                            .map(|(strip, i): &(&mut LedStripData, usize)| *i >= strip.len() - 1)
                            .unwrap_or(true)
                        {
                            if let Some(strip) = strips.next() {
                                current_strip = Some((strip, 0usize));
                            }
                        }

                        if let Some((led_strip, i)) = current_strip.as_mut() {
                            led_strip[*i].r = c.red;
                            led_strip[*i].g = c.green;
                            led_strip[*i].b = c.blue;

                            // dbg!(&i);

                            *i += 1;
                        } else {
                            warn!("Input to /led_strips exceeded number of LED strips");
                        }
                    }
                    osc_type => {
                        warn!(
                            "Invalid input to /led_strips. Expected Color, received: {:?}",
                            osc_type
                        )
                    }
                }
            }
        }
        ([universe, "dmx", channel_index], [Float(value)]) => {
            let universe: usize = match universe.parse() {
                Ok(universe) => universe,
                Err(_err) => {
                    warn!("Invalid DMX Universe: {:?}", universe);
                    return;
                }
            };
            let channel_index: usize = match channel_index.parse() {
                Ok(channel_index) => channel_index,
                Err(_err) => {
                    warn!("Invalid DMX channel: {:?}", channel_index);
                    return;
                }
            };

            let color_and_led_index = universe * 512 + channel_index;
            let color_index = color_and_led_index % 3;
            let global_led_index = (color_and_led_index - color_index) / 3;

            let mut leds_before_strip = 0;
            let strip = strips.find_map(|strip| {
                if leds_before_strip <= global_led_index
                    && leds_before_strip + strip.len() > global_led_index
                {
                    Some(strip)
                } else {
                    leds_before_strip += strip.len();
                    None
                }
            });

            let strip = match strip {
                Some(strip) => strip,
                None => {
                    warn!("LED global index overflow: {:?}", global_led_index);
                    return;
                }
            };

            let led_index = global_led_index - leds_before_strip;
            if led_index > strip.len() {
                warn!("LED per-strip index overflow: {:?}", led_index);
                return;
            }

            let led = &mut strip[led_index];
            let value = (value * 255.0) as u8;
            info!("u8 value: {:?}", value);

            match color_index {
                0 => led.r = value,
                1 => led.g = value,
                2 => led.b = value,
                _ => {
                    warn!("Invalid color index: {:?}", color_index);
                    return;
                }
            };

            info!(
                "Setting LED #{:?} index: {:?} to: {:?}",
                global_led_index, color_index, value
            );
        }
        _ => {
            // info!("Unsupported packet received: {:?}", packet);
            info!("Unsupported packet received");
            return;
        }
    };
}
