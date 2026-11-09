//! Explicit systolic/diastolic readings precede the rational-fraction stage.
use super::{integer, units, Text};
use crate::pyre::{re, LazyRe, R};

static CUED: LazyRe = re!(
    r"\b(blood[ -]pressure|B\.?P\.?)([ \t]*(?:(?:is|was|at|of|reads?|reading|measured|recorded)[ \t]+){0,4}[:=]?[ \t]*)([0-9]{1,3})[ \t]*/[ \t]*([0-9]{1,3})(?:[ \t]*(mmHg|kPa))?(?![\w/])",
    "i"
);
static MMHG: LazyRe = re!(
    r"(?<![\w./])([0-9]{2,3})[ \t]*/[ \t]*([0-9]{2,3})[ \t]*mmHg(?!\w|/)",
    "i"
);
static MATH_CUE: LazyRe = re!(r"\b(fraction|ratio|divide|divided)\b", "i");

fn pressure(a: &str, b: &str, unit: Option<&str>) -> String {
    let mut out = format!("{} over {}", integer(a), integer(b));
    let key = unit.map(|u| {
        if u.eq_ignore_ascii_case("mmHg") {
            "mmHg"
        } else {
            "kPa"
        }
    });
    if let Some((_, plural, _)) = key.and_then(units::lookup) {
        out += &format!(" {plural}");
    }
    out
}

pub(super) fn apply(t: &mut Text) -> R<()> {
    t.apply(&CUED, |m| {
        let label = if m.g(1).replace('.', "").eq_ignore_ascii_case("BP") {
            "B P"
        } else {
            m.g(1)
        };
        let separator = m.g(2).trim_end();
        format!("{label}{separator} {}", pressure(m.g(3), m.g(4), m.get(5)))
    })?;
    t.recognize(&MMHG, |m| {
        let before = m
            .before()
            .rsplit(['.', '!', '?', ';', '\n'])
            .next()
            .unwrap_or("");
        if MATH_CUE.search(before).ok().flatten().is_some() {
            return None;
        }
        Some(pressure(m.g(1), m.g(2), Some("mmHg")))
    })
}
