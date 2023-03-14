use std::sync::Mutex;

use pulsectl::controllers::{
    types::{ApplicationInfo, DeviceInfo},
    AppControl, DeviceControl, SinkController,
};

/// A friendlier interface for interacting with PulseAudio
pub struct PulseAudio {
    device: DeviceInfo,
    applications: Mutex<Vec<Application>>,
}

impl PulseAudio {
    pub fn new() -> Self {
        let mut handler = SinkController::create().unwrap();

        let device = handler
            .get_default_device()
            .expect("Could not get default device");

        Self {
            device,
            applications: Default::default(),
        }
    }

    pub fn update_applications(&self) {
        let mut handler = SinkController::create().unwrap();

        let new_applications: Vec<Application> = handler
            .list_applications()
            .expect("Couldn't list applications")
            .into_iter()
            .map(|info| info.into())
            .collect();

        let mut applications = self.applications.lock().unwrap();
        *applications = new_applications;
    }

    pub fn applications(&self) -> Vec<Application> {
        let applications = self.applications.lock().unwrap();
        (*applications).clone()
    }

    pub fn device_name(&self) -> String {
        self.device.name.clone().expect("Driver should have name")
    }
}

#[derive(Clone)]
pub struct Application {
    pub name: String,
    pub sink_input_index: u32,
}

impl From<ApplicationInfo> for Application {
    fn from(info: ApplicationInfo) -> Self {
        let full_name = info
            .proplist
            .get_str("application.name")
            .or_else(|| info.proplist.get_str("media.name"))
            .or_else(|| info.name.as_ref().map(|s| s.to_owned()))
            .unwrap_or_else(|| "Unknown application".to_string());

        Self {
            name: full_name,
            sink_input_index: info.index,
        }
    }
}
