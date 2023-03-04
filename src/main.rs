use std::{
    io::{copy, Stdin, Stdout},
    process::{ChildStdout, Command, Stdio},
};

use pulsectl::controllers::{
    types::{ApplicationInfo, DeviceInfo},
    AppControl, DeviceControl, SinkController,
};

use crate::audio::ParecStream;

mod audio;
mod dickcord;

#[tokio::main]
async fn main() {
    let mut handler = SinkController::create().unwrap();

    let device = handler
        .get_default_device()
        .expect("Could not get default device");

    println!("Pulseshitter");
    println!(
        "Using device: {}",
        device
            .driver
            .clone()
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

    println!("You selected {}", app.name.clone().unwrap());

    dickcord::dickcord(device, app).await;
}

/**
*
* spawn("parec", [
     "--verbose",
     "--device",
     source.deviceName,
     "--monitor-stream",
     String(source.sinkInputIndex),
     // discord.js voice 'raw' wants this
     "--format=s16le",
     // pin rate and channels to what discord requires
     "--rate=48000",
     "--channels=2",
     // set latency and processing time as low as parec allows and let
     // pulseaudio do its best instead -- the defaults are very high to
     // "power saving reasons" which is suboptimal for sharing live audio
     "--latency=1",
     "--process-time=1",
   ])
*/
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
