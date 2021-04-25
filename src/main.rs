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
    let mut thedevice = None;

    for device in context.devices().unwrap().iter() {
        let device_desc = device.device_descriptor().unwrap();

        println!(
            "Bus {:03} Device {:03} ID {:04x}:{:04x}",
            device.bus_number(),
            device.address(),
            device_desc.vendor_id(),
            device_desc.product_id(),
        );

        if device_desc.vendor_id() == 944 && device_desc.product_id() == 111 {
            thedevice = Some(device);
        }
    }

    match thedevice {
        None => {
            panic!("Couldn't find a suitable USB-MIDI device");
        }
        Some(thedevice) => main2(thedevice).unwrap(),
    }
}

fn main2(device: libusb::Device) -> Result<(), AppError> {
    let mut handle = device.open()?;
    handle.reset()?;

    let timeout = Duration::from_secs(1);
    let languages = handle.read_languages(timeout)?;

    println!("Active configuration: {}", handle.active_configuration()?);
    println!("Languages: {:?}", languages);

    Ok(())
}
