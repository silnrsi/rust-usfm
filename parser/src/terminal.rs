#![allow(dead_code)]
use super::Result;
use nom::{
    bytes::complete::{is_not, take_while1},
    character::{
        complete::{self as character, char, none_of, one_of},
        is_alphanumeric,
    },
    combinator::{eof, opt, recognize, value},
    error::context,
    multi::{many0_count, many1_count},
    sequence::{delimited, preceded},
    Parser,
};

#[inline]
pub(crate) fn bom(input: &str) -> Result<bool> {
    opt(char('\u{FEFF}')).map(|opt| opt.is_some()).parse(input)
}

#[inline] // NL
pub(crate) fn line_ending(input: &str) -> Result<&str> {
    value("\n", character::line_ending).parse(input)
}

#[inline] // NL
pub(crate) fn line_ending1(input: &str) -> Result<&str> {
    value("\n", many1_count(character::line_ending)).parse(input)
}

fn reduce_space(spaces: &str) -> &str {
    match spaces {
        "" => "",
        ws if ws.contains('\n') => "\n",
        _ => " ",
    }
}

#[inline] // NL
pub(crate) fn multispace0(input: &str) -> Result<&str> {
    character::multispace0.map(reduce_space).parse(input)
}

#[inline] // NL
pub(crate) fn multispace1(input: &str) -> Result<&str> {
    character::multispace1.map(reduce_space).parse(input)
}

#[inline] // NL
pub(crate) fn space0(input: &str) -> Result<&str> {
    character::space0.map(reduce_space).parse(input)
}

#[inline] // NL
pub(crate) fn space1(input: &str) -> Result<&str> {
    value(" ", character::space1).parse(input)
}

pub(crate) fn name(input: &str) -> Result<&str> {
    take_while1(|c| is_alphanumeric(c as u8) || c == '-' || c == '_').parse(input)
}

pub(crate) fn text(input: &str) -> Result<&str> {
    let escape_sequence = preceded(char('\\'), one_of(r#"/~\|"#));
    let non_break = preceded(char('/'), none_of("\\/"));
    let medial_line_ending = preceded(multispace1, none_of("\\/"));
    let specials = recognize(medial_line_ending.or(non_break).or(escape_sequence));
    let runs = is_not("\\/\r\n").or(specials);
    recognize(many0_count(runs)).or(eof).parse(input)
}

pub(crate) mod marker {
    use super::{multispace1, Result};
    use nom::{
        branch::alt,
        bytes::complete::{self as bytes},
        character::complete::{char, one_of},
        combinator::{eof, peek, recognize, value},
        error::context,
        sequence::delimited,
        Parser,
    };

    pub fn end(input: &str) -> Result<()> {
        let parser = value((), alt((multispace1, peek(recognize(one_of("\\|"))), eof)));
        context("end of tag name", parser).parse(input)
    }

    pub fn tag(id: &str) -> impl Fn(&str) -> Result<&str> + '_ {
        move |input| context("marker tag", delimited(char('\\'), bytes::tag(id), end)).parse(input)
    }
}

pub(crate) fn marker(input: &str) -> Result<&str> {
    context("marker", delimited(char('\\'), self::name, marker::end)).parse(input)
}

pub(crate) mod attrib {
    use super::Result;
    use nom::{
        character::complete::one_of,
        Parser,
    };

    fn text(input: &str) -> Result<&str> {
        escaped(none_of("\\ \t?"), '\\', one_of(r#""\=~/|"#)).parse(input)
    }
}

#[cfg(test)]
mod test {
    use super::{
        line_ending, line_ending1, marker, multispace0, multispace1, space0, space1, text,
    };

    use nom::{
        error::{
            ErrorKind::{Alt, CrLf, Eof, Many1Count, Space, Tag},
            VerboseError,
            VerboseErrorKind::{Char, Context, Nom},
        },
        Err,
    };

    const HS: &str = "\u{0009}\u{0020}";
    const NL: &str = "\u{000A}\u{000D}\u{000A}\u{000D}";
    // const SPACES: &str = "\u{09}\u{20}\u{A0}\u{1680}\u{2000}\u{2001}\u{2002}\u{2003}\u{2004}\u{2005}\u{2006}\u{2007}\u{2008}\u{2009}\u{200A}\u{200B}\u{2028}\u{2029}\u{202F}\u{205F}";
    // const MULTISPACES: &str = "\u{09}\u{0A}\u{0B}\u{0C}\u{0D}\u{20}\u{A0}\u{1680}\u{2000}\u{2001}\u{2002}\u{2003}\u{2004}\u{2005}\u{2006}\u{2007}\u{2008}\u{2009}\u{200A}\u{200B}\u{2028}\u{2029}\u{202F}\u{205F}";

    type Result<'i, O = &'i str> = super::Result<'i, O>;

    #[test]
    fn horizontal_space_terminals() {
        assert_eq!(space1("\t") as Result, Ok(("", " ")));
        assert_eq!(space1(" ") as Result, Ok(("", " ")));
        assert_eq!(space1(HS) as Result, Ok(("", " ")));
        assert_eq!(
            space1(NL),
            Err(Err::Error(VerboseError {
                errors: vec![(NL, Nom(Space))]
            }))
        );
        assert_eq!(space0(HS) as Result, Ok(("", " ")));
        assert_eq!(space0(NL) as Result, Ok((NL, "")));
    }

    #[test]
    fn any_whitespace_terminals() {
        assert_eq!(multispace0(HS) as Result, Ok(("", " ")));
        assert_eq!(multispace0(NL) as Result, Ok(("", "\n")));
        assert_eq!(multispace1(HS) as Result, Ok(("", " ")));
        assert_eq!(multispace1(NL) as Result, Ok(("", "\n")));
    }

    #[test]
    fn newline_terminals() {
        assert_eq!(line_ending("\u{000A}") as Result, Ok(("", "\n")));
        assert_eq!(line_ending("\u{000D}\u{000A}") as Result, Ok(("", "\n")));
        // TODO: Is carriage return really in the data, and does this case really need to work?
        // assert_eq!(line_ending("\u{000D}"), Ok(("", "\r")));
        assert_eq!(
            line_ending("\u{000D}"),
            Err(Err::Error(VerboseError {
                errors: vec![("\r", Nom(CrLf))]
            }))
        );
        assert_eq!(line_ending("\u{000A}\u{000D}") as Result, Ok(("\r", "\n")));
        assert_eq!(line_ending1(NL) as Result, Ok(("\r", "\n")));
        assert_eq!(line_ending1("\u{000A}\u{000D}") as Result, Ok(("\r", "\n")));
        assert_eq!(
            line_ending1("\u{000D}"),
            Err(Err::Error(VerboseError {
                errors: vec![("\r", Nom(Many1Count))]
            }))
        );
        assert_eq!(line_ending("\u{000A}\u{000D}") as Result, Ok(("\r", "\n")));
    }

    #[test]
    fn marker_parser() {
        assert_eq!(marker::tag("c")(r"\c 1"), Ok(("1", "c")));
        assert_eq!(marker::tag("c")(r"\c\v 1"), Ok(("\\v 1", "c")));
        assert_eq!(marker::tag("c")(r"\c|attrib=2"), Ok(("|attrib=2", "c")));
        assert_eq!(
            marker::tag("c")(r"\c!attrib=2"),
            Err(Err::Error(VerboseError {
                errors: vec![
                    ("!attrib=2", Nom(Eof)),
                    ("!attrib=2", Nom(Alt)),
                    ("!attrib=2", Context("end of tag name")),
                    ("\\c!attrib=2", Context("marker tag"))
                ]
            }))
        );
        assert_eq!(
            marker::tag("v")(r"\c 1"),
            Err(Err::Error(VerboseError {
                errors: vec![("c 1", Nom(Tag)), ("\\c 1", Context("marker tag"))]
            }))
        );
        assert_eq!(
            marker::tag("v")(r"v 1"),
            Err(Err::Error(VerboseError {
                errors: vec![("v 1", Char('\\')), ("v 1", Context("marker tag"))]
            }))
        );
    }

    #[test]
    fn text_parser() {
        assert_eq!(text("Some text") as Result, Ok(("", "Some text")));
        assert_eq!(text("Some // text") as Result, Ok(("// text", "Some ")));
        assert_eq!(text("Some text\\v 1") as Result, Ok(("\\v 1", "Some text")));
        assert_eq!(
            text("Some text   \r\n   \\v 1") as Result,
            Ok(("\r\n   \\v 1", "Some text   "))
        );
        assert_eq!(
            text(r#"Some text \\ \~ \/"#) as Result,
            Ok((r"", r#"Some text \\ \~ \/"#))
        );
    }

    // #[test]
    // fn end_marker_parser() {
    //     assert_eq!(endmarker("f")(r"\f* text") as Result, Ok((" text", "f")));
    //     assert_eq!(endmarker("f")(r"\f*text") as Result, Ok(("text", "f")));
    //     assert_eq!(endmarker("f")(r"\f*\v 1") as Result, Ok(("\\v 1", "f")));
    //     assert_eq!(
    //         endmarker("f")(r"\w\f*"),
    //         Err(Err::Error(VerboseError {
    //             errors: vec![
    //                 ("w\\f*", Nom(ErrorKind::Tag)),
    //                 ("\\w\\f*", Context("end tag"))
    //             ]
    //         }))
    //     );
    //     assert_eq!(
    //         endmarker("f")(r"f* text"),
    //         Err(Err::Error(VerboseError {
    //             errors: vec![("f* text", Char('\\')), ("f* text", Context("end tag"))]
    //         }))
    //     );

    //     // let inlinetag = |t| flat_map(marker(t), |opener| preceded(text, endmarker(opener)));
    //     // assert_eq!(inlinetag("f")(r"\f text\f*"), Ok(("", "f")));
    // }

    // #[test]
    // fn pmarker_parser() {
    //     assert_eq!(pmarker("c")("\n \t \\c 1") as Result, Ok(("1", "c")));
    //     assert_eq!(pmarker("c")("\r\n\t\\c\\v 1") as Result, Ok(("\\v 1", "c")));
    //     assert_eq!(
    //         pmarker("c")("\t\\c|attrib=2") as Result,
    //         Ok(("|attrib=2", "c"))
    //     );
    //     assert_eq!(
    //         pmarker("c")("\n\t\\c!attrib=2"),
    //         Err(Err::Error(VerboseError {
    //             errors: vec![
    //                 ("!attrib=2", Nom(ErrorKind::Eof)),
    //                 ("!attrib=2", Nom(ErrorKind::Alt)),
    //                 ("!attrib=2", Context("end of tag")),
    //                 ("\n\t\\c!attrib=2", Context("paragraph tag"))
    //             ]
    //         }))
    //     );
    //     use nom::{error::convert_error, Finish};
    //     let input = "\\c!attrib=2";
    //     let res = pmarker("c")(input);
    //     println!("parsed errors:\n{:#?}", res.clone().unwrap_err());
    //     println!(
    //         "verbose errors:\n{:#}",
    //         convert_error(input, res.finish().unwrap_err())
    //     );
    //     assert_eq!(
    //         pmarker("v")(" \n\t\\c 1"),
    //         Err(Err::Error(VerboseError {
    //             errors: vec![
    //                 ("c 1", Nom(ErrorKind::Tag)),
    //                 (" \n\t\\c 1", Context("paragraph tag"))
    //             ]
    //         }))
    //     );
    //     assert_eq!(
    //         pmarker("v")("\n\t v 1"),
    //         Err(Err::Error(VerboseError {
    //             errors: vec![("v 1", Char('\\')), ("\n\t v 1", Context("paragraph tag"))]
    //         }))
    //     );
    // }
}
