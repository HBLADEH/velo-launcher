use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use quick_xml::events::Event as XmlEvent;
use std::fmt::Write;

#[derive(Default)]
struct Notes {
    text: String,
    lists: Vec<Option<u64>>,
}

impl Notes {
    fn separate(&mut self, paragraph: bool) {
        self.text
            .truncate(self.text.trim_end_matches([' ', '\t']).len());
        if !self.text.is_empty() && !self.text.ends_with('\n') {
            self.text.push('\n');
        }
        if paragraph && !self.text.is_empty() && !self.text.ends_with("\n\n") {
            self.text.push('\n');
        }
    }

    fn start_list(&mut self, first: Option<u64>) {
        self.separate(false);
        self.lists.push(first);
    }

    fn end_list(&mut self) {
        self.lists.pop();
        self.separate(self.lists.is_empty());
    }

    fn item(&mut self) {
        self.separate(false);
        for _ in 1..self.lists.len() {
            self.text.push_str("  ");
        }
        if let Some(Some(next)) = self.lists.last_mut() {
            let _ = write!(self.text, "{next}. ");
            *next = next.saturating_add(1);
        } else {
            self.text.push_str("• ");
        }
    }

    fn html_text(&mut self, value: &str) {
        for character in value.chars() {
            if character.is_ascii_whitespace() {
                if !self.text.is_empty() && !self.text.ends_with([' ', '\n']) {
                    self.text.push(' ');
                }
            } else {
                self.text.push(character);
            }
        }
    }

    fn finish(mut self) -> String {
        self.text.truncate(self.text.trim_end().len());
        self.text
    }
}

pub(super) fn markdown(source: &str) -> String {
    let mut notes = Notes::default();
    for event in Parser::new(source) {
        match event {
            Event::Start(Tag::List(first)) => notes.start_list(first),
            Event::Start(Tag::Item) => notes.item(),
            Event::End(TagEnd::Item) => notes.separate(false),
            Event::End(TagEnd::List(_)) => notes.end_list(),
            Event::End(TagEnd::Paragraph) => notes.separate(notes.lists.is_empty()),
            Event::End(TagEnd::Heading(_)) => notes.separate(false),
            Event::End(TagEnd::CodeBlock | TagEnd::BlockQuote(_)) => {
                notes.separate(true);
            }
            Event::Text(value) | Event::Code(value) => notes.text.push_str(&value),
            Event::SoftBreak => notes.text.push(' '),
            Event::HardBreak => notes.separate(false),
            Event::Rule => notes.separate(true),
            Event::Html(value) => {
                notes.text.push_str(&html(&value));
                notes.separate(true);
            }
            Event::InlineHtml(value) if matches!(value.as_ref(), "<br>" | "<br/>" | "<br />") => {
                notes.separate(false);
            }
            _ => {}
        }
    }
    notes.finish()
}

pub(super) fn html(source: &str) -> String {
    let mut notes = Notes::default();
    let mut reader = quick_xml::Reader::from_str(source);
    reader.config_mut().check_end_names = false;
    let mut preformatted = false;
    loop {
        match reader.read_event() {
            Ok(XmlEvent::Start(tag)) => match tag.name().as_ref() {
                b"ul" => notes.start_list(None),
                b"ol" => {
                    let first = tag
                        .try_get_attribute("start")
                        .ok()
                        .flatten()
                        .and_then(|attr| std::str::from_utf8(&attr.value).ok()?.parse().ok());
                    notes.start_list(Some(first.unwrap_or(1)));
                }
                b"li" => notes.item(),
                b"br" => notes.separate(false),
                b"pre" => preformatted = true,
                _ => {}
            },
            Ok(XmlEvent::Empty(tag)) if tag.name().as_ref() == b"br" => notes.separate(false),
            Ok(XmlEvent::End(tag)) => match tag.name().as_ref() {
                b"ul" | b"ol" => notes.end_list(),
                b"li" => notes.separate(false),
                b"p" => notes.separate(notes.lists.is_empty()),
                b"h1" | b"h2" | b"h3" | b"h4" | b"h5" | b"h6" => notes.separate(false),
                b"blockquote" => notes.separate(true),
                b"pre" => {
                    preformatted = false;
                    notes.separate(true);
                }
                _ => {}
            },
            Ok(XmlEvent::Text(value)) => {
                if let Ok(value) = value.xml_content(quick_xml::XmlVersion::Implicit1_0) {
                    if preformatted {
                        notes.text.push_str(&value);
                    } else {
                        notes.html_text(&value);
                    }
                }
            }
            Ok(XmlEvent::GeneralRef(reference)) => {
                if let Ok(name) = reference.decode() {
                    if name == "nbsp" {
                        notes.text.push('\u{a0}');
                    } else {
                        let entity = format!("&{name};");
                        let decoded = quick_xml::escape::unescape(&entity);
                        notes.text.push_str(decoded.as_deref().unwrap_or(&entity));
                    }
                }
            }
            Ok(XmlEvent::Eof) | Err(_) => break,
            _ => {}
        }
    }
    notes.finish()
}

#[cfg(test)]
mod tests {
    #[test]
    fn markdown_and_atom_notes_keep_inline_links_with_their_list_item() {
        let markdown = "## 问题修复\r\n\r\n- 修复窗口圆角 ([#9](https://example.test/9)) ([`b8cb975`](https://example.test/commit))\r\n- 支持 A & B\r\n\r\n## 验证\r\n\r\n3. 第三步\r\n4. 第四步";
        let html = r##"<h2>问题修复</h2><ul>
<li>修复窗口圆角 (<a href="https://example.test/9"><span>#9</span></a>) (<a href="https://example.test/commit"><code>b8cb975</code></a>)</li>
<li>支持 A &amp; B</li></ul><h2>验证</h2><ol start="3"><li>第三步</li><li>第四步</li></ol>"##;
        let expected =
            "问题修复\n• 修复窗口圆角 (#9) (b8cb975)\n• 支持 A & B\n\n验证\n3. 第三步\n4. 第四步";
        assert_eq!(super::markdown(markdown), expected);
        assert_eq!(super::html(html), expected);
    }
}
