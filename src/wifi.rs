use core::{convert::Infallible, net::SocketAddrV4, str::FromStr as _};

use embassy_executor::Spawner;
use embassy_net::{
    udp::{PacketMetadata, UdpSocket},
    Runner, Stack, StackResources,
};
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, zerocopy_channel::Receiver};
use embassy_time::{with_timeout, Duration, Timer};
use esp_hal::{
    peripherals::{RADIO_CLK, TIMG0, WIFI},
    rng::Rng,
    timer::timg::TimerGroup,
};
use esp_println::println;
use esp_wifi::{
    wifi::{ClientConfiguration, WifiController, WifiDevice, WifiEvent, WifiStaDevice},
    EspWifiController,
};

use crate::mk_static::mk_static;

pub fn wifi_from_peripherals(
    timer_group: TIMG0,
    rng: Rng,
    radio_clk: RADIO_CLK,
    wifi: WIFI,
) -> (WifiDevice<'static, WifiStaDevice>, WifiController<'static>) {
    let timg0 = TimerGroup::new(timer_group);

    let wifi_controller = mk_static!(
        EspWifiController<'static>,
        esp_wifi::init(timg0.timer0, rng, radio_clk).unwrap()
    );

    esp_wifi::wifi::new_with_mode(wifi_controller, wifi, WifiStaDevice).unwrap()
}

pub async fn connect(
    rx: Receiver<'static, NoopRawMutex, i16>,
    spawner: &Spawner,
    device: WifiDevice<'static, WifiStaDevice>,
    mut controller: WifiController<'static>,
    rng: Rng,
) {
    controller.start_async().await.unwrap();

    spawner.spawn(keep_wifi_connected(controller)).unwrap();

    let stack = start_network_stack(spawner, device, rng);

    spawner.spawn(send_deltas(rx, stack)).unwrap();
}

#[allow(dead_code)]
async fn debug_networks<const N: usize>(wifi_controller: &mut WifiController<'static>) {
    let (networks, n) = wifi_controller.scan_n_async::<N>().await.unwrap();
    println!("Found {n} networks");
    println!("Networks: {networks:?}");
}

async fn connect_to_wifi(wifi_controller: &mut WifiController<'static>) -> Result<(), ()> {
    let client_config = ClientConfiguration {
        ssid: env!("WIFI_SSID").try_into().unwrap(),
        password: env!("WIFI_PASSWORD").try_into().unwrap(),
        ..Default::default()
    };

    wifi_controller
        .set_configuration(&esp_wifi::wifi::Configuration::Client(client_config))
        .unwrap();

    with_timeout(Duration::from_secs(5), wifi_controller.connect_async())
        .await
        .map_err(|e| {
            println!("Timed out connecting to wifi: {e:?}");
        })?
        .map_err(|e| {
            println!("Failed to connect to wifi: {e:?}");
        })?;

    Ok(())
}

#[embassy_executor::task]
async fn keep_wifi_connected(mut controller: WifiController<'static>) {
    println!("Connecting to wifi");
    loop {
        if matches!(controller.is_connected(), Ok(true)) {
            controller.wait_for_event(WifiEvent::StaDisconnected).await;
        } else {
            #[allow(clippy::collapsible_else_if)]
            match connect_to_wifi(&mut controller).await {
                Ok(()) => println!("Connected to wifi"),
                Err(()) => println!("Retrying connecting to wifi"),
            }
        }
    }
}

fn start_network_stack(
    spawner: &Spawner,
    device: WifiDevice<'static, WifiStaDevice>,
    rng: Rng,
) -> Stack<'static> {
    let (stack, runner) = embassy_net::new(
        device,
        embassy_net::Config::dhcpv4(Default::default()),
        mk_static!(StackResources<3>, StackResources::new()),
        generate_random_seed(rng),
    );

    spawner.spawn(net_task(runner)).unwrap();

    stack
}

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, WifiDevice<'static, WifiStaDevice>>) {
    runner.run().await
}

fn generate_random_seed(mut rng: Rng) -> u64 {
    let mut seed = [0; 8];
    rng.read(&mut seed);
    u64::from_ne_bytes(seed)
}

#[embassy_executor::task]
async fn send_deltas(mut rx: Receiver<'static, NoopRawMutex, i16>, stack: Stack<'static>) {
    async fn connect_and_start_sending(
        rx: &mut Receiver<'static, NoopRawMutex, i16>,
        stack: Stack<'static>,
        rx_buffer: &mut [u8],
        tx_buffer: &mut [u8],
    ) -> Result<Infallible, ()> {
        let mut rx_metadata = [PacketMetadata::EMPTY; 5];
        let mut tx_metadata = [PacketMetadata::EMPTY; 5];

        let mut socket = UdpSocket::new(
            stack,
            &mut rx_metadata,
            rx_buffer,
            &mut tx_metadata,
            tx_buffer,
        );
        socket
            .bind((
                stack
                    .config_v4()
                    .ok_or_else(|| println!("Missing local ip address, can't bind socket"))?
                    .address
                    .address(),
                1234,
            ))
            .unwrap();
        let remote_addr = SocketAddrV4::from_str(env!("REMOTE_ADDR")).unwrap();
        rx.receive_done(); // drop the first value in case we have a bunch of turns saved up
        loop {
            let count = rx.receive().await;
            println!("Sending count to hub: {count}");

            let mut msg = [0; 6];
            msg[..4].copy_from_slice(&id().to_be_bytes());
            msg[4..].copy_from_slice(&count.to_be_bytes());
            socket.send_to(&msg, remote_addr).await.map_err(|e| {
                println!("Error sending count to hub: {e:?}");
            })?;
            socket.flush().await;
            println!("Sent count to hub");
            Timer::after(Duration::from_millis(10)).await;
            rx.receive_done();
        }
    }

    let mut rx_buffer = [0; 0];
    let mut tx_buffer = [0; 1024];
    loop {
        // Returns when our connection fails
        let Err(()) =
            connect_and_start_sending(&mut rx, stack, &mut rx_buffer, &mut tx_buffer).await;
        Timer::after(Duration::from_secs(1)).await;
    }
}

fn id() -> u32 {
    env!("ID").parse().unwrap()
}
