//! "At 5pm", "in 20 minutes", "tomorrow morning", "next Friday at 3":
//! turns the time in a reminder into a date and time, deterministically.
//! The language model never works out dates; it passes the words here.

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, Weekday};

/// A time of day as said: hour and minute, and whether "am"/"pm" was given.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Clock {
    hour: u32,
    minute: u32,
    /// `Some(true)` for pm, `Some(false)` for am, `None` when not said.
    pm: Option<bool>,
}

/// The day a phrase names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Day {
    Today,
    Tomorrow,
    /// The next such weekday after today.
    Next(Weekday),
}

/// A parsed time phrase, resolved against the current time later.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Spec {
    /// "in 20 minutes".
    After(Duration),
    /// "at 5pm", "tomorrow at 9", "on Monday", "tonight".
    On(Option<Day>, Option<Clock>),
}

const MORNING: Clock = Clock {
    hour: 9,
    minute: 0,
    pm: Some(false),
};
const AFTERNOON: Clock = Clock {
    hour: 3,
    minute: 0,
    pm: Some(true),
};
const EVENING: Clock = Clock {
    hour: 6,
    minute: 0,
    pm: Some(true),
};
const NIGHT: Clock = Clock {
    hour: 8,
    minute: 0,
    pm: Some(true),
};

fn number(word: &str) -> Option<i64> {
    let small = [
        "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
        "eleven", "twelve",
    ];
    match word {
        "a" | "an" => Some(1),
        _ => small
            .iter()
            .position(|w| *w == word)
            .map(|n| n as i64)
            .or_else(|| word.parse().ok()),
    }
}

fn weekday(word: &str) -> Option<Weekday> {
    Some(match word.trim_end_matches(['s', ',']) {
        "monday" | "mon" => Weekday::Mon,
        "tuesday" | "tue" | "tues" => Weekday::Tue,
        "wednesday" | "wed" => Weekday::Wed,
        "thursday" | "thu" | "thur" | "thurs" => Weekday::Thu,
        "friday" | "fri" => Weekday::Fri,
        "saturday" | "sat" => Weekday::Sat,
        "sunday" | "sun" => Weekday::Sun,
        _ => return None,
    })
}

/// "am"/"pm" in any of their spellings.
fn meridiem(word: &str) -> Option<bool> {
    match word.trim_end_matches(['.', ',']) {
        "am" | "a.m" => Some(false),
        "pm" | "p.m" => Some(true),
        _ => None,
    }
}

/// A clock time at the start of `words` ("5", "5pm", "5 pm", "5:30",
/// "17:30", "5.30 p.m.", "noon", "midnight", "5 o'clock"), and how many
/// words it used.
fn clock(words: &[&str]) -> Option<(Clock, usize)> {
    let first = *words.first()?;
    match first {
        "noon" | "midday" => {
            return Some((
                Clock {
                    hour: 12,
                    minute: 0,
                    pm: Some(true),
                },
                1,
            ))
        }
        "midnight" => {
            return Some((
                Clock {
                    hour: 12,
                    minute: 0,
                    pm: Some(false),
                },
                1,
            ))
        }
        _ => {}
    }
    // Split "5:30pm" into "5:30" and "pm".
    let digits_end = first
        .find(|c: char| !(c.is_ascii_digit() || c == ':' || c == '.'))
        .unwrap_or(first.len());
    let (time, suffix) = first.split_at(digits_end);
    let time = time.trim_end_matches('.');
    if time.is_empty() {
        return None;
    }
    let (hour, minute) = match time.split_once([':', '.']) {
        Some((h, m)) if m.len() == 2 => (h.parse().ok()?, m.parse().ok()?),
        Some(_) => return None,
        None => (time.parse().ok()?, 0),
    };
    let mut used = 1;
    let mut pm = meridiem(suffix);
    if !suffix.is_empty() && pm.is_none() {
        return None; // "5x" is not a time.
    }
    if pm.is_none() {
        if let Some(next) = words.get(1).and_then(|w| meridiem(w)) {
            pm = Some(next);
            used = 2;
        } else if words.get(1) == Some(&"o'clock") || words.get(1) == Some(&"oclock") {
            used = 2;
        }
    }
    let valid = match pm {
        Some(_) => (1..=12).contains(&hour),
        None => hour <= 23,
    } && minute <= 59;
    valid.then_some((Clock { hour, minute, pm }, used))
}

/// The time phrase at the start of `words`, and how many words it used.
fn phrase(words: &[&str]) -> Option<(Spec, usize)> {
    let first = *words.first()?;
    let at_clock = |rest: &[&str]| -> Option<(Clock, usize)> {
        match rest.first() {
            Some(&"at") | Some(&"@") | Some(&"around") | Some(&"by") => {
                clock(&rest[1..]).map(|(c, n)| (c, n + 1))
            }
            Some(&"morning") => Some((MORNING, 1)),
            Some(&"afternoon") => Some((AFTERNOON, 1)),
            Some(&"evening") => Some((EVENING, 1)),
            Some(&"night") => Some((NIGHT, 1)),
            _ => None,
        }
    };
    let day_then_clock = |day: Day, used: usize| {
        let (time, extra) = at_clock(&words[used..]).map_or((None, 0), |(c, n)| (Some(c), n));
        Some((Spec::On(Some(day), time), used + extra))
    };
    match first {
        "in" => {
            let (amount, unit_at) = if words.get(1) == Some(&"half") {
                // "in half an hour"
                match (words.get(2), words.get(3)) {
                    (Some(&"an"), Some(u)) | (Some(&"a"), Some(u)) if u.starts_with("hour") => {
                        return Some((Spec::After(Duration::minutes(30)), 4))
                    }
                    _ => return None,
                }
            } else {
                (number(words.get(1)?)?, 2)
            };
            if !(1..=10_000).contains(&amount) {
                return None;
            }
            let unit = words.get(unit_at)?.trim_end_matches(['.', ',']);
            let duration = match unit {
                "minute" | "minutes" | "min" | "mins" => Duration::minutes(amount),
                "hour" | "hours" | "hr" | "hrs" => Duration::hours(amount),
                "day" | "days" => Duration::days(amount),
                "week" | "weeks" => Duration::weeks(amount),
                _ => return None,
            };
            Some((Spec::After(duration), unit_at + 1))
        }
        "at" | "@" | "around" | "by" => {
            let (time, used) = clock(&words[1..])?;
            // "at 5pm tomorrow", "at 9 on Monday"
            let rest = &words[1 + used..];
            let day = match rest.first() {
                Some(&"tomorrow") => Some((Day::Tomorrow, 1)),
                Some(&"today") | Some(&"tonight") => Some((Day::Today, 1)),
                Some(&"on") | Some(&"next") | Some(&"this") => rest
                    .get(1)
                    .and_then(|w| weekday(w))
                    .map(|d| (Day::Next(d), 2)),
                Some(w) => weekday(w).map(|d| (Day::Next(d), 1)),
                None => None,
            };
            match day {
                Some((day, n)) => Some((Spec::On(Some(day), Some(time)), 1 + used + n)),
                None => Some((Spec::On(None, Some(time)), 1 + used)),
            }
        }
        "tomorrow" => day_then_clock(Day::Tomorrow, 1),
        "today" => day_then_clock(Day::Today, 1),
        "tonight" => {
            let (time, extra) = at_clock(&words[1..]).map_or((NIGHT, 0), |(c, n)| (c, n));
            Some((Spec::On(Some(Day::Today), Some(time)), 1 + extra))
        }
        "this" => {
            let (time, n) = at_clock(&words[1..])?;
            Some((Spec::On(Some(Day::Today), Some(time)), 1 + n))
        }
        "on" | "next" => {
            let day = weekday(words.get(1)?)?;
            day_then_clock(Day::Next(day), 2)
        }
        word => {
            let day = weekday(word)?;
            day_then_clock(Day::Next(day), 1)
        }
    }
}

/// `clock` on `date`, choosing am or pm when it was not said: the next time
/// it comes round for today, and a daytime guess for other days.
fn at(date: NaiveDate, time: Clock, now: NaiveDateTime, today: bool) -> Option<NaiveDateTime> {
    let hour24 = |pm: bool| match (time.hour, pm) {
        (12, false) => 0,
        (12, true) => 12,
        (h, true) => h + 12,
        (h, false) => h,
    };
    let make = |h: u32| NaiveTime::from_hms_opt(h, time.minute, 0).map(|t| date.and_time(t));
    match time.pm {
        Some(pm) => make(hour24(pm)),
        None if time.hour > 12 || time.hour == 0 => make(time.hour),
        None if today => {
            // "at 9": the next 9 o'clock today, morning first.
            let morning = make(hour24(false))?;
            if time.hour < 12 && morning > now {
                Some(morning)
            } else {
                make(hour24(true))
            }
        }
        // Another day: 1–6 means afternoon, 7–11 morning, 12 noon.
        None => make(hour24(time.hour <= 6 || time.hour == 12)),
    }
}

/// When `spec` happens, given that it is `now`.
fn resolve(spec: Spec, now: NaiveDateTime) -> Option<NaiveDateTime> {
    match spec {
        Spec::After(duration) => now.checked_add_signed(duration),
        Spec::On(day, time) => {
            let today = now.date();
            let date = match day {
                None | Some(Day::Today) => today,
                Some(Day::Tomorrow) => today.succ_opt()?,
                Some(Day::Next(weekday)) => {
                    let ahead = (7 + weekday.num_days_from_monday() as i64
                        - today.weekday().num_days_from_monday() as i64)
                        % 7;
                    today + Duration::days(if ahead == 0 { 7 } else { ahead })
                }
            };
            let time = time.unwrap_or(MORNING);
            let when = at(date, time, now, date == today)?;
            if when > now {
                Some(when)
            } else if day.is_none() {
                // "at 3pm" after 3pm: tomorrow at 3pm.
                at(today.succ_opt()?, time, now, false)
            } else {
                None // "today at 9am" said at noon has already passed.
            }
        }
    }
}

/// The result of splitting a reminder into what and when.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parsed {
    pub task: String,
    pub due: Option<NaiveDateTime>,
}

/// Why a time could not be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WhenError {
    /// The time named has already passed ("today at 9am" at noon).
    Passed,
}

fn tidy_task(words: &[&str]) -> String {
    let mut words = words.to_vec();
    while words
        .first()
        .is_some_and(|w| ["to", "that", "about", "me", "i", "please"].contains(w))
    {
        words.remove(0);
    }
    while words
        .last()
        .is_some_and(|w| ["please", "thanks", "and"].contains(w))
    {
        words.pop();
    }
    words
        .join(" ")
        .trim_end_matches(['.', '!', '?', ','])
        .to_string()
}

/// Splits "call mom at 5pm" (or "at 5pm to call mom") into the task and
/// when it is due. Without a time phrase, the whole text is the task.
pub fn split(text: &str, now: NaiveDateTime) -> Result<Parsed, WhenError> {
    let lower = text.to_lowercase();
    let words: Vec<&str> = lower.split_whitespace().collect();
    let original: Vec<&str> = text.split_whitespace().collect();
    for start in 0..words.len() {
        let Some((spec, used)) = phrase(&words[start..]) else {
            continue;
        };
        let end = start + used;
        // The phrase must end the text, or open it before "to …".
        let task_words: Vec<&str> = if end == words.len() {
            original[..start].to_vec()
        } else if start == 0 && ["to", "that", "about"].contains(&words[end]) {
            original[end..].to_vec()
        } else {
            continue;
        };
        let task = tidy_task(&task_words);
        if task.is_empty() {
            continue;
        }
        let due = resolve(spec, now).ok_or(WhenError::Passed)?;
        return Ok(Parsed {
            task,
            due: Some(due),
        });
    }
    Ok(Parsed {
        task: tidy_task(&original),
        due: None,
    })
}

/// Reads a time phrase on its own (from the language model): `None` when
/// it is not one Cubby understands.
pub fn parse_phrase(text: &str, now: NaiveDateTime) -> Option<Result<NaiveDateTime, WhenError>> {
    let lower = text.to_lowercase();
    let words: Vec<&str> = lower.split_whitespace().collect();
    match phrase(&words) {
        Some((spec, used)) if used == words.len() => {
            Some(resolve(spec, now).ok_or(WhenError::Passed))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Monday 5 October 2026, 10:00.
    fn now() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 5)
            .unwrap()
            .and_hms_opt(10, 0, 0)
            .unwrap()
    }

    fn on(day: u32, h: u32, m: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, day)
            .unwrap()
            .and_hms_opt(h, m, 0)
            .unwrap()
    }

    fn due(text: &str) -> Option<NaiveDateTime> {
        split(text, now()).unwrap().due
    }

    fn task(text: &str) -> String {
        split(text, now()).unwrap().task
    }

    #[test]
    fn reads_clock_times_in_every_common_spelling() {
        assert_eq!(due("call mom at 5pm"), Some(on(5, 17, 0)));
        assert_eq!(due("call mom at 5 pm"), Some(on(5, 17, 0)));
        assert_eq!(due("call mom at 5:30pm"), Some(on(5, 17, 30)));
        assert_eq!(due("call mom at 5.30 p.m."), Some(on(5, 17, 30)));
        assert_eq!(due("call mom at 17:45"), Some(on(5, 17, 45)));
        assert_eq!(due("call mom at noon"), Some(on(5, 12, 0)));
        assert_eq!(due("call mom at 11am"), Some(on(5, 11, 0)));
        assert_eq!(due("call mom at 3 o'clock"), Some(on(5, 15, 0)));
    }

    #[test]
    fn picks_the_next_time_it_comes_round() {
        // It is 10:00: "at 11" is this morning, "at 9" tonight, "at 9am" tomorrow.
        assert_eq!(due("water plants at 11"), Some(on(5, 11, 0)));
        assert_eq!(due("water plants at 9"), Some(on(5, 21, 0)));
        assert_eq!(due("water plants at 9am"), Some(on(6, 9, 0)));
    }

    #[test]
    fn reads_durations() {
        assert_eq!(due("check the oven in 20 minutes"), Some(on(5, 10, 20)));
        assert_eq!(due("check the oven in an hour"), Some(on(5, 11, 0)));
        assert_eq!(due("check the oven in half an hour"), Some(on(5, 10, 30)));
        assert_eq!(due("check the oven in two hours"), Some(on(5, 12, 0)));
        assert_eq!(due("renew passport in 3 days"), Some(on(8, 10, 0)));
    }

    #[test]
    fn reads_days() {
        assert_eq!(due("pay rent tomorrow"), Some(on(6, 9, 0)));
        assert_eq!(due("pay rent tomorrow at 7pm"), Some(on(6, 19, 0)));
        assert_eq!(due("pay rent tomorrow evening"), Some(on(6, 18, 0)));
        assert_eq!(
            due("pay rent tomorrow at 5"),
            Some(on(6, 17, 0)),
            "5 means afternoon"
        );
        assert_eq!(due("pay rent tonight"), Some(on(5, 20, 0)));
        assert_eq!(due("pay rent this afternoon"), Some(on(5, 15, 0)));
        // Monday 5 October: "on Monday" is next week; Friday is the 9th.
        assert_eq!(due("pay rent on monday"), Some(on(12, 9, 0)));
        assert_eq!(due("pay rent next friday at 3"), Some(on(9, 15, 0)));
        assert_eq!(due("pay rent friday at 3pm"), Some(on(9, 15, 0)));
        assert_eq!(due("pay rent at 5pm tomorrow"), Some(on(6, 17, 0)));
    }

    #[test]
    fn separates_the_task_from_the_time() {
        assert_eq!(task("to call mom at 5pm"), "call mom");
        assert_eq!(task("at 5pm to call mom"), "call mom");
        assert_eq!(task("in 10 minutes to check the oven"), "check the oven");
        assert_eq!(
            task("tomorrow to pay the electricity bill"),
            "pay the electricity bill"
        );
        assert_eq!(
            task("Call Mom at 5pm"),
            "Call Mom",
            "the person's own capitals stay"
        );
        // "at" inside the task is not a time.
        assert_eq!(due("look at the photos"), None);
        assert_eq!(task("look at the photos"), "look at the photos");
        assert_eq!(due("meet at the station at 6pm"), Some(on(5, 18, 0)));
        assert_eq!(task("meet at the station at 6pm"), "meet at the station");
    }

    #[test]
    fn a_time_that_already_passed_is_an_error_not_a_guess() {
        assert_eq!(
            split("call bank today at 9am", now()),
            Err(WhenError::Passed)
        );
        assert_eq!(
            split("call bank this morning", now()),
            Err(WhenError::Passed)
        );
    }

    #[test]
    fn rejects_nonsense_times() {
        assert_eq!(due("party at 25:00"), None);
        assert_eq!(due("party at 13pm"), None);
        assert_eq!(due("party in 0 minutes"), None);
        assert_eq!(due("party in some minutes"), None);
        assert_eq!(due("buy milk"), None);
    }

    #[test]
    fn reads_a_phrase_on_its_own() {
        assert_eq!(
            parse_phrase("tomorrow at 9am", now()),
            Some(Ok(on(6, 9, 0)))
        );
        assert_eq!(parse_phrase("in 5 minutes", now()), Some(Ok(on(5, 10, 5))));
        assert_eq!(
            parse_phrase("today at 8am", now()),
            Some(Err(WhenError::Passed))
        );
        assert_eq!(parse_phrase("whenever", now()), None);
        assert_eq!(parse_phrase("at 5pm and also some words", now()), None);
    }
}
