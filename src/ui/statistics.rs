//! YETI³ Cleaner statistics presentation.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Period {
    SevenDays,
    ThirtyDays,
    NinetyDays,
    AllTime,
}

impl Period {
    pub const fn title(self) -> &'static str {
        match self {
            Self::SevenDays => "7 дней",
            Self::ThirtyDays => "30 дней",
            Self::NinetyDays => "90 дней",
            Self::AllTime => "Всё время",
        }
    }

    pub const fn days(self) -> Option<u64> {
        match self {
            Self::SevenDays => Some(7),
            Self::ThirtyDays => Some(30),
            Self::NinetyDays => Some(90),
            Self::AllTime => None,
        }
    }
}

pub const PERIODS: [Period; 4] = [
    Period::SevenDays,
    Period::ThirtyDays,
    Period::NinetyDays,
    Period::AllTime,
];

pub fn normalized_bar(value: u64, maximum: u64, width: f64) -> f64 {
    if maximum == 0 {
        return 0.0;
    }

    ((value as f64 / maximum as f64) * width).clamp(0.0, width)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn periods_are_stable() {
        assert_eq!(Period::SevenDays.days(), Some(7));
        assert_eq!(Period::ThirtyDays.days(), Some(30));
        assert_eq!(Period::NinetyDays.days(), Some(90));
        assert_eq!(Period::AllTime.days(), None);
    }

    #[test]
    fn bars_are_normalized() {
        assert_eq!(normalized_bar(50, 100, 200.0), 100.0);
        assert_eq!(normalized_bar(0, 0, 200.0), 0.0);
    }
}
