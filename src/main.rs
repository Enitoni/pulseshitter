use app::App;

mod app;
mod audio;
mod dickcord;
mod interface;
mod state;

fn main() {
    let app = App::new();

    match app {
        Ok(app) => app.run_tui(),
        Err(err) => {
            eprintln!("pulseshitter failed to start. please verify that you have all the required components installed.");
            eprintln!("error: {}", err);
        }
    }
}
