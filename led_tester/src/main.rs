#![no_std]
#![no_main]

use apa102_spi::Apa102;
use esp_println::println;
use riscv_rt::entry;
use smart_leds::{SmartLedsWrite, RGB8};

use esp32c3_hal::{
    clock::ClockControl,
    gpio::IO,
    pac::Peripherals,
    prelude::*,
    spi::{Spi, SpiMode},
    Delay, RtcCntl, Timer,
};

use esp_backtrace as _;

const LED_COUNT: usize = 144 * 10;

#[entry]
fn main() -> ! {
    let peripherals = Peripherals::take().unwrap();

    let mut system = peripherals.SYSTEM.split();
    let clocks = ClockControl::boot_defaults(system.clock_control).freeze();

    // Disable the watchdog timers. For the ESP32-C3, this includes the Super WDT,
    // the RTC WDT, and the TIMG WDTs.
    let mut rtc_cntl = RtcCntl::new(peripherals.RTC_CNTL);
    let mut timer0 = Timer::new(peripherals.TIMG0);
    let mut timer1 = Timer::new(peripherals.TIMG1);

    rtc_cntl.set_super_wdt_enable(false);
    rtc_cntl.set_wdt_enable(false);
    timer0.disable();
    timer1.disable();

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
        1u32.kHz(),
        SpiMode::Mode0,
        &mut system.peripheral_clock_control,
        &clocks,
    );

    let mut apa102 = Apa102::new(spi);
    let mut delay = Delay::new(&clocks);

    let purple: [RGB8; LED_COUNT] = [RGB8 {
        r: 0x15,
        g: 0x00,
        b: 0x15,
    }; LED_COUNT];

    let green: [RGB8; LED_COUNT] = [RGB8 {
        r: 0x00,
        g: 0x15,
        b: 0x00,
    }; LED_COUNT];

    init_logger();

    println!("Starting LED Tester (SPI Connected!)");

    loop {
        apa102.write(&mut purple.iter().cloned()).unwrap();
        println!("Purple");
        delay.delay_ms(1000u32);

        apa102.write(&mut green.iter().cloned()).unwrap();
        println!("Green");
        delay.delay_ms(1000u32);
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
