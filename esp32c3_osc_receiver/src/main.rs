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
use smoltcp::socket::{Socket, TcpSocket};

use esp_backtrace as _;

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

    let mut storage = create_network_stack_storage!(3, 8, 1);
    let ethernet = create_network_interface(network_stack_storage!(storage));
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

    println!("Start busy loop on main");

    let mut stage = 0;
    let mut idx = 0;
    let mut buffer = [0u8; 8000];
    let mut waiter = 50000;

    let mut socket_handle: Option<SocketHandle> = None;

    for (handle, socket) in wifi_interface.network_interface().sockets_mut() {
        // println!("{:?}", socket);
        match socket {
            Socket::Tcp(_) => socket_handle = Some(handle),
            _ => {}
        }
    }

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
                        .get_socket_and_context::<TcpSocket>(socket_handle.unwrap());

                    // // UDP
                    // socket.bind(1234).unwrap();

                    // TCP
                    socket.listen(1234).unwrap();

                    stage = 2;
                    // println!("Lets receive UDP packets!");
                    println!("Lets receive TCP packets!");
                }
                // 1 => {
                //     let socket = wifi_interface
                //         .network_interface()
                //         .get_socket::<TcpSocket>(http_socket_handle.unwrap());

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
                        .get_socket::<TcpSocket>(socket_handle.unwrap());

                    println!("Waiting to receive a packet...");
                    if let Ok(s) = socket.recv_slice(&mut buffer[idx..]) {
                        println!("RX: {:?}", s);
                        if s > 0 {
                            idx += s;
                        }
                    } else {
                        stage = 3;

                        if idx > 0 {
                            println!("Received a TCP Packet! Bytes: {:?}", idx);
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
                //         .get_socket::<TcpSocket>(socket_handle.unwrap());

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
