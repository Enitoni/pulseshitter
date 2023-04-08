use std::{
    env,
    io::Stdin,
    sync::{mpsc, Arc},
    thread,
};

use crate::audio::AudioSystem;

mod audio;
mod dickcord;
mod pulse;

#[tokio::main]

async fn main() {
    setup_logger();

    let pulse = pulse::PulseAudio::new();

    // Run this once to get list of applications
    pulse.update_applications();

    println!("Pulseshitter");
    println!("Using device: {}", pulse.device_name());

    let stdin = std::io::stdin();
    let applications = pulse.applications();

    for app in applications.iter() {
        println!(
            "{} - {} ({})",
            app.sink_input_index, &app.name, &app.sink_input_name
        );
    }

    let index = stdin.prompt("Select the id of the application you want to stream");
    let index: u32 = index.trim().parse().expect("Failed to parse input");

    let app = applications
        .into_iter()
        .find(|a| a.sink_input_index == index)
        .expect("Selected application does not exist");

    println!("You selected {}", &app.name);

    let audio = Arc::new(AudioSystem::new(pulse));
    let (sender, receiver) = mpsc::sync_channel::<()>(0);

    thread::spawn({
        let audio = audio.clone();

        move || {
            // Wait for serenity
            receiver.recv().unwrap();

            audio.set_application(app);
            AudioSystem::run(audio);
        }
    });

    dickcord::dickcord(sender, audio.clone()).await
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

fn setup_logger() -> Result<(), fern::InitError> {
    if env::var("ENABLE_LOGGING").is_err() {
        return Ok(());
    };

    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{} {}] {}",
                record.level(),
                record.target(),
                message
            ))
        })
        .level(log::LevelFilter::Debug)
        .chain(std::io::stdout())
        .chain(fern::log_file("output.log")?)
        .apply()?;

    Ok(())
}
