use embassy_executor::Spawner;
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, zerocopy_channel::Sender};
use esp_hal::{
    gpio::interconnect::PeripheralInput,
    pcnt::{
        channel::{CtrlMode, EdgeMode},
        unit::Unit,
    },
    peripheral::Peripheral,
};
use esp_println::println;

pub fn listen(tx: Sender<'static, NoopRawMutex, i16>, spawner: &Spawner, unit: Unit<'static, 0>) {
    println!("Initializing rotary listener");

    spawner.spawn(update_task(tx, unit)).unwrap();
}

pub fn initialize_pcnt_unit<E, C, const NUM: usize>(
    unit: &Unit<'_, NUM>,
    edge: impl Peripheral<P = E>,
    ctrl: impl Peripheral<P = C>,
) where
    E: PeripheralInput,
    C: PeripheralInput,
{
    unit.set_filter(Some(10u16 * 80))
        .expect("Filter is statically set to a valid value");

    unit.channel0.set_ctrl_signal(ctrl);
    unit.channel0.set_edge_signal(edge);
    unit.channel0
        .set_ctrl_mode(CtrlMode::Reverse, CtrlMode::Keep);
    unit.channel0
        .set_input_mode(EdgeMode::Decrement, EdgeMode::Increment);
}

#[embassy_executor::task]
async fn update_task(mut tx: Sender<'static, NoopRawMutex, i16>, unit: Unit<'static, 0>) {
    loop {
        let count = tx.send().await;
        *count = unit.value();
        unit.clear();
        println!("Read count: {count}");
        tx.send_done();
    }
}
