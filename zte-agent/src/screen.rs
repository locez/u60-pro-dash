#[path = "../../shared/screen_preferences.rs"]
mod preferences;
use serde_json::{Value, json};
const PATH: &str = "/data/local/tmp/u60-native-screen/appearance.json";
pub fn get() -> (u16, Value) {
    match preferences::read(std::path::Path::new(PATH)) {
        Ok(data) => (200, json!({"ok":true,"data":data})),
        Err(error) => (503, json!({"ok":false,"error":error})),
    }
}
pub fn put(body: &[u8]) -> (u16, Value) {
    let patch = match serde_json::from_slice::<Value>(body) {
        Ok(v) if body.len() <= 8192 => v,
        _ => {
            return (
                400,
                json!({"ok":false,"error":"Invalid screen preferences"}),
            );
        }
    };
    match preferences::patch(std::path::Path::new(PATH), &patch) {
        Ok(data) => (200, json!({"ok":true,"data":data})),
        Err(error) => (400, json!({"ok":false,"error":error})),
    }
}
