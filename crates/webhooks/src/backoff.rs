use std::time::Duration;

// === Fixed backoff schedule

const BACKOFF_SCHEDULE: [Duration; 5] = [
    Duration::from_secs(60),
    Duration::from_secs(5 * 60),
    Duration::from_secs(15 * 60),
    Duration::from_secs(60 * 60),
    Duration::from_secs(6 * 60 * 60),
];

// `attempts_made` is the number of delivery attempts already recorded
// (post-increment). Returns the delay before the next attempt, or `None`
// once the schedule is exhausted, meaning the event is terminally failed.
pub fn next_backoff(attempts_made: u32) -> Option<Duration> {
    if attempts_made == 0 {
        return Some(Duration::ZERO);
    }

    let index = (attempts_made - 1) as usize;

    return BACKOFF_SCHEDULE.get(index).copied();
}

// === Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_attempt_backs_off_one_minute() {
        assert_eq!(next_backoff(1), Some(Duration::from_secs(60)));
    }

    #[test]
    fn last_scheduled_attempt_backs_off_six_hours() {
        assert_eq!(next_backoff(5), Some(Duration::from_secs(6 * 60 * 60)));
    }

    #[test]
    fn exhausted_schedule_returns_none() {
        assert_eq!(next_backoff(6), None);
    }
}
