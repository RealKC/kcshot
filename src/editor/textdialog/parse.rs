use std::fmt::Write as _;

use once_cell::sync::Lazy;
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use pulldown_cmark_escape::escape_html;
use regex::Regex;

const ERROR_MSG: &str = "fmt::Write on a string shouldn't fail";

pub fn markdown2pango(markdown: &str) -> String {
    let parser = Parser::new_ext(markdown, Options::ENABLE_STRIKETHROUGH);
    let mut pango_markup = String::with_capacity(markdown.len());

    for event in parser {
        match event {
            Event::Start(tag) => handle_tag_start(&mut pango_markup, tag),
            Event::End(tag) => handle_tag_end(&mut pango_markup, tag),
            // We escape pango unsafe characters because if '<' and '>' end up in regular text or
            // code, Pango simply won't display the thing at all if it is not a valid tag, which
            // is undesirable
            Event::Text(text) => escape_html(&mut pango_markup, text.as_ref()).expect(ERROR_MSG),
            Event::Code(code) => escape_html(&mut pango_markup, code.as_ref()).expect(ERROR_MSG),
            Event::Html(html) | Event::InlineHtml(html) => {
                handle_html(&mut pango_markup, html.as_ref());
            }
            Event::InlineMath(_) | Event::DisplayMath(_) => {
                unreachable!("Math mode was not enabled");
            }
            Event::SoftBreak | Event::HardBreak => pango_markup.push('\n'),
            // Noops
            Event::FootnoteReference(_) | Event::Rule | Event::TaskListMarker(_) => {}
        }
    }

    pango_markup
}

fn handle_tag_end(pango_markup: &mut String, tag: TagEnd) {
    let mut format = |tag_name: &str| write!(pango_markup, "</{tag_name}>");

    match tag {
        TagEnd::CodeBlock => format("tt"),
        TagEnd::Emphasis => format("i"),
        TagEnd::Strong => format("b"),
        TagEnd::Strikethrough => format("s"),
        // no-ops
        TagEnd::Heading(_)
        | TagEnd::Paragraph
        | TagEnd::BlockQuote(_)
        | TagEnd::List(_)
        | TagEnd::Item
        | TagEnd::FootnoteDefinition
        | TagEnd::Table
        | TagEnd::TableHead
        | TagEnd::TableRow
        | TagEnd::TableCell
        | TagEnd::Link
        | TagEnd::Image
        | TagEnd::HtmlBlock
        | TagEnd::DefinitionList
        | TagEnd::DefinitionListTitle
        | TagEnd::DefinitionListDefinition
        | TagEnd::MetadataBlock(_) => return,
    }
    .expect(ERROR_MSG);
}

fn handle_tag_start(pango_markup: &mut String, tag: Tag) {
    let mut format = |tag_name: &str| write!(pango_markup, "<{tag_name}>");

    match tag {
        Tag::CodeBlock(_) => format("tt"),
        Tag::Emphasis => format("i"),
        Tag::Strong => format("b"),
        Tag::Strikethrough => format("s"),
        // no-ops
        Tag::Heading { .. }
        | Tag::Paragraph
        | Tag::BlockQuote(_)
        | Tag::List(_)
        | Tag::Item
        | Tag::FootnoteDefinition(_)
        | Tag::Table(_)
        | Tag::TableHead
        | Tag::TableRow
        | Tag::TableCell
        | Tag::Link { .. }
        | Tag::Image { .. }
        | Tag::HtmlBlock
        | Tag::DefinitionList
        | Tag::DefinitionListTitle
        | Tag::DefinitionListDefinition
        | Tag::MetadataBlock(_) => return,
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
