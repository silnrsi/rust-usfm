#![allow(dead_code)]
use std::{collections::HashMap, fs::File, io, path::Path, sync::OnceLock};

use nom::{
    branch::alt,
    bytes::complete::{tag, take},
    combinator::{cut, opt, value, verify},
    error::make_error,
    multi::many0,
    number::complete::float,
    sequence::{delimited, terminated},
    AsChar, Err, Parser,
};

use crate::{
    extension::{Category, Extensions},
    terminal::{self, line_ending1, marker},
};

use super::Result;

#[derive(Debug, Default)]
struct Rope {
    segments: String,
}

#[derive(Debug, Default)]
pub struct Document {
    source: Rope,
    nodes: Option<Node>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
enum Content {
    Text(String),
    Para(Node),
    Book(Node),
    OptBreak,
}

impl<'s> Default for Content {
    fn default() -> Self {
        Content::Text(Default::default())
    }
}

impl<S: AsRef<str>> From<S> for Content {
    fn from(value: S) -> Self {
        Content::Text(value.as_ref().to_owned())
    }
}

#[derive(Debug, Default, PartialEq, Eq, Clone)]
struct Node {
    style: String,
    attributes: HashMap<String, String>,
    content: Vec<Content>,
}

struct State {
    doc: Document,
    markers: Extensions,
    version: f32,
}

impl<'i> State {
    const USFM_SRC: &'static str = include_str!("../docs/grammar/usfm.ext");

    fn usfm_ext() -> &'static Extensions {
        static USFM_EXT: OnceLock<Extensions> = OnceLock::new();
        USFM_EXT.get_or_init(|| {
            let mut res = Extensions::from(Self::USFM_SRC);
            res.shrink_to_fit();
            res
        })
    }

    pub fn new() -> Self {
        State {
            doc: Document::default(),
            markers: Self::usfm_ext().clone(),
            version: 3.0,
        }
    }

    pub fn with_markers<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let mut doc = Self::new();
        doc.markers = doc.markers.update_from_reader(File::open(path.as_ref())?)?;
        Ok(doc)
    }

    fn text(input: &str) -> Result<Content> {
        terminal::text
            .map(|s| Content::Text(s.to_owned()))
            .parse(input)
    }

    fn para_text(input: &str) -> Result<Content> {
        terminal::text
            .map(|s| Content::Text(s.trim_ascii_end().to_owned()))
            .parse(input)
    }

    fn optbreak(input: &str) -> Result<Content> {
        value(Content::OptBreak, tag("\\")).parse(input)
    }

    fn identification(&mut self, input: &'i str) -> Result<'i, Content> {
        let code = terminated(
            verify(take(3usize), |s: &str| {
                let (a, b) = s.chars().fold((0u8, 0u8), |(a, b), c| {
                    (a + c.is_ascii_uppercase() as u8, b + c.is_dec_digit() as u8)
                });
                a + b <= 3
            }),
            terminal::space1,
        );

        let (input, _) = terminal::bom(input)?;
        let (input, (code, text)) =
            delimited(marker::tag("id"), code.and(opt(Self::text)), line_ending1).parse(input)?;

        let (input, version) =
            opt(delimited(marker::tag("usfm"), cut(float), line_ending1)).parse(input)?;

        if let Some(version) = version {
            self.version = version;
        }

        let content = text.as_slice().into();
        Ok((
            input,
            Content::Book(Node {
                style: "id".into(),
                attributes: [("code".into(), code.to_owned())].into(),
                content,
            }),
        ))
    }

    fn marker(&self, cat: Category) -> impl Fn(&str) -> Result<&str> + '_ {
        move |input| {
            let (input, style) = terminal::marker(input)?;
            match self.markers.get(style) {
                Some(marker) if marker.category == cat => Ok((input, style)),
                Some(_) => Err(Err::Error(make_error(input, nom::error::ErrorKind::Tag))),
                None => Err(Err::Error(make_error(input, nom::error::ErrorKind::Tag))),
            }
        }
    }

    fn headers(&self, input: &'i str) -> Result<'i, Vec<Content>> {
        let marker = alt((
            self.marker(Category::Header),
            marker::tag("rem"),
            marker::tag("sts"),
        ));
        let header = terminated(marker.and(Self::para_text), line_ending1).map(|(style, text)| {
            Content::Para(Node {
                style: style.into(),
                content: vec![text],
                ..Node::default()
            })
        });
        many0(header).parse(input)
    }

    fn titles(&self, input: &'i str) -> Result<'i, Vec<Content>> {
        let marker = self.marker(Category::Title).or(marker::tag("rem"));
        let content = alt((Self::text, Self::optbreak));
        let title = terminated(marker.and(content), line_ending1).map(|(style, rest)| {
            Content::Para(Node {
                style: style.into(),
                content: vec![rest],
                ..Node::default()
            })
        });
        many0(title).parse(input)
    }

    // fn get_subparser<'i, O, E>(&self, style: &str) -> impl nom::Parser<&str, O, E>
    // where
    //     E: ParseError<&str> + ContextError<&str>,
    // {
    //     // match self.markers.get(style)?.category
    //     // {
    //         // Cell => {},
    //         // Char => {},
    //         // Crossreference => {},
    //         // CrossreferenceChar => {},
    //         // Footnote => {},
    //         // FootnoteChar => {},
    //         // Header => {},
    //         // Internal => {},
    //         // IntroChar => {},
    //         // Introduction => {},
    //         // List => {},
    //         // ListChar => {},
    //         // Milestone => {},
    //         // OtherPara => {},
    //         // SectionPara => {},
    //         // Title => {},
    //         // VersePara => {},
    //         // _ => {},

    //     // }
    //     unimplemented!()
    // }
}

// impl<'i, 'a, E> nom::Parser<&str, Document, E> for State
// where
//     E: ParseError<&str> + ContextError<&str>,
// {
//     fn parse(&mut self, input: &str) -> Result<Document> {
//         let (input, _) = bom
//     }
// }

#[cfg(test)]
mod test {
    use super::{Content, Node, State};

    #[test]
    fn book_identification() {
        let mut parser = State::new();

        let parse =
            parser.identification("\\id MAT 41MATGNT92.SFM, Good News Translation, June 2003\n");
        assert_eq!(
            parse,
            Ok((
                "",
                Content::Book(Node {
                    style: "id".into(),
                    attributes: [("code".into(), "MAT".into())].into(),
                    content: vec!["41MATGNT92.SFM, Good News Translation, June 2003".into()]
                })
            ))
        );
        assert_eq!(parser.version, 3.0);

        let parse = parser.identification(
            "\\id MAT 41MATGNT92.SFM, Good News Translation, June 2003\n\
                    \\usfm 3.1\n",
        );
        assert_eq!(
            parse,
            Ok((
                "",
                Content::Book(Node {
                    style: "id".into(),
                    attributes: [("code".into(), "MAT".into())].into(),
                    content: vec!["41MATGNT92.SFM, Good News Translation, June 2003".into()]
                })
            ))
        );
        assert_eq!(parser.version, 3.1);
    }

    #[test]
    fn book_headers() {
        let parser = State::new();

        let parse = parser.headers(
            "\\ide some blurb\n\
                    \\h1 Heading 1\n\
                    \\rem A remarkable remark\n",
        );
        assert_eq!(
            parse,
            Ok((
                "",
                vec![
                    Content::Para(Node {
                        style: "ide".into(),
                        attributes: Default::default(),
                        content: vec!["some blurb".into()]
                    }),
                    Content::Para(Node {
                        style: "h1".into(),
                        attributes: Default::default(),
                        content: vec!["Heading 1".into()]
                    }),
                    Content::Para(Node {
                        style: "rem".into(),
                        attributes: Default::default(),
                        content: vec!["A remarkable remark".into()]
                    }),
                ]
            ))
        );
    }
}
