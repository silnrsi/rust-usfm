use nom::{error::VerboseError, IResult};

pub(crate) mod terminal;

type Result<'i, O> = IResult<&'i str, O, VerboseError<&'i str>>;