use std::fmt::Display;

#[derive(Debug, PartialEq)]
pub struct ParseError<'a> {
    error: String,
    text: &'a str,
    span: &'a str,
    next: Option<Box<ParseError<'a>>>,
}

impl Display for ParseError<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let lineno = find_line_number(self.span, self.text);
        let line = self.text.lines().nth(lineno - 1).unwrap_or("<EOF>");
        let column = self.span.as_ptr() as usize - line.as_ptr() as usize;
        write!(f, "Error: {}\n", self.error)?;
        write!(f, "     |\n")?;
        write!(f, "{:4} | {}\n", lineno, line)?;
        if column == 0 {
            write!(f, "     |\n")?;
        } else {
            write!(f, "     | {0:1$}^\n", "", column)?;
        }
        if let Some(next) = &self.next {
            write!(f, "{}", next)?;
        }
        Ok(())
    }
}

impl std::error::Error for ParseError<'_> {}

impl<'a> ParseError<'a> {
    pub fn new(
        error: String,
        text: &'a str,
        span: &'a str,
        next: Option<Box<ParseError<'a>>>,
    ) -> ParseError<'a> {
        ParseError {
            error,
            text,
            span,
            next,
        }
    }
}

fn find_line_number(pos: &str, whole: &str) -> usize {
    let pos = pos.as_ptr() as usize - whole.as_ptr() as usize;
    whole[..pos].chars().filter(|&c| c == '\n').count() + 1
}
