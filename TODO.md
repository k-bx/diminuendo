## TODO (bottom to top)

- **p2** plug device dynamically (check periodically or subscribe)
- **p3** support multiple devices
- **p2** select a USB device intelligently or let the user select it, currently my KORG is hardcoded

## NOTES

- setup:
  - rpi: install `libudev-dev`, `libusb` from source (`apt-get build-dep libusb && ./configure && make && sudo checkinstall`)
- run with `git pull && cargo build && RUST_BACKTRACE=1 sudo ./target/debug/diminuendo`
