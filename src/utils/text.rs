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

pub fn extract_css_background_url(css_style: &str) -> Option<String> {
    static BACKGROUND_URL_RE: OnceLock<Regex> = OnceLock::new();
    BACKGROUND_URL_RE
        .get_or_init(|| Regex::new(r#"background\s*:\s*url\(\s*['"]?(?<url>[^'")\s]+)['"]?\s*\)"#).unwrap())
        .captures(css_style)
        .and_then(|m| m.name("url").map(|u| u.as_str().to_string()))
}

pub fn extract_background_url_value(css_style: &str) -> Option<String> {
    extract_css_background_url(css_style)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_extract_background_url_with_single_quotes() {
        let css = "background: url('https://cdn.cimovix.store/cover/597c7b407a02cc0a92167e7a371eca25.webp');";
        let result = extract_css_background_url(css);
        assert_eq!(
            result,
            Some("https://cdn.cimovix.store/cover/597c7b407a02cc0a92167e7a371eca25.webp".to_string())
        );
    }

    #[test]
    fn should_extract_background_url_with_double_quotes() {
        let css = r#"background: url("https://example.com/image.jpg");"#;
        let result = extract_css_background_url(css);
        assert_eq!(
            result,
            Some("https://example.com/image.jpg".to_string())
        );
    }

    #[test]
    fn should_extract_background_url_without_quotes() {
        let css = "background: url(https://example.com/image.jpg);";
        let result = extract_css_background_url(css);
        assert_eq!(result, Some("https://example.com/image.jpg".to_string()));
    }

    #[test]
    fn should_extract_background_url_with_spaces() {
        let css = "background  :  url( 'https://example.com/image.jpg' );";
        let result = extract_css_background_url(css);
        assert_eq!(
            result,
            Some("https://example.com/image.jpg".to_string())
        );
    }

    #[test]
    fn should_return_none_for_invalid_css() {
        let css = "color: red;";
        let result = extract_css_background_url(css);
        assert_eq!(result, None);
    }
}
