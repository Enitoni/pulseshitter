use std::io::{copy, ErrorKind, Read, Seek};
use std::process::{ChildStdout, Command, Stdio};

use pulsectl::controllers::types::{ApplicationInfo, DeviceInfo};
use songbird::input::reader::MediaSource;
use songbird::input::{Codec, Container, Input, Reader};

pub struct ParecStream(ChildStdout);

impl ParecStream {
    pub fn new(device: DeviceInfo, app: ApplicationInfo) -> Self {
        let child = run_parec_stream(device, app);
        Self(child)
    }

    pub fn into_input(self) -> Input {
        Input::new(
            true,
            Reader::Extension(Box::new(self)),
            Codec::FloatPcm,
            Container::Raw,
            None,
        )
    }
}

impl MediaSource for ParecStream {
    fn byte_len(&self) -> Option<u64> {
        None
    }

    fn is_seekable(&self) -> bool {
        false
    }
}

impl Seek for ParecStream {
    fn seek(&mut self, _: std::io::SeekFrom) -> std::io::Result<u64> {
        unreachable!()
    }
}

impl Read for ParecStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

fn run_parec_stream(device: DeviceInfo, app: ApplicationInfo) -> ChildStdout {
    let mut child = Command::new("parec")
        .stdout(Stdio::piped())
        .arg("--verbose")
        .arg("--device")
        .arg(device.name.unwrap())
        .arg("--monitor-stream")
        .arg(app.index.to_string())
        .arg("--format=f32le")
        .arg("--rate=48000")
        .arg("--channels=2")
        .arg("--latency=1")
        .arg("--process-time=1")
        .spawn()
        .expect("Could not spawn parec instance");

    child.stdout.take().expect("Take stdout from child")
}
