use std::fmt::Write as _;

use once_cell::sync::Lazy;
use pulldown_cmark::{escape::escape_html, Event, Options, Parser, Tag};
use regex::Regex;

const ERROR_MSG: &str = "fmt::Write on a string shouldn't fail";

pub fn markdown2pango(markdown: &str) -> String {
    let parser = Parser::new_ext(markdown, Options::ENABLE_STRIKETHROUGH);
    let mut pango_markup = String::with_capacity(markdown.len());

    for event in parser {
        match event {
            Event::Start(tag) => handle_tag(&mut pango_markup, tag, true),
            Event::End(tag) => handle_tag(&mut pango_markup, tag, false),
            // We escape pango unsafe characters because if '<' and '>' end up in regular text or
            // code, Pango simply won't display the thing at all if it is not a valid tag, which
            // is undesirable
            Event::Text(text) => escape_html(&mut pango_markup, text.as_ref()).expect(ERROR_MSG),
            Event::Code(code) => escape_html(&mut pango_markup, code.as_ref()).expect(ERROR_MSG),
            Event::Html(html) => handle_html(&mut pango_markup, html.as_ref()),
            Event::SoftBreak | Event::HardBreak => pango_markup.push('\n'),
            Event::FootnoteReference(_) | Event::Rule | Event::TaskListMarker(_) => {} // Noops
        }
    }

    pango_markup
}

fn handle_tag(pango_markup: &mut String, tag: Tag, is_start: bool) {
    let mut format = |tag_name: &str| {
        write!(
            pango_markup,
            "<{}{tag_name}>",
            if is_start { "" } else { "/" }
        )
    };

    match tag {
        Tag::CodeBlock(_) => format("tt"),
        Tag::Emphasis => format("i"),
        Tag::Strong => format("b"),
        Tag::Strikethrough => format("s"),
        Tag::Heading(_, _, _)
        | Tag::Paragraph
        | Tag::BlockQuote
        | Tag::List(_)
        | Tag::Item
        | Tag::FootnoteDefinition(_)
        | Tag::Table(_)
        | Tag::TableHead
        | Tag::TableRow
        | Tag::TableCell
        | Tag::Link(_, _, _)
        | Tag::Image(_, _, _) => return, // no-ops
    }
    .expect(ERROR_MSG);
}

/// Escapes regular HTML, but leaves [Pango markup][pango-markup] untouched
///
/// [pango-markup]: https://docs.gtk.org/Pango/pango_markup.html
fn handle_html(pango_markup: &mut String, html: &str) {
    static PANGO_TAG_REGEX: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"(</|<)(b|big|i|s|sub|sup|small|tt|u)\s?>").expect("The regex should be valid")
    });

    if PANGO_TAG_REGEX.is_match(html) {
        pango_markup.push_str(html);
        return;
    }

    // We handle <span> separately from the rest of the tags because it can have attributes and we
    // just assume all attributes the user gives are valid, Pango should reject them gracefully.
    // (It will probably ignore them.)
    let offset = if html.starts_with("</") { 2 } else { 1 };
    if html[offset..].starts_with("span") {
        pango_markup.push_str(html);
        return;
    }

    escape_html(pango_markup, html).expect("Writing to a string shouldn't fail");
}

#[cfg(test)]
mod tests {
    use super::markdown2pango;

    #[test]
    fn basic() {
        let bold = markdown2pango("**uwu**");
        let italic = markdown2pango("*owo*");
        let strikethrough = markdown2pango("~~UwU~~");
        let combined = markdown2pango("**uwu *owo* ~~UwU~~**");
        let triple_backtick = markdown2pango("```\nOwO\n```");

        assert_eq!(bold, "<b>uwu</b>");
        assert_eq!(italic, "<i>owo</i>");
        assert_eq!(strikethrough, "<s>UwU</s>");
        assert_eq!(combined, "<b>uwu <i>owo</i> <s>UwU</s></b>");
        assert_eq!(triple_backtick, "<tt>OwO\n</tt>");
    }

    #[test]
    fn pango() {
        let html = markdown2pango("hmmm <u >underline </u>");
        assert_eq!(html, "hmmm <u >underline </u>");

        let big = markdown2pango("<big>BIG</big>");
        assert_eq!(big, "<big>BIG</big>");

        let all_the_tags_orig = "<b><u><i><s><tt><small><span><sub><sup><big></b ></u > </i ></s ></tt ><small ></span ></sub ></sup ></big >";
        let all_the_tags = markdown2pango(all_the_tags_orig);
        assert_eq!(all_the_tags_orig, all_the_tags);
    }
}
