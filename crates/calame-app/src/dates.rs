//! Dates lisibles, en français et à l'heure locale : en-tête de la bibliothèque, « Modifié il y a
//! 12 min », légendes des couvertures.

use chrono::{DateTime, Datelike, Local, NaiveDateTime, TimeZone, Timelike};

const WEEKDAYS: [&str; 7] = [
    "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche",
];
const MONTHS: [&str; 12] = [
    "janvier",
    "février",
    "mars",
    "avril",
    "mai",
    "juin",
    "juillet",
    "août",
    "septembre",
    "octobre",
    "novembre",
    "décembre",
];
const MONTHS_SHORT: [&str; 12] = [
    "janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.",
    "déc.",
];

pub fn now() -> NaiveDateTime {
    Local::now().naive_local()
}

/// Date locale d'un horodatage Unix (secondes).
pub fn local(unix: i64) -> Option<NaiveDateTime> {
    Local
        .timestamp_opt(unix, 0)
        .single()
        .map(|d: DateTime<Local>| d.naive_local())
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().collect::<String>() + chars.as_str())
        .unwrap_or_default()
}

/// « Dimanche 1 novembre ».
pub fn long_day(date: NaiveDateTime) -> String {
    let weekday = WEEKDAYS[date.weekday().num_days_from_monday() as usize];
    let month = MONTHS[date.month0() as usize];
    capitalize(&format!("{weekday} {} {month}", date.day()))
}

/// « Bonjour. » de 5 h à 18 h, « Bonsoir. » sinon.
pub fn greeting(date: NaiveDateTime) -> &'static str {
    if (5..18).contains(&date.hour()) {
        "Bonjour."
    } else {
        "Bonsoir."
    }
}

/// Moment relatif : « à l'instant », « il y a 12 min », « il y a 3 h », « hier », « vendredi »,
/// « 26 oct. », « 26 oct. 2025 ».
pub fn relative(now: NaiveDateTime, then: NaiveDateTime) -> String {
    let seconds = (now - then).num_seconds().max(0);
    let days = (now.date() - then.date()).num_days();
    if seconds < 60 {
        "à l'instant".into()
    } else if seconds < 3_600 {
        format!("il y a {} min", seconds / 60)
    } else if days == 0 {
        format!("il y a {} h", seconds / 3_600)
    } else if days == 1 {
        "hier".into()
    } else if days < 7 {
        WEEKDAYS[then.weekday().num_days_from_monday() as usize].into()
    } else if then.year() == now.year() {
        format!("{} {}", then.day(), MONTHS_SHORT[then.month0() as usize])
    } else {
        format!(
            "{} {} {}",
            then.day(),
            MONTHS_SHORT[then.month0() as usize],
            then.year()
        )
    }
}

/// « Note du 1 nov. à 20 h 41 » : nom d'une note rapide.
pub fn note_title(date: NaiveDateTime) -> String {
    format!(
        "Note du {} {} à {} h {:02}",
        date.day(),
        MONTHS_SHORT[date.month0() as usize],
        date.hour(),
        date.minute()
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use chrono::NaiveDate;

    fn at(y: i32, m: u32, d: u32, h: u32, min: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(y, m, d)
            .and_then(|date| date.and_hms_opt(h, min, 0))
            .expect("date valide")
    }

    #[test]
    fn long_day_and_greeting() {
        let date = at(2026, 11, 1, 20, 38);
        assert_eq!(long_day(date), "Dimanche 1 novembre");
        assert_eq!(greeting(date), "Bonsoir.");
        assert_eq!(greeting(at(2026, 11, 2, 9, 0)), "Bonjour.");
        assert_eq!(greeting(at(2026, 11, 2, 3, 0)), "Bonsoir.");
    }

    #[test]
    fn relative_moments() {
        let now = at(2026, 11, 1, 20, 38);
        assert_eq!(relative(now, at(2026, 11, 1, 20, 38)), "à l'instant");
        assert_eq!(relative(now, at(2026, 11, 1, 20, 26)), "il y a 12 min");
        assert_eq!(relative(now, at(2026, 11, 1, 15, 0)), "il y a 5 h");
        assert_eq!(relative(now, at(2026, 10, 31, 23, 0)), "hier");
        assert_eq!(relative(now, at(2026, 10, 30, 9, 0)), "vendredi");
        assert_eq!(relative(now, at(2026, 10, 20, 9, 0)), "20 oct.");
        assert_eq!(relative(now, at(2025, 12, 24, 9, 0)), "24 déc. 2025");
    }

    #[test]
    fn quick_note_titles() {
        assert_eq!(
            note_title(at(2026, 11, 1, 20, 5)),
            "Note du 1 nov. à 20 h 05"
        );
    }
}
