// Copyright: Ankitects Pty Ltd and contributors
// License: GNU AGPL, version 3 or later; http://www.gnu.org/licenses/agpl.html

use htmlescape;
use lazy_static::lazy_static;
use regex::Regex;
use std::borrow::Cow;
use std::collections::HashSet;
use std::ptr;

#[derive(Debug, PartialEq)]
pub enum AVTag<'a> {
    SoundOrVideo(Cow<'a, str>),
    TextToSpeech {
        args: Vec<&'a str>,
        field_text: Cow<'a, str>,
    },
}

lazy_static! {
    static ref HTML: Regex = Regex::new(concat!(
        "(?si)",
        // wrapped text
        r"(<!--.*?-->)|(<style.*?>.*?</style>)|(<script.*?>.*?</script>)",
        // html tags
        r"|(<.*?>)",
    ))
    .unwrap();

    static ref IMG_TAG: Regex = Regex::new(
        // group 1 is filename
        r#"(?i)<img[^>]+src=["']?([^"'>]+)["']?[^>]*>"#
    ).unwrap();

    // videos are also in sound tags
    static ref AV_TAGS: Regex = Regex::new(
        r#"(?xs)
            \[sound:(.*?)\]     # 1 - the filename in a sound tag
            |
            \[anki:tts\]
                \[(.*?)\]       # 2 - arguments to tts call
                (.*?)           # 3 - field text
            \[/anki:tts\]
            "#).unwrap();

    static ref CLOZED_TEXT: Regex = Regex::new(
        r"(?s)\{\{c(\d+)::.+?\}\}"
    ).unwrap();
}

pub fn strip_html(html: &str) -> Cow<str> {
    HTML.replace_all(html, "")
}

pub fn decode_entities(html: &str) -> Cow<str> {
    if html.contains('&') {
        match htmlescape::decode_html(html) {
            Ok(text) => text,
            Err(e) => format!("{:?}", e),
        }
        .into()
    } else {
        // nothing to do
        html.into()
    }
}

pub fn strip_html_for_tts(html: &str) -> Cow<str> {
    match HTML.replace_all(html, " ") {
        Cow::Borrowed(_) => decode_entities(html),
        Cow::Owned(s) => decode_entities(&s).to_string().into(),
    }
}

pub fn strip_av_tags(text: &str) -> Cow<str> {
    AV_TAGS.replace_all(text, "")
}

pub fn av_tags_in_string(text: &str) -> impl Iterator<Item = AVTag> {
    AV_TAGS.captures_iter(text).map(|caps| {
        if let Some(av_file) = caps.get(1) {
            AVTag::SoundOrVideo(decode_entities(av_file.as_str()))
        } else {
            let args = caps.get(2).unwrap();
            let field_text = caps.get(3).unwrap();
            AVTag::TextToSpeech {
                args: args.as_str().split(' ').collect(),
                field_text: strip_html_for_tts(field_text.as_str()),
            }
        }
    })
}

pub fn strip_html_preserving_image_filenames(html: &str) -> Cow<str> {
    let without_fnames = IMG_TAG.replace_all(html, r" $1 ");
    let without_html = HTML.replace_all(&without_fnames, "");
    // no changes?
    if let Cow::Borrowed(b) = without_html {
        if ptr::eq(b, html) {
            return Cow::Borrowed(html);
        }
    }
    // make borrow checker happy
    without_html.into_owned().into()
}

pub fn cloze_numbers_in_string(html: &str) -> HashSet<u16> {
    let mut hash = HashSet::with_capacity(4);
    for cap in CLOZED_TEXT.captures_iter(html) {
        if let Ok(n) = cap[1].parse() {
            hash.insert(n);
        }
    }
    hash
}

#[cfg(test)]
mod test {
    use crate::text::{
        av_tags_in_string, cloze_numbers_in_string, strip_av_tags, strip_html,
        strip_html_preserving_image_filenames, AVTag,
    };
    use std::collections::HashSet;

    #[test]
    fn test_stripping() {
        assert_eq!(strip_html("test"), "test");
        assert_eq!(strip_html("t<b>e</b>st"), "test");
        assert_eq!(strip_html("so<SCRIPT>t<b>e</b>st</script>me"), "some");

        assert_eq!(
            strip_html_preserving_image_filenames("<img src=foo.jpg>"),
            " foo.jpg "
        );
        assert_eq!(
            strip_html_preserving_image_filenames("<img src='foo.jpg'><html>"),
            " foo.jpg "
        );
        assert_eq!(strip_html_preserving_image_filenames("<html>"), "");
    }

    #[test]
    fn test_cloze() {
        assert_eq!(
            cloze_numbers_in_string("test"),
            vec![].into_iter().collect::<HashSet<u16>>()
        );
        assert_eq!(
            cloze_numbers_in_string("{{c2::te}}{{c1::s}}t{{"),
            vec![1, 2].into_iter().collect::<HashSet<u16>>()
        );
    }

    #[test]
    fn test_audio() {
        let s = "abc[sound:fo&amp;o.mp3]def[anki:tts][lang=en_US voices=Bob,Jane]foo<br>1&gt;2[/anki:tts]gh";
        assert_eq!(strip_av_tags(s), "abcdefgh");
        assert_eq!(
            av_tags_in_string(s).collect::<Vec<_>>(),
            vec![
                AVTag::SoundOrVideo("fo&o.mp3".into()),
                AVTag::TextToSpeech {
                    args: vec!["lang=en_US", "voices=Bob,Jane"],
                    field_text: "foo 1>2".into()
                },
            ]
        );
    }
}