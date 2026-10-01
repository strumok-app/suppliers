use chrono::{Datelike, Utc};
use serde::Serialize;

/// Subset of AniList `MediaSort` values.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[allow(clippy::enum_variant_names)] // names mirror AniList's `*_DESC` values
pub enum MediaSort {
    TrendingDesc,
    PopularityDesc,
    ScoreDesc,
}

/// Subset of AniList `MediaStatus` values.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MediaStatus {
    Releasing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MediaSeason {
    Winter,
    Spring,
    Summer,
    Fall,
}

impl MediaSeason {
    /// Uses anime broadcast quarters (Jan-Mar = Winter, Apr-Jun = Spring, ...)
    /// rather than AniList's meteorological definition, as that is how
    /// seasonal charts are commonly grouped.
    pub fn from_month(month: u32) -> Self {
        match month {
            1..=3 => Self::Winter,
            4..=6 => Self::Spring,
            7..=9 => Self::Summer,
            _ => Self::Fall,
        }
    }

    pub fn current() -> (Self, i32) {
        let now = Utc::now();
        (Self::from_month(now.month()), now.year())
    }

    pub fn next(self, year: i32) -> (Self, i32) {
        match self {
            Self::Winter => (Self::Spring, year),
            Self::Spring => (Self::Summer, year),
            Self::Summer => (Self::Fall, year),
            Self::Fall => (Self::Winter, year + 1),
        }
    }
}

/// Predefined AniList `Page.media` filter. `None` fields are omitted from the
/// request variables: AniList treats an explicit `null` as "field is null"
/// rather than "no filter", which yields empty results.
#[derive(Debug, Clone, Default, Serialize)]
pub struct BrowseFilter {
    pub sort: Vec<MediaSort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<MediaStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub season: Option<MediaSeason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub season_year: Option<i32>,
}

impl BrowseFilter {
    pub fn trending() -> Self {
        Self {
            sort: vec![MediaSort::TrendingDesc, MediaSort::PopularityDesc],
            ..Default::default()
        }
    }

    pub fn popular_this_season() -> Self {
        let (season, year) = MediaSeason::current();
        Self::popular_in_season(season, year)
    }

    pub fn upcoming_next_season() -> Self {
        let (season, year) = MediaSeason::current();
        let (season, year) = season.next(year);
        Self::popular_in_season(season, year)
    }

    pub fn popular_in_season(season: MediaSeason, year: i32) -> Self {
        Self {
            sort: vec![MediaSort::PopularityDesc],
            season: Some(season),
            season_year: Some(year),
            ..Default::default()
        }
    }

    pub fn airing() -> Self {
        Self {
            sort: vec![MediaSort::PopularityDesc],
            status: Some(MediaStatus::Releasing),
            ..Default::default()
        }
    }

    pub fn all_time_popular() -> Self {
        Self {
            sort: vec![MediaSort::PopularityDesc],
            ..Default::default()
        }
    }

    pub fn top_rated() -> Self {
        Self {
            sort: vec![MediaSort::ScoreDesc],
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_map_month_to_season() {
        assert_eq!(MediaSeason::from_month(1), MediaSeason::Winter);
        assert_eq!(MediaSeason::from_month(4), MediaSeason::Spring);
        assert_eq!(MediaSeason::from_month(9), MediaSeason::Summer);
        assert_eq!(MediaSeason::from_month(12), MediaSeason::Fall);
    }

    #[test]
    fn should_roll_next_season_over_year() {
        assert_eq!(MediaSeason::Fall.next(2026), (MediaSeason::Winter, 2027));
        assert_eq!(MediaSeason::Spring.next(2026), (MediaSeason::Summer, 2026));
    }

    #[test]
    fn should_serialize_filter_as_anilist_variables() {
        let value =
            serde_json::to_value(BrowseFilter::popular_in_season(MediaSeason::Fall, 2026)).unwrap();

        assert_eq!(
            value,
            serde_json::json!({
                "sort": ["POPULARITY_DESC"],
                "season": "FALL",
                "season_year": 2026,
            })
        );
    }
}
