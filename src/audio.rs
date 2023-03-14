use std::io::{BufRead, BufReader, Read, Seek};
use std::process::{Child, ChildStderr, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use songbird::input::reader::MediaSource;
use songbird::input::{Codec, Container, Input, Reader};

use crate::pulse::{Application, PulseAudio};

/// Keeps track of the selected application and provides a reader to discord
pub struct AudioSystem {
    selected_app: Mutex<Option<Application>>,
    pulse: Arc<PulseAudio>,
    stream: AudioStream,
}

impl AudioSystem {
    pub fn new(pulse: Arc<PulseAudio>) -> Self {
        Self {
            pulse,
            selected_app: Default::default(),
            stream: Default::default(),
        }
    }

    pub fn set_application(&self, app: Application) {
        {
            let mut selected_app = self.selected_app.lock().unwrap();
            *selected_app = Some(app.clone());
        }

        self.stream.respawn(self.pulse.device_name(), app)
    }

    pub fn stream(&self) -> AudioStream {
        self.stream.clone()
    }
}

/// Provides the raw audio of a parec stream
#[derive(Clone)]
pub struct AudioStream {
    /// The current parec stream, if any
    parec: Arc<Mutex<Option<Parec>>>,
}

impl AudioStream {
    pub fn respawn(&self, device: String, app: Application) {
        let new_parec = Parec::new(device, app);

        let mut parec = self.parec.lock().unwrap();
        *parec = Some(new_parec);
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

impl Default for AudioStream {
    fn default() -> Self {
        Self {
            parec: Arc::new(Mutex::new(None)),
        }
    }
}

impl Read for AudioStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut lock = self.parec.lock().unwrap();
        let parec = (*lock).as_mut();

        match parec {
            Some(parec) => parec.stdout.read(buf),
            None => {
                buf.fill(0);
                Ok(buf.len())
            }
        }
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

struct Parec {
    child: Mutex<Child>,
    stdout: ChildStdout,
    stderr: Option<ChildStderr>,
}

impl Parec {
    fn new(device: String, app: Application) -> Self {
        let mut child = Command::new("parec")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .arg("--verbose")
            .arg("--device")
            .arg(device)
            .arg("--monitor-stream")
            .arg(app.sink_input_index.to_string())
            .arg("--format=float32le")
            .arg("--rate=48000")
            .arg("--channels=2")
            .arg("--latency=1")
            .arg("--process-time=1")
            .spawn()
            .expect("Could not spawn parec instance");

        let stdout = child.stdout.take().expect("Take stdout from child");
        let stderr = child.stderr.take().expect("Take stderr from child");

        Self {
            child: child.into(),
            stderr: Some(stderr),
            stdout,
        }
    }
}

// We must implement this otherwise when a Parec stream is dropped, the child will continue to live
impl Drop for Parec {
    fn drop(&mut self) {
        self.child.lock().unwrap().kill().expect("Child killed");
    }
}

pub fn run_check_thread(audio: Arc<AudioSystem>) {
    thread::spawn(move || loop {
        let parec_stderr = {
            let mut parec = audio.stream.parec.lock().unwrap();

            (*parec).as_mut().and_then(|parec| parec.stderr.take())
        };

        match parec_stderr {
            Some(stderr) => {
                // Check for stream moved message
                let mut reader = BufReader::new(stderr);

                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).expect("Read line");

                    println!("{}", &line);

                    if line.contains(STREAM_MOVED_MESSAGE) {
                        println!("AAAAAAAAAAAAAAAAA");
                        break;
                    }
                }
            }
            None => {
                thread::sleep(Duration::from_millis(100));
            }
        };
    });
}

const STREAM_MOVED_MESSAGE: &str = "Stream moved to";
