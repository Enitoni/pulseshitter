use super::pulse::{PulseClient, SinkInput};
use crossbeam::atomic::AtomicCell;
use lazy_static::lazy_static;
use libpulse_binding::context::subscribe::Operation;
use parking_lot::{Mutex, RwLock};
use regex::Regex;
use std::{
    fmt::Display,
    sync::Arc,
    time::{Duration, Instant},
};
use strsim::jaro;

/// Due to Discord having an agreement with Spotify, you cannot actually stream Spotify audio on Discord
/// without it pausing your Spotify playback after a few seconds when it detects you may be doing this.
///
/// Because of this, pulseshitter is technically a workaround, as there is no way for Discord to detect that you may be streaming Spotify through it.
/// In order to be on the safe side regarding TOS and legal matters, Spotify streaming is disabled by default.
///
/// If you don't care about this, you can compile pulseshitter with the environment variable below present to enable it anyway.
const ALLOW_SPOTIFY_STREAMING: Option<&'static str> = option_env!("ALLOW_SPOTIFY_STREAMING");
const SPOTIFY_NAME: &str = "Spotify";

// These are words commonly used in vague source names that is not useful to the user
const VAGUE_WORDS: [&str; 10] = [
    "play", "audio", "voice", "stream", "driver", "webrtc", "engine", "playback", "callback",
    "alsa",
];

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
        self.stored_sources
            .lock()
            .clone()
            .into_iter()
            .filter(|s| {
                ALLOW_SPOTIFY_STREAMING.is_some()
                    || s.name().to_uppercase() != SPOTIFY_NAME.to_uppercase()
            })
            .collect()
    }

    pub fn current_source(&self) -> Option<Source> {
        self.current_source.lock().clone().filter(|s| s.available())
    }

    pub fn selected_source(&self) -> Option<Source> {
        self.selected_source.lock().clone()
    }

    pub(super) fn select(&self, source: Option<Source>) {
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

    pub fn handle_sink_input_event(&self, index: u32, operation: Operation) {
        let mut current_sources = self.stored_sources.lock();

        let new_sources: Vec<Source> = self
            .client
            .sink_inputs()
            .expect("Gets sink inputs")
            .into_iter()
            .map(|f| f.into())
            .collect();

        let source = new_sources
            .into_iter()
            .find(|x| x.sink_input().index == index);

        let existing_source = current_sources
            .iter()
            .find(|x| x.sink_input().index == index);

        match operation {
            Operation::New => {
                let selected_source = self.selected_source();
                let new_source = source.expect("New source exists by index");

                let new_as_selected = selected_source
                    .filter(|s| !s.available() && s.compare(&new_source).is_similar_enough());

                if let Some(selected) = new_as_selected {
                    selected.update(new_source);
                } else {
                    current_sources.push(new_source);
                }
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

        current_sources.retain(|s| !s.is_dead());
    }
}

/// A sink input simplified for ease of use
#[derive(Debug, Clone)]
pub struct Source {
    kind: SourceKind,
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

#[derive(Debug)]
enum SourceComparison {
    Exact,
    Partial(f64),
    None,
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

    /// Checks to see how similar this source is to an old one
    fn compare(&self, rhs: &Source) -> SourceComparison {
        // It is unlikely that there will ever be conflicts, so if the indices match, this is most likely the same source.
        let is_same_index = self.index() == rhs.index();
        let is_same_name = *self.name.read() == *rhs.name.read();

        if is_same_index || is_same_name {
            return SourceComparison::Exact;
        }

        if self.application != rhs.application {
            return SourceComparison::None;
        }

        let score = jaro(&self.name.read(), &rhs.name.read());
        SourceComparison::Partial(score)
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

    pub fn index(&self) -> u32 {
        self.sink_input.lock().index
    }

    pub fn available(&self) -> bool {
        self.available.load()
    }

    pub fn volume(&self) -> f32 {
        self.volume.load()
    }

    pub fn name(&self) -> String {
        self.name.read().clone()
    }
}

impl SourceComparison {
    fn is_similar_enough(&self) -> bool {
        match self {
            SourceComparison::Partial(score) => *score > 0.3,
            SourceComparison::Exact => true,
            SourceComparison::None => false,
        }
    }
}

/// When apps like browsers have multiple tabs, there is no way to differentiate the source from each one without covering these edge cases.
/// That is the purpose of this enum.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SourceKind {
    Standalone,
    BrowserTab(BrowserKind),
}

impl SourceKind {
    fn parse<T: AsRef<str>>(candidates: &[T]) -> Self {
        candidates
            .iter()
            .map(AsRef::as_ref)
            .map(BrowserKind::parse)
            .find_map(|k| k.map(Into::into))
            .unwrap_or(Self::Standalone)
    }

    fn determine_name<T: AsRef<str>>(&self, candidates: &[T]) -> String {
        match self {
            SourceKind::BrowserTab(b) => b.determine_tab_name(candidates),
            SourceKind::Standalone => candidates
                .iter()
                .map(AsRef::as_ref)
                .map(ToOwned::to_owned)
                .next()
                .unwrap_or_else(|| "Unidentifiable audio source".to_string()),
        }
    }
}

/// Currently supported Browser edgecases
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BrowserKind {
    Firefox,
    Chrome,
}

impl BrowserKind {
    const FIREFOX: &str = "Firefox";
    const CHROME: &str = "Chrome";

    fn parse<T: AsRef<str>>(name: T) -> Option<Self> {
        match name.as_ref().to_uppercase() {
            x if x == Self::FIREFOX.to_uppercase() => Self::Firefox.into(),
            x if x == Self::CHROME.to_uppercase() => Self::Chrome.into(),
            _ => None,
        }
    }

    fn determine_tab_name<T: AsRef<str>>(&self, candidates: &[T]) -> String {
        let browser_name = self.to_string();

        candidates
            .iter()
            .map(AsRef::as_ref)
            .map(ToOwned::to_owned)
            .find(|c| c.to_uppercase() != browser_name.to_uppercase())
            .unwrap_or_else(|| format!("Unidentifiable {} Tab", browser_name))
    }
}

impl Display for BrowserKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::Firefox => Self::FIREFOX,
            Self::Chrome => Self::CHROME,
        };

        write!(f, "{}", name)
    }
}

impl From<BrowserKind> for SourceKind {
    fn from(value: BrowserKind) -> Self {
        Self::BrowserTab(value)
    }
}

impl From<SinkInput> for Source {
    fn from(raw: SinkInput) -> Self {
        let mut name_candidates: Vec<_> = [
            Some(raw.name.clone()),
            raw.props.get_str("application.process.binary"),
            raw.props.get_str("application.name"),
            raw.props.get_str("media.name"),
            raw.props.get_str("node.name"),
        ]
        .into_iter()
        .flatten()
        .filter_map(|s| {
            let score = calculate_name_quality(&s);

            if score > 1 {
                Some((s, score))
            } else {
                None
            }
        })
        .collect();

        name_candidates.sort_by(|(_, a), (_, b)| b.cmp(a));
        let name_candidates: Vec<_> = name_candidates.into_iter().map(|(s, _)| s).collect();

        let kind = SourceKind::parse(&name_candidates);
        let name = kind.determine_name(&name_candidates);

        let application = raw
            .props
            .get_str("application.process.binary")
            .or_else(|| raw.props.get_str("application.name"))
            .unwrap_or_else(|| "Unknown app".to_string());

        let volume = AtomicCell::new(raw.volume);

        Self {
            kind,
            application,
            volume: volume.into(),
            name: RwLock::new(name).into(),
            sink_input: Mutex::new(raw).into(),
            available: AtomicCell::new(true).into(),
            age: AtomicCell::new(Instant::now()).into(),
        }
    }
}

lazy_static! {
    static ref WORD_SPLIT_REGEX: Regex =
        Regex::new(r"([^.,\-_\sA-Z]+)|([^.,\-_\sa-z][^.\sA-Z]+)").unwrap();
}

fn str_is_doublecase(str: &str) -> bool {
    str.chars().filter(|c| c.is_lowercase()).count() < str.len()
}

fn calculate_name_quality(str: &str) -> i32 {
    let mut score = 0;

    score += str_is_doublecase(str) as i32;

    let words: Vec<_> = WORD_SPLIT_REGEX
        .find_iter(str)
        .map(|m| m.as_str())
        .collect();

    score += words.into_iter().fold(0, |acc, w| {
        let is_vague = VAGUE_WORDS
            .iter()
            .any(|word| w.to_uppercase() == word.to_uppercase());

        if is_vague {
            acc - 1
        } else {
            acc + 1
        }
    });

    score
}
