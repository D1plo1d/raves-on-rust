#![no_std]
#![no_main]
#![feature(alloc_error_handler)]
#![feature(c_variadic)]
#![feature(const_mut_refs)]

use alloc::{boxed::Box, vec::Vec};
use apa102_spi::Apa102;
use embedded_svc::wifi::{
    ClientConfiguration, ClientConnectionStatus, ClientIpStatus, ClientStatus, Configuration,
    Status, Wifi,
};
use esp32c3_hal::{
    clock::ClockControl,
    gpio::IO,
    pac::Peripherals,
    prelude::*,
    spi::{Spi, SpiMode},
    RtcCntl,
};
use esp_backtrace as _;
use esp_println::println;
use esp_wifi::wifi::initialize;
use esp_wifi::wifi::utils::create_network_interface;
use esp_wifi::wifi_interface::timestamp;
use esp_wifi::{create_network_stack_storage, network_stack_storage};
use osc_receiver::led_strip::{LedStrip, RGB8SmartLedsWrite};
use riscv_rt::entry;
use smart_leds::RGB8;
use smoltcp::iface::SocketHandle;
use smoltcp::socket::{Socket, UdpPacketMetadata, UdpSocket, UdpSocketBuffer};

#[macro_use]
extern crate alloc;

const SSID: &str = env!("SSID");
const PASSWORD: &str = env!("PASSWORD");
const LED_TYPE: &str = env!("LED_TYPE");

#[global_allocator]
static ALLOCATOR: esp_alloc::EspHeap = esp_alloc::EspHeap::empty();

fn init_heap() {
    use core::mem::MaybeUninit;

    const HEAP_SIZE: usize = 64 * 1024;
    static mut HEAP: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];

    unsafe {
        ALLOCATOR.init(HEAP.as_ptr() as usize, HEAP_SIZE);
    }
}

#[alloc_error_handler]
fn oom(_: core::alloc::Layout) -> ! {
    println!("Out of alloc memory");
    loop {}
}

// Set the first and third LED to indicate connection status
fn set_indicator_leds(smart_leds: &mut Vec<LedStrip>, color: RGB8) {
    let colors = [color.clone(), RGB8::default(), color.clone()];

    smart_leds
        .iter_mut()
        .next()
        .unwrap()
        .smart_led
        .write_rgb8(&mut colors.into_iter())
        .unwrap();
}

#[entry]
fn main() -> ! {
    esp_wifi::init_heap();
    init_heap();

    let mut peripherals = Peripherals::take().unwrap();

    let mut system = peripherals.SYSTEM.split();
    let clocks = ClockControl::boot_defaults(system.clock_control).freeze();

    let mut rtc_cntl = RtcCntl::new(peripherals.RTC_CNTL);
    // let mut timer0 = Timer::new(peripherals.TIMG0);
    // let mut timer1 = Timer::new(peripherals.TIMG1);

    // Disable watchdog timers
    rtc_cntl.set_super_wdt_enable(false);
    rtc_cntl.set_wdt_enable(false);
    // timer0.disable();
    // timer1.disable();

    let mut smart_leds: Vec<&mut dyn RGB8SmartLedsWrite> = Vec::new();

    let mut boxed_smart_led: Box<dyn RGB8SmartLedsWrite> = if LED_TYPE == "APA102" {
        let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);
        // Connect these two to the LEDs
        let sclk = io.pins.gpio6;
        let mosi = io.pins.gpio7;

        let miso = io.pins.gpio2;
        let cs = io.pins.gpio10;

        let spi = Spi::new(
            peripherals.SPI2,
            sclk,
            mosi,
            Some(miso),
            Some(cs),
            // These LED strips should work up to 12 MHz but depending on wiring interference may limit that
            12u32.MHz(),
            SpiMode::Mode0,
            &mut system.peripheral_clock_control,
            &clocks,
        );

        let apa102 = Apa102::new(spi);

        Box::new(apa102)
    } else if LED_TYPE == "WS2812B" {
        let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);
        // Connect these two to the LEDs
        let sclk = io.pins.gpio6;
        let mosi = io.pins.gpio7;

        let miso = io.pins.gpio2;
        let cs = io.pins.gpio10;

        let spi = Spi::new(
            peripherals.SPI2,
            sclk,
            mosi,
            Some(miso),
            Some(cs),
            3u32.MHz(),
            SpiMode::Mode0,
            &mut system.peripheral_clock_control,
            &clocks,
        );

        let ws2812b = ws2812_spi::Ws2812::new(spi);

        Box::new(ws2812b)
    } else {
        panic!("Invalid LED_TYPE. Must be either 'WS2812B' or 'APA102'.");
    };

    smart_leds.push(boxed_smart_led.as_mut());

    let mut led_strips = smart_leds
        .into_iter()
        .map(LedStrip::new)
        .collect::<Vec<_>>();

    // Set the indicator LEDs to red to indicate that the microcontroller has restarted
    set_indicator_leds(
        &mut led_strips,
        RGB8 {
            r: 0xF,
            g: 0x0,
            b: 0x0,
        },
    );

    // Create 2 sockets - one for DHCP and one for a placeholder TCP socket which will be replaced by UDP later on
    let mut storage = create_network_stack_storage!(2, 8, 1);
    let mut ethernet = create_network_interface(network_stack_storage!(storage));

    // Remove a TCP socket to make room for the UDP socket
    {
        let mut tcp_socket_handle: Option<SocketHandle> = None;

        for (handle, socket) in ethernet.sockets_mut() {
            // println!("{:?}", socket);
            match socket {
                Socket::Tcp(_) => tcp_socket_handle = Some(handle),
                _ => {}
            }
        }

        ethernet.remove_socket(tcp_socket_handle.unwrap());
    }

    // Add the udp socket, replacing the previous TCP socket
    let socket_handle = {
        const ETHERNET_MTU: usize = 1500 as usize;
        static mut UDP_RX_DATA: [u8; ETHERNET_MTU] = [0; ETHERNET_MTU];

        let udp_rx_buffer =
            unsafe { UdpSocketBuffer::new(vec![UdpPacketMetadata::EMPTY], &mut UDP_RX_DATA[..]) };

        let udp_tx_buffer = UdpSocketBuffer::new(vec![UdpPacketMetadata::EMPTY], vec![0u8; 0]);

        let udp_socket = UdpSocket::new(udp_rx_buffer, udp_tx_buffer);

        ethernet.add_socket(udp_socket)
    };

    let mut wifi_interface = esp_wifi::wifi_interface::Wifi::new(ethernet);

    init_logger();

    initialize(
        &mut peripherals.SYSTIMER,
        &mut peripherals.INTERRUPT_CORE0,
        peripherals.RNG,
    )
    .unwrap();

    // println!("{:?}", wifi_interface.get_status());

    // println!("Start Wifi Scan");
    // let res = wifi_interface.scan();
    // println!("Found Wifi Networks:");
    // if let Ok(res) = res {
    //     for ap in res {
    //         println!("- {:?}", ap.ssid);
    //     }
    // }

    let client_config = Configuration::Client(ClientConfiguration {
        ssid: SSID.into(),
        password: PASSWORD.into(),
        ..Default::default()
    });
    wifi_interface.set_configuration(&client_config).unwrap();

    // println!("{:?}", wifi_interface.get_capabilities());
    // println!("{:?}", wifi_interface.get_status());

    println!("Initializing Wifi Interface...");

    // Set the indicator LEDs to orange to indicate that the microcontroller is waiting on the wifi interface
    set_indicator_leds(
        &mut led_strips,
        RGB8 {
            r: 0xF,
            g: 0xF,
            b: 0x0,
        },
    );

    // wait to get connected
    loop {
        if let Status(ClientStatus::Started(_), _) = wifi_interface.get_status() {
            break;
        }
    }
    // println!("{:?}", wifi_interface.get_status());

    println!("Connecting to {}...", &SSID);

    // Set the first and third LED to green to indicate that the microcontroller is connecting to Wifi
    set_indicator_leds(
        &mut led_strips,
        RGB8 {
            r: 0x0,
            g: 0xF,
            b: 0x0,
        },
    );

    let mut stage = 0;

    loop {
        if let Err(err) = wifi_interface.poll_dhcp() {
            println!("DHCP Error: {:?}", err);
        }
        if let Err(err) = wifi_interface.network_interface().poll(timestamp()) {
            println!("Wifi Error: {:?}", err);
        }

        // println!("{:?}", wifi_interface.get_status());
        if let Status(
            ClientStatus::Started(ClientConnectionStatus::Connected(ClientIpStatus::Done(config))),
            _,
        ) = wifi_interface.get_status()
        {
            match stage {
                0 => {
                    let (socket, _cx) = wifi_interface
                        .network_interface()
                        .get_socket_and_context::<UdpSocket>(socket_handle);

                    // Udp
                    socket.bind(9000).unwrap();

                    stage = 1;

                    // Turn off the connection indicator LEDs
                    set_indicator_leds(&mut led_strips, RGB8::default());

                    println!("Wifi Connected! Listening on {}:9000", config.ip);
                }
                1 => {
                    let socket = wifi_interface
                        .network_interface()
                        .get_socket::<UdpSocket>(socket_handle);

                    if let Ok((udp_packet, _)) = socket.recv() {
                        // for c in udp_packet {
                        //     print!("{}", *c as char);
                        // }
                        // println!("");
                        // println!("<EOF>");

                        let osc_packet = rosc::decoder::decode_udp(udp_packet);

                        if let Ok((&[], osc_packet)) = osc_packet {
                            // println!("Valid packet! ({:?} Bytes)", udp_packet.len());
                            LedStrip::update(&mut led_strips, osc_packet);
                        } else {
                            println!("Invalid packet");
                        }
                    }
                }
                _ => (),
            }
        }
    }
}

#[export_name = "DefaultHandler"]
pub fn default_handler() {
    println!("DefaultHandler called!");
}

pub fn init_logger() {
    unsafe {
        log::set_logger_racy(&LOGGER).unwrap();
        log::set_max_level(log::LevelFilter::Info);
    }
}

static LOGGER: SimpleLogger = SimpleLogger;
struct SimpleLogger;

impl log::Log for SimpleLogger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        println!("{} - {}", record.level(), record.args());
    }

    fn flush(&self) {}
}
