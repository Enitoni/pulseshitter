use std::sync::Arc;

use libpulse_binding::{
    context::{Context, FlagSet as ContextFlagSet, State},
    def::Retval,
    mainloop::standard::{IterateResult, Mainloop},
    proplist::{properties, Proplist},
    sample::{Format, Spec},
};
use parking_lot::Mutex;

use crate::audio::SAMPLE_RATE;

/// Abstracts connections and interfacing with pulseaudio
pub struct PulseClient {
    mainloop: Arc<Mutex<Mainloop>>,
    context: Arc<Mutex<Context>>,
    spec: Spec,
}

impl PulseClient {
    pub fn new() -> Result<Self, PulseClientError> {
        let spec = Spec {
            format: Format::S16NE,
            channels: 2,
            rate: SAMPLE_RATE as u32,
        };

        let mut proplist = Proplist::new().ok_or(PulseClientError::Fatal(
            "Failed to create proplist".to_string(),
        ))?;

        proplist
            .set_str(properties::APPLICATION_NAME, "pulseshitter")
            .and_then(|_| {
                proplist.set_str(properties::APPLICATION_VERSION, env!("CARGO_PKG_VERSION"))
            })
            .map_err(|_| {
                PulseClientError::Fatal("Failed to set proplist properties".to_string())
            })?;

        let mainloop = Mainloop::new().ok_or(PulseClientError::Fatal(
            "Failed to create mainloop".to_string(),
        ))?;

        let mut context = Context::new_with_proplist(&mainloop, "pulseshitter", &proplist).ok_or(
            PulseClientError::Fatal("Failed to create context".to_string()),
        )?;

        context
            .connect(None, ContextFlagSet::NOFLAGS, None)
            .map_err(|_| PulseClientError::ConnectionFailed)?;

        let instance = Self {
            mainloop: Mutex::new(mainloop).into(),
            context: Mutex::new(context).into(),
            spec,
        };

        instance.wait_until_ready()?;
        Ok(instance)
    }

    fn wait_until_ready(&self) -> Result<(), PulseClientError> {
        let mut mainloop = self.mainloop.lock();
        let context = self.context.lock();

        loop {
            match mainloop.iterate(false) {
                IterateResult::Quit(_) | IterateResult::Err(_) => {
                    return Err(PulseClientError::Fatal(
                        "Failed mainloop iterate state".to_string(),
                    ));
                }
                IterateResult::Success(_) => {}
            }
            match context.get_state() {
                State::Ready => {
                    break;
                }
                State::Failed | State::Terminated => {
                    return Err(PulseClientError::Fatal(
                        "Context state failed/terminated".to_string(),
                    ));
                }
                _ => {}
            }
        }

        Ok(())
    }
}

impl Drop for PulseClient {
    fn drop(&mut self) {
        self.mainloop.lock().quit(Retval(0));
        self.context.lock().disconnect();
    }
}

#[derive(Debug)]
pub enum PulseClientError {
    ConnectionFailed,
    Fatal(String),
}
