use crossbeam::atomic::AtomicCell;
use lazy_static::lazy_static;
use parking_lot::{Mutex, RwLock};
use regex::Regex;
use std::{
    cmp::Ordering,
    fmt::Display,
    io::{BufRead, BufReader},
    process::{Command, Stdio},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};
use strsim::jaro;

use super::AudioSystem;
use pactl::*;

type SinkInputIdx = Arc<AtomicCell<u32>>;

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

/// Abstracts pulseaudio/pipewire related implementations
#[derive(Debug)]
pub struct PulseAudio {
    current_device: Mutex<Device>,
    current_source: Mutex<Option<Source>>,

    /// The source the user selected.
    /// Not to be confused with current source which is what is currently being streamed.
    selected_source: Mutex<Option<Source>>,
    sources: SourceManager,
}

impl PulseAudio {
    pub fn new() -> Self {
        let default_device: Device = Sink::default().into();

        let new = Self {
            current_device: default_device.into(),
            current_source: Default::default(),

            selected_source: Default::default(),
            sources: Default::default(),
        };

        new.update();
        new
    }

    pub fn update(&self) {
        self.sources.update(SinkInput::list());
    }

    pub fn sources(&self) -> Vec<Source> {
        self.sources.list()
    }

    pub fn current_source(&self) -> Option<Source> {
        self.current_source.lock().clone()
    }

    pub fn set_current_source(&self, source: Source) {
        *self.current_source.lock() = Some(source)
    }

    pub fn selected_source(&self) -> Option<Source> {
        self.selected_source.lock().clone()
    }

    pub fn set_selected_source(&self, source: Source) {
        *self.selected_source.lock() = Some(source)
    }

    pub fn current_device(&self) -> Device {
        self.current_device.lock().clone()
    }

    pub fn clear(&self) {
        *self.current_source.lock() = None;
        *self.selected_source.lock() = None;
    }

    pub fn invalid(&self) {
        *self.current_source.lock() = None;
    }
}

impl Default for PulseAudio {
    fn default() -> Self {
        Self::new()
    }
}

/// A parsed and normalized device
#[derive(Debug, Clone)]
pub struct Device {
    id: String,
    name: String,
}

impl Device {
    pub fn name(&self) -> String {
        self.name.clone()
    }

    pub fn id(&self) -> String {
        self.id.clone()
    }
}

impl From<Sink> for Device {
    fn from(raw: Sink) -> Self {
        let name = [
            raw.properties.get("device.product.name").cloned(),
            raw.properties.get("device.description").cloned(),
            raw.properties.get("node.name").cloned(),
            Some(raw.name.clone()),
        ]
        .into_iter()
        .flatten()
        .next()
        .unwrap_or_default();

        Self { id: raw.name, name }
    }
}

/// A parsed and normalized audio source
#[derive(Debug, Clone)]
pub struct Source {
    input_index: SinkInputIdx,
    name: Arc<RwLock<String>>,

    /// This will be false when listing applications from pulsectl does not include this source
    available: Arc<AtomicCell<bool>>,
    age: Arc<AtomicCell<Instant>>,

    kind: SourceKind,
}

enum SourceComparison {
    Exact,
    Partial(f64),
    None,
}

impl SourceComparison {
    fn score(&self) -> f64 {
        match self {
            SourceComparison::Exact => f64::MAX,
            SourceComparison::Partial(score) => *score,
            SourceComparison::None => 0.,
        }
    }
}

impl Source {
    /// How long should a source persist for after it is unavailable
    const MAX_LIFESPAN: Duration = Duration::from_secs(60);

    /// Unfortunately, there isn't a way to identify new streams that come from the same source as old ones,
    /// so this function tries to do its best to see if this source may be the same as the rhs.
    fn compare(&self, rhs: &Source) -> SourceComparison {
        // It is unlikely that there will ever be conflicts, so if the indices match, this is most likely the same source.
        let is_same_index = self.input_index() == rhs.input_index();
        let is_same_name = *self.name.read() == *rhs.name.read();

        if is_same_index || is_same_name {
            return SourceComparison::Exact;
        }

        if self.kind != rhs.kind {
            return SourceComparison::None;
        }

        let score = jaro(&self.name.read(), &rhs.name.read());

        match score {
            x if x > 0.5 => SourceComparison::Partial(x),
            _ => SourceComparison::None,
        }
    }

    fn update(&self, new: Source) {
        self.input_index.store(new.input_index.load());
        self.age.store(new.age.load());
        self.available.store(true);
        *self.name.write() = new.name()
    }

    fn is_dead(&self) -> bool {
        !self.available() && self.age.load().elapsed() >= Self::MAX_LIFESPAN
    }

    pub fn name(&self) -> String {
        self.name.read().clone()
    }

    pub fn input_index(&self) -> u32 {
        self.input_index.load()
    }

    pub fn available(&self) -> bool {
        self.available.load()
    }

    pub fn kind(&self) -> SourceKind {
        self.kind
    }
}

impl From<SinkInput> for Source {
    fn from(raw: SinkInput) -> Self {
        let mut name_candidates: Vec<_> = [
            raw.properties.get("application.process.binary"),
            raw.properties.get("application.name"),
            raw.properties.get("media.name"),
            raw.properties.get("node.name"),
        ]
        .into_iter()
        .flatten()
        .filter_map(|s| {
            let score = calculate_name_quality(s);

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

        Self {
            kind,
            name: Arc::new(name.into()),
            available: Arc::new(true.into()),
            age: Arc::new(Instant::now().into()),
            input_index: Arc::new((raw.index as u32).into()),
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

#[derive(Debug, Default)]
struct SourceManager(Mutex<Vec<Source>>);

impl SourceManager {
    const MINIMUM_RESTORE_SCORE: f64 = 0.5;

    fn update(&self, incoming: Vec<SinkInput>) {
        let parsed_incoming: Vec<_> = incoming
            .into_iter()
            .map(Source::from)
            .filter(|s| {
                ALLOW_SPOTIFY_STREAMING.is_some()
                    || s.name().to_uppercase() != SPOTIFY_NAME.to_uppercase()
            })
            .collect();

        let mut existing_sources = self.0.lock();

        // This will be set to true again for the existing ones
        existing_sources
            .iter()
            .for_each(|source| source.available.store(false));

        for new_source in parsed_incoming {
            let existing = existing_sources
                .iter()
                .find(|old| matches!(old.compare(&new_source), SourceComparison::Exact));

            match existing {
                Some(s) => s.update(new_source),
                None => existing_sources.push(new_source),
            }
        }

        for source in existing_sources
            .clone()
            .into_iter()
            .filter(|s| s.available())
        {
            let to_remove = existing_sources
                .iter()
                .enumerate()
                .filter(|(_, s)| !s.available())
                .find(|(_, s)| {
                    matches!(s.compare(&source), SourceComparison::Exact) || s.is_dead()
                });

            if let Some((i, _)) = to_remove {
                existing_sources.remove(i);
            }
        }

        existing_sources.sort_by(|a, b| b.age.load().cmp(&a.age.load()));
    }

    fn list(&self) -> Vec<Source> {
        self.0
            .lock()
            .clone()
            .into_iter()
            .filter(|s| !s.is_dead())
            .collect()
    }

    fn restore(&self, target: Source) -> Option<Source> {
        let sources = self.0.lock();

        let mut candidates: Vec<_> = sources
            .iter()
            .filter(|c| c.available())
            .map(|c| (c, c.compare(&target).score()))
            .collect();

        //

        candidates.sort_by(|(_, a), (_, b)| {
            if a > b {
                Ordering::Greater
            } else {
                Ordering::Less
            }
        });

        candidates
            .into_iter()
            .find_map(|(source, score)| {
                if score > Self::MINIMUM_RESTORE_SCORE && source.available() {
                    Some(source)
                } else {
                    None
                }
            })
            .cloned()
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

pub fn spawn_pulse_thread(audio: Arc<AudioSystem>) {
    let run = move || loop {
        let mut child = Command::new("pactl")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .arg("subscribe")
            .spawn()
            .expect("spawns pactl subscriber");

        let stdout = child.stdout.take().unwrap();
        let mut reader = BufReader::new(stdout);

        loop {
            let mut line = String::new();
            let result = reader.read_line(&mut line);

            if result.is_err() {
                break;
            }

            if !line.contains("sink-input") {
                continue;
            }

            audio.pulse.update();

            let current = audio.pulse.current_source();
            let selected = audio.pulse.selected_source();

            if let (Some(selected), None) = (selected, current) {
                let restored = audio.pulse.sources.restore(selected);

                if let Some(restored) = restored {
                    audio.set_source(restored);
                }
            }
        }

        child.wait().expect("child exits correctly");
    };

    thread::Builder::new()
        .name("audio-pulse".to_string())
        .spawn(run)
        .unwrap();
}

mod pactl {
    use std::{
        collections::HashMap,
        io::Read,
        process::{Command, Stdio},
    };

    use serde::Deserialize;
    use serde_json::Value;

    #[derive(Debug)]
    pub struct SinkInput {
        pub index: u64,
        pub volume: f32,
        pub properties: HashMap<String, String>,
    }

    impl SinkInput {
        fn parse(value: &Value) -> Self {
            let index = value
                .get("index")
                .and_then(|i| i.as_u64())
                .expect("index is parsed correctly");

            let volume = value
                .get("volume")
                .and_then(|v| {
                    let parse_channel = |v: &Value| {
                        v.get("db")
                            .and_then(|db| db.as_str())
                            .and_then(|str| str[..3].parse::<f32>().ok())
                            .map(|v| 10f32.powf(v / 20.))
                    };

                    let left = v.get("front-left").and_then(parse_channel);
                    let right = v.get("front-right").and_then(parse_channel);

                    left.zip(right)
                })
                .map(|(left, right)| left + right / 2.)
                .unwrap_or(1.);

            let properties: HashMap<String, String> = value
                .get("properties")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .expect("properties is parsed correctly");

            Self {
                index,
                volume,
                properties,
            }
        }

        pub fn list() -> Vec<Self> {
            let mut child = Command::new("pactl")
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .arg("--format=json")
                .arg("list")
                .arg("sink-inputs")
                .spawn()
                .expect("spawns pactl");

            let stdout = child.stdout.take().unwrap();
            let result: Vec<Value> = serde_json::from_reader(stdout).expect("parses pactl output");

            child.wait().expect("pactl exited correctly");
            result.into_iter().map(|v| Self::parse(&v)).collect()
        }
    }

    #[derive(Debug, Deserialize)]
    pub struct Sink {
        pub name: String,
        pub properties: HashMap<String, String>,
    }

    impl Sink {
        pub fn default() -> Self {
            let mut child = Command::new("pactl")
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .arg("get-default-sink")
                .spawn()
                .expect("spawns pactl");

            let mut stdout = child.stdout.take().unwrap();
            let mut default_sink = String::new();

            stdout
                .read_to_string(&mut default_sink)
                .expect("read from pactl");

            child.wait().expect("pactl exited correctly");

            Self::list()
                .into_iter()
                .find(|sink| sink.name == default_sink.trim())
                .expect("find default sink")
        }

        pub fn list() -> Vec<Self> {
            let mut child = Command::new("pactl")
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .arg("--format=json")
                .arg("list")
                .arg("sinks")
                .spawn()
                .expect("spawns pactl");

            let stdout = child.stdout.take().unwrap();
            let result: Vec<Self> = serde_json::from_reader(stdout).expect("parses pactl output");

            child.wait().expect("pactl exited correctly");
            result
        }
    }
}
