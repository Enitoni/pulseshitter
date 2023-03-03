use pulsectl::controllers::{DeviceControl, SinkController};

fn main() {
    let mut handler = SinkController::create().unwrap();

    let device = handler
        .get_default_device()
        .expect("Could not get default device");

    println!("Pulseshitter");
    println!("Using device: {}", device.name.unwrap());
}
