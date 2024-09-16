use std::{
    collections::HashMap,
    fmt::Display,
    io::{self, Read},
    ops::Deref,
};

use nom::{
    branch::{alt, permutation},
    bytes::complete::tag_no_case,
    character::complete::{char, not_line_ending},
    combinator::{cut, eof, iterator, opt, success, value},
    error::{context, convert_error, VerboseError},
    multi::separated_list1,
    sequence::{delimited, terminated},
    Finish, Parser,
};

use super::Result;
use crate::terminal;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Extensions(HashMap<String, Marker>);

impl Deref for Extensions {
    type Target = HashMap<String, Marker>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

type Attributes = HashMap<String, bool>;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Marker {
    pub name: String,
    pub attributes: Attributes,
    pub category: Category,
    pub closes: Option<String>,
    pub closedby: Option<String>,
    pub default: Option<String>,
    pub description: Option<String>,
}

impl Marker {
    fn update_from(&mut self, overrides: Marker) {
        assert_eq!(self.name, overrides.name);

        if overrides.closes.is_some() {
            self.closes = overrides.closes
        }
        if overrides.closedby.is_some() {
            self.closedby = overrides.closedby
        }
        if overrides.default.is_some() {
            self.default = overrides.default
        }
        if overrides.description.is_some() {
            self.description = overrides.description
        }
        self.attributes.extend(overrides.attributes.into_iter());
    }
}

impl Display for Marker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("\\marker ")
            .and(f.write_str(self.name.as_str()))?;
        if self.category != Category::Unknown {
            f.write_fmt(format_args!("\\category {}", self.category))?;
        }
        if !self.attributes.is_empty() {
            f.write_fmt(format_args!("\\category {}", self.category))?;
        }
        if let Some(ref close) = self.closes {
            f.write_fmt(format_args!("\\closes {close}"))?;
        }
        if let Some(ref closedby) = self.closedby {
            f.write_fmt(format_args!("\\closedby {closedby}"))?;
        }
        if let Some(ref defattrib) = self.default {
            f.write_fmt(format_args!("\\defattrib {defattrib}"))?;
        }
        if let Some(ref description) = self.description {
            f.write_fmt(format_args!("\\description {description}"))?;
        }
        Ok(())
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Cell,
    Char,
    Crossreference,
    CrossreferenceChar,
    Footnote,
    FootnoteChar,
    Header,
    Internal,
    IntroChar,
    Introduction,
    List,
    ListChar,
    Milestone,
    OtherPara,
    SectionPara,
    Title,
    VersePara,
    #[default]
    Unknown,
}

impl Display for Category {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(format!("{:?}", self).to_lowercase().as_str())
    }
}

fn category(input: &str) -> Result<Category> {
    let parser = alt((
        value(Category::Cell, tag_no_case("cell")),
        value(Category::VersePara, tag_no_case("versepara")),
        value(Category::Char, tag_no_case("char")),
        value(Category::Introduction, tag_no_case("introduction")),
        value(Category::SectionPara, tag_no_case("sectionpara")),
        value(Category::Milestone, tag_no_case("milestone")),
        value(Category::Internal, tag_no_case("internal")),
        value(Category::ListChar, tag_no_case("listchar")),
        value(Category::List, tag_no_case("list")),
        value(Category::Header, tag_no_case("header")),
        value(
            Category::CrossreferenceChar,
            tag_no_case("crossreferencechar"),
        ),
        value(Category::FootnoteChar, tag_no_case("footnotechar")),
        value(Category::OtherPara, tag_no_case("otherpara")),
        value(Category::Footnote, tag_no_case("footnote")),
        value(Category::Title, tag_no_case("title")),
        value(Category::Crossreference, tag_no_case("crossreference")),
        value(Category::IntroChar, tag_no_case("introchar")),
    ));
    context("Category", parser).parse(input)
}

fn field<'a, 'i: 'a, O, F>(id: &'a str, mut value: F) -> impl FnMut(&'i str) -> Result<O> + '_
where
    F: Parser<&'i str, O, VerboseError<&'i str>> + 'i,
{
    move |input| {
        let parser = delimited(
            terminal::marker::tag(id).and(terminal::space0),
            |input| value.parse(input),
            terminal::line_ending.or(eof),
        );
        context("record field", parser).parse(input)
    }
}

fn record(input: &str) -> Result<Marker> {
    let attribute = |input| {
        terminal::name
            .and(opt(char('?')).map(|o| o.is_some()))
            .parse(input)
    };
    let attributes = separated_list1(terminal::space1, attribute);
    let (input, name) = field("marker", terminal::name).parse(input)?;
    cut(terminated(
        permutation((
            opt(field("attributes", attributes)),
            field("category", category).or(success(Category::Unknown)),
            opt(field("closes", terminal::marker::name)),
            opt(field("closedby", terminal::marker::name)),
            opt(field("defattrib", terminal::attrib::name)),
            opt(field("description", not_line_ending)),
        )),
        terminal::line_ending1.or(eof),
    ))
    .map(|field| Marker {
        name: name.to_owned(),
        attributes: Attributes::from_iter(
            field
                .0
                .unwrap_or_default()
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v)),
        ),
        category: field.1,
        closes: field.2.map(str::to_owned),
        closedby: field.3.map(str::to_owned),
        default: field.4.map(str::to_owned),
        description: field.5.map(str::to_owned),
    })
    .parse(input)
}

impl Extensions {
    pub fn from_reader<R: Read>(reader: R) -> io::Result<Self> {
        let input = io::read_to_string(reader)?;
        let input = input.trim();
        let mut it = iterator(input, record);
        let parsed: Extensions = Extensions(it.map(|m| (m.name.clone(), m)).collect());
        it.finish()
            .finish()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, convert_error(input, e)))?;
        Ok(parsed)
    }

    pub fn update_from_str(mut self, input: impl AsRef<str>) -> io::Result<Self> {
        let input = input.as_ref().trim();
        let mut it = iterator(input, record);
        for m in it.into_iter() {
            self.0
                .entry(m.name.clone())
                .and_modify(|e| e.update_from(m.clone()))
                .or_insert(m);
        }
        it.finish()
            .finish()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, convert_error(input, e)))?;
        Ok(self)
    }

    pub fn update_from_reader<R: Read>(self, reader: R) -> io::Result<Self> {
        self.update_from_str(io::read_to_string(reader)?)
    }
}

impl From<&str> for Extensions {
    fn from(value: &str) -> Self {
        Extensions::default()
            .update_from_str(value)
            .expect("struct Extensions")
    }
}

#[cfg(test)]
mod tests {
    use nom::{
        character::complete::not_line_ending,
        error::{context, VerboseError},
        IResult,
    };

    use super::{field, record, Category, Extensions, Marker};

    type Result<'i, O = &'i str> = IResult<&'i str, O, VerboseError<&'i str>>;

    #[test]
    fn parse_field_combinator() {
        let mut parser = field("test", context("rest of line", not_line_ending));
        const OK: Result = Ok(("", "value"));

        assert_eq!(parser("\\test value"), OK);
        assert_eq!(parser("\\test value\n"), OK);
        assert_eq!(parser("\\test  value\n"), OK);
        assert_eq!(parser("\\test\tvalue\n"), OK);
        assert_eq!(parser("\\test\n"), Ok(("", "")));
        assert_eq!(parser("\\test "), Ok(("", "")));
        assert_eq!(parser("\\test"), Ok(("", "")));
    }

    #[test]
    fn parse_record() {
        assert_eq!(
            record("\\marker test\n\\category internal\n") as Result<Marker>,
            Ok((
                "",
                Marker {
                    name: "test".into(),
                    attributes: Default::default(),
                    category: Category::Internal,
                    closes: None,
                    closedby: None,
                    default: None,
                    description: None
                }
            ))
        );
        assert_eq!(
            record(
                "\\marker test\n\
                 \\category internal\n\
                 \\description A testing marker"
            ) as Result<Marker>,
            Ok((
                "",
                Marker {
                    name: "test".into(),
                    attributes: Default::default(),
                    category: Category::Internal,
                    closes: None,
                    closedby: None,
                    default: None,
                    description: Some("A testing marker".into())
                }
            ))
        );
        assert_eq!(
            record(
                "\\marker test\n\
                 \\category listchar\n\
                 \\defattrib gloss\n\
                 \\description A testing marker"
            ) as Result<Marker>,
            Ok((
                "",
                Marker {
                    name: "test".into(),
                    attributes: Default::default(),
                    category: Category::ListChar,
                    closes: None,
                    closedby: None,
                    default: Some("gloss".into()),
                    description: Some("A testing marker".into())
                }
            ))
        );
        assert_eq!(
            record(
                "\\marker       test\n\
                 \\attributes   gloss matte? oil?\n\
                 \\category     internal\n\
                 \\defattrib    gloss\n\
                 \\description  A testing marker"
            ) as Result<Marker>,
            Ok((
                "",
                Marker {
                    name: "test".into(),
                    attributes: [
                        ("gloss".into(), false),
                        ("oil".into(), true),
                        ("matte".into(), true)
                    ]
                    .into(),
                    category: Category::Internal,
                    closes: None,
                    closedby: None,
                    default: Some("gloss".into()),
                    description: Some("A testing marker".into())
                }
            ))
        );
    }

    #[test]
    fn parse_records() {
        let test = r#"

\marker it
\category char
\description A character style, use italic text


\marker jmp
\attributes href? link-href?
\category char
\defattrib href
\description For associating linking attributes to a span of text

\marker k
\category char
\defattrib key
\description For a keyword

\marker k1
\category otherpara
\description Concordance main entry text or keyword, level 1"#;
        assert_eq!(
            Extensions::from_reader(test.as_bytes()).expect("Extensions"),
            Extensions(
                [
                    (
                        "it".into(),
                        Marker {
                            name: "it".into(),
                            attributes: [].into(),
                            category: Category::Char,
                            closes: None,
                            closedby: None,
                            default: None,
                            description: Some("A character style, use italic text".into())
                        }
                    ),
                    (
                        "jmp".into(),
                        Marker {
                            name: "jmp".into(),
                            attributes: [("href".into(), true), ("link-href".into(), true)].into(),
                            category: Category::Char,
                            closes: None,
                            closedby: None,
                            default: Some("href".into()),
                            description: Some(
                                "For associating linking attributes to a span of text".into()
                            )
                        }
                    ),
                    (
                        "k".into(),
                        Marker {
                            name: "k".into(),
                            attributes: [].into(),
                            category: Category::Char,
                            closes: None,
                            closedby: None,
                            default: Some("key".into()),
                            description: Some("For a keyword".into())
                        }
                    ),
                    (
                        "k1".into(),
                        Marker {
                            name: "k1".into(),
                            attributes: [].into(),
                            category: Category::OtherPara,
                            closes: None,
                            closedby: None,
                            default: None,
                            description: Some(
                                "Concordance main entry text or keyword, level 1".into()
                            )
                        }
                    )
                ]
                .into()
            )
        )
    }
}
