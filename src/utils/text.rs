use std::sync::OnceLock;

use regex::Regex;

pub fn extract_digits(text: &str) -> u32 {
    let mut acc: u32 = 0;

    for ch in text.chars() {
        if let Some(digit) = ch.to_digit(10) {
            acc = acc * 10 + digit;
        }
    }

    acc
}

pub fn extract_file_property(script: &str) -> Option<&str> {
    static FILE_PROPERTY_RE: OnceLock<Regex> = OnceLock::new();
    FILE_PROPERTY_RE
        .get_or_init(|| Regex::new(r#"file:\s?['"](?<file>[^"]+)['"]"#).unwrap())
        .captures(script)
        .and_then(|m| Some(m.name("file")?.as_str()))
}

pub fn to_full_url(url: &str) -> String {
    if url.starts_with("//") {
        format!("https:{url}")
    } else {
        url.into()
    }
}

pub fn sanitize_text(text: &str) -> String {
    static SANITIZE_TEXT_REGEXP: OnceLock<regex::Regex> = OnceLock::new();
    let re = SANITIZE_TEXT_REGEXP.get_or_init(|| Regex::new(r#"[\n\t\s]+"#).unwrap());

    re.replace_all(text, " ").into_owned().trim().into()
}

pub fn strip_html(text: &str) -> String {
    static STRIP_HTML_REGEXP: OnceLock<regex::Regex> = OnceLock::new();
    let re = STRIP_HTML_REGEXP.get_or_init(|| Regex::new(r#"<[^>]*>"#).unwrap());

    let striped = re.replace_all(text, "");

    sanitize_text(&striped)
}

pub fn to_title_case(input: &str) -> String {
    input
        .split('_')
        .map(|word| {
            word.chars()
                .next()
                .map_or(String::new(), |c| c.to_uppercase().to_string() + &word[1..])
        })
        .collect::<Vec<String>>()
        .join(" ")
}
