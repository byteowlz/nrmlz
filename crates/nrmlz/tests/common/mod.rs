pub fn assert_readings_in_context(
    lang: nrmlz::Lang,
    readings: &[(&str, &str)],
    contexts: &[(&str, &str)],
) {
    for (token, spoken) in readings {
        for (prefix, suffix) in contexts {
            let input = format!("{prefix}{token}{suffix}");
            assert_eq!(
                nrmlz::normalize(lang, &input),
                format!("{prefix}{spoken}{suffix}"),
                "context changed the reading: {input:?}"
            );
        }
    }
}
