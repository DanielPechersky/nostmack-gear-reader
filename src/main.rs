#![no_std]
#![no_main]

mod mk_static;
mod rotary_listener;
mod wifi;

use embassy_executor::Spawner;
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, zerocopy_channel::Channel};
use esp_alloc as _;
use esp_backtrace as _;
use esp_hal::{
    clock::CpuClock,
    gpio::{Input, Pull},
    pcnt::Pcnt,
    rng::Rng,
    timer::timg::TimerGroup,
};
use esp_println::println;
use mk_static::mk_static;

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    esp_println::logger::init_logger_from_env();

    println!("Hello, world!");

    let peripherals = esp_hal::init({
        let mut config = esp_hal::Config::default();
        // Configure the CPU to run at the maximum frequency.
        config.cpu_clock = CpuClock::max();
        config
    });

    esp_alloc::heap_allocator!(72 * 1024);

    println!("Initializing embassy");

    let timg1 = TimerGroup::new(peripherals.TIMG1);
    esp_hal_embassy::init(timg1.timer0);

    println!("Opening channel");

    let channel_buffer = mk_static!([i16; 1], [0; 1]);
    let channel = mk_static!(Channel::<NoopRawMutex, i16>, Channel::new(channel_buffer));
    let (tx, rx) = channel.split();

    let rng = Rng::new(peripherals.RNG);

    let (device, controller) = wifi::wifi_from_peripherals(
        peripherals.TIMG0,
        rng,
        peripherals.RADIO_CLK,
        peripherals.WIFI,
    );
    wifi::connect(rx, &spawner, device, controller, rng).await;

    let pcnt = Pcnt::new(peripherals.PCNT);
    let edge = Input::new(peripherals.GPIO26, Pull::Up);
    let ctrl = Input::new(peripherals.GPIO27, Pull::Up);
    rotary_listener::initialize_pcnt_unit(&pcnt.unit0, edge, ctrl);
    rotary_listener::listen(tx, &spawner, pcnt.unit0);
}
