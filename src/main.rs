#![feature(backtrace)]
use libusb::{Device, DeviceDescriptor};
use std::backtrace::Backtrace;
use std::slice;
use std::time::Duration;
use thiserror::Error;

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

    for mut device in context.devices().unwrap().iter() {
        let device_desc = device.device_descriptor().unwrap();

        // println!(
        //     "Bus {:03} Device {:03} ID {:04x}:{:04x}",
        //     device.bus_number(),
        //     device.address(),
        //     device_desc.vendor_id(),
        //     device_desc.product_id(),
        // );

        if device_desc.vendor_id() == 0x944 && device_desc.product_id() == 0x111 {
            main2(&mut device, &device_desc).unwrap()
        }
    }
    println!("Exiting...");
}

fn main2(device: &mut Device, device_desc: &DeviceDescriptor) -> Result<(), AppError> {
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
        Some(endpoint) => read_endpoint(&mut handle, endpoint, libusb::TransferType::Interrupt),
        None => println!("No readable interrupt endpoint"),
    }

    match find_readable_endpoint(device, device_desc, libusb::TransferType::Bulk) {
        Some(endpoint) => read_endpoint(&mut handle, endpoint, libusb::TransferType::Bulk),
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
                            println!(" - read: {:?}", vec);
                        }
                        Err(err) => println!("could not read from endpoint: {}", err),
                    }
                }
                libusb::TransferType::Bulk => {
                    match handle.read_bulk(endpoint.address, buf, timeout) {
                        Ok(len) => {
                            unsafe { vec.set_len(len) };
                            println!(" - read: {:?}", vec);
                        }
                        Err(err) => println!("could not read from endpoint: {}", err),
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
