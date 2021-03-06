// This file is part of sv_check and subject to the terms of MIT Licence
// Copyright (c) 2019, clams@mail.com

use crate::lex::position::Position;

use std::{fs,path,io, mem, str, iter};

#[cfg(not(target_os = "windows"))]
pub fn path_display<P: AsRef<path::Path>>(p: P) -> String {
    p.as_ref().display().to_string()
}

#[cfg(target_os = "windows")]
pub fn path_display<P: AsRef<path::Path>>(p: P) -> String {
    const VERBATIM_PREFIX: &str = r#"\\?\"#;
    let p = p.as_ref().display().to_string();
    if p.starts_with(VERBATIM_PREFIX) {
        p[VERBATIM_PREFIX.len()..].to_string()
    } else {
        p
    }
}

/// Structure holding source code to parse with function to read char by char
///  and keeping information on current position in line/column.
#[derive(Debug, Clone)]
pub struct Source {
    /// filename used to initialize the code
    pub filename : path::PathBuf,
    /// String representing the source code to analyze
    _code : String,
    /// Current position in the code
    pub pos : Position,
    // // Character iterator
    chars : iter::Peekable<str::Chars<'static>>
}

impl Source {

    /// Create a Source struct from a file.
    /// Return an io error if unable to open the file
    pub fn from_file(filename: path::PathBuf) -> Result<Source,io::Error>  {
        let _code = fs::read_to_string(&filename)?;
        let chars = unsafe { mem::transmute(_code.chars().peekable()) };
        let pos = Position::new();
        Ok(Source {filename, _code, pos, chars})
    }

    pub fn get_char(&mut self) -> Option<char> {
        let c = self.chars.next()?;
        self.pos.incr(c);
        // println!("Parse_ident: char={} at {:?}", c,self.pos );
        Some(c)
    }

    pub fn peek_char(&mut self) -> Option<&char> {
        self.chars.peek()
    }

    pub fn get_filename(&self) -> String {
        path_display(&self.filename)
    }
}
