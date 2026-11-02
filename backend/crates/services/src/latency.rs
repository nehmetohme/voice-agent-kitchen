//! Latency histograms of the voice path: every reply's stage timings, folded into fixed
//! buckets (the ones `vak-telemetry` documents) plus a window of recent samples for
//! percentiles. In memory, per process: telemetry export arrives with M7.

use std::collections::VecDeque;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use vak_domain::StageLatencies;
use vak_pipeline::SessionObserver;

/// Bucket upper bounds in milliseconds.
pub const BUCKETS_MS: [u64; 16] = [
    10, 25, 50, 75, 100, 150, 200, 300, 400, 500, 750, 1_000, 1_500, 2_000, 3_000, 5_000,
];
/// Samples kept per stage for percentiles.
const WINDOW: usize = 1_000;

/// A stage of the voice path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Stt,
    /// The opt-out decision, beside the LLM.
    Decision,
    LlmTtft,
    TtsFirstAudio,
    MouthToEar,
}

impl Stage {
    pub const ALL: [Self; 5] = [
        Self::Stt,
        Self::Decision,
        Self::LlmTtft,
        Self::TtsFirstAudio,
        Self::MouthToEar,
    ];

    /// The stage's name in reports, e.g. `mouth_to_ear`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Stt => "stt",
            Self::Decision => "decision",
            Self::LlmTtft => "llm_ttft",
            Self::TtsFirstAudio => "tts_first_audio",
            Self::MouthToEar => "mouth_to_ear",
        }
    }

    fn of(self, latencies: &StageLatencies) -> Option<Duration> {
        match self {
            Self::Stt => latencies.stt(),
            // Reported on its own: `decision_latency`.
            Self::Decision => None,
            Self::LlmTtft => latencies.llm_ttft(),
            Self::TtsFirstAudio => latencies.tts_first_audio(),
            Self::MouthToEar => latencies.mouth_to_ear(),
        }
    }
}

#[derive(Debug, Default)]
struct Histogram {
    /// One count per bucket of [`BUCKETS_MS`], and one more for anything slower.
    counts: [u64; BUCKETS_MS.len() + 1],
    recent: VecDeque<u64>,
}

impl Histogram {
    fn record(&mut self, millis: u64) {
        let bucket = BUCKETS_MS
            .iter()
            .position(|bound| millis <= *bound)
            .unwrap_or(BUCKETS_MS.len());
        self.counts[bucket] += 1;
        if self.recent.len() == WINDOW {
            self.recent.pop_front();
        }
        self.recent.push_back(millis);
    }

    fn report(&self, stage: Stage) -> StageReport {
        let mut sorted: Vec<u64> = self.recent.iter().copied().collect();
        sorted.sort_unstable();
        let percentile =
            |p: usize| (!sorted.is_empty()).then(|| sorted[(sorted.len() - 1) * p / 100]);
        StageReport {
            stage,
            count: self.counts.iter().sum(),
            p50_ms: percentile(50),
            p95_ms: percentile(95),
            buckets: BUCKETS_MS
                .iter()
                .map(|bound| Some(*bound))
                .chain([None])
                .zip(self.counts)
                .map(|(le_ms, count)| Bucket { le_ms, count })
                .collect(),
        }
    }
}

/// One bucket: replies at or under `le_ms` (and over the previous bound); `None` is the
/// overflow bucket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bucket {
    pub le_ms: Option<u64>,
    pub count: u64,
}

/// One stage's histogram and percentiles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageReport {
    pub stage: Stage,
    /// Replies measured since the process started.
    pub count: u64,
    /// Over the last 1000 replies.
    pub p50_ms: Option<u64>,
    pub p95_ms: Option<u64>,
    pub buckets: Vec<Bucket>,
}

/// The voice path's latency so far.
#[derive(Debug, Default)]
pub struct LatencyStats {
    stages: Mutex<[Histogram; Stage::ALL.len()]>,
}

impl LatencyStats {
    #[must_use]
    pub fn report(&self) -> Vec<StageReport> {
        let stages = self.stages.lock().unwrap_or_else(PoisonError::into_inner);
        Stage::ALL
            .iter()
            .zip(stages.iter())
            .map(|(stage, histogram)| histogram.report(*stage))
            .collect()
    }
}

impl SessionObserver for LatencyStats {
    fn reply_latencies(&self, latencies: &StageLatencies) {
        let mut stages = self.stages.lock().unwrap_or_else(PoisonError::into_inner);
        for (stage, histogram) in Stage::ALL.iter().zip(stages.iter_mut()) {
            if let Some(duration) = stage.of(latencies) {
                histogram.record(whole_millis(duration));
            }
        }
    }

    fn decision_latency(&self, latency: Duration) {
        let mut stages = self.stages.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(histogram) = Stage::ALL
            .iter()
            .position(|stage| *stage == Stage::Decision)
            .and_then(|index| stages.get_mut(index))
        {
            histogram.record(whole_millis(latency));
        }
    }
}

fn whole_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn ms(millis: u64) -> Duration {
        Duration::from_millis(millis)
    }

    #[test]
    fn replies_fill_buckets_and_percentiles() {
        let stats = LatencyStats::default();
        assert!(
            stats
                .report()
                .iter()
                .all(|stage| stage.count == 0 && stage.p50_ms.is_none())
        );
        for mouth_to_ear in [800, 900, 1_000, 1_200, 9_000] {
            stats.reply_latencies(&StageLatencies::new(
                Some(ms(40)),
                Some(ms(110)),
                None,
                Some(ms(mouth_to_ear)),
            ));
        }
        let report = stats.report();
        let names: Vec<&str> = report.iter().map(|stage| stage.stage.as_str()).collect();
        assert_eq!(
            names,
            [
                "stt",
                "decision",
                "llm_ttft",
                "tts_first_audio",
                "mouth_to_ear"
            ]
        );
        let mouth = &report[4];
        assert_eq!(mouth.count, 5);
        assert_eq!(mouth.p50_ms, Some(1_000));
        assert_eq!(mouth.p95_ms, Some(1_200));
        let count_in = |le| {
            mouth
                .buckets
                .iter()
                .find(|b| b.le_ms == le)
                .map(|b| b.count)
        };
        assert_eq!(count_in(Some(1_000)), Some(3), "800, 900 and 1000");
        assert_eq!(count_in(Some(1_500)), Some(1));
        assert_eq!(count_in(None), Some(1), "9 s overflows");
        assert_eq!(report[3].count, 0, "no TTS timing in these replies");
        assert_eq!(report[1].count, 0, "replies carry no decision timing");

        stats.decision_latency(ms(180));
        stats.decision_latency(ms(2_000));
        let decision = &stats.report()[1];
        assert_eq!(decision.count, 2);
        assert_eq!(decision.p50_ms, Some(180));
        assert_eq!(report[0].buckets.len(), BUCKETS_MS.len() + 1);
    }

    #[test]
    fn the_window_keeps_the_latest_samples() {
        let mut histogram = Histogram::default();
        for _ in 0..WINDOW {
            histogram.record(5_000);
        }
        for _ in 0..WINDOW {
            histogram.record(10);
        }
        let report = histogram.report(Stage::Stt);
        assert_eq!(report.count, 2_000);
        assert_eq!(report.p95_ms, Some(10));
    }
}
