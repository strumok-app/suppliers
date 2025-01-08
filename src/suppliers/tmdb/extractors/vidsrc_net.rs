use core::str;
use std::{cmp::min, sync::OnceLock};

use anyhow::{anyhow, Ok};
use base64::{prelude::BASE64_STANDARD, Engine};

use crate::{models::ContentMediaItemSource, utils};

use super::SourceParams;

const URL: &str = "https://vidsrc.xyz";
const HOST_URL: &str = "https://edgedeliverynetwork.com";

pub async fn extract(params: &SourceParams) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let tmdb_id = params.id.to_string();
    let id = params.imdb_id.as_ref().unwrap_or(&tmdb_id);

    let url = match &params.ep {
        Some(ep) => {
            let e = ep.e;
            let s = ep.s;
            format!("{URL}/embed/tv/{id}/{s}-{e}")
        }
        None => format!("{URL}/embed/movie/{id}"),
    };

    let client = utils::create_client();
    let iframe_html1 = client.get(&url).send().await?.text().await?;

    static IFRAME2_SRC_RE: OnceLock<regex::Regex> = OnceLock::new();
    let second_url = IFRAME2_SRC_RE
        .get_or_init(|| regex::Regex::new(r#"id="player_iframe" src="(?<url>[^"]+)""#).unwrap())
        .captures(&iframe_html1)
        .and_then(|m| m.name("url"))
        .map(|m| utils::to_full_url(m.as_str()))
        .ok_or_else(|| anyhow!("No second iframe found"))?;

    // tokio::time::sleep(Duration::from_millis(300)).await;

    let iframe_html2 = client
        .get(&second_url)
        .header("Referer", &url)
        .send()
        .await?
        .text()
        .await?;

    static IFRAME3_SRC_RE: OnceLock<regex::Regex> = OnceLock::new();
    let third_url = IFRAME3_SRC_RE
        .get_or_init(|| regex::Regex::new(r#"src: '(?<url>/prorcp/[^']+)'"#).unwrap())
        .captures(&iframe_html2)
        .and_then(|m| m.name("url"))
        .map(|m| {
            let path = m.as_str();
            format!("{HOST_URL}{path}")
        })
        .ok_or_else(|| anyhow!("No third iframe found"))?;

    let iframe_html3 = client
        .get(&third_url)
        .header("Referer", &second_url)
        .send()
        .await?
        .text()
        .await?;

    static PARAMS_RE: OnceLock<regex::Regex> = OnceLock::new();
    let (id, content) = PARAMS_RE
        .get_or_init(|| {
            regex::Regex::new(
                r#"<div id="(?<id>[^"]+)" style="display:none;">(?<content>[^>]+)</div>"#,
            )
            .unwrap()
        })
        .captures(&iframe_html3)
        .and_then(|m| {
            let id = m.name("id")?.as_str();
            let content = m.name("content")?.as_str();
            Some((id, content))
        })
        .ok_or_else(|| anyhow!("No params in third iframe found"))?;

    let decoder_res = match id {
        "NdonQLf1Tzyx7bMG" => decoder1(content),
        "sXnL9MQIry" => decoder2(content),
        "IhWrImMIGL" => decoder3(content),
        "xTyBxQyGTA" => decoder4(content),
        "ux8qjPHC66" => decoder5(content),
        "eSfH1IRMyL" => decoder6(content),
        "KJHidj7det" => decoder7(content),
        "o2VSUnjnZl" => decoder8(content),
        "Oi3v1dAlaM" => decoder9(content, 5),
        "TsA2KGDGux" => decoder9(content, 7),
        "JoAHUMCLXV" => decoder9(content, 3),
        _ => return Err(anyhow!("Unknow encoding method: {id}")),
    };

    let decoded =
        decoder_res.map_err(|err| anyhow!("decoder {id} failed with content: {content}: {err}"))?;

    // println!("{decoded}");

    Ok(vec![ContentMediaItemSource::Video {
        link: decoded,
        description: "VidSrc.net".into(),
        headers: None,
    }])
}

// NdonQLf1Tzyx7bMG
fn decoder1(a: &str) -> anyhow::Result<String> {
    const B: usize = 3usize;
    let mut c = String::new();
    let l = a.len();
    for (d, _) in a.char_indices().step_by(B) {
        c.push_str(&a[d..min(d + B, l)])
    }

    Ok(c)
}

// sXnL9MQIry
fn decoder2(a: &str) -> anyhow::Result<String> {
    let b = "pWB9V)[*4I`nJpp?ozyB~dbr9yt!_n4u".as_bytes();
    let shift = 3u8;

    let d = a
        .as_bytes()
        .chunks(2)
        .map(|ch| u8::from_str_radix(String::from_utf8(ch.to_vec()).unwrap().as_str(), 16).unwrap())
        .enumerate()
        .map(|(e, v)| (v ^ b[e % b.len()]) - shift)
        .collect::<Vec<_>>();

    String::from_utf8(BASE64_STANDARD.decode(d)?).map_err(|err| anyhow!(err))
}

// IhWrImMIGL
fn decoder3(a: &str) -> anyhow::Result<String> {
    let d = a
        .as_bytes()
        .iter()
        .map(|b| {
            let ch = *b;
            match char::from(ch) {
                'a'..='m' | 'A'..='M' => ch + 13,
                'n'..='z' | 'N'..='Z' => ch - 13,
                _ => ch,
            }
        })
        .collect::<Vec<_>>();

    String::from_utf8(BASE64_STANDARD.decode(d)?).map_err(|err| anyhow!(err))
}

// xTyBxQyGTA
fn decoder4(a: &str) -> anyhow::Result<String> {
    let d = a
        .as_bytes()
        .iter()
        .enumerate()
        .filter(|(index, _)| *index % 2usize == 0)
        .map(|(_, v)| *v)
        .collect::<Vec<_>>();

    String::from_utf8(BASE64_STANDARD.decode(d)?).map_err(|err| anyhow!(err))
}

// ux8qjPHC66
fn decoder5(a: &str) -> anyhow::Result<String> {
    let b = "X9a(O;FMV2-7VO5x;Ao\u{0005}:dN1NoFs?j,".as_bytes();

    let d = a
        .as_bytes()
        .chunks(2)
        .map(|ch| u8::from_str_radix(String::from_utf8(ch.to_vec()).unwrap().as_str(), 16).unwrap())
        .enumerate()
        .map(|(e, v)| (v ^ b[e % b.len()]))
        .collect::<Vec<_>>();

    String::from_utf8(BASE64_STANDARD.decode(d)?).map_err(|err| anyhow!(err))
}

// eSfH1IRMyL
fn decoder6(a: &str) -> anyhow::Result<String> {
    let d = a
        .as_bytes()
        .iter()
        .rev()
        .map(|i| *i - 1u8)
        .collect::<Vec<_>>()
        .chunks(2)
        .map(|ch| u8::from_str_radix(String::from_utf8(ch.to_vec()).unwrap().as_str(), 16).unwrap())
        .collect::<Vec<_>>();

    String::from_utf8(d).map_err(|err| anyhow!(err))
}

// KJHidj7det
fn decoder7(a: &str) -> anyhow::Result<String> {
    let b = &a[10..a.len() - 16];
    let c = r#"3SAY~#%Y(V%>5d/Yg"$G[Lh1rK4a;7ok"#.as_bytes();
    let d = BASE64_STANDARD.decode(b)?;
    let e = c.iter().cycle().take(d.len()).copied().collect::<Vec<_>>();

    let f = d
        .iter()
        .enumerate()
        .map(|(i, v)| v ^ e[i])
        .collect::<Vec<_>>();

    String::from_utf8(f).map_err(|err| anyhow!(err))
}

// o2VSUnjnZl
fn decoder8(a: &str) -> anyhow::Result<String> {
    let shift = 1u8;

    let d = a
        .as_bytes()
        .iter()
        .map(|b| {
            let ch = char::from(*b);
            match ch {
                'a'..='z' => {
                    let shifted = *b - shift;
                    if shifted < 'a' as u8 {
                        shifted + 26
                    } else {
                        shifted
                    }
                }
                'A'..='Z' => {
                    let shifted = *b - shift;
                    if shifted < 'A' as u8 {
                        shifted + 26
                    } else {
                        shifted
                    }
                }
                _ => *b,
            }
        })
        .collect::<Vec<_>>();

    String::from_utf8(d).map_err(|err| anyhow!(err))
}

// Oi3v1dAlaM, TsA2KGDGux, JoAHUMCLXV
fn decoder9(a: &str, f: u8) -> anyhow::Result<String> {
    let c = a
        .chars()
        .rev()
        .map(|ch| match ch {
            '-' => '+',
            '_' => '/',
            _ => ch,
        })
        .filter(|ch| *ch != '\n')
        .collect::<String>();

    let d = BASE64_STANDARD.decode(&c)?;

    let e = d.into_iter().map(|b| b - f).collect::<Vec<_>>();

    String::from_utf8(e).map_err(|err| anyhow!(err))
}

#[cfg(test)]
mod test {
    use crate::suppliers::tmdb::extractors::Episode;

    use super::*;

    // Oi3v1dAlaM
    #[test]
    fn decoder9_1() {
        let content = "=0je4I3M3pWe4Zmc0YkRWZ0R-gkce52fG5TU9RlS-0XT6MTe2NHbshzZRpDcqRHZP5HX893R1VXS91jffF3e-JVPKxDf2onRqVlOSZFUWdUe9gXWwZzU55jWQ9FSPZEck53Xx9Ef452d2d3d71Xeud1N70VdwVDVN9naJtnP64HX513XTZWU1dGb2ZW
dvh0e5lEbIxmd3gGU3slOsZzMHdkTK9mTy9mb_1zfbFVNzVDTfxDfa1TSTZlbLxla-YXU903T4pmSZFnU78VO-ZDSahmZPNlVW12b101WYZVf21zazFXXm1UeqhVU95Td1Uzfe5WcatWe651fYB3fe93d4NleJFXayx3MQJXPLZWTyh2MpxlRt1nf9Vjf5cUfRR1aQtzNWhHW2U1U5dEczQFS8gFSzslNIp0arhV
PXR3b0lWN3M1RpdTafp3OztnUud1c3AHdq1HVwp3cZNFVe9VUyp1X5hmUI5zS1YkVH5ESm9FfH1FfGZkRGZkRGZkROhXONRDfqNHZyZma3lHe0IHdoNzcqtnZtJnZqdXe4hne0NnbypXczYzd5hnc5RDN_gXd5lXb";

        let res = decoder9(content, 5).unwrap();

        println!("{res}")
    }

    #[tokio::test()]
    async fn should_load_source() {
        let res = extract(&SourceParams {
            id: 1399,
            imdb_id: Some("tt0944947".into()),
            ep: Some(Episode { e: 1, s: 1 }),
        })
        .await;

        println!("{res:#?}")
    }
}
