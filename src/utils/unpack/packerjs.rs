use std::{collections::HashMap, fmt, sync::OnceLock};

use regex::{Captures, Regex, RegexBuilder};

/// Unpacks P.A.C.K.E.R. packed js code.
pub fn unpack(source: &str) -> Result<String, UnpackError> {
    static RE: OnceLock<Regex> = OnceLock::new();

    let (payload, symtab, base, count) = filter_args(source)?;

    if count != symtab.len() {
        return Err(UnpackError {
            message: "Malformed p.a.c.k.e.r. symtab.",
        });
    }

    let sanitized_payload = payload.replace("\\\\", "\\").replace("\\'", "'");
    let source = RE
        .get_or_init(|| Regex::new(r"\b\w+\b").unwrap())
        .replace_all(&sanitized_payload, |cap: &Captures| {
            let word = cap.get(0).unwrap().as_str();
            let idx = unbase(base, word) as usize;
            let mut sym = if idx < symtab.len() {
                symtab[idx]
            } else {
                word
            };
            if sym.is_empty() {
                sym = word;
            }
            sym.to_string()
        });
    Ok(source.to_string())
}

#[derive(Debug)]
pub struct UnpackError {
    message: &'static str,
}

impl fmt::Display for UnpackError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "unpack error: {}", &self.message) // user-facing output
    }
}

pub fn detect(source: &str) -> bool {
    source
        .replace(' ', "")
        .starts_with("eval(function(p,a,c,k,e,")
}

const ALPHABET_62: &str = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
const ALPHABET_95: &str = r##" !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~"##;

fn unbase(base: u32, string: &str) -> u32 {
    static ALPHABET_62_DICT: OnceLock<HashMap<char, u32>> = OnceLock::new();
    static ALPHABET_95_DICT: OnceLock<HashMap<char, u32>> = OnceLock::new();

    match base {
        n @ 2..=36 => u32::from_str_radix(string, n).unwrap(),
        37..62 => dict_convert(
            base,
            string,
            ALPHABET_62_DICT.get_or_init(|| {
                HashMap::from_iter(
                    ALPHABET_62
                        .chars()
                        .enumerate()
                        .map(|(idx, ch)| (ch, idx as u32)),
                )
            }),
        ),
        _ => dict_convert(
            base,
            string,
            ALPHABET_95_DICT.get_or_init(|| {
                HashMap::from_iter(
                    ALPHABET_95
                        .chars()
                        .enumerate()
                        .map(|(idx, ch)| (ch, idx as u32)),
                )
            }),
        ),
    }
}

fn dict_convert(base: u32, string: &str, dict: &HashMap<char, u32>) -> u32 {
    string
        .chars()
        .rev()
        .enumerate()
        .map(|(i, ch)| (base ^ (i as u32)) * (*dict.get(&ch).unwrap()))
        .sum()
}

fn filter_args(source: &str) -> Result<(&str, Vec<&str>, u32, usize), UnpackError> {
    static JUICER1: OnceLock<Regex> = OnceLock::new();
    static JUICER2: OnceLock<Regex> = OnceLock::new();

    let juicers = [
        JUICER1.get_or_init(|| {
            RegexBuilder::new(
                r"}\('(.*)', *(\d+|\[\]), *(\d+), *'(.*)'\.split\('\|'\), *(\d+), *(.*)\)\)",
            )
            .dot_matches_new_line(true)
            .build()
            .unwrap()
        }),
        JUICER2.get_or_init(|| {
            RegexBuilder::new(r"}\('(.*)', *(\d+|\[\]), *(\d+), *'(.*)'\.split\('\|'\)")
                .dot_matches_new_line(true)
                .build()
                .unwrap()
        }),
    ];

    for juicer in juicers {
        let maybe_args =
            juicer
                .captures(source)
                .and_then(|caps| -> Option<(&str, Vec<&str>, u32, usize)> {
                    let payload = caps.get(1)?.as_str();

                    let radix_str = caps.get(2)?.as_str();
                    let radix = if radix_str == "[]" {
                        62
                    } else {
                        radix_str.parse::<u32>().ok()?
                    };

                    let count = caps.get(3)?.as_str().parse::<usize>().ok()?;
                    let symtab: Vec<_> = caps.get(4)?.as_str().split("|").collect();

                    Some((payload, symtab, radix, count))
                });

        if let Some(tuple) = maybe_args {
            return Ok(tuple);
        }
    }

    Err(UnpackError {
        message: "Could not make sense of p.a.c.k.e.r data (unexpected code structure)",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_DATA1: &str = "eval(function(p,a,c,k,e,r){e=String;if(!''\
    .replace(/^/,String)){while(c--)r[c]=k[c]||c;k=[function(e){return r[e]}];e=\
    function(){return'\\w+'};c=1};while(c--)if(k[c])p=p.replace(new RegExp('\\b'+\
    e(c)+'\\b','g'),k[c]);return p}('1 0=2;3(0)',4,4,'x|var|5|alert'.split('|'),0,{}))";

    static TEST_DATA2: &str = r##"eval(function(p,a,c,k,e,d){while(c--)if(k[c])p=p.replace(new RegExp('\\b'+c.toString(a)+'\\b','g'),k[c]);return p}('y("a7").a6({a5:[{2k:"1l://a4.a3.2j/a2/a1/a0/9z/9y.9x?t=9w&s=45&e=9v&f=49&9u=2m&i=0.4&9t=9s&9r=2m&9q=2m&9p=4k"}],9o:"1l://4h.4g/9n.4f?v=4k",9m:"2l%",9l:"2l%",9k:"9j",9i:"4i.15",9h:\'9g\',9f:\'9e\',9d:{9c:{4j:"#1w",9b:"#1w"},9a:{99:"#1w"},98:{4j:"#1w"}},97:"1j",o:[{2k:"/4b?4a=94&1c=4i&93=1l://4h.4g/92.4f",91:"90"}],28:{8z:1,8y:\'#8x\',8w:\'#8v\',8u:"8t",8s:30,8r:2l,},\'8q\':{"8p":"8o"},8n:"8m",8l:"1l://8k.2j",2i:{2k:"/8j/2y/2i.q","1v":u,8i:"1l://8h.2j/?8g=2i",1f:"8f-8e",8d:"5",1v:u},8c:1j,26:[0.25,0.5,0.75,1,1.25,1.5,2]});r 2h=\'8b\'+$.8a(\'89\');n(!1g.23(2h)){y().w(\'88\',j(1h){1g.2r(2h,\'1j\');87.86()})}r 2f,2g;r 85=0,84=0,83=0;r k=y();r 4e=0,82=0,81=0,1b=0;$.80({7z:{\'7y-7x\':\'7w-7u\'}});k.w(\'7t\',j(x){n(5>0&&x.1f>=5&&2g!=1){2g=1;$(\'1k.7s\').7r(\'7q\')}n(x.1f>=1b+5||x.1f<1b){1b=x.1f;2e.7p(\'2d\',7o.7n(1b),{7m:60*60*24*7})}});k.w(\'1p\',j(x){4e=x.1f});k.w(\'7k\',j(x){4d(x)});k.w(\'7j\',j(){$(\'1k.4c\').7i();2e.7h(\'2d\')});k.w(\'7g\',j(x){});j 4d(x){$(\'1k.4c\').1v();$(\'#7f\').1v();n(2f)1y;2f=1;1u=0;n(7e.7d===7c){1u=1}$.40(\'/4b?4a=7b&7a=79&78=49-46-77-45-76&74=1&72=&1u=\'+1u,j(42){$(\'#71\').70(42)});r 1b=2e.40(\'2d\');n(1b>0){y().1p(1b)}}j 6z(){r o=k.20(3z);3y.3x(o);n(o.1c>1){2p(i=0;i<o.1c;i++){n(o[i].1z==3z){3y.3x(\'!!=\'+i);k.2n(i)}}}}k.w(\'6y\',j(){y().29(\'<q 3j="3i://3h.3g.3f/3e/q" 3d="a-q-1e a-q-1e-6x" 3c="0 0 1s 1s" 3b="u"><1q d="m 25.6w,57.6u v 6t.3 c 0.6s,2.6r 2.6q,4.6p 4.8,4.8 h 62.7 v -19.3 h -48.2 v -96.4 3w 6o.6n v 19.3 c 0,5.3 3.6,7.2 8,4.3 l 41.8,-27.9 c 2.6m,-1.6l 4.6k,-5.6j 2.7,-8 -0.6i,-1.6h -1.6g,-2.6f -2.7,-2.7 l -41.8,-27.9 c -4.4,-2.9 -8,-1 -8,4.3 v 19.3 3w 30.6e c -2.6d,0.6c -4.6b,2.6a -4.9,4.9 z m 69.68,73.67 c -3.3v,-6.3u -10.3t,-10.3s -17.7,-10.6 -7.3r,0.3q -13.3p,4.3o -17.7,10.6 -8.1t,14.3n -8.1t,32.3m 0,46.3 3.3v,6.3u 10.3t,10.3s 17.7,10.6 7.3r,-0.3q 13.3p,-4.3o 17.7,-10.6 8.1t,-14.3n 8.1t,-32.3m 0,-46.3 z m -17.7,47.2 c -7.8,0 -14.4,-11 -14.4,-24.1 0,-13.1 6.6,-24.1 14.4,-24.1 7.8,0 14.4,11 14.4,24.1 0,13.1 -6.5,24.1 -14.4,24.1 z m -47.66,9.65 v -51 l -4.8,4.8 -6.8,-6.8 13,-12.64 c 3.63,-3.61 8.5z,-0.5y 8.2,3.4 v 62.5x z"></1q></q>\',"5w 10 35",j(){y().1p(y().34()+10)},"3k");$("1k[2a=3k]").31().2z(\'.a-1e-2b\');y().29(\'<q 3j="3i://3h.3g.3f/3e/q" 3d="a-q-1e a-q-1e-2b" 3c="0 0 1s 1s" 3b="u"><1q d="5v.2,5u.5t.1a,21.1a,0,0,0-17.7-10.6,21.1a,21.1a,0,0,0-17.7,10.6,44.1r,44.1r,0,0,0,0,46.3,21.1a,21.1a,0,0,0,17.7,10.6,21.1a,21.1a,0,0,0,17.7-10.6,44.1r,44.1r,0,0,0,0-46.5s-17.7,47.2c-7.8,0-14.4-11-14.4-24.5r.6-24.1,14.4-24.1,14.4,11,14.4,24.5q.4,3a.5p,95.5,3a.5o-43.4,9.7v-5n-4.8,4.8-6.8-6.8,13-5m.8,4.8,0,0,1,8.2,3.5l.7l-9.6-.5k-5j.5i.5h.39,4.39,0,0,1-4.8,4.5g.6v-19.5f.2v-96.5e.5d.5c,5.3-3.6,7.2-8,4.3l-41.8-27.5b.38,6.38,0,0,1-2.7-8,5.37,5.37,0,0,1,2.7-2.5a.8-27.59.4-2.9,8-1,8,4.58.56.55.36,4.36,0,0,1,54.1,57.53"></1q></q>\',"52 10 35",j(){r 1o=y().34()-10;n(1o<0)1o=0;y().1p(1o)},"33");$("1k[2a=33]").31().2z(\'.a-1e-2b\');});k.w("p",j(1h){r o=k.20();n(o.1c<2)1y;$(\'.a-g-b-2a\').w(\'50\',()=>{$(\'.a-b-p\').18(\'16-1n\',\'u\');$(\'.a-b-p\').18(\'16-1i\',\'u\');$(\'#a-g-b-p\').1m(\'a-g-b-1d\')});$(\'.a-g-4z-4y\').4x(j(){$(\'#a-g-b-p\').1m(\'a-g-b-1d\');$(\'.a-b-p\').18(\'16-1i\',\'u\')});k.29("/2y/4w.q","4v 4u",j(){$(\'.a-2u\').4t(\'a-g-2t\');$(\'.a-g-28, .a-2x-2w, .a-g-26\').18(\'16-1n\',\'u\');$(\'.a-g-28, .a-2x-2w, .a-g-26\').18(\'16-1i\',\'u\');n($(\'.a-2u\').4s(\'a-g-2t\')){$(\'.a-b-p\').18(\'16-1n\',\'1j\');$(\'.a-b-p\').18(\'16-1i\',\'1j\');$(\'.a-g-b\').1m(\'a-g-b-1d\');$(\'#a-g-b-p\').2s(\'a-g-b-1d\');$(\'.a-g-b:4r\').2s(\'a-g-b-1d\');}4q{$(\'.a-b-p\').18(\'16-1n\',\'u\');$(\'.a-b-p\').18(\'16-1i\',\'u\');$(\'.a-g-b-p\').1m(\'a-g-b-1d\')}},"4p");k.w("4o",j(1h){1g.2r(\'22\',1h.o[1h.4n].1z)});n(1g.23(\'22\')){4m("2q(1g.23(\'22\'));",4l)}});r 1x;j 2q(2o){r o=k.20();n(o.1c>1){2p(i=0;i<o.1c;i++){n(o[i].1z==2o){n(i==1x){1y}1x=i;k.2n(i)}}}}',36,368,'||||||||||jw|submenu|||||settings|||function|player|||if|tracks|audioTracks|svg|var|||false||on||jwplayer||||||||aria||attr||589|lastt|length|active|icon|position|localStorage|event|expanded|true|div|https|removeClass|checked|tt|seek|path|769|240|60009|adb|hide|10D68A|current_audio|return|name|getAudioTracks||default_audio|getItem|||playbackRates||captions|addButton|button|rewind||ttn1kbhx78xlja|ls|vvplay|vvad|reloadKey|logo|com|file|100|Qx9xrn01ypu0F|setCurrentAudioTrack|audio_name|for|audio_set|setItem|addClass|open|controls||quality|tooltip|images|insertAfter||detach||ff00|getPosition|sec|974|887|013|867|178|focusable|viewBox|class|2000|org|w3|www|http|xmlns|ff11||06475|23525|29374|97928|30317|31579|29683|38421|30626|72072|H|log|console|track_name|get||data|||1734702290||||11634219|op|dl|video_ad|doPlay|prevt|jpg|cc|laving|1441|text|31148|300|setTimeout|currentTrack|audioTrackChanged|dualSound|else|last|hasClass|toggleClass|Track|Audio|dualy|mousedown|buttons|topbar|click||Rewind|778Z|214|2A4|3H209||3v19|9c4|7l41|9a6|3c0|1v19|4H79|3h48|8H146|3a4|2v125|130|1Zm162|4v62|13a4|51l|278Zm|278|1S103|1s6|3Zm|078a21|131|M113|Forward|69999|88605|21053||03598||02543|99999|72863|77056|04577|422413|163|210431|860275|03972|689569|893957|124979|52502|174985|57502|04363|13843|480087|93574|99396|160|76396|164107|63589|03604|125|778||993957|rewind2|ready|set_audio_track|html|fviews|referer||embed||11a18662b4e439035a8db8d230faf9d0|219|hash|n1kbhx78xlja|file_code|view|undefined|cRAds1|window|over_player_msg|pause|remove|show|complete|play||ttl|round|Math|set|slow|fadeIn|video_ad_fadein|time|cache||no|Cache|Content|headers|ajaxSetup|v2done|tott|pop3done|vastdone2|vastdone1|reload|location|error|file_id|cookie|jwplayer_reload_232011_|playbackRateControls|margin|left|top|ref|earnvids|link|static|vidhide|aboutlink|VidHide|abouttext|1080p|1408|qualityLabels|fontOpacity|backgroundOpacity|Tahoma|fontFamily|303030|backgroundColor|FFFFFF|color|userFontScale|thumbnails|kind|n1kbhx78xlja0000|url|get_slides|||androidhls|menus|progress|timeslider|icons|controlbar|skin|none|fullscreenOrientationLock|auto|preload|duration|uniform|stretching|height|width|n1kbhx78xlja_xt|image|asn|p2|p1|500|sp|srv|129600|LmJXV3DaVljRtJmve3UyQ3t4geCTxFnF7rpMQmmP9h0|m3u8|master|n1kbhx78xlja_o|02326|01|hls2|milocdn|m8yTSnzumRSd|sources|setup|vplayer'.split('|')))"##;
    //"var a=1",

    #[test]
    fn check_valid() {
        assert!(detect(TEST_DATA1));
    }
    #[test]
    fn extract_args() {
        let (payload, symtab, radix, count) = filter_args(TEST_DATA1).unwrap();
        assert_eq!(payload, "1 0=2;3(0)");
        assert_eq!(symtab, ["x", "var", "5", "alert"]);
        assert_eq!(radix, 4);
        assert_eq!(count, 4);
    }
    #[test]
    fn unpack_code() {
        let res = unpack(TEST_DATA2);
        println!("{res:#?}")
        //assert_eq!(unpack(TEST_DATA1).unwrap(), "var x=5;alert(x)");
    }
}
