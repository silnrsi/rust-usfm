use nom::{error::VerboseError, IResult};

pub mod extension;
pub(crate) mod terminal;

type Result<'i, O> = IResult<&'i str, O, VerboseError<&'i str>>;
