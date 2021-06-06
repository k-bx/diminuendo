#![feature(backtrace)]
use chrono::prelude::*;
use libusb::{Device, DeviceDescriptor};
use sqlx::sqlite::SqlitePoolOptions;
use std::backtrace::Backtrace;
use std::slice;
use std::time::Duration;
use thiserror::Error;
use tokio;
use tokio::sync::mpsc;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Error: {msg}")]
    BL { msg: String },
    #[error("libusb::Error")]
    Libusb {
        #[from]
        source: libusb::Error,
        backtrace: Backtrace,
    },
}

pub fn bl_err(s: &str) -> AppError {
    AppError::BL { msg: s.to_string() }
}

#[derive(Debug)]
struct Endpoint {
    config: u8,
    iface: u8,
    setting: u8,
    address: u8,
}

fn main() {
    let mut context = libusb::Context::new().unwrap();
    // context.set_log_level(libusb::LogLevel::Debug);
    context.set_log_level(libusb::LogLevel::Info);

    for device in context.devices().unwrap().iter() {
        let device_desc = device.device_descriptor().unwrap();

        println!(
            "Bus {:03} Device {:03} ID {:04x}:{:04x}",
            device.bus_number(),
            device.address(),
            device_desc.vendor_id(),
            device_desc.product_id(),
        );
    }

    for mut device in context.devices().unwrap().iter() {
        let device_desc = device.device_descriptor().unwrap();

        let is_mini_korg = device_desc.vendor_id() == 0x944 && device_desc.product_id() == 0x111;
        let is_big_yamaha = device_desc.vendor_id() == 0x499 && device_desc.product_id() == 0x1039;
        if is_mini_korg || is_big_yamaha {
            main2(&mut device, &device_desc).unwrap()
        }
    }
    println!("Exiting...");
}

fn main2(device: &mut Device, device_desc: &DeviceDescriptor) -> Result<(), AppError> {
    let (events_snd, events_rcv): (
        mpsc::UnboundedSender<Vec<u8>>,
        mpsc::UnboundedReceiver<Vec<u8>>,
    ) = mpsc::unbounded_channel();

    std::thread::spawn(move || {
        event_writer(events_rcv);
    });

    let mut handle = device.open()?;
    handle.reset()?;

    let timeout = Duration::from_secs(1);
    let languages = handle.read_languages(timeout)?;

    println!("Active configuration: {}", handle.active_configuration()?);
    println!("Languages: {:?}", languages);

    if languages.len() > 0 {
        let language = languages[0];

        println!(
            "Manufacturer: {:?}",
            handle
                .read_manufacturer_string(language, device_desc, timeout)
                .ok()
        );
        println!(
            "Product: {:?}",
            handle
                .read_product_string(language, device_desc, timeout)
                .ok()
        );
        println!(
            "Serial Number: {:?}",
            handle
                .read_serial_number_string(language, device_desc, timeout)
                .ok()
        );
    } else {
        eprintln!("Warning: languages.len() == 0");
    }

    match find_readable_endpoint(device, device_desc, libusb::TransferType::Interrupt) {
        Some(endpoint) => read_endpoint(
            &mut handle,
            endpoint,
            libusb::TransferType::Interrupt,
            &events_snd,
        ),
        None => println!("No readable interrupt endpoint"),
    }

    match find_readable_endpoint(device, device_desc, libusb::TransferType::Bulk) {
        Some(endpoint) => read_endpoint(
            &mut handle,
            endpoint,
            libusb::TransferType::Bulk,
            &events_snd,
        ),
        None => println!("No readable bulk endpoint"),
    }

    Ok(())
}

fn find_readable_endpoint(
    device: &mut libusb::Device,
    device_desc: &libusb::DeviceDescriptor,
    transfer_type: libusb::TransferType,
) -> Option<Endpoint> {
    for n in 0..device_desc.num_configurations() {
        let config_desc = match device.config_descriptor(n) {
            Ok(c) => c,
            Err(_) => continue,
        };

        for interface in config_desc.interfaces() {
            for interface_desc in interface.descriptors() {
                for endpoint_desc in interface_desc.endpoint_descriptors() {
                    if endpoint_desc.direction() == libusb::Direction::In
                        && endpoint_desc.transfer_type() == transfer_type
                    {
                        return Some(Endpoint {
                            config: config_desc.number(),
                            iface: interface_desc.interface_number(),
                            setting: interface_desc.setting_number(),
                            address: endpoint_desc.address(),
                        });
                    }
                }
            }
        }
    }

    None
}

fn read_endpoint(
    handle: &mut libusb::DeviceHandle,
    endpoint: Endpoint,
    transfer_type: libusb::TransferType,
    events_snd: &mpsc::UnboundedSender<Vec<u8>>,
) {
    println!("Reading from endpoint: {:?}", endpoint);

    let has_kernel_driver = match handle.kernel_driver_active(endpoint.iface) {
        Ok(true) => {
            handle.detach_kernel_driver(endpoint.iface).ok();
            true
        }
        _ => false,
    };

    println!(" - kernel driver? {}", has_kernel_driver);

    match configure_endpoint(handle, &endpoint) {
        Ok(_) => loop {
            let mut vec = Vec::<u8>::with_capacity(256);
            let buf =
                unsafe { slice::from_raw_parts_mut((&mut vec[..]).as_mut_ptr(), vec.capacity()) };

            let timeout = Duration::from_secs(1);

            match transfer_type {
                libusb::TransferType::Interrupt => {
                    match handle.read_interrupt(endpoint.address, buf, timeout) {
                        Ok(len) => {
                            unsafe { vec.set_len(len) };
                            process(&vec, &events_snd);
                        }
                        Err(err) => {
                            println!("could not read from endpoint: {}; exiting", err);
                            return;
                        }
                    }
                }
                libusb::TransferType::Bulk => {
                    match handle.read_bulk(endpoint.address, buf, timeout) {
                        Ok(len) => {
                            unsafe { vec.set_len(len) };
                            process(&vec, events_snd);
                        }
                        Err(err) => {
                            println!("could not read from endpoint: {}; exiting", err);
                            return;
                        }
                    }
                }
                _ => (),
            }
        },
        Err(err) => println!("could not configure endpoint: {}", err),
    }

    if has_kernel_driver {
        handle.attach_kernel_driver(endpoint.iface).ok();
    }
}

fn configure_endpoint<'a>(
    handle: &'a mut libusb::DeviceHandle,
    endpoint: &Endpoint,
) -> libusb::Result<()> {
    handle.set_active_configuration(endpoint.config)?;
    handle.claim_interface(endpoint.iface)?;
    handle.set_alternate_setting(endpoint.iface, endpoint.setting)?;
    Ok(())
}

fn process(vec: &Vec<u8>, events_sdr: &mpsc::UnboundedSender<Vec<u8>>) {
    let mut nonzeroes = vec.clone();

    // clean up the signal
    let mut i = 0;
    while i + 1 < nonzeroes.len() {
        let mut do_inc = true;
        if nonzeroes[0] == 0x00 {
            nonzeroes.remove(i);
        } else if nonzeroes[i] == 0x0f && nonzeroes[i + 1] == 0xf8 {
            nonzeroes.remove(i);
            nonzeroes.remove(i);
            do_inc = false;
        } else if nonzeroes[i] == 0x0f && nonzeroes[i + 1] == 0xfe {
            nonzeroes.remove(i);
            nonzeroes.remove(i);
            do_inc = false;
        }
        if do_inc {
            i += 1;
        }
    }

    if nonzeroes.iter().map(|x| *x == 0x00).all(|x| x == true) {
        return;
    }

    if nonzeroes.len() > 0 {
        events_sdr.send(nonzeroes.clone()).unwrap();
    }
    let nonzeroes_hex_strs: Vec<String> =
        nonzeroes.iter().map(|x| format!("{:#04X}", *x)).collect();
    let nonzero_hex_str = nonzeroes_hex_strs.join(", ");
    if nonzeroes.len() > 0 {
        println!(" - read nonzeroes: [{}]", nonzero_hex_str);
    }
}

#[tokio::main]
async fn event_writer(mut events_rcv: mpsc::UnboundedReceiver<Vec<u8>>) {
    let pool = SqlitePoolOptions::new()
        .connect("sqlite:/home/pi/storage/diminuendo.sqlite")
        .await
        .unwrap();

    while let Some(nonzeroes) = events_rcv.recv().await {
        let t = Utc::now();
        let nonzeroes: Vec<u8> = nonzeroes;
        if nonzeroes.len() > 0 {
            sqlx::query("insert into events (ts, events) values (?, ?)")
                .bind(t.timestamp_millis())
                .bind(nonzeroes)
                .execute(&pool)
                .await
                .unwrap();
        }
    }
}
