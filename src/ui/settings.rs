//! Settings-screen visual grouping.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    General,
    Cleanup,
    Development,
    AiMl,
    Containers,
    Protection,
}

impl Section {
    pub const fn title(self) -> &'static str {
        match self {
            Self::General => "Общие",
            Self::Cleanup => "Очистка",
            Self::Development => "Разработка",
            Self::AiMl => "AI / ML",
            Self::Containers => "Контейнеры",
            Self::Protection => "Защита",
        }
    }
}
