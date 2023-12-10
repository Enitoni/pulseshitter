use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use crossbeam::atomic::AtomicCell;
use libpulse_binding::context::subscribe::Operation;
use parking_lot::{Mutex, RwLock};

use super::pulse::{PulseClient, PulseClientEvent, SinkInput};

/// Keeps track of active sources and diffing
pub struct SourceSelector {
    client: Arc<PulseClient>,
    current_source: Mutex<Option<Source>>,

    /// The source the user selected.
    /// Not to be confused with current source which is what is currently being streamed.
    selected_source: Mutex<Option<Source>>,
    stored_sources: Mutex<Vec<Source>>,
}

impl SourceSelector {
    pub fn new(client: Arc<PulseClient>) -> Self {
        let sources: Vec<Source> = client
            .sink_inputs()
            .expect("Gets sink inputs")
            .into_iter()
            .map(|f| f.into())
            .collect();

        Self {
            client,
            stored_sources: sources.into(),
            current_source: Default::default(),
            selected_source: Default::default(),
        }
    }

    pub fn sources(&self) -> Vec<Source> {
        self.stored_sources.lock().clone()
    }

    pub fn select(&self, source: Option<Source>) {
        match source {
            Some(x) => {
                *self.current_source.lock() = Some(x.clone());
                *self.selected_source.lock() = Some(x.clone());
            }
            None => {
                *self.current_source.lock() = None;
                *self.selected_source.lock() = None;
            }
        }
    }

    fn handle_sink_input_event(&self, sink_inputs: Vec<SinkInput>, event: PulseClientEvent) {
        let PulseClientEvent::SinkInput { index, operation } = event;

        let mut current_sources = self.stored_sources.lock();
        let new_sources: Vec<Source> = sink_inputs.into_iter().map(|f| f.into()).collect();

        let source = new_sources
            .into_iter()
            .find(|x| x.sink_input().index == index);

        let existing_source = current_sources
            .iter()
            .find(|x| x.sink_input().index == index);

        match operation {
            Operation::New => {
                current_sources.push(source.expect("New source exists by index"));
            }
            Operation::Changed => {
                existing_source
                    .expect("Existing source exists")
                    .update(source.expect("Changed source exists by index"));
            }
            Operation::Removed => {
                existing_source.expect("Existing source exists").remove();
            }
        }
    }
}

/// A sink input simplified for ease of use
#[derive(Debug, Clone)]
pub struct Source {
    sink_input: Arc<Mutex<SinkInput>>,

    /// The best fitting name for this source
    name: Arc<RwLock<String>>,

    /// The binary that spawned the associated sink input
    application: String,

    /// This will be false when listing applications from pulsectl does not include this source
    available: Arc<AtomicCell<bool>>,
    age: Arc<AtomicCell<Instant>>,

    /// Volume of the sink input, used for normalization
    volume: Arc<AtomicCell<f32>>,
}

impl Source {
    /// How long should a source persist for after it is unavailable
    const MAX_LIFESPAN: Duration = Duration::from_secs(60);

    fn update(&self, incoming: Source) {
        self.age.store(Instant::now());

        *self.name.write() = incoming.name.read().clone();
        *self.sink_input.lock() = incoming.sink_input.lock().clone();

        self.volume.store(incoming.volume.load());
        self.available.store(true);
    }

    fn remove(&self) {
        self.available.store(false);
    }

    fn is_dead(&self) -> bool {
        !self.available() && self.age.load().elapsed() >= Self::MAX_LIFESPAN
    }

    pub fn sink_input(&self) -> SinkInput {
        self.sink_input.lock().clone()
    }

    pub fn available(&self) -> bool {
        self.available.load()
    }

    pub fn volume(&self) -> f32 {
        self.volume.load()
    }
}

impl From<SinkInput> for Source {
    fn from(raw: SinkInput) -> Self {
        let name_candidates: Vec<_> = [
            Some(raw.name.clone()),
            raw.props.get_str("application.process.binary"),
            raw.props.get_str("application.name"),
            raw.props.get_str("media.name"),
            raw.props.get_str("node.name"),
        ]
        .into_iter()
        .flatten()
        .collect();

        let application = raw
            .props
            .get_str("application.process.binary")
            .or_else(|| raw.props.get_str("application.name"))
            .unwrap_or_else(|| "Unknown app".to_string());

        let name = name_candidates[0].to_string();
        let volume = AtomicCell::new(raw.volume);

        Self {
            application,
            volume: volume.into(),
            name: RwLock::new(name).into(),
            sink_input: Mutex::new(raw).into(),
            available: AtomicCell::new(true).into(),
            age: AtomicCell::new(Instant::now()).into(),
        }
    }
}
