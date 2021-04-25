#![feature(backtrace)]
use std::backtrace::Backtrace;
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

fn main() {
    let context = libusb::Context::new().unwrap();

    for device in context.devices().unwrap().iter() {
        let device_desc = device.device_descriptor().unwrap();

        println!(
            "Bus {:03} Device {:03} ID {:04x}:{:04x}",
            device.bus_number(),
            device.address(),
            device_desc.vendor_id(),
            device_desc.product_id(),
        );

        if device_desc.vendor_id() == 0x944 && device_desc.product_id() == 0x111 {
            main2(device, &device_desc).unwrap()
        }
    }
    println!("Exiting...");
}

fn main2(device: libusb::Device, device_desc: &libusb::DeviceDescriptor) -> Result<(), AppError> {
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

    Ok(())
}
