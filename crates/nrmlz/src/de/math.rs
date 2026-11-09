//! Formulas: LaTeX ($a^2 + b^2 = c^2$, \frac, \sqrt) early in clean_markup, Unicode and plain-text formulas
//! (E = mc², √2, 2^10, 5 + 3, 10 - 4) after the units. Port of the "mathematics" section of german.py.

use std::sync::LazyLock;

use crate::pyre::{re, LazyRe, Match, Re, R};
use crate::pystr::{back_chars, fwd_chars, is_decimal_str, is_digit, strip};

use super::numbers::{cardinal_str, numbers};
use super::units::TABLE as UNIT_TABLE;
use super::{Fix, Profile};

fn greek(name: &str) -> Option<&'static str> {
    Some(match name {
        "alpha" => "Alpha",
        "beta" => "Beta",
        "gamma" => "Gamma",
        "delta" => "Delta",
        "epsilon" | "varepsilon" => "Epsilon",
        "zeta" => "Zeta",
        "eta" => "Eta",
        "theta" | "vartheta" => "Theta",
        "iota" => "Iota",
        "kappa" => "Kappa",
        "lambda" => "Lambda",
        "mu" => "My",
        "nu" => "Ny",
        "xi" => "Xi",
        "pi" => "Pi",
        "rho" => "Rho",
        "sigma" => "Sigma",
        "tau" => "Tau",
        "upsilon" => "Ypsilon",
        "phi" | "varphi" => "Phi",
        "chi" => "Chi",
        "psi" => "Psi",
        "omega" => "Omega",
        _ => return None,
    })
}

fn latex_command(name: &str) -> Option<&'static str> {
    Some(match name {
        "cdot" | "times" => " mal ",
        "div" => " geteilt durch ",
        "pm" => " plus minus ",
        "mp" => " minus plus ",
        "approx" => " ungefähr ",
        "neq" | "ne" => " ungleich ",
        "leq" | "le" => " kleiner gleich ",
        "geq" | "ge" => " größer gleich ",
        "lt" => " kleiner als ",
        "gt" => " größer als ",
        "infty" => " unendlich ",
        "to" | "rightarrow" => " gegen ",
        "Rightarrow" | "implies" => ", daraus folgt, ",
        "iff" => " genau dann, wenn ",
        "in" => " Element von ",
        "sum" => " Summe ",
        "prod" => " Produkt ",
        "int" => " Integral ",
        "lim" => " Limes ",
        "sin" => " Sinus ",
        "cos" => " Kosinus ",
        "tan" => " Tangens ",
        "log" => " Logarithmus ",
        "ln" => " natürlicher Logarithmus ",
        "exp" => " e hoch ",
        "partial" => " partiell ",
        "nabla" => " Nabla ",
        "cdots" | "ldots" | "dots" => " und so weiter ",
        "circ" | "degree" => " Grad ",
        "percent" => " Prozent ",
        _ => return None,
    })
}

/// `str.translate(_SUPERSCRIPT)`
fn superscript(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '⁰' => '0',
            '¹' => '1',
            '²' => '2',
            '³' => '3',
            '⁴' => '4',
            '⁵' => '5',
            '⁶' => '6',
            '⁷' => '7',
            '⁸' => '8',
            '⁹' => '9',
            '⁻' => '-',
            '⁺' => '+',
            c => c,
        })
        .collect()
}

/// `str.translate(_SUBSCRIPT)`
fn subscript(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '₀'..='₉' => char::from(b'0' + (c as u32 - '₀' as u32) as u8),
            c => c,
        })
        .collect()
}

static MATH_SYMBOLS: &[(&str, &str)] = &[
    ("√", " Wurzel aus "),
    ("∛", " dritte Wurzel aus "),
    ("π", " Pi "),
    ("∞", " unendlich "),
    ("≠", " ungleich "),
    ("÷", " geteilt durch "),
    ("∑", " Summe "),
    ("∫", " Integral "),
    ("∂", " partiell "),
    ("∆", " Delta "),
    ("Δ", " Delta "),
    ("α", " Alpha "),
    ("β", " Beta "),
    ("γ", " Gamma "),
    ("δ", " Delta "),
    ("ε", " Epsilon "),
    ("θ", " Theta "),
    ("λ", " Lambda "),
    ("σ", " Sigma "),
    ("φ", " Phi "),
    ("ω", " Omega "),
    ("Ω", " Omega "),
    ("ρ", " Rho "),
    ("τ", " Tau "),
    ("η", " Eta "),
    ("∈", " Element von "),
    ("∀", " für alle "),
    ("∃", " es gibt "),
];

// ------------------------------------------------------------------------------------------- LaTeX

static LATEX: LazyRe = re!(r"\$\$(.+?)\$\$|\$(?=\S)([^$\n]+?)(?<=\S)\$(?!\d)|\\\((.+?)\\\)|\\\[(.+?)\\\]", "s"; needs &["$", "\\"]);
static INT_LIMITS: LazyRe = re!(r"\\int\s*_\s*\{?([^{}\s^]+)\}?\s*\^\s*\{?([^{}\s]+)\}?");
static SUM_LIMITS: LazyRe =
    re!(r"\\(sum|prod)\s*_\s*\{([^{}=]+)=([^{}]+)\}\s*\^\s*\{?([^{}\s]+)\}?");
static LIM: LazyRe = re!(r"\\lim\s*_\s*\{([^{}]+?)\s*\\to\s*([^{}]+)\}");
static FRAC: LazyRe = re!(r"\\[dt]?frac\s*\{([^{}]*)\}\s*\{([^{}]*)\}");
static COMPOUND: LazyRe = re!(r"\s|[-+=]|\\pm");
static ROOT_N: LazyRe = re!(r"\\sqrt\s*\[([^\]]*)\]\s*\{([^{}]*)\}");
static ROOT: LazyRe = re!(r"\\sqrt\s*\{([^{}]*)\}");
static TEXT_CMD: LazyRe =
    re!(r"\\(?:text|mathrm|mathbf|mathit|operatorname|textrm)\s*\{([^{}]*)\}");
static POWER_GROUP: LazyRe = re!(r"\^\s*\{([^{}]*)\}");
static INDEX_GROUP: LazyRe = re!(r"_\s*\{([^{}]*)\}");
static DEGREE: LazyRe = re!(r"\^\s*\\circ");
static ROOT_CHAR: LazyRe = re!(r"\\sqrt\s*(\w)");
static POWER_CHAR: LazyRe = re!(r"\^\s*(-?\w)");
static INDEX_CHAR: LazyRe = re!(r"_\s*(\w)");
static SIZING: LazyRe = re!(r"\\(?:left|right|big|Big|bigg|Bigg)\b");
static SPACING: LazyRe = re!(r"\\[,;:! ]|\\q?quad\b");
static COMMAND: LazyRe = re!(r"\\([A-Za-z]+)");
static PRIME: LazyRe = re!(r"([A-Za-z])'");
static FUNC_OF: LazyRe = re!(r"\b(Sinus|Kosinus|Tangens|Logarithmus)\s*\(([^()]*)\)");
static NAMED_FUNC: LazyRe = re!(r"\b([fghFGHP](?:\s+Strich)*)\s*\(([^()]*)\)");
static PRODUCT_BRACKET: LazyRe = re!(r"\b([a-zA-Z0-9])\s*\(");
static FACTORIAL: LazyRe = re!(r"(?<=\w)\s*!");
static EQUALS: LazyRe = re!(r"\s*=\s*");
static PLUS: LazyRe = re!(r"\s*\+\s*");
static MINUS_SIGN: LazyRe = re!(r"\s*[-−]\s*");
static TIMES_SIGN: LazyRe = re!(r"\s*[*·]\s*");
static SLASH: LazyRe = re!(r"\s*/\s*");
static LESS: LazyRe = re!(r"\s*<\s*");
static GREATER: LazyRe = re!(r"\s*>\s*");
static NUMBER_VARIABLE: LazyRe = re!(r"(?<=\d)(?=[A-Za-z])");
static TWO_VARIABLES: LazyRe = re!(r"\b([a-z])([a-z])\b");
static MULTI_SPACE: LazyRe = re!(r"\s{2,}");

fn latex_fraction(m: &Match) -> R<String> {
    let (num, den) = (strip(m.g(1)), strip(m.g(2)));
    // compound numerator: "..., geteilt durch 2 a" instead of "... durch 2 a"
    let compound = COMPOUND.search(num)?.is_some();
    Ok(format!(
        " ({num}){} durch ({den}) ",
        if compound { ", geteilt" } else { "" }
    ))
}

fn root(degree: &str, content: &str) -> String {
    let translated = superscript(degree);
    let g = strip(&translated);
    let word = if is_decimal_str(g) {
        match crate::pystr::small_int(g) {
            2 => "zweite".to_string(),
            3 => "dritte".to_string(),
            4 => "vierte".to_string(),
            5 => "fünfte".to_string(),
            _ => format!("{g}-te"),
        }
    } else {
        g.to_string()
    };
    format!(" {word} Wurzel aus ({content}) ")
}

pub(crate) fn latex_to_text(s: &str, p: Profile) -> R<String> {
    // limits belong to the operator: "Integral von 0 bis 1", not "Integral 0 hoch 1"
    let mut s = INT_LIMITS.sub(s, r" Integral von \1 bis \2 ")?;
    s = SUM_LIMITS.sub_fn(&s, |m| {
        let op = if m.g(1) == "sum" { "Summe" } else { "Produkt" };
        Ok(format!(
            " {op} von {} gleich {} bis {} über ",
            m.g(2),
            m.g(3),
            m.g(4)
        ))
    })?;
    s = LIM.sub(&s, r" Limes für \1 gegen \2 von ")?;
    loop {
        // from the inside out, nested
        let previous = s.clone();
        s = FRAC.sub_fn(&s, latex_fraction)?;
        s = ROOT_N.sub_fn(&s, |m| Ok(root(m.g(1), m.g(2))))?;
        s = ROOT.sub(&s, r" Wurzel aus (\1) ")?;
        s = TEXT_CMD.sub(&s, r" \1 ")?;
        s = POWER_GROUP.sub(&s, r" hoch (\1) ")?;
        s = INDEX_GROUP.sub(&s, r" \1 ")?;
        if s == previous {
            break;
        }
    }
    s = DEGREE.sub_lit(&s, " Grad ")?;
    s = ROOT_CHAR.sub(&s, r" Wurzel aus \1 ")?;
    s = POWER_CHAR.sub(&s, r" hoch \1 ")?;
    s = INDEX_CHAR.sub(&s, r" \1 ")?;
    s = SIZING.sub_lit(&s, " ")?;
    s = SPACING.sub_lit(&s, " ")?;
    s = COMMAND.sub_fn(&s, |m| {
        let name = m.g(1);
        Ok(match latex_command(name) {
            Some(w) => w.to_string(),
            None => match greek(&name.to_lowercase()) {
                Some(g) => format!(" {g} "),
                None => format!(" {name} "),
            },
        })
    })?;
    s = s.replace(['{', '}'], " ");
    s = PRIME.sub(&s, r"\1 Strich ")?; // f'(x)
                                       // f(x) -> f von x, Sinus von x. Other letters before a bracket are rather a product: n(n+1) is n mal (n plus 1)
    s = FUNC_OF.sub(&s, r"\1 von \2")?;
    s = NAMED_FUNC.sub(&s, r"\1 von \2")?;
    s = PRODUCT_BRACKET.sub(&s, r"\1 mal (")?;
    s = s.replace(['(', ')'], " ");
    s = FACTORIAL.sub_lit(&s, " Fakultät")?;
    s = EQUALS.sub_lit(&s, " gleich ")?;
    s = PLUS.sub_lit(&s, " plus ")?;
    s = MINUS_SIGN.sub_lit(&s, " minus ")?;
    s = TIMES_SIGN.sub_lit(&s, " mal ")?;
    s = SLASH.sub_lit(&s, " durch ")?;
    s = LESS.sub_lit(&s, " kleiner als ")?;
    s = GREATER.sub_lit(&s, " größer als ")?;
    // "2a", "4ac": number and variable spoken apart
    s = NUMBER_VARIABLE.sub_lit(&s, " ")?;
    s = TWO_VARIABLES.sub(&s, r"\1 \2")?; // ac, mc, dx -> a c, m c, d x
                                          // numbers are written out here already, otherwise expand_units would read "x hoch 2 d x" as "2 d" = two days
    let s = MULTI_SPACE.sub_lit(&numbers(&s, p)?, " ")?;
    Ok(strip(&s).to_string())
}

/// Read LaTeX formulas ($…$, $$…$$, \(…\), \[…\]) as German words.
pub fn latex(text: &str, p: Profile) -> R<String> {
    LATEX.sub_fn(text, |m| {
        let inner = (1..=4).find_map(|i| m.get(i)).unwrap_or("");
        Ok(format!(" {} ", latex_to_text(inner, p)?))
    })
}

// ------------------------------------------------------------------------------------------- plain formulas

// A dash between two operands. "8.000–12.000", "10-15", "1990 – 2000" are ranges ("bis"); in a calculation it
// is minus: "10 – 4 = 6", "x = 10 - 4", "3 + 5 - 2". The minus sign U+2212 is always minus.
static DASH: LazyRe = re!(r"(\b\d+(?:[.,]\d+)*|\b[a-zA-Z]\b|\))([ \t]*)([-–−])([ \t]*)(?=\(|\d|[a-zA-Z](?![\wÀ-ɏ]))"; needs &["-", "–", "−"]);
const FORMULA_PART: &str = r"(?:\d+(?:[.,]\d+)*|\b[a-zA-Z]\b|\b(?:plus|minus|mal|hoch|geteilt|durch)\b|[=≠≈≤≥±<>+*×·÷^()−–-])";
static FORMULA_LEFT: LazyRe = LazyLock::new(|| Re::new(&format!(r"(?:{FORMULA_PART}[ \t]*)+$")));
static FORMULA_RIGHT: LazyRe = LazyLock::new(|| Re::new(&format!(r"(?:[ \t]*{FORMULA_PART})+")));
static OPERATOR: LazyRe = re!(r"[=≠≈≤≥±<>+*×·÷^−]|\b(?:plus|mal|hoch|geteilt)\b");
static QUESTION_BEFORE: LazyRe = re!(
    r"\b(?:ergibt|ergeben|rechne|berechne|subtrahiere)[ \t]*$",
    "i"
);
static RESULT_AFTER: LazyRe =
    re!(r"[ \t]*(?:ergibt|ergeben|ist|sind|gleich)[ \t]+(?:minus[ \t]+)?\d");
static LETTER: LazyRe = re!(r"[^\W\d_]");
const SENTENCE_BOUNDARIES: [&str; 5] = [". ", "! ", "? ", "; ", "\n"];
static TIMES_COUNT: LazyRe = re!(r"(?<![\w.,])(\d+)(?:[ \t]?×|x)(?=[ \t]+[^\W\d]|[ \t]*[.,;:!?)]|[ \t]*$)"; needs &["×", "x"]);
static TIMES_AFTER: LazyRe =
    re!(r"[ \t]+(?:plus|minus|mal|gleich|hoch|durch|geteilt|kleiner|größer)\b|[ \t]*[-+=<>*/^]");
static TIMES_BEFORE: LazyRe = re!(r"(?:[=+<>]|\bgleich|\bplus|\bminus)[ \t]*$");

/// "2x täglich" is zweimal; "3x + 2" and "= 3x" are drei x (a variable).
fn times_count(m: &Match) -> R<String> {
    let after = &m.text[m.end()..fwd_chars(m.text, m.end(), 12)];
    let before = &m.text[back_chars(m.text, m.start(), 10)..m.start()];
    if TIMES_AFTER.match_at(after, 0)?.is_some() || TIMES_BEFORE.search(before)?.is_some() {
        return Ok(format!("{} x", m.g(1)));
    }
    let count = if m.g(1) == "1" {
        "ein".to_string()
    } else {
        cardinal_str(m.g(1))?
    };
    Ok(count + "mal")
}

fn is_calculation(s: &str, m: &Match) -> R<bool> {
    let start = SENTENCE_BOUNDARIES
        .iter()
        .map(|z| s[..m.start()].rfind(z).map_or(0, |i| i + 1))
        .max()
        .unwrap_or(0);
    let end = SENTENCE_BOUNDARIES
        .iter()
        .filter_map(|z| s[m.end()..].find(z).map(|i| i + m.end()))
        .min();
    let end = end.unwrap_or(s.len());
    let l0 = FORMULA_LEFT
        .search_range(s, start, m.start())?
        .map_or(m.start(), |l| l.start());
    let r1 = FORMULA_RIGHT
        .match_range(s, m.end(), end)?
        .map_or(m.end(), |r| r.end());
    let formula = format!("{}{} {}", &s[l0..m.start()], m.g(1), &s[m.end()..r1]);
    if OPERATOR.search(&formula)?.is_some() {
        return Ok(true);
    }
    if QUESTION_BEFORE.search_range(s, start, l0)?.is_some()
        && LETTER.search(&s[r1..end])?.is_none()
    {
        return Ok(true);
    }
    Ok(RESULT_AFTER.match_range(s, r1, end)?.is_some())
}

fn dash(m: &Match) -> R<String> {
    let (left, sign) = (m.g(1), m.g(3));
    let next = &m.text[m.end()..fwd_chars(m.text, m.end(), 1)];
    let numeric =
        left.chars().next().is_some_and(is_digit) && next.chars().next().is_some_and(is_digit);
    if sign == "−" || is_calculation(m.text, m)? {
        return Ok(format!("{left} minus "));
    }
    if numeric {
        return Ok(format!("{left} bis "));
    }
    if sign == "-" && !m.g(2).is_empty() && !m.g(4).is_empty() {
        return Ok(format!("{left} minus "));
    }
    Ok(m.all().to_string())
}

static TWO_LETTERS_POWER: LazyRe = re!(r"\b([a-z]{2})(?=[²³])"; needs &["²", "³"]);
static SUPERSCRIPTS: LazyRe = re!(r"(?<=[\w)])([⁰¹²³⁴⁵⁶⁷⁸⁹⁻⁺]+)"; needs &["⁰", "¹", "²", "³", "⁴", "⁵", "⁶", "⁷", "⁸", "⁹", "⁻", "⁺"]);
static SUBSCRIPTS: LazyRe =
    re!(r"[₀₁₂₃₄₅₆₇₈₉]+"; needs &["₀", "₁", "₂", "₃", "₄", "₅", "₆", "₇", "₈", "₉"]);
static CARET: LazyRe = re!(r"(?<=[\w)])\s*\^\s*(-?[\w.,]+)"; needs &["^"]);
static PLUS_OP: LazyRe = re!(r"(?<=[\w)])\s*\+\s*(?=[\w(])"; needs &["+"]);
static FUNC_CALL: LazyRe = re!(r"\b([fghFGH])\(([a-zA-Z0-9]{1,3})\)"; needs &["("]);
static NUMBER_LETTER_OP: LazyRe = re!(r"(?<![\w.,])(\d+)([a-zA-Z])(?=[ \t]*[-+=<>*/^−–])"; needs &["-", "+", "=", "<", ">", "*", "/", "^", "−", "–"]);
static TIMES_SYMBOL: LazyRe = re!(r"[ \t]*×[ \t]*"; needs &["×"]);
static DIVIDED: LazyRe = re!(r"(?<=\d)[ \t]+/[ \t]+(?=\d)"; needs &["/"]);
static DOT_OP: LazyRe = re!(r"(?<=[\w)])\s+·\s+(?=[\w(])"; needs &["·"]);
static LESS_OP: LazyRe = re!(r"(?<=[\w)])\s+<\s+(?=[\w(])"; needs &["<"]);
static GREATER_OP: LazyRe = re!(r"(?<=[\w)])\s+>\s+(?=[\w(])"; needs &[">"]);

// Fix::StandaloneUnits: a unit with a power and no number before it ("pro m²", "in kg/m³", "€/m²"). The units
// stage only reads units after a number; without this, "m²" would become "m hoch 2" below.
static POWER_UNITS: LazyLock<(Re, Re)> = LazyLock::new(|| {
    let mut all: Vec<&str> = UNIT_TABLE.keys().copied().collect();
    all.sort_by(|a, b| b.chars().count().cmp(&a.chars().count()).then(a.cmp(b)));
    let powered: Vec<&str> = all
        .iter()
        .copied()
        .filter(|k| k.contains(['²', '³']) || k.ends_with("^2") || k.ends_with("^3"))
        .collect();
    let alt = |v: &[&str]| {
        v.iter()
            .map(|k| super::units::escape_py(k))
            .collect::<Vec<_>>()
            .join("|")
    };
    let (all, powered) = (alt(&all), alt(&powered));
    (
        Re::new(&format!(
            r"(?<![\w.,/²³^])(?:({all})/)?({powered})(?![\wÀ-ɏ²³^])"
        )),
        Re::new(&format!(r"(?<=[\wÀ-ɏ)])/({powered})(?![\wÀ-ɏ²³^])")),
    )
});
static PER_BEFORE: LazyRe = re!(r"\b(?:pro|je|per)\s*$", "i");

fn standalone_units(text: &str) -> R<String> {
    if !text.contains(['²', '³', '^']) {
        return Ok(text.to_string());
    }
    let (unit, slash) = &*POWER_UNITS;
    let text = unit.sub_fn(text, |m| {
        let per = PER_BEFORE.search(m.before())?.is_some();
        let power = &UNIT_TABLE[m.g(2)];
        Ok(match m.get(1) {
            // "kg/m³": quantity per unit, the numerator in the plural ("Kilowattstunden pro Quadratmeter")
            Some(n) => {
                let n = &UNIT_TABLE[n];
                format!(
                    "{} pro {}",
                    if per { n.singular } else { n.plural },
                    power.singular
                )
            }
            None => (if per { power.singular } else { power.plural }).to_string(),
        })
    })?;
    slash.sub_fn(&text, |m| {
        Ok(format!(" pro {}", UNIT_TABLE[m.g(1)].singular))
    })
}

/// Formulas without LaTeX: a², 10⁻³, H₂O, √2, 2^10, 5 + 3, 10 - 4.
pub fn math_symbols(text: &str, p: Profile) -> R<String> {
    if text.is_empty() {
        return Ok(text.to_string());
    }
    let owned;
    let text = if p.has(Fix::StandaloneUnits) {
        owned = standalone_units(text)?;
        owned.as_str()
    } else {
        text
    };
    // "mc²" are two quantities, not a word - unless it is a unit (cm²)
    let mut text = TWO_LETTERS_POWER.sub_fn(text, |m| {
        let w = m.g(1);
        Ok(if UNIT_TABLE.contains_key(format!("{w}²").as_str()) {
            w.to_string()
        } else {
            w.chars().map(String::from).collect::<Vec<_>>().join(" ")
        })
    })?;
    text = SUPERSCRIPTS.sub_fn(&text, |m| {
        Ok(format!(
            " hoch {} ",
            superscript(m.g(1)).replace('-', "minus ")
        ))
    })?;
    text = SUBSCRIPTS.sub_fn(&text, |m| Ok(format!(" {} ", subscript(m.all()))))?;
    text = CARET.sub(&text, r" hoch \1")?;
    for (symbol, word) in MATH_SYMBOLS {
        text = text.replace(symbol, word);
    }
    // operators only between operands: "C++" or a dash in a sentence stay as they are
    text = PLUS_OP.sub_lit(&text, " plus ")?;
    // f(x) also outside LaTeX: "f von x"
    text = FUNC_CALL.sub(&text, r"\1 von \2")?;
    // "2x - 1", "3a + b": split number and variable before an operator, then minus/plus apply
    text = NUMBER_LETTER_OP.sub(&text, r"\1 \2")?;
    text = TIMES_COUNT.sub_fn(&text, times_count)?;
    text = TIMES_SYMBOL.sub_lit(&text, " mal ")?;
    text = DIVIDED.sub_lit(&text, " durch ")?; // "12 / 4"; "24/7" and "3/4" stay
    text = DASH.sub_fn(&text, dash)?;
    text = DOT_OP.sub_lit(&text, " mal ")?;
    text = LESS_OP.sub_lit(&text, " kleiner als ")?;
    GREATER_OP.sub_lit(&text, " größer als ")
}
