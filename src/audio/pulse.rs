use std::{
    sync::{mpsc, Arc},
    thread,
    time::Duration,
};

use libpulse_binding::{
    callbacks::ListResult,
    context::{
        introspect::Introspector,
        subscribe::{Facility, InterestMaskSet},
        Context, FlagSet as ContextFlagSet, State,
    },
    def::Retval,
    mainloop::standard::{IterateResult, Mainloop},
    proplist::{properties, Proplist},
    sample::{Format, Spec},
    volume::Volume,
};
use parking_lot::Mutex;

use crate::audio::SAMPLE_RATE;

/// Abstracts connections and interfacing with pulseaudio
pub struct PulseClient {
    context: Arc<Mutex<Context>>,
    introspector: Introspector,
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

        let (context_sender, context_rec) = mpsc::channel();

        thread::spawn(move || {
            let (context, mut mainloop) = match Self::setup_mainloop(proplist) {
                Ok(tuple) => tuple,
                Err(e) => {
                    context_sender.send(Err(e)).unwrap();
                    return;
                }
            };

            context_sender.send(Ok(context)).unwrap();
            mainloop.run().unwrap();
        });

        let context = context_rec
            .recv_timeout(Duration::from_millis(1000))
            .map_err(|_| PulseClientError::Fatal("Did not receive context".to_string()))??;

        let introspector = context.introspect();
        let client = Self {
            context: Mutex::new(context).into(),
            introspector,
            spec,
        };

        Ok(client)
    }

    fn setup_mainloop(proplist: Proplist) -> Result<(Context, Mainloop), PulseClientError> {
        let mut mainloop = Mainloop::new().ok_or(PulseClientError::Fatal(
            "Failed to create mainloop".to_string(),
        ))?;

        let mut context = Context::new_with_proplist(&mainloop, "pulseshitter", &proplist).ok_or(
            PulseClientError::Fatal("Failed to create context".to_string()),
        )?;

        context
            .connect(None, ContextFlagSet::NOFLAGS, None)
            .map_err(|_| PulseClientError::ConnectionFailed)?;

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

        Ok((context, mainloop))
    }

    pub fn subscribe_to_events(&self) {
        let mut context = self.context.lock();

        // Set up the callback that will handle events.
        context.set_subscribe_callback(Some(Box::new(|facility_opt, operation, index| {
            if let Some(facility) = facility_opt {
                match facility {
                    Facility::SinkInput => {
                        println!(
                            "Sink input event: index = {}, operation = {:?}",
                            index, operation
                        );
                    }
                    Facility::Source => {
                        println!(
                            "Source event: index = {}, operation = {:?}",
                            index, operation
                        );
                    }
                    _ => {}
                }
            }
        })));

        // Subscribe to all relevant events.
        context.subscribe(
            InterestMaskSet::SINK_INPUT | InterestMaskSet::SOURCE,
            |success| {
                if !success {
                    eprintln!("Failed to subscribe to sink and source events");
                } else {
                    println!("subscribed");
                }
            },
        );
    }

    pub fn sink_inputs(&self) -> Result<Vec<SinkInput>, PulseClientError> {
        let (sender, receiver) = mpsc::channel();

        self.introspector.get_sink_input_info_list({
            move |list| match list {
                ListResult::End => sender.send(ListResult::End).unwrap(),
                ListResult::Error => sender.send(ListResult::Error).unwrap(),
                ListResult::Item(item) => {
                    let volume = item.volume.max().0 as f32 / Volume::NORMAL.0 as f32;

                    let sink_input = SinkInput {
                        index: item.index,
                        props: item.proplist.clone(),
                        name: item
                            .name
                            .clone()
                            .map(|n| n.to_string())
                            .unwrap_or("Unknown".to_string()),
                        volume,
                    };

                    sender.send(ListResult::Item(sink_input)).unwrap();
                }
            }
        });

        let mut result = vec![];

        loop {
            match receiver.recv().unwrap() {
                ListResult::End => break,
                ListResult::Item(x) => result.push(x),
                ListResult::Error => return Err(PulseClientError::ListError),
            }
        }

        Ok(result)
    }
}

#[derive(Debug, Clone)]
pub struct SinkInput {
    name: String,
    index: u32,
    volume: f32,
    props: Proplist,
}

impl Drop for PulseClient {
    fn drop(&mut self) {
        // self.mainloop.lock().quit(Retval(0));
        self.context.lock().disconnect();
    }
}

#[derive(Debug)]
pub enum PulseClientError {
    ConnectionFailed,
    ListError,
    Fatal(String),
}
