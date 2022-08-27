use std::{sync::Arc, thread, time::Duration};

use std::env;
use embedded_svc::{
    http::server::{registry::Registry, Response},
    wifi::{
        AccessPointConfiguration, ApIpStatus, ApStatus, ClientConfiguration,
        ClientConnectionStatus, ClientIpStatus, ClientStatus, Configuration as WifiConfig, Status,
        Wifi,
    },
};
use esp_idf_svc::{
    http::server::{Configuration, EspHttpServer},
    netif::EspNetifStack,
    nvs::EspDefaultNvs,
    sysloop::EspSysLoopStack,
    wifi::EspWifi,
};
use esp_idf_sys as _; // If using the `binstart` feature of `esp-idf-sys`, always keep this module imported

#[allow(dead_code)]

const SSID: &str = env!("RUST_ESP32_STD_DEMO_WIFI_SSID");
const PASS: &str = env!("RUST_ESP32_STD_DEMO_WIFI_PASS");

const CERTIFICATE: &str = include_str!("certs/server_certificate.pem");
const PRIVATE_KEY: &str = include_str!("certs/private_key.pem");

fn main() -> anyhow::Result<()> {
    // Temporary. Will disappear once ESP-IDF 4.4 is released, but for now it is necessary to call this function once,
    // or else some patches to the runtime implemented by esp-idf-sys might not link properly.
    esp_idf_sys::link_patches();

    let netif_stack = Arc::new(EspNetifStack::new()?);
    let sys_loop_stack = Arc::new(EspSysLoopStack::new()?);
    let default_nvs = Arc::new(EspDefaultNvs::new()?);
    let _wifi = wifi(netif_stack, sys_loop_stack, default_nvs)?;

    let conf = Configuration {
        server_certificate: Some(CERTIFICATE),
        private_key: Some(PRIVATE_KEY),
        ..Default::default()
    };

    let mut server = EspHttpServer::new(&conf)?;
    server.handle_get("/", |_eq, resp| {
        resp.send_str("hello world")?;
        Ok(())
    })?;

    loop {
        thread::sleep(Duration::from_secs(10));
    }
}

fn wifi(
    netif_stack: Arc<EspNetifStack>,
    sys_loop_stack: Arc<EspSysLoopStack>,
    default_nvs: Arc<EspDefaultNvs>,
) -> anyhow::Result<Box<EspWifi>> {
    let mut wifi = Box::new(EspWifi::new(netif_stack, sys_loop_stack, default_nvs)?);

    println!("Wifi created, about to scan");

    let ap_infos = wifi.scan()?;

    let ours = ap_infos.into_iter().find(|a| a.ssid == SSID);

    let channel = if let Some(ours) = ours {
        println!(
            "Found configured access point {} on channel {}",
            SSID, ours.channel
        );
        Some(ours.channel)
    } else {
        println!(
            "Configured access point {} not found during scanning, will go with unknown channel",
            SSID
        );
        None
    };

    wifi.set_configuration(&WifiConfig::Mixed(
        ClientConfiguration {
            ssid: SSID.into(),
            password: PASS.into(),
            channel,
            ..Default::default()
        },
        AccessPointConfiguration {
            ssid: "aptest".into(),
            channel: channel.unwrap_or(1),
            ..Default::default()
        },
    ))?;

    println!("Wifi configuration set, about to get status");

    wifi.wait_status_with_timeout(Duration::from_secs(20), |status| !status.is_transitional())
        .map_err(|e| anyhow::anyhow!("Unexpected Wifi status: {:?}", e))?;

    let status = wifi.get_status();

    if let Status(
        ClientStatus::Started(ClientConnectionStatus::Connected(ClientIpStatus::Done(
            _ip_settings,
        ))),
        ApStatus::Started(ApIpStatus::Done),
    ) = status
    {
        println!("Wifi connected");
    } else {
        println!("Unexpected Wifi status: {:?}", status);
    }

    Ok(wifi)
}
