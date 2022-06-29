#![no_std]
#![no_main]
#![feature(c_variadic)]
#![feature(const_mut_refs)]

use embedded_svc::wifi::{
    ClientConfiguration, ClientConnectionStatus, ClientIpStatus, ClientStatus, Configuration,
    Status, Wifi,
};
use esp32c3_hal::{pac::Peripherals, RtcCntl};
use esp_println::{print, println};
use esp_wifi::wifi::initialize;
use esp_wifi::wifi::utils::create_network_interface;
use esp_wifi::wifi_interface::timestamp;
use esp_wifi::{create_network_stack_storage, network_stack_storage};
use riscv_rt::entry;
use smoltcp::iface::SocketHandle;
use smoltcp::socket::{Socket, TcpSocket, UdpPacketMetadata, UdpSocket, UdpSocketBuffer};

use esp_backtrace as _;
use smoltcp::storage::PacketMetadata;

#[macro_use]
extern crate alloc;

const SSID: &str = env!("SSID");
const PASSWORD: &str = env!("PASSWORD");

#[entry]
fn main() -> ! {
    let mut peripherals = Peripherals::take().unwrap();

    let mut rtc_cntl = RtcCntl::new(peripherals.RTC_CNTL);

    // Disable watchdog timers
    rtc_cntl.set_super_wdt_enable(false);
    rtc_cntl.set_wdt_enable(false);

    // Create 2 sockets - one for DHCP and one for a placeholder TCP socket which will be replaced by UDP later on
    let mut storage = create_network_stack_storage!(2, 8, 1);
    let mut ethernet = create_network_interface(network_stack_storage!(storage));

    // Remove a TCP socket to make room for the UDP socket
    let mut tcp_socket_handle: Option<SocketHandle> = None;

    for (handle, socket) in ethernet.sockets_mut() {
        // println!("{:?}", socket);
        match socket {
            Socket::Tcp(_) => tcp_socket_handle = Some(handle),
            _ => {}
        }
    }

    ethernet.remove_socket(tcp_socket_handle.unwrap());

    // Add the udp socket, replacing the previous TCP socket
    let socket_handle = {
        const size: usize = u8::MAX as usize;

        let udp_rx_buffer = UdpSocketBuffer::new(vec![UdpPacketMetadata::EMPTY], vec![0u8; 64]);
        let udp_tx_buffer = UdpSocketBuffer::new(vec![UdpPacketMetadata::EMPTY], vec![0u8; 128]);

        // let udp_rx_buffer = UdpSocketBuffer::new(vec![UdpPacketMetadata::EMPTY], vec![0; size]);
        // let udp_tx_buffer = UdpSocketBuffer::new(vec![UdpPacketMetadata::EMPTY], vec![0; size]);

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

    println!("{:?}", wifi_interface.get_status());

    println!("Start Wifi Scan");
    let res = wifi_interface.scan();
    println!("Found Wifi Networks:");
    if let Ok(res) = res {
        for ap in res {
            println!("- {:?}", ap.ssid);
        }
    }

    println!("Call wifi_connect");
    let client_config = Configuration::Client(ClientConfiguration {
        ssid: SSID.into(),
        password: PASSWORD.into(),
        ..Default::default()
    });
    let res = wifi_interface.set_configuration(&client_config);
    println!("wifi_connect returned {:?}", res);

    println!("{:?}", wifi_interface.get_capabilities());
    println!("{:?}", wifi_interface.get_status());

    // wait to get connected
    loop {
        if let Status(ClientStatus::Started(_), _) = wifi_interface.get_status() {
            break;
        }
    }
    println!("{:?}", wifi_interface.get_status());

    println!("Wifi Connected! Start main loop.");

    let mut stage = 0;
    let mut idx = 0;
    let mut buffer = [0u8; 8000];
    let mut waiter = 50000;

    loop {
        wifi_interface.poll_dhcp().ok();
        wifi_interface.network_interface().poll(timestamp()).ok();

        if let Status(
            ClientStatus::Started(ClientConnectionStatus::Connected(ClientIpStatus::Done(config))),
            _,
        ) = wifi_interface.get_status()
        {
            match stage {
                0 => {
                    println!("My IP config is {:?}", config);
                    println!("Lets connect");
                    let (socket, _cx) = wifi_interface
                        .network_interface()
                        .get_socket_and_context::<UdpSocket>(socket_handle);

                    // Udp
                    socket.bind(1234).unwrap();

                    stage = 2;
                    // println!("Lets receive UDP packets!");
                    println!("Lets receive UDP packets!");
                }
                // 1 => {
                //     let socket = wifi_interface
                //         .network_interface()
                //         .get_socket::<UdpSocket>(http_socket_handle);

                //     if socket
                //         .send_slice(&b"Hello World!\r\n\r\n"[..])
                //         .is_ok()
                //     {
                //         stage = 2;
                //         println!("Lets receive");
                //     }
                // }
                2 => {
                    let socket = wifi_interface
                        .network_interface()
                        .get_socket::<UdpSocket>(socket_handle);

                    println!("Waiting to receive a packet...");
                    if let Ok((s, _)) = socket.recv_slice(&mut buffer[idx..]) {
                        println!("RX: {:?}", s);
                        if s > 0 {
                            idx += s;
                        }
                    } else {
                        stage = 3;

                        if idx > 0 {
                            println!("Received a UDP Packet! Bytes: {:?}", idx);
                            println!("{:?}", buffer);

                            for c in &buffer[..idx] {
                                print!("{}", *c as char);
                            }
                        }
                        // println!("");
                    }
                }
                // 3 => {
                //     println!("Close");
                //     let socket = wifi_interface
                //         .network_interface()
                //         .get_socket::<TcpSocket>(socket_handle);

                //     socket.close();
                //     stage = 4;
                // }
                // 4 => {
                //     waiter -= 1;
                //     if waiter == 0 {
                //         idx = 0;
                //         waiter = 50000;
                //         stage = 0;
                //     }
                // }
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
