//! Context for an unformatted long number: an identifier read digit by digit, or an amount read as a cardinal.
//!
//! Formatting wins over this decision: explicit separators, dates and phone formats are handled earlier. This
//! only covers a plain integer run, where the same digits can be a phone/record number or a quantity.
use crate::pyre::{re, LazyRe, Re};
use crate::Lang;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NumberReading {
    Digits,
    Amount,
    Default,
}

static EN_DIGIT_CUE: LazyRe = re!(
    r"(?i)(?:phone(?:[ \t]+number)?|telephone(?:[ \t]+number)?|call|dial|fax|pin|code|id|#|extension|ext\.?|reach[ \t]+me[ \t]+at|(?:account|order|tracking|customer|serial|invoice|ticket|case|reference)[ \t]+(?:number|no\.?|nr\.?|#))[ \t]*(?:(?:is|was|:|=)[ \t]*)?$"
);
static DE_DIGIT_CUE: LazyRe = re!(
    r"(?i)(?:telefon(?:nummer)?|rufnummer|handy(?:nummer)?|fax(?:nummer)?|code|passwort|kennwort|(?:kunden|bestell|rechnungs|auftrags|sendungs|vertrags|mitglieds)nummer|tracking(?:nummer)?|durchwahl|id|kennzeichen|nummer)[ \t]*(?:(?:ist|lautet|:|=)[ \t]*)?$"
);
static EN_AMOUNT_CUE: LazyRe = re!(
    r"(?i)^[ \t]*(?:dollars?|euros?|pounds?|units?|items?|pieces?|people|persons?|kg|kilograms?|percent|miles?|kilometers?|meters?|years?|%)(?!\w)"
);
static DE_AMOUNT_CUE: LazyRe = re!(
    r"(?i)^[ \t]*(?:stück|euro|eur|dollar|pfund|franken|kg|kilogramm|prozent|einwohner|menschen|personen|meter|kilometer|jahre?|monate?|%)(?!\w)"
);

fn cues(lang: Lang) -> (&'static Re, &'static Re) {
    match lang {
        Lang::En => (&EN_DIGIT_CUE, &EN_AMOUNT_CUE),
        Lang::De => (&DE_DIGIT_CUE, &DE_AMOUNT_CUE),
    }
}

/// `before` is limited to the recent clause; `after` is matched from its start at the number.
pub(crate) fn decide(lang: Lang, before: &str, after: &str, grouped: bool) -> NumberReading {
    let tail = before
        .char_indices()
        .rev()
        .nth(79)
        .map_or(before, |(i, _)| &before[i..]);
    let (digit_cue, amount_cue) = cues(lang);
    if digit_cue.is_match(tail).unwrap_or(false) {
        NumberReading::Digits
    } else if amount_cue.is_match(after).unwrap_or(false) || grouped {
        NumberReading::Amount
    } else {
        NumberReading::Default
    }
}
