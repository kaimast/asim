use serde::{Deserialize, Serialize};

/// The time at which the simulation started
pub const START_TIME: Time = Time(0);

/// Elapsed time in nanoseconds
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, PartialOrd, Ord, Eq)]
pub struct Time(u64);

#[derive(Debug, Default, Clone, Copy, PartialEq, PartialOrd, Ord, Eq)]
pub struct Duration(u64);

impl Time {
    pub const fn from_micros(micros: u64) -> Self {
        Self(micros)
    }

    pub const fn from_millis(millis: u64) -> Self {
        Self::from_micros(millis * 1000)
    }

    pub const fn from_seconds(seconds: u64) -> Self {
        Self::from_millis(seconds * 1000)
    }

    pub const fn from_minutes(minutes: u64) -> Self {
        Self::from_seconds(minutes * 60)
    }

    pub const fn from_hours(hours: u64) -> Self {
        Self::from_minutes(60 * hours)
    }

    /// Get elapsed hours (rounded down)
    pub fn to_hours(&self) -> u64 {
        self.to_seconds() / (60 * 60)
    }

    /// Get elapsed minutes (rounded down)
    pub fn to_minutes(self) -> u64 {
        self.to_seconds() / 60
    }

    /// Get elapsed seconds (rounded down)
    pub fn to_seconds(&self) -> u64 {
        self.0 / 1_000_000
    }

    pub fn to_millis(&self) -> u64 {
        self.0 / 1_000
    }

    pub fn as_micros(&self) -> u64 {
        self.0
    }

    pub fn as_millis_f64(&self) -> f64 {
        (self.0 as f64) / 1_000.0
    }

    pub fn as_seconds_f64(&self) -> f64 {
        (self.0 as f64) / (1_000_000.0)
    }
}

impl Duration {
    pub const ZERO: Self = Self(0);

    pub const fn from_micros(micros: u64) -> Self {
        Self(micros)
    }

    pub const fn from_millis(millis: u64) -> Self {
        Self::from_micros(millis * 1000)
    }

    pub const fn from_seconds(seconds: u64) -> Self {
        Self::from_millis(seconds * 1000)
    }

    pub const fn from_minutes(minutes: u64) -> Self {
        Self::from_seconds(minutes * 60)
    }

    pub const fn from_hours(hours: u64) -> Self {
        Self::from_minutes(60 * hours)
    }

    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }

    /// Get duration in hours (rounded down)
    pub fn to_hours(self) -> u64 {
        self.to_seconds() / (60 * 60)
    }

    /// Get duration in minutes (rounded down)
    pub fn to_minutes(self) -> u64 {
        self.to_seconds() / 60
    }

    /// Get duration in seconds (rounded down)
    pub fn to_seconds(self) -> u64 {
        self.0 / (1_000_000)
    }

    pub fn to_millis(self) -> u64 {
        self.0 / 1_000
    }

    pub fn as_micros(&self) -> u64 {
        self.0
    }

    pub fn as_millis_f64(&self) -> f64 {
        (self.0 as f64) / 1_000.0
    }

    pub fn as_seconds_f64(&self) -> f64 {
        (self.0 as f64) / (1_000_000.0)
    }
}

impl std::ops::Add<Duration> for Time {
    type Output = Self;

    fn add(self, other: Duration) -> Self {
        Self(self.0 + other.0)
    }
}

impl std::ops::AddAssign<Duration> for Duration {
    fn add_assign(&mut self, other: Duration) {
        self.0 += other.0
    }
}

impl std::ops::Add for Duration {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0)
    }
}

impl std::ops::Sub for Duration {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self(self.0 - other.0)
    }
}

impl std::ops::Sub<Self> for Time {
    type Output = Duration;

    fn sub(self, other: Self) -> Duration {
        Duration(self.0 - other.0)
    }
}

impl std::ops::Sub<Duration> for Time {
    type Output = Self;

    fn sub(self, other: Duration) -> Self {
        Self(self.0 - other.0)
    }
}

impl std::fmt::Display for Duration {
    fn fmt(&self, w: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        write!(w, "{}μs", self.0)
    }
}

impl std::fmt::Display for Time {
    fn fmt(&self, w: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        let hrs = self.to_hours();
        let minutes = self.to_minutes() - hrs * 60;
        let secs = self.to_seconds() - minutes * 60 - hrs * 60 * 60;
        let millis = self.as_millis_f64() % 1000.0;

        if hrs > 0 {
            write!(w, "{hrs:02}h ")?;
        }
        if minutes > 0 {
            write!(w, "{minutes:02}min ")?;
        }

        write!(w, "{secs:02}s {millis:.3}ms")
    }
}

#[cfg(test)]
mod tests {
    use super::{Duration, Time};

    #[test]
    fn duration_from_seconds() {
        let duration = Duration::from_seconds(2);

        assert_eq!(2, duration.to_seconds());
        assert_eq!(2_000, duration.to_millis());
        assert_eq!(2_000_000, duration.as_micros());
    }

    #[test]
    fn time_from_seconds() {
        let time = Time::from_seconds(2);

        assert_eq!(2, time.to_seconds());
        assert_eq!(2_000, time.to_millis());
        assert_eq!(2_000_000, time.as_micros());
    }
}
