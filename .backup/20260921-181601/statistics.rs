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
}
