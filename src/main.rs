use std::io::Stdin;

use pulsectl::controllers::{AppControl, DeviceControl, SinkController};

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

    let applications = handler
        .list_applications()
        .expect("Could not get application list");

    println!("Found {} applications:", applications.len());

    for app in applications.iter() {
        println!("{} - {}", app.index, app.name.as_ref().unwrap());
    }

    let index = stdin.prompt("Select the id of the application you want to stream");
    let index: u32 = index.trim().parse().expect("Failed to parse input");

    let app = applications
        .into_iter()
        .find(|a| a.index == index)
        .expect("Application exists");

    println!("You selected {}", app.name.unwrap());
}

trait Prompt {
    fn prompt(&self, message: &str) -> String;
}

impl Prompt for Stdin {
    fn prompt(&self, message: &str) -> String {
        let mut result = String::new();
        println!("{}: ", message);

        self.read_line(&mut result).expect("Read line correctly");
        result
    }
}
