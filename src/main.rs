use std::io::Stdin;

use pulsectl::controllers::{DeviceControl, SinkController, AppControl};

fn main() {
    let mut handler = SinkController::create().unwrap();

    let device = handler
        .get_default_device()
        .expect("Could not get default device");

    println!("Pulseshitter");
    println!(
        "Using device: {}",
        device
            .driver
            .unwrap_or_else(|| "Unknown driver".to_string())
    );

    let stdin = std::io::stdin();

    let applications = handler.list_applications().expect("Could not get application list");

    println!("Found {} applications:", applications.len());

    for app in applications {
        println!("{} - {}", app.connection_id, app.name.unwrap());
    }
}

trait Prompt {
    fn prompt(&self, message: String) -> String;
}

impl Prompt for Stdin {
    fn prompt(&self, message: String) -> String {
        let mut result = String::new();
        println!("{}: ", message);

        self.read_line(&mut result).expect("Read line correctly");
        result
    }
}
