use std::{
    sync::{mpsc, Arc},
    thread,
    time::Duration,
};

use crossbeam::channel::{unbounded, Receiver, Sender};
use libpulse_binding::{
    callbacks::ListResult,
    context::{
        introspect::Introspector,
        subscribe::{Facility, InterestMaskSet, Operation},
        Context, FlagSet as ContextFlagSet, State,
    },
    error::Code,
    mainloop::standard::{IterateResult, Mainloop},
    proplist::{properties, Proplist},
    sample::{Format, Spec},
    stream::{FlagSet as StreamFlagSet, PeekResult, State as StreamState, Stream},
    volume::Volume,
};
use parking_lot::{Mutex, RwLock};

use crate::audio::SAMPLE_RATE;

/// Abstracts connections and interfacing with pulseaudio
pub struct PulseClient {
    context: Arc<Mutex<Context>>,
    introspector: Introspector,
    props: Proplist,
    spec: Spec,

    pub(super) events: Receiver<PulseClientEvent>,
    event_sender: Sender<PulseClientEvent>,
}

impl PulseClient {
    pub fn new() -> Result<Self, PulseClientError> {
        let spec = Spec {
            format: Format::F32le,
            channels: 2,
            rate: SAMPLE_RATE as u32,
        };

        let mut proplist = Proplist::new().ok_or(PulseClientError::Fatal(
            "Failed to create proplist".to_string(),
        ))?;

        let props = proplist.clone();

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
        let (sender, receiver) = unbounded();

        let client = Self {
            event_sender: sender,
            events: receiver,
            context: Mutex::new(context).into(),
            introspector,
            props,
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
        let sender = self.event_sender.clone();

        // Set up the callback that will handle events.
        context.set_subscribe_callback(Some(Box::new(move |facility_opt, operation, index| {
            if let Some(Facility::SinkInput) = facility_opt {
                sender
                    .send(PulseClientEvent::SinkInput {
                        index,
                        operation: operation.expect("SinkEvent always has an operation"),
                    })
                    .expect("Send event")
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
                        sink: item.sink,
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

    pub fn record(&self, sink_input: &SinkInput) -> Result<SinkInputStream, PulseClientError> {
        let props = self.props.clone();

        let stream = SinkInputStream::new(self.context.clone(), props, &self.spec);
        stream.connect_to_sink_input(sink_input)?;
        stream.set_event_callbacks();

        Ok(stream)
    }
}

impl Drop for PulseClient {
    fn drop(&mut self) {
        self.context.lock().disconnect();
    }
}

#[derive(Debug)]
pub enum PulseClientError {
    ConnectionFailed,
    ListError,
    Fatal(String),
}

pub enum PulseClientEvent {
    SinkInput { index: u32, operation: Operation },
}

#[derive(Debug, Clone)]
pub struct SinkInput {
    pub(super) name: String,
    pub(super) index: u32,
    pub(super) sink: u32,
    pub(super) volume: f32,
    pub(super) props: Proplist,
}

/// Represents a stream of audio from a sink input
pub struct SinkInputStream {
    context: Arc<Mutex<Context>>,
    stream: Arc<Mutex<Stream>>,
    buffer: Arc<RwLock<Vec<u8>>>,
    status: Arc<RwLock<SinkInputStreamStatus>>,
}

impl SinkInputStream {
    fn new(context: Arc<Mutex<Context>>, mut props: Proplist, spec: &Spec) -> Self {
        let stream = {
            let mut context = context.lock();

            let stream = Stream::new_with_proplist(
                &mut context,
                "pulseshitter-stream",
                spec,
                None,
                &mut props,
            )
            .expect("Creates stream");

            Arc::new(Mutex::new(stream))
        };

        Self {
            context,
            stream,
            buffer: Default::default(),
            status: Default::default(),
        }
    }

    fn set_event_callbacks(&self) {
        let context = self.context.clone();
        let mut locked_stream = self.stream.lock();

        locked_stream.set_state_callback(Some(Box::new({
            let stream = self.stream.clone();
            let status = self.status.clone();

            move || {
                let mut status = status.write();

                match stream.lock().get_state() {
                    StreamState::Ready => *status = SinkInputStreamStatus::Connected,
                    StreamState::Unconnected | StreamState::Creating => {
                        *status = SinkInputStreamStatus::Connecting
                    }
                    StreamState::Terminated => *status = SinkInputStreamStatus::Terminated,
                    StreamState::Failed => {
                        let err: Code = context.lock().errno().try_into().expect("Error is valid");

                        match err {
                            Code::Timeout => *status = SinkInputStreamStatus::TimedOut,
                            x => {
                                *status = SinkInputStreamStatus::Failed(
                                    x.to_string().unwrap_or_else(|| "Unknown".to_string()),
                                )
                            }
                        }
                    }
                }

                dbg!(&status);
            }
        })));

        locked_stream.set_read_callback(Some(Box::new({
            let stream = self.stream.clone();
            let buffer = self.buffer.clone();

            move |_| {
                let mut stream = stream.lock();

                match stream.peek() {
                    Ok(result) => match result {
                        PeekResult::Empty => {}
                        PeekResult::Hole(_) => stream.discard().expect("Discards if hole"),
                        PeekResult::Data(data) => {
                            buffer.write().extend_from_slice(data);
                            stream.discard().expect("Discards after data");
                        }
                    },
                    Err(_) => {
                        unimplemented!()
                    }
                }
            }
        })));

        locked_stream.set_suspended_callback(Some(Box::new({
            let stream = self.stream.clone();
            let status = self.status.clone();

            move || {
                let stream = stream.lock();
                let mut status = status.write();

                if stream.is_suspended().unwrap_or_default() {
                    *status = SinkInputStreamStatus::Suspended
                } else {
                    *status = SinkInputStreamStatus::Connected
                }
            }
        })));
    }

    fn connect_to_sink_input(&self, sink_input: &SinkInput) -> Result<(), PulseClientError> {
        let mut stream = self.stream.lock();

        stream
            .set_monitor_stream(sink_input.index)
            .expect("Sets monitor stream");

        stream
            .connect_record(
                Some(sink_input.sink.to_string().as_str()),
                None,
                StreamFlagSet::DONT_MOVE,
            )
            .expect("Connects stream for recording");

        Ok(())
    }
}

impl Drop for SinkInputStream {
    fn drop(&mut self) {
        let mut stream = self.stream.lock();

        if stream.get_state().is_good() {
            stream.disconnect().unwrap_or_else(|e| {
                eprintln!("Failed to disconnect stream: {}", e);
            })
        }
    }
}

#[derive(Debug, Default, Clone)]
pub enum SinkInputStreamStatus {
    #[default]
    Idle,
    TimedOut,
    Connected,
    Suspended,
    Terminated,
    Connecting,
    Failed(String),
}
