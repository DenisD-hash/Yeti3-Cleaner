//! YETI³ Cleaner — Glass + Ice design system.

#[derive(Debug, Clone, Copy)]
pub struct Metrics {
    pub radius: f64,
    pub card_radius: f64,
    pub padding: f64,
    pub gap: f64,
    pub sidebar_width: f64,
    pub title_size: f64,
    pub body_size: f64,
    pub caption_size: f64,
}

impl Metrics {
    pub const fn signature(self) -> f64 {
        self.radius
            + self.card_radius
            + self.padding
            + self.gap
            + self.sidebar_width
            + self.title_size
            + self.body_size
            + self.caption_size
    }
}

pub const METRICS: Metrics = Metrics {
    radius: 18.0,
    card_radius: 13.0,
    padding: 24.0,
    gap: 14.0,
    sidebar_width: 150.0,
    title_size: 22.0,
    body_size: 13.0,
    caption_size: 11.0,
};

#[derive(Debug, Clone, Copy)]
pub struct Rgba {
    pub red: f64,
    pub green: f64,
    pub blue: f64,
    pub alpha: f64,
}

impl Rgba {
    pub const fn components(self) -> (f64, f64, f64, f64) {
        (self.red, self.green, self.blue, self.alpha)
    }
}

pub const GLASS: Rgba = Rgba {
    red: 0.025,
    green: 0.075,
    blue: 0.115,
    alpha: 0.92,
};

pub const GLASS_CARD: Rgba = Rgba {
    red: 0.035,
    green: 0.105,
    blue: 0.155,
    alpha: 0.82,
};

pub const ICE: Rgba = Rgba {
    red: 0.10,
    green: 0.72,
    blue: 1.00,
    alpha: 1.0,
};

pub const ICE_SOFT: Rgba = Rgba {
    red: 0.32,
    green: 0.82,
    blue: 1.00,
    alpha: 0.72,
};

pub const TEXT_PRIMARY: Rgba = Rgba {
    red: 0.94,
    green: 0.98,
    blue: 1.00,
    alpha: 1.0,
};

pub const TEXT_SECONDARY: Rgba = Rgba {
    red: 0.60,
    green: 0.72,
    blue: 0.80,
    alpha: 1.0,
};

pub const SUCCESS: Rgba = Rgba {
    red: 0.20,
    green: 0.95,
    blue: 0.66,
    alpha: 1.0,
};

pub const WARNING: Rgba = Rgba {
    red: 1.00,
    green: 0.70,
    blue: 0.20,
    alpha: 1.0,
};

pub const ERROR: Rgba = Rgba {
    red: 1.00,
    green: 0.28,
    blue: 0.32,
    alpha: 1.0,
};

pub fn human_bytes(value: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * KIB;
    const GIB: f64 = 1024.0 * MIB;
    const TIB: f64 = 1024.0 * GIB;

    let value = value as f64;

    if value >= TIB {
        format!("{:.2} ТБ", value / TIB)
    } else if value >= GIB {
        format!("{:.2} ГБ", value / GIB)
    } else if value >= MIB {
        format!("{:.1} МБ", value / MIB)
    } else if value >= KIB {
        format!("{:.1} КБ", value / KIB)
    } else {
        format!("{value:.0} Б")
    }
}

pub fn human_duration(ms: u64) -> String {
    let seconds = ms / 1000;

    if seconds < 60 {
        format!("{seconds} сек")
    } else {
        format!("{} мин {} сек", seconds / 60, seconds % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_is_complete() {
        assert!(METRICS.signature() > 0.0);
        assert_eq!(ICE.components().3, 1.0);
        assert_eq!(human_bytes(1024), "1.0 КБ");
        assert_eq!(human_duration(65_000), "1 мин 5 сек");
    }
}
