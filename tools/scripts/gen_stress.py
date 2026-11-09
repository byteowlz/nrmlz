"""Generate tests/corpus/stress-de.jsonl: synthetic German inputs that exercise every rule of the Articulino
frontend (dates, times, money, percent, units, phone numbers, ordinals, fractions, ranges, e-mail/URL,
abbreviations, math symbols, LaTeX, Markdown/markup, emoji, letters), plus every abbreviation and unit key of
the data tables in context. Deterministic (fixed seed). Only synthetic text, no private data.

    uv run --no-project python tools/scripts/gen_stress.py crates/nrmlz/data/de > tests/corpus/stress-de.jsonl
"""
import json
import random
import sys
from pathlib import Path

R = random.Random(20261102)
DATA = Path(sys.argv[1] if len(sys.argv) > 1 else "crates/nrmlz/data/de")
ABBR = json.loads((DATA / "abbreviations.json").read_text(encoding="utf-8"))
UNITS = json.loads((DATA / "units.json").read_text(encoding="utf-8"))["units"]

MONTHS = "Januar Februar März April Mai Juni Juli August September Oktober November Dezember".split()
WEEKDAYS = "Montag Dienstag Mittwoch Donnerstag Freitag Samstag Sonntag".split()
WD_ABBR = "Mo Di Mi Do Fr Sa So".split()
NAMES = ["Anna", "Jonas", "Frau Schmidt", "Herr Yilmaz", "das Team", "die Firma", "unser Kunde", "Lena"]
CITIES = ["Berlin", "Nürnberg", "München", "Hamburg", "Köln", "Leipzig", "Erlangen", "Dresden"]
PREPS = ["in", "mit", "nach", "vor", "seit", "bei", "von", "aus", "zu", "für", "durch", "gegen", "ohne", "um",
         "über", "auf", "ab", "unter", "binnen", "innerhalb", "etwa", "rund", "genau", ""]
ARTICLES = ["am", "im", "vom", "zum", "zur", "beim", "dem", "den", "des", "einem", "einen", "einer", "jedem",
            "jeden", "der", "die", "das", "ins", "ein", "eine", "jede", "jeder", "jedes", "Am", "DER", "Die"]
END = [".", "!", "?", "", ".", ".", ";", ":", ",", " …"]

out = []


def add(text):
    out.append(text)


def num(lo, hi):
    return R.randint(lo, hi)


def dec(maxint=999, places=2):
    return f"{num(0, maxint)},{R.randint(0, 10 ** places - 1):0{places}d}"


def thousands(n):
    s = f"{n:,}".replace(",", ".")
    return s


def pick(xs):
    return R.choice(xs)


# --------------------------------------------------------------------------------------- dates
for _ in range(120):
    d, m, y = num(1, 31), num(1, 12), num(1890, 2099)
    art = pick(ARTICLES)
    form = R.randint(0, 9)
    if form == 0:
        add(f"{pick(NAMES)} kommt {art} {d}. {MONTHS[m - 1]} {y}{pick(END)}")
    elif form == 1:
        add(f"{pick(CITIES)}, {d}. {MONTHS[m - 1]} {y}")
    elif form == 2:
        add(f"{art} {pick(WEEKDAYS)}, {d}. {MONTHS[m - 1]} treffen wir uns{pick(END)}")
    elif form == 3:
        add(f"Vom {d}. bis {min(d + 3, 31)}. {MONTHS[m - 1]} ist geschlossen.")
    elif form == 4:
        add(f"Termine: {d}.{m}.{y}, {num(1, 28):02d}.{num(1, 12):02d}. und {num(1, 28)}.{num(1, 12)}.")
    elif form == 5:
        add(f"{art} {d}.{m:02d}. geht es los, am {pick(WD_ABBR)}, {num(1, 28):02d}.{num(1, 12):02d}.{y % 100:02d}")
    elif form == 6:
        add(f"Der {d}. {MONTHS[m - 1]}, {num(1, 28)}. {MONTHS[(m) % 12]} und {num(1, 28)}. {MONTHS[(m + 1) % 12]} sind frei.")
    elif form == 7:
        add(f"zwischen {d}. und {min(d + 2, 31)}. {MONTHS[m - 1]}, am ersten, {num(2, 9)}. und {num(10, 20)}. {MONTHS[m - 1]}")
    elif form == 8:
        add(f"Python {num(2, 3)}.{num(0, 13)}. und Version {num(1, 9)}.{num(0, 99)}.{num(0, 9)} {pick(['ab', 'seit', 'bis'])} {d}.{m}.")
    else:
        add(f"{d}.–{min(d + 4, 31)}. {MONTHS[m - 1]} {y}, {d}. oder {d + 1}. {MONTHS[m - 1]}, am dritten und {num(4, 9)}. Mai")
for d in [0, 1, 31, 32, 99]:
    for m in [0, 1, 12, 13]:
        add(f"am {d}.{m}.2026 und {d}.{m}. und den {d}.")

# --------------------------------------------------------------------------------------- ordinals
for _ in range(80):
    n = num(1, 999)
    add(f"{pick(ARTICLES)} {n}. {pick(['Platz', 'Mal', 'Stock', 'Runde', 'Geburtstag', 'Jahrhundert'])}{pick(END)}")
for art in ARTICLES:
    add(f"Er war {art} {num(1, 120)}.")
    add(f"Samstag, {art} {num(1, 31)}., von 16 Uhr")

# --------------------------------------------------------------------------------------- times, scores
for _ in range(100):
    h, mi = num(0, 26), num(0, 61)
    form = R.randint(0, 4)
    if form == 0:
        add(f"Um {h}:{mi:02d} Uhr {pick(['beginnt', 'endet'])} der Kurs{pick(END)}")
    elif form == 1:
        add(f"{h:02d}:{mi:02d}–{(h + 2) % 24:02d}:{mi:02d} geöffnet, Pause {num(1, 12)}:00")
    elif form == 2:
        add(f"Das Spiel endete {num(0, 9)}:{num(0, 9)}, Halbzeit {num(0, 5)}:{num(0, 5)}.")
    elif form == 3:
        add(f"Verhältnis {num(1, 99)}:{num(1, 99)} und {num(1, 3)}:{num(10, 59)}:{num(10, 59)}")
    else:
        add(f"ab 1 Uhr, gegen 1:{mi:02d}, um {h}.{mi:02d} Uhr und {h} Uhr")

# --------------------------------------------------------------------------------------- money
CUR = ["€", "Euro", "US-Dollar", "Dollar", "Pfund", "Franken", "$", "£", "¥", "CHF"]
for _ in range(120):
    form = R.randint(0, 6)
    c = pick(CUR)
    if form == 0:
        add(f"Das kostet {dec()} {c}{pick(END)}")
    elif form == 1:
        add(f"Preis: {pick(['$', '€', '£', '¥'])}{num(1, 9999)} bzw. {pick(['$', '€'])} {dec(99)}")
    elif form == 2:
        add(f"{thousands(num(1000, 99999999))},{num(0, 99):02d} € Umsatz und 1,00 Euro Gebühr")
    elif form == 3:
        add(f"1 € und 1,50 {c}, 2 {c}, 1,8 Mio. Euro, 3 Mrd. {c}, {num(2, 9)} Tsd. Euro")
    elif form == 4:
        add(f"Ein Abo für {num(1, 99)},{num(0, 99):02d}€ im Monat, {num(1, 9)},00 € Rabatt.")
    elif form == 5:
        add(f"{num(1, 500)} CHF, {num(1, 500)}CHF, {num(1, 99)} % Steuer, {dec(99, 1)}% Zins")
    else:
        add(f"Er zahlte {pick(['$', '€'])}{thousands(num(1000, 999999))} und {num(1, 9)}.{num(100, 999)},{num(10, 99)} {c}.")

# --------------------------------------------------------------------------------------- units (every key)
for unit, (sg, pl, g, space) in UNITS.items():
    n = pick(["1", "-1", "2", str(num(3, 999)), dec(99, 1), thousands(num(1000, 99999)), f"{num(1, 9)}-{num(10, 20)}",
              f"{num(1, 9)} – {num(10, 20)}"])
    sp = pick([" ", ""])
    prep = pick(PREPS)
    add(f"{prep} {n}{sp}{unit} {pick(['und', 'oder', 'bei'])} 1 {unit}{pick(END)}".strip())
    add(f"{prep} 1{sp}{unit}/{pick(list(UNITS))}, {n}{unit}-Teil, {num(2, 9)}{unit}x")
for prep in PREPS:
    for u in ["h", "km", "Minute", "min", "kg", "°C", "m", "l", "t"]:
        add(f"{prep} 1 {u}".strip() + pick([".", " später", "", " pro Tag"]))
for _ in range(40):
    add(f"Er ist {num(1, 2)},{num(0, 99):02d} m groß, das Brett {num(1, 3)},{num(0, 9)}0 m lang, {pick(PREPS)} 1,05 m.")
add("16A, 16 A, 4K-Monitor, 10 T-Shirts, 5G, 230 V, 5 km/h, 1 h, 50 km/h, 3 kg/m³, 9,81 m/s², 100 mbar")
add("10\u00a0–\u00a015 Tage, 3\u00a0-\u00a04 Wochen, 1990\u2009–\u20092000, 7 \u2013\u00a09 Uhr")
add("Vitamin C, Ampere A, -5 °C bis -10 °C, -1 °C, 20–25 °C, also 22 ± 3 Grad, 85 m², 1 m², 3 cm³")

# --------------------------------------------------------------------------------------- phone numbers, postcodes
for _ in range(80):
    form = R.randint(0, 5)
    if form == 0:
        add(f"Ruf an: +49 {num(150, 179)} {num(1000000, 9999999)}{pick(END)}")
    elif form == 1:
        add(f"Tel. 0{num(10, 99999)} {num(100, 999999)}, Fax 0{num(100, 999)}/{num(1000, 99999)}-{num(0, 99)}")
    elif form == 2:
        add(f"Nummer +{num(10000000, 999999999999)} oder +{num(1, 999)} {num(10, 99)} {num(100, 9999)}")
    elif form == 3:
        add(f"PLZ {num(10000, 99999)}, PLZ: {num(10000, 99999)}, die PLZ ist {num(10000, 99999)}")
    elif form == 4:
        add(f"Postleitzahl {num(10000, 99999)}, Postleitzahl ist {num(10000, 99999)}, in {num(10000, 99999)} {pick(CITIES)}")
    else:
        add(f"0{num(100, 999)}-{num(100000, 999999)} und 0176 {num(1000000, 9999999)}.5 und 0{num(10, 99)},5")

# --------------------------------------------------------------------------------------- numbers
for _ in range(150):
    form = R.randint(0, 9)
    if form == 0:
        add(f"{num(0, 10 ** num(1, 12))} Leute, {num(1100, 1999)} und {num(2000, 2099)}")
    elif form == 1:
        add(f"{thousands(num(1000, 10 ** 12))} Einwohner, {num(1, 9)}.{num(100, 999)}.{num(100, 999)}")
    elif form == 2:
        add(f"{dec(9999, R.randint(1, 4))} Prozent, {num(0, 9)},{num(0, 9)} und {num(10, 99)},{num(100, 99999)}")
    elif form == 3:
        add(f"IP 192.168.{num(0, 255)}.{num(0, 255)}, Version {num(1, 9)}.{num(0, 9)}, {num(1, 9)}.{num(0, 9)}.{num(0, 9)}")
    elif form == 4:
        add(f"{num(1, 99)}-{num(100, 999)} Stück, {num(1, 9)}–{num(10, 20)} Tage, {num(1990, 2000)} – {num(2001, 2030)}")
    elif form == 5:
        add(f"#{num(1, 999)} und # {num(1, 99)}, Nr. {num(1, 999)}, Nr.{num(1, 9)}, Ticket #{num(1000, 9999)}")
    elif form == 6:
        add(f"{num(1, 9)}/{num(2, 10)}, {num(1, 99)}/{num(1, 99)}, 24/7, 9/11, 1/2 Liter, 3/4 Stunde, 0/5")
    elif form == 7:
        add(f"0{num(0, 99999)}, 00{num(1, 9)}, 007, {num(10 ** 9, 10 ** 11)}, {'9' * num(10, 30)}")
    elif form == 8:
        add(f"A{num(1, 9)}, {num(1, 9)}D, Win{num(7, 11)}, {num(2, 9)}er-Pack, {num(10, 99)}jährig, -{num(1, 99)} Punkte")
    else:
        add(f"{num(2, 9)} x {num(2, 9)} m, {num(2, 9)}x{num(2, 9)}, {num(1, 9)} × {num(1, 9)}, 1 Million, 1 Milliarde, 1 Billion")
add("Sehr große Zahl: " + "1" + "0" * 605 + " und 1." + ".".join(["000"] * 202) + " Stück")
add("Riesig: " + "9" * 650 + "x täglich, " + "1" + ".000" * 5 + ",5")

# --------------------------------------------------------------------------------------- math
MATH = [
    "E = mc², a² + b² = c², x³, 10⁻³, H₂O, CO₂, 2^10, 2^-3, e^x, (a+b)^2",
    "√2, ∛8, π ≈ 3,14, ∞, x ≠ y, 10 ÷ 2, ∑ x, ∫ f, ∂x, Δt, α, β, γ, δ, ε, θ, λ, σ, φ, ω, Ω, ρ, τ, η, ∈, ∀, ∃",
    "5 + 3 = 8, 10 - 4 = 6, 10 – 4 = 6, x = 10 - 4, 3 + 5 - 2, Was ergibt 10 - 4? 10 - 4 ergibt 6.",
    "8.000–12.000 Euro, 10-15 Leute, 1990 – 2000, 20–25 °C, also 22 ± 3 Grad, 7 − 2",
    "C++, C#, f(x) = 2x - 1, g(t), h(abc), 3a + b, 2x täglich, 3x + 2, y = 3x, 4× so viel, 1x im Jahr",
    "12 / 4 = 3, 24/7, 3 · 4, a < b, x > 5, 3*4, 5 * 3, 2 x 3",
    "Rechne 7 - 3. Berechne 12 - 5 - 2. Subtrahiere 9 - 4! 8 - 3 ist 5. 6 - 1 sind fünf. 4 - 2 gleich 2.",
    "a - b, a – b, a-b, x - y = z, Seite 10 - 12, Kapitel 3 – 4, (3) - (4), n(n+1), 5!",
    "mc², cm², km², dm³, ab², 2 m²",
]
for m in MATH:
    add(m)
for _ in range(80):
    a, b, c = num(1, 99), num(1, 99), num(1, 999)
    op = pick(["+", "-", "–", "−", "×", "·", "/", "*", "=", "<", ">", "^"])
    sp = pick([" ", ""])
    add(f"{pick(['', 'Also ', 'Was ergibt ', 'Rechne '])}{a}{sp}{op}{sp}{b}{pick([' = ' + str(c), ' ergibt ' + str(c), '', ' Meter', ' Personen'])}{pick(END)}")

# --------------------------------------------------------------------------------------- LaTeX
LATEX = [
    r"Die Formel $a^2 + b^2 = c^2$ gilt.",
    r"$$\frac{a}{b} = \sqrt{x}$$",
    r"Lösung: $x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}$",
    r"\(\int_0^1 x^2 \, dx = \frac{1}{3}\)",
    r"\[\sum_{i=1}^{n} i = \frac{n(n+1)}{2}\]",
    r"$\lim_{x \to 0} \frac{\sin(x)}{x} = 1$",
    r"$\sqrt[3]{27} = 3$ und $\sqrt[n]{x}$ und $\sqrt[4]{16}$ und $\sqrt[³]{8}$",
    r"$f'(x) = 2x$, $g(x)$, $P(A)$, $\alpha + \beta = \gamma$, $\Omega$",
    r"$x \leq 5$, $y \geq 3$, $a \neq b$, $a \approx b$, $x \in A$, $a \cdot b$, $a \times b$, $a \div b$",
    r"$\text{Fläche} = \pi r^2$, $\mathrm{d}x$, $\mathbf{v}$, $\operatorname{sgn}(x)$",
    r"$T = 20^\circ$, $e^{i\pi} + 1 = 0$, $x_1, x_2$, $a_{n+1}$, $\infty$, $\partial f$, $\nabla f$",
    r"$\Rightarrow$ $\implies$ $\iff$ $\to$ $\rightarrow$ $\ldots$ $\cdots$ $\dots$ $\log x$ $\ln x$ $\exp(x)$",
    r"$\left( \frac{1}{2} \right)$ $\big( x \big)$ $a \quad b$ $a \qquad b$ $a\,b$ $a\;b$ $a\!b$",
    r"$\dfrac{3}{4}$, $\tfrac{1}{2}$, $\cos(x)$, $\tan(x)$, $\log(x)$, $\prod_{k=1}^{n} k$, $\mu$, $\varepsilon$",
    r"Der Preis ist $5 und $10.",
    r"Kosten $5$ und $ 5 $ und $x$ und $$",
    r"$n! = n \cdot (n-1)!$, $a < b$, $a > b$, $a/b$, $50\percent$, $\degree$",
    r"$\sqrt2$ und $\sqrt x$ und $a*b$ und $2 · 3$ und $x^{2}*y$",
]
for x in LATEX:
    add(x)

# --------------------------------------------------------------------------------------- markup
MARKUP = [
    "# Überschrift\n\nText mit **fett** und *kursiv* und __unterstrichen__ und _schräg_.",
    "## Wirtschaft:\n- erster Punkt\n- zweiter Punkt\n* dritter\n+ vierter\n• fünfter",
    "1. Erstens das\n2. Zweitens das\n3) Drittens\n10. Zehntens\n0. null",
    "25. Juli 2000 war ein Freitag.\n3. Oktober ist Feiertag.",
    "**3. Wirtschaft:** Die Lage ist gut.\n**Fazit**: passt",
    "| Name | Wert |\n|---|---|\n| A | 1 |\n| B | 2 |",
    "Vorher\n\n---\n\n***\n___\n- - -\nNachher",
    "> Ein Zitat\n> zweite Zeile\nnormal",
    "Siehe [die Doku](https://example.com/doku) und [hier]().",
    "```python\nprint('hallo')\n```\nDanach `code` im Satz.",
    "Die Datei daten.py. und main.py und test.pyc und .python",
    "selbst_bauen und mein_name_hier, __init__ und _privat_",
    "Schreib an max.mustermann@beispiel.de oder info+test@firma-x.co.uk oder a_b-c@sub.domain.com.",
    "Web: www.example.de, https://www.uni-erlangen.de/studium/master?x=1, http://foo.io/a/b.c/, example.org/pfad",
    "Domains: z.B. nicht, Version 3.2 nicht, aber shop.online, app.dev, my-site.info und test.gov.",
    "Super 😂 toll 👍🏽 und 🇩🇪 Flagge, 1️⃣ Taste, ❤️ Herz, ☀ Sonne, ⭐ Stern, 〰 Welle",
    "Es wird regnen :) Nimm einen Schirm. Toll ;-) wirklich :D xD ^^ <3 :-( :P =) :/ ¯\\_(ツ)_/¯ fertig",
    "Uhrzeit 12:30 und 1:3 bleiben, aber :) geht.",
    "Berlin → Hauptstadt, A -> B, x => y, links ← rechts, ⟶ weiter",
    "!!!\n???\n...\n— \n(Text)\nEnde:",
    "Zeile eins\nZeile zwei:\nZeile \"drei\"\nZeile (vier)\nZeile ‘fünf’\n",
    "Text mit * Sternchen * und _ Strich _ und **halb fett",
    "Liste:\n  - eingerückt\n      - tief\n\n\n\nEnde",
    "5 * 3 = 15 und 5*3 und a * b",
]
for x in MARKUP:
    add(x)
for _ in range(60):
    lines = []
    for _ in range(R.randint(2, 6)):
        kind = R.randint(0, 6)
        if kind == 0:
            lines.append("#" * R.randint(1, 6) + " " + pick(["Titel", "Ergebnis", "Zusammenfassung:"]))
        elif kind == 1:
            lines.append(pick(["-", "*", "+", "•"]) + " " + pick(["Punkt", "**Wichtig**", "am 3. Mai", "5 kg Mehl"]))
        elif kind == 2:
            lines.append(f"{num(0, 12)}{pick(['.', ')'])} {pick(['Schritt', 'Juli 2020', 'März', 'Teil'])}")
        elif kind == 3:
            lines.append(f"| {num(1, 99)} | {dec()} € | {pick(['ja', 'nein'])} |")
        elif kind == 4:
            lines.append(pick(["", "---", "> Hinweis", "```", "`x`"]))
        elif kind == 5:
            lines.append(f"Text mit _{pick(['kursiv', 'x'])}_ und **{pick(['z. B.', 'PDF', '3.'])}**")
        else:
            lines.append(pick(["Das ist gut", "Fertig:", "Ende.", "(Klammer)", "\"Zitat\""]))
    add("\n".join(lines))

# --------------------------------------------------------------------------------------- abbreviations (every key)
for k, v in list(ABBR["abbreviations"].items()) + list(ABBR["symbols"].items()):
    spaced = k.replace(" ", "") if " " in k and R.random() < 0.5 else k
    add(f"{pick(['Das ist', 'Wir haben', 'Siehe', ''])} {spaced} {pick(['das Ende', 'Er kam', 'und mehr', '3 Stück'])}{pick(END)}".strip())
    add(f"Im Text {k}")
    add(f"x{k}y und ({k}) und {k}.")
SENT = ["Er", "Sie", "Es", "Der", "Die", "Das", "Danach", "Dann", "Später", "Im", "Am", "Heute", "Jedoch",
        "Allerdings", "Hund", "zeigte", "ihn"]
for _ in range(60):
    k = pick([k for k in ABBR["abbreviations"] if k.endswith(".")])
    add(f"im {num(1, 21)}. Jh. {k} {pick(SENT)} zeigte etwas.")
for a in WD_ABBR:
    for b in WD_ABBR[:3]:
        add(f"{a}–{b} geöffnet, {a}.-{b}. 9 Uhr, {a} - {b}. 10 Uhr, {a}.–{b}. Ende")

# --------------------------------------------------------------------------------------- letters
LETTERS = [
    "PDF, KI, ADAC, DSGVO, ISBN, HTTPS, VPN, ÖPNV, USB-Stick, USA, EU, WHO, ICE, IC, DB",
    "NASA, NATO, TÜV, PIN, TAN, RAM, RAMs, PINs, JSON, WLAN, TOML, GIF, GIFs, PLZ, ASCII",
    "DAS IST GUT. BITTE SOFORT MELDEN! ACHTUNG, NICHT BERÜHREN. ICH BIN DA. ABC DEF",
    "Ludwig II, Papst Johannes XXIII, Kapitel IV, Band XI, MMXXIV",
    "Plan B, Vitamin C, Typ A, x und y, U-Bahn, E-Mail, x-mal, I am here, o.k., a.m., A.",
    "Pfad /srv/mnt/tmp, cd bin, ls, hm, hmm, pst, brr, tss, sch, ßß, Straße",
    "GmbH, iOS, macOS, CDU/CSU, SPD, FDP, AfD, ARD, ZDF, BMW, VW, HSV, SUV, FAQ, AGB, BGB, StVO",
    "Ä Ö Ü ä ö ü, ÄÖÜ, ÖBB, Ü-Ei, ä-Laut, Rolls-Royce/Snecma, Ein/Aus, km/h, und/oder",
    "Es ist OK. ok? Ok! okay, Okay. ui, nginx, NGINX, Homebrew, homebrew, C++",
    "Die Firma XYZ AG, die ABCD-Gruppe, 3D-Druck, MP3, H2O, CO2-Ausstoß, A4-Blatt",
]
for x in LETTERS:
    add(x)
for _ in range(60):
    w = "".join(R.choice("ABCDEFGHIJKLMNOPQRSTUVWXYZÄÖÜ") for _ in range(R.randint(1, 7)))
    add(f"{pick(['Das', 'DAS', 'BITTE', ''])} {w}{pick(['', 's', '-Team', '.'])} {pick(['ist', 'SOFORT', 'gut', 'NICHT'])}".strip())

# --------------------------------------------------------------------------------------- mixed realistic
for _ in range(400):
    parts = [
        f"Am {num(1, 28)}. {pick(MONTHS)} um {num(6, 22)}:{pick(['00', '15', '30', '45'])} Uhr",
        f"kostet es {dec(99)} €",
        f"ca. {num(2, 500)} {pick(['MB', 'GB', 'km', 'kg', 'min', 'h', '%'])}",
        f"z. B. {pick(['PDF', 'ZIP', 'die KI', 'ein USB-Stick'])}",
        f"ruf {pick(['mich', 'uns'])} unter 0{num(100, 9999)} {num(10000, 9999999)} an",
        f"{pick(['der', 'die', 'am', 'zum'])} {num(1, 30)}. {pick(['Platz', 'Termin', 'Versuch'])}",
        f"{num(2, 12)}x täglich",
        f"Version {num(1, 5)}.{num(0, 20)}",
        f"{thousands(num(1000, 9999999))} Nutzer",
        f"{num(10, 99)} % mehr",
        f"mail an {pick(['info', 'kontakt', 'a.b'])}@{pick(['firma', 'beispiel', 'test-shop'])}.{pick(['de', 'com', 'at', 'ch'])}",
        f"{pick(['1', '2', '3'])} h später",
        f"{pick(['Mo', 'Di', 'Fr'])}–{pick(['Do', 'Sa', 'So'])} {num(8, 10)}–{num(16, 20)} Uhr",
        f"bzw. {num(1, 9)}/{num(2, 10)}",
        f"Nr. {num(1, 99)}",
        f"{pick(['E = mc²', '3 + 4 = 7', '10 - 4', 'x²'])}",
        f"{pick(['**wichtig**', '_Hinweis_', '`code`', '😀', ':)'])}",
    ]
    R.shuffle(parts)
    n = R.randint(2, 5)
    s = pick(["", "Hinweis: ", "Also ", "- ", "1. ", "## "]) + ", ".join(parts[:n]) + pick(END)
    add(s[0].upper() + s[1:] if s else s)

# --------------------------------------------------------------------------------------- edge cases
EDGE = ["", " ", "\n", "\t", ".", "1", "-", "--", "$", "$$", "€", "%", "1.", "1,", "a", "I", "Ä", "ß", "😀",
        "\u200d", "\ufeff", "x\u0308", "Cafe\u0301", "１２３", "٣٤", "²", "½", "Ⅻ", "１２:３０ Uhr", "\u00a0 1 \u2009 kg",
        "1\u202f000 €", "A\u2011B", "line1\r\nline2\r\n", "\x1c1\x1f", "\u0001", "Tab\tTab", "  führend und folgend  ",
        "Ende mit Punkt z. B.", "z. B.", "usw.", "Er kam am 3.", "den 3.", "am 1. Uhr", "#", "# ", "**", "__", "_",
        "|", "||", "|a|", ">", "```", "````", "1)", "99.", "100. Mal", "a@b.de", "@", "www.", "http://", ".de",
        "Mo.", "Mo–", "–Fr", "3x", "x3", "x²", "²x", "^2", "2^", "√", "π", "1/", "/2", "1:2:3", "24:00", "25:00",
        "00:00 Uhr", "0,0", ",5", "5,", "1.000.", ".5", "-0", "+0", "+", "+49", "0", "00", "-1", "−1", "1−", "1 −",
        "\\(x\\)", "\\[x\\]", "\\(", "\\[", "$x", "x$", "\\frac{1}{2}", "\\alpha", "{}", "()"]
for x in EDGE:
    add(x)

seen = set()
for i, t in enumerate(out):
    if t in seen:
        continue
    seen.add(t)
    sys.stdout.write(json.dumps({"id": f"s{i:05d}", "text": t}, ensure_ascii=False) + "\n")
