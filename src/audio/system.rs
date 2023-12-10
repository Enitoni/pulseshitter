use std::{
    io::{Read, Seek},
    sync::Arc,
    thread,
    time::Duration,
};

use parking_lot::Mutex;
use ringbuf::{consumer, HeapConsumer, HeapProducer, HeapRb};
use songbird::input::{reader::MediaSource, Codec, Container, Input, Reader};

use super::{
    analysis::{spawn_analysis_thread, StereoMeter},
    pulse::{
        PulseClient, PulseClientError, PulseClientEvent, SinkInputStream, SinkInputStreamStatus,
    },
    source::{Source, SourceSelector},
    AudioConsumer, AudioProducer, BUFFER_SIZE, LATENCY_IN_SECONDS, SAMPLE_IN_BYTES, SAMPLE_RATE,
};

pub type AudioStatus = SinkInputStreamStatus;

/// Manages all audio related stuff
pub struct AudioSystem {
    client: Arc<PulseClient>,

    selector: Arc<SourceSelector>,
    stream: Arc<Mutex<Option<SinkInputStream>>>,

    producer: AudioProducer,
    consumer: AudioConsumer,

    meter: Arc<StereoMeter>,
}

#[derive(Clone)]
pub struct AudioContext {
    pub meter: Arc<StereoMeter>,
    selector: Arc<SourceSelector>,

    // TODO: This is temporary
    stream: Arc<Mutex<Option<SinkInputStream>>>,
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
        spawn_audio_thread(audio.clone());
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

    pub fn context(&self) -> AudioContext {
        AudioContext {
            meter: self.meter.clone(),
            stream: self.stream.clone(),
            selector: self.selector.clone(),
        }
    }

    fn refresh_stream(&self) {
        let current_source = self.selector.current_source();

        if let Some(source) = current_source {
            let stream = self
                .client
                .record(&source.sink_input())
                .expect("Creates recording stream");

            dbg!("creates new stream");

            *self.stream.lock() = Some(stream);
        } else {
            *self.stream.lock() = None;
        }
    }
}

impl AudioContext {
    pub fn sources(&self) -> Vec<Source> {
        self.selector.sources()
    }

    pub fn current_source(&self) -> Option<Source> {
        self.selector.current_source()
    }

    pub fn selected_source(&self) -> Option<Source> {
        self.selector.selected_source()
    }

    pub fn status(&self) -> SinkInputStreamStatus {
        self.stream
            .lock()
            .as_ref()
            .map(|x| x.status())
            .unwrap_or_default()
    }
}

fn spawn_audio_thread(audio: Arc<AudioSystem>) {
    let run = move || {
        let producer = audio.producer.clone();
        let stream = audio.stream.clone();

        loop {
            let mut time_to_wait = LATENCY_IN_SECONDS;
            let mut buf = [0; BUFFER_SIZE];

            //if let Some(stream) = &mut *stream.lock() {
            //    let bytes_read = stream.read(&mut buf).unwrap_or_default();
            //
            //    producer.lock().push_slice(&buf);
            //}

            //audio.meter.write(&buf);
            thread::sleep(Duration::from_secs_f32(time_to_wait));
        }
    };

    thread::Builder::new()
        .name("audio".to_string())
        .spawn(run)
        .unwrap();
}

fn spawn_event_thread(audio: Arc<AudioSystem>) {
    let run = move || {
        let events = audio.client.events.clone();
        let mut producer = audio.producer.lock();

        loop {
            match events.recv().unwrap() {
                PulseClientEvent::SinkInput { index, operation } => {
                    audio.selector.handle_sink_input_event(index, operation);
                    audio.refresh_stream();
                }
                PulseClientEvent::Audio(data) => {
                    producer.push_slice(&data);
                    audio.meter.write(&data);
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
