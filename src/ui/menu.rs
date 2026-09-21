//! Premium YETI³ status-menu presentation.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayState {
    Idle,
    Cleaning,
    Paused,
    Complete,
    Error,
}

impl TrayState {
    pub const fn resource(self) -> &'static str {
        match self {
            Self::Idle => "idle.png",
            Self::Cleaning => "cleaning-00.png",
            Self::Paused => "paused.png",
            Self::Complete => "complete.png",
            Self::Error => "error.png",
        }
    }
}

pub const PRODUCT_NAME: &str = "YETI³ Cleaner";
pub const PRODUCT_TAGLINE: &str = "Deep macOS Cleanup";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_static_state_has_asset() {
        assert_eq!(TrayState::Idle.resource(), "idle.png");
        assert_eq!(TrayState::Paused.resource(), "paused.png");
        assert_eq!(TrayState::Complete.resource(), "complete.png");
        assert_eq!(TrayState::Error.resource(), "error.png");
    }
}
