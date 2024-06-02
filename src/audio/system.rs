use std::{
    io::{Read, Seek},
    sync::Arc,
    thread,
};

use parking_lot::Mutex;
use ringbuf::HeapRb;
use songbird::input::{reader::MediaSource, Codec, Container, Input, Reader};

use super::{
    analysis::{raw_samples_from_bytes, spawn_analysis_thread, StereoMeter},
    pulse::{PulseClient, PulseClientError, PulseClientEvent, SinkInputStream},
    source::{Source, SourceSelector},
    AudioConsumer, AudioProducer, BUFFER_SIZE, SAMPLE_IN_BYTES,
};

/// Manages all audio related stuff
pub struct AudioSystem {
    client: Arc<PulseClient>,

    selector: Arc<SourceSelector>,
    stream: Arc<Mutex<Option<SinkInputStream>>>,

    producer: AudioProducer,
    consumer: AudioConsumer,

    meter: Arc<StereoMeter>,
}

impl AudioSystem {
    pub fn new() -> Result<Arc<Self>, PulseClientError> {
        let client = Arc::new(PulseClient::new()?);
        client.subscribe_to_events();

        let selector = Arc::new(SourceSelector::new(client.clone()));

        let (audio_producer, audio_consumer) = HeapRb::new(BUFFER_SIZE).split();

        let audio = Arc::new(Self {
            client,
            selector,
            stream: Default::default(),
            meter: StereoMeter::new().into(),
            producer: Mutex::new(audio_producer).into(),
            consumer: Mutex::new(audio_consumer).into(),
        });

        spawn_analysis_thread(audio.meter.clone());
        spawn_event_thread(audio.clone());
        Ok(audio)
    }

    pub fn select(&self, source: Option<Source>) {
        self.selector.select(source);
        self.refresh_stream();
    }

    pub fn stream(&self) -> AudioStream {
        AudioStream(self.consumer.clone())
    }

    pub fn sources(&self) -> Vec<Source> {
        self.selector.sources()
    }

    pub fn current_source(&self) -> Option<Source> {
        self.selector.current_source()
    }

    pub fn selected_source(&self) -> Option<Source> {
        self.selector.selected_source()
    }

    pub fn meter_value_ranged(&self) -> (f32, f32) {
        self.meter.value_ranged()
    }

    fn refresh_stream(&self) {
        let current_source = self.selector.current_source();

        if let Some(source) = current_source {
            let stream = self
                .client
                .record(&source.sink_input())
                .expect("Creates recording stream");

            *self.stream.lock() = Some(stream);
        } else {
            *self.stream.lock() = None;
        }
    }
}

fn spawn_event_thread(audio: Arc<AudioSystem>) {
    let run = move || {
        let events = audio.client.events.clone();
        let mut producer = audio.producer.lock();

        loop {
            match events.recv().unwrap() {
                PulseClientEvent::SinkInput { index, operation } => {
                    let old_id = audio
                        .selector
                        .current_source()
                        .map(|s| s.index())
                        .unwrap_or_default();

                    audio.selector.handle_sink_input_event(index, operation);

                    let new_id = audio
                        .selector
                        .current_source()
                        .map(|s| s.index())
                        .unwrap_or_default();

                    if new_id != old_id {
                        audio.refresh_stream();
                    }
                }
                PulseClientEvent::Audio(data) => {
                    let current_source_volume = audio
                        .selector
                        .current_source()
                        .map(|s| s.volume())
                        .unwrap_or(1.);

                    let normalized_bytes = normalize_volume(&data, current_source_volume);

                    producer.push_slice(&normalized_bytes);
                    audio.meter.write(&normalized_bytes);
                }
            };
        }
    };

    thread::Builder::new()
        .name("audio-events".to_string())
        .spawn(run)
        .unwrap();
}

#[derive(Clone)]
pub struct AudioStream(AudioConsumer);

impl AudioStream {
    pub fn into_input(self) -> Input {
        // Clear the stream to minimize latency
        self.0.lock().clear();

        Input::new(
            true,
            Reader::Extension(Box::new(self)),
            Codec::FloatPcm,
            Container::Raw,
            None,
        )
    }
}

impl Read for AudioStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut consumer = self.0.lock();

        let stereo = SAMPLE_IN_BYTES * 2;
        let safe_length = buf.len() / stereo * stereo;

        consumer
            .read_exact(&mut buf[..safe_length])
            .unwrap_or_default();

        Ok(safe_length)
    }
}

impl Seek for AudioStream {
    fn seek(&mut self, _: std::io::SeekFrom) -> std::io::Result<u64> {
        unreachable!()
    }
}

impl MediaSource for AudioStream {
    fn byte_len(&self) -> Option<u64> {
        None
    }

    fn is_seekable(&self) -> bool {
        false
    }
}

fn normalize_volume(bytes: &[u8], incoming_volume: f32) -> Vec<u8> {
    let reciprocal = 1. / incoming_volume;
    let db_loudness = 10. * reciprocal.log(3.);
    let signal_factor = 10f32.powf(db_loudness / 20.);

    raw_samples_from_bytes(bytes)
        .into_iter()
        .map(|s| s * signal_factor)
        .flat_map(|s| s.to_le_bytes())
        .collect()
}
