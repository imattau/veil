#[derive(Debug, Clone, Copy)]
pub struct LossScenario {
    pub loss_rate_percent: u8,
    pub max_latency_ms: u32,
}

pub const PRACTICAL_BASELINE: LossScenario = LossScenario {
    loss_rate_percent: 10,
    max_latency_ms: 250,
};

pub fn practical_baseline() -> LossScenario {
    PRACTICAL_BASELINE
}

#[cfg(test)]
mod tests {
    use super::practical_baseline;

    #[test]
    fn practical_baseline_is_reasonable() {
        let baseline = practical_baseline();
        assert_eq!(baseline.loss_rate_percent, 10);
        assert_eq!(baseline.max_latency_ms, 250);
    }
}
