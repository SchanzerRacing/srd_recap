#[derive(Debug, PartialEq)]
pub enum Event {
    Start {
        timestamp: i64,
        truncated: bool,
    },
    Stop {
        timestamp: i64,
        truncated: bool,
        successful: bool,
    },
}

#[derive(Default)]
pub struct Tracker {
    was_driving: Option<bool>,
    last_timestamp: Option<i64>,
}

impl Tracker {
    pub fn observe(&mut self, timestamp: i64, state: u8) -> Option<Event> {
        let event: Option<Event>;

        match (self.was_driving, state) {
            (None, 3) => {
                event = Some(Event::Start {
                    timestamp,
                    truncated: true,
                });
            }
            (Some(false), 3) => {
                event = Some(Event::Start {
                    timestamp,
                    truncated: false,
                });
            }
            (Some(true), 1) | (Some(true), 2) | (Some(true), 4) => {
                event = Some(Event::Stop {
                    timestamp,
                    truncated: false,
                    successful: false,
                });
            }
            (Some(true), 5) => {
                event = Some(Event::Stop {
                    timestamp,
                    truncated: false,
                    successful: true,
                });
            }
            _ => event = None,
        }

        self.was_driving = Some(state == 3);
        self.last_timestamp = Some(timestamp);

        event
    }

    pub fn driving(&self) -> bool {
        self.was_driving.unwrap_or(false)
    }

    pub fn finish(self) -> Option<Event> {
        let timestamp = self.last_timestamp.expect("last timestamp should exist");
        if self.was_driving.unwrap_or(false) {
            Some(Event::Stop {
                timestamp: timestamp,
                truncated: true,
                successful: false,
            })
        } else {
            None
        }
    }
}
