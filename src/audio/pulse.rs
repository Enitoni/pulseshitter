use crossbeam::atomic::AtomicCell;
use parking_lot::Mutex;
use pulsectl::controllers::{AppControl, DeviceControl};
use std::{
    cmp::Ordering,
    fmt::Display,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};
use strsim::jaro;

use super::AudioSystem;

type SinkInputIdx = Arc<AtomicCell<u32>>;
type RawDevice = pulsectl::controllers::types::DeviceInfo;
type RawSource = pulsectl::controllers::types::ApplicationInfo;

/// Due to Discord having an agreement with Spotify, you cannot actually stream Spotify audio on Discord
/// without it pausing your Spotify playback after a few seconds when it detects you may be doing this.
///
/// Because of this, pulseshitter is technically a workaround, as there is no way for Discord to detect that you may be streaming Spotify through it.
/// In order to be on the safe side regarding TOS and legal matters, Spotify streaming is disabled by default.
///
/// If you don't care about this, you can compile pulseshitter with the environment variable below present to enable it anyway.
const ALLOW_SPOTIFY_STREAMING: Option<&'static str> = option_env!("ALLOW_SPOTIFY_STREAMING");
const SPOTIFY_NAME: &str = "spotify";

/// This is a list of known vague names that applications will use for their audio sources.
const VAGUE_NAMES: [&str; 5] = [
    "Playback",
    "playStream",
    "audioStream",
    "WEBRTC VoiceEngine",
    "AudioCallbackDriver",
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
        let default_device: Device = Self::handler()
            .get_default_device()
            .expect("get default device")
            .into();

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
        self.sources
            .update(Self::handler().list_applications().unwrap_or_default());
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

    /// pulsectl uses Rc and RefCell which makes it non-sync.
    /// Because of this, the controller needs to be recreated every time we want to use it.
    fn handler() -> pulsectl::controllers::SinkController {
        pulsectl::controllers::SinkController::create().expect("create sinkcontroller")
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

impl From<RawDevice> for Device {
    fn from(raw: RawDevice) -> Self {
        let name = [
            raw.proplist.get_str("device.product.name"),
            raw.proplist.get_str("device.description"),
            raw.name.clone(),
            raw.monitor_name,
        ]
        .into_iter()
        .flatten()
        .next()
        .unwrap_or_default();

        Self {
            id: raw.name.expect("device must have a name"),
            name,
        }
    }
}

/// A parsed and normalized audio source
#[derive(Debug, Clone)]
pub struct Source {
    input_index: SinkInputIdx,
    name: Arc<Mutex<String>>,

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
        if self.input_index() == rhs.input_index() {
            return SourceComparison::Exact;
        }

        let mut score = 0.;
        score += jaro(&self.name.lock(), &rhs.name.lock());
        score += (self.kind == rhs.kind) as i32 as f64;

        match score {
            x if x == 2. => SourceComparison::Exact,
            x if x > 0.5 => SourceComparison::Partial(x),
            _ => SourceComparison::None,
        }
    }

    fn update(&self, new: Source) {
        self.input_index.store(new.input_index.load());
        self.age.store(new.age.load());
        self.available.store(true);

        *self.name.lock() = new.name()
    }

    fn is_dead(&self) -> bool {
        !self.available() && self.age.load().elapsed() >= Self::MAX_LIFESPAN
    }

    pub fn name(&self) -> String {
        self.name.lock().clone()
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

impl From<RawSource> for Source {
    fn from(raw: RawSource) -> Self {
        let mut name_candidates: Vec<_> = [
            raw.proplist.get_str("application.name"),
            raw.proplist.get_str("application.process.binary"),
            raw.proplist.get_str("media.name"),
            raw.name,
        ]
        .into_iter()
        .flatten()
        .filter(|n| {
            !VAGUE_NAMES
                .iter()
                .any(|s| s.to_lowercase() == n.to_lowercase())
        })
        .collect();

        let all_props: Vec<_> = raw
            .proplist
            .iter()
            .filter_map(|k| raw.proplist.get_str(&k).map(|v| (k, v)))
            .collect();

        // Favor capitalized app names
        name_candidates.sort_by(|a, _| {
            if str_is_lowercase(a) {
                Ordering::Greater
            } else {
                Ordering::Less
            }
        });

        let kind = SourceKind::parse(&name_candidates);
        let name = kind.determine_name(&name_candidates);

        if name == "Unknown firefox tab" {
            dbg!(&all_props);
        }

        Self {
            kind,
            name: Arc::new(name.into()),
            age: Arc::new(Instant::now().into()),
            input_index: Arc::new(raw.index.into()),
            available: Arc::new(true.into()),
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
            SourceKind::Standalone => candidates
                .iter()
                .map(AsRef::as_ref)
                .map(ToOwned::to_owned)
                .next()
                .unwrap_or_else(|| "Unknown source".to_string()),
            SourceKind::BrowserTab(b) => b.determine_tab_name(candidates),
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
    const FIREFOX: &str = "firefox";
    const CHROME: &str = "chrome";

    fn parse<T: AsRef<str>>(name: T) -> Option<Self> {
        match name.as_ref().to_lowercase() {
            x if x == Self::FIREFOX => Self::Firefox.into(),
            x if x == Self::CHROME => Self::Chrome.into(),
            _ => None,
        }
    }

    fn determine_tab_name<T: AsRef<str>>(&self, candidates: &[T]) -> String {
        let browser_name = self.to_string();

        candidates
            .iter()
            .map(AsRef::as_ref)
            .map(ToOwned::to_owned)
            .find(|c| c.to_lowercase() != browser_name)
            .unwrap_or_else(|| format!("Unknown {} tab", browser_name))
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
    const MINIMUM_RESTORE_SCORE: f64 = 1.;

    fn update(&self, incoming: Vec<RawSource>) {
        let parsed_incoming: Vec<_> = incoming
            .into_iter()
            .map(Source::from)
            .filter(|s| ALLOW_SPOTIFY_STREAMING.is_some() || s.name() != SPOTIFY_NAME)
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

        existing_sources.retain(|s| !s.is_dead())
    }

    fn list(&self) -> Vec<Source> {
        self.0.lock().clone()
    }

    fn restore(&self, target: Source) -> Option<Source> {
        let sources = self.0.lock();

        let mut candidates: Vec<_> = sources
            .iter()
            .map(|c| (c, c.compare(&target).score()))
            .collect();

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

fn str_is_lowercase(str: &str) -> bool {
    str.chars().all(|c| c.is_lowercase())
}

pub fn spawn_pulse_thread(audio: Arc<AudioSystem>) {
    let run = move || {
        let tick_rate = 100;

        loop {
            audio.pulse.update();

            let current = audio.pulse.current_source();
            let selected = audio.pulse.selected_source();

            if current.is_none() && selected.is_some() {
                dbg!(&current, &selected);
            }

            if let (Some(selected), None) = (selected, current) {
                let restored = audio.pulse.sources.restore(selected);

                if let Some(restored) = restored {
                    audio.set_source(restored);
                }
            }

            thread::sleep(Duration::from_millis(tick_rate));
        }
    };

    thread::Builder::new()
        .name("audio-pulse".to_string())
        .spawn(run)
        .unwrap();
}
