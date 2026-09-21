//! Transparent animated YETI³ launcher.

pub const FRAME_COUNT: usize = 24;
pub const FRAME_INTERVAL_SECONDS: f64 = 0.055;

pub fn frame_name(frame: usize) -> String {
    format!("frame-{:02}.png", frame % FRAME_COUNT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launcher_frames_are_stable() {
        assert_eq!(frame_name(0), "frame-00.png");
        assert_eq!(frame_name(23), "frame-23.png");
        assert_eq!(frame_name(24), "frame-00.png");
    }
}
