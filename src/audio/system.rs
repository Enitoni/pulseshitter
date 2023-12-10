use std::{
    io::{Read, Seek},
    sync::Arc,
    thread,
    time::Duration,
};

use parking_lot::Mutex;
use ringbuf::{HeapConsumer, HeapProducer, HeapRb};
use songbird::input::{reader::MediaSource, Codec, Container, Input, Reader};

use super::{
    analysis::StereoMeter,
    pulse::{PulseClient, PulseClientError, SinkInputStream, SinkInputStreamStatus},
    source::{Source, SourceSelector},
    BUFFER_SIZE, SAMPLE_IN_BYTES, SAMPLE_RATE,
};

type AudioProducer = Arc<Mutex<HeapProducer<u8>>>;
type AudioConsumer = Arc<Mutex<HeapConsumer<u8>>>;

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

        spawn_audio_thread(audio.clone());
        spawn_event_thread(audio.clone());
        Ok(audio)
    }

    pub fn status(&self) -> SinkInputStreamStatus {
        self.stream
            .lock()
            .as_ref()
            .map(|x| x.status())
            .unwrap_or_default()
    }

    pub fn select(&self, source: Option<Source>) {
        self.selector.select(source);
        self.refresh_stream();
    }

    pub fn stream(&self) -> AudioStream {
        AudioStream(self.consumer.clone())
    }

    fn refresh_stream(&self) {
        let current_source = self.selector.current_source();

        if let Some(source) = current_source {
            let stream = self
                .client
                .record(&source.sink_input())
                .expect("Creates recording stream");

            *self.stream.lock() = Some(stream);
        }
    }
}

fn spawn_audio_thread(audio: Arc<AudioSystem>) {
    let run = move || {
        let producer = audio.producer.clone();
        let stream = audio.stream.clone();

        let time_to_wait = 1. / SAMPLE_RATE as f32;

        loop {
            if let Some(stream) = &mut *stream.lock() {
                let mut buf = [0; BUFFER_SIZE];

                let bytes_read = stream.read(&mut buf).unwrap_or_default();

                producer.lock().push_slice(&buf);
                audio.meter.write(&buf);
            }

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

        loop {
            let x = events.recv().unwrap();
            audio.selector.handle_sink_input_event(x);
            audio.refresh_stream();
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

        consumer.read(&mut buf[..safe_length]).unwrap_or_default();

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
