use std::error::Error;

#[derive(Debug, Clone)]
pub struct HlsTrack {
    pub name: String,
    pub url: String,
}

/// Checks if a link is an HLS stream and extracts tracks
pub async fn parse_hls_stream(url: &str) -> Result<Vec<HlsTrack>, Box<dyn Error>> {
    // Fetch the content from the URL
    let response = reqwest::get(url).await?;
    let content = response.text().await?;

    // Check if it's an HLS playlist (starts with #EXTM3U)
    if !content.trim_start().starts_with("#EXTM3U") {
        return Err("Not a valid HLS stream".into());
    }

    // Check if it's a master playlist (contains #EXT-X-STREAM-INF)
    if content.contains("#EXT-X-STREAM-INF") {
        // Parse master playlist
        parse_master_playlist(&content, url)
    } else {
        // It's a media playlist, return as single track
        Ok(vec![HlsTrack {
            name: "Default".to_string(),
            url: url.to_string(),
        }])
    }
}

fn parse_master_playlist(content: &str, base_url: &str) -> Result<Vec<HlsTrack>, Box<dyn Error>> {
    let mut tracks = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim();

        if line.starts_with("#EXT-X-STREAM-INF") {
            // Extract track information
            let bandwidth = extract_attribute(line, "BANDWIDTH");
            let resolution = extract_attribute(line, "RESOLUTION");
            let name_attr = extract_attribute(line, "NAME");

            // Get the next non-empty, non-comment line as the URL
            i += 1;
            while i < lines.len() {
                let url_line = lines[i].trim();
                if !url_line.is_empty() && !url_line.starts_with('#') {
                    let track_url = resolve_url(base_url, url_line);

                    // Generate a descriptive name
                    let name = if let Some(n) = name_attr {
                        n
                    } else if let Some(res) = resolution {
                        if let Some(bw) = bandwidth {
                            format!("{} ({}kbps)", res, parse_bandwidth_kbps(&bw))
                        } else {
                            res
                        }
                    } else if let Some(bw) = bandwidth {
                        format!("{}kbps", parse_bandwidth_kbps(&bw))
                    } else {
                        format!("Track {}", tracks.len() + 1)
                    };

                    tracks.push(HlsTrack {
                        name,
                        url: track_url,
                    });
                    break;
                }
                i += 1;
            }
        }
        i += 1;
    }

    if tracks.is_empty() {
        return Err("No tracks found in master playlist".into());
    }

    Ok(tracks)
}

fn extract_attribute(line: &str, attr: &str) -> Option<String> {
    let search = format!("{}=", attr);
    if let Some(start) = line.find(&search) {
        let start = start + search.len();
        let rest = &line[start..];

        // Handle quoted values
        if rest.starts_with('"') {
            if let Some(end) = rest[1..].find('"') {
                return Some(rest[1..=end].to_string());
            }
        } else {
            // Handle non-quoted values (ends at comma or end of line)
            let end = rest.find(',').unwrap_or(rest.len());
            return Some(rest[..end].trim().to_string());
        }
    }
    None
}

fn parse_bandwidth_kbps(bandwidth: &str) -> String {
    if let Ok(bw) = bandwidth.parse::<u64>() {
        format!("{}", bw / 1000)
    } else {
        bandwidth.to_string()
    }
}

fn resolve_url(base_url: &str, relative_url: &str) -> String {
    if relative_url.starts_with("http://") || relative_url.starts_with("https://") {
        relative_url.to_string()
    } else {
        // Extract base path from URL
        let base = if let Some(pos) = base_url.rfind('/') {
            &base_url[..=pos]
        } else {
            base_url
        };

        if relative_url.starts_with('/') {
            // Absolute path - extract protocol and domain
            if let Some(protocol_end) = base_url.find("://") {
                if let Some(domain_end) = base_url[protocol_end + 3..].find('/') {
                    let domain = &base_url[..protocol_end + 3 + domain_end];
                    return format!("{}{}", domain, relative_url);
                }
            }
            format!("{}{}", base_url, relative_url)
        } else {
            // Relative path
            format!("{}{}", base, relative_url)
        }
    }
}

// Example usage
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let url = "https://example.com/playlist.m3u8";

    match parse_hls_stream(url).await {
        Ok(tracks) => {
            println!("Found {} track(s):", tracks.len());
            for (i, track) in tracks.iter().enumerate() {
                println!("{}. {} - {}", i + 1, track.name, track.url);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_attribute() {
        let line = r#"#EXT-X-STREAM-INF:BANDWIDTH=1280000,RESOLUTION=720x480,NAME="HD""#;
        assert_eq!(
            extract_attribute(line, "BANDWIDTH"),
            Some("1280000".to_string())
        );
        assert_eq!(
            extract_attribute(line, "RESOLUTION"),
            Some("720x480".to_string())
        );
        assert_eq!(extract_attribute(line, "NAME"), Some("HD".to_string()));
    }

    #[test]
    fn test_resolve_url() {
        let base = "https://example.com/videos/stream.m3u8";
        assert_eq!(
            resolve_url(base, "track1.m3u8"),
            "https://example.com/videos/track1.m3u8"
        );
        assert_eq!(
            resolve_url(base, "/absolute/track1.m3u8"),
            "https://example.com/absolute/track1.m3u8"
        );
        assert_eq!(
            resolve_url(base, "https://other.com/track1.m3u8"),
            "https://other.com/track1.m3u8"
        );
    }
}
