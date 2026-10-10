//! Camera capture + barcode detection using the browser BarcodeDetector API.

use js_sys::{Array, Function, JsString, Object, Promise, Reflect};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{HtmlVideoElement, MediaDevices, MediaStream, MediaStreamConstraints};

const BARCODE_DETECTOR: &str = "BarcodeDetector";
const RAW_VALUE: &str = "rawValue";
const FACING_MODE: &str = "facingMode";
const ENVIRONMENT: &str = "environment";
const VIDEO_KEY: &str = "video";
const STOP_METHOD: &str = "stop";

fn window() -> web_sys::Window {
    web_sys::window().expect("no window")
}

pub fn supported() -> bool {
    let global = JsValue::from(window());
    Reflect::get(&global, &JsString::from(BARCODE_DETECTOR))
        .map(|v| v.is_function())
        .unwrap_or(false)
}

pub struct Scanner {
    stream: MediaStream,
    detector: JsValue,
}

impl Scanner {
    pub async fn start(video: &HtmlVideoElement) -> Result<Scanner, JsValue> {
        let devices: MediaDevices = window().navigator().media_devices()?;
        let constraints = Object::new().into();
        let video_opts = Object::new();
        Reflect::set(
            &video_opts,
            &JsString::from(FACING_MODE),
            &JsString::from(ENVIRONMENT),
        )?;
        Reflect::set(&constraints, &JsString::from(VIDEO_KEY), &video_opts)?;
        let media: MediaStreamConstraints = constraints.unchecked_into();
        let promise: Promise = devices.get_user_media_with_constraints(&media)?;
        let stream: MediaStream = JsFuture::from(promise).await?.into();
        video.set_src_object(Some(&stream));
        let _ = video.play();
        let ctor: Function =
            Reflect::get(&window(), &JsString::from(BARCODE_DETECTOR))?.dyn_into()?;
        let detector = Reflect::construct(&ctor, &Array::new())?;
        Ok(Scanner { stream, detector })
    }

    pub async fn read_once(&self, video: &HtmlVideoElement) -> Option<String> {
        let detect: Function = Reflect::get(&self.detector, &JsString::from("detect"))
            .ok()?
            .dyn_into()
            .ok()?;
        let promise: Promise = detect.call1(&self.detector, video).ok()?.into();
        let found = JsFuture::from(promise).await.ok()?;
        let codes: Array = found.dyn_into().ok()?;
        codes
            .iter()
            .find_map(|c| Reflect::get(&c, &JsString::from(RAW_VALUE)).ok())
            .and_then(|v| v.as_string())
    }

    pub fn stop(self) {
        for track in self.stream.get_tracks().iter() {
            if let Ok(f) = Reflect::get(&track, &JsString::from(STOP_METHOD))
                .and_then(|v| v.dyn_into::<Function>())
            {
                let _ = f.call0(&track);
            }
        }
    }
}
