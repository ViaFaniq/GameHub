use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum Vdf {
    Str(String),
    Obj(HashMap<String, Vdf>),
}

impl Vdf {
    pub fn get(&self, key: &str) -> Option<&Vdf> {
        match self {
            Vdf::Obj(map) => map.get(key),
            Vdf::Str(_) => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Vdf::Str(s) => Some(s),
            Vdf::Obj(_) => None,
        }
    }

    pub fn entries(&self) -> Option<&HashMap<String, Vdf>> {
        match self {
            Vdf::Obj(map) => Some(map),
            Vdf::Str(_) => None,
        }
    }
}

struct Parser<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
}

impl<'a> Parser<'a> {
    fn skip_ws(&mut self) {
        while let Some(&c) = self.chars.peek() {
            if c.is_whitespace() {
                self.chars.next();
            } else {
                break;
            }
        }
    }

    fn read_string(&mut self) -> Option<String> {
        self.skip_ws();
        if self.chars.peek() != Some(&'"') {
            return None;
        }
        self.chars.next();
        let mut out = String::new();
        while let Some(c) = self.chars.next() {
            match c {
                '\\' => match self.chars.next() {
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some(other) => out.push(other),
                    None => break,
                },
                '"' => return Some(out),
                _ => out.push(c),
            }
        }
        Some(out)
    }

    fn parse_entries(&mut self) -> HashMap<String, Vdf> {
        let mut map = HashMap::new();
        loop {
            self.skip_ws();
            match self.chars.peek() {
                Some('}') => {
                    self.chars.next();
                    break;
                }
                Some('"') => {
                    let key = match self.read_string() {
                        Some(k) => k,
                        None => break,
                    };
                    self.skip_ws();
                    let value = if self.chars.peek() == Some(&'{') {
                        self.chars.next();
                        Vdf::Obj(self.parse_entries())
                    } else {
                        match self.read_string() {
                            Some(v) => Vdf::Str(v),
                            None => break,
                        }
                    };
                    map.insert(key, value);
                }
                Some(_) => {
                    self.chars.next();
                }
                None => break,
            }
        }
        map
    }
}

pub fn parse(text: &str) -> HashMap<String, Vdf> {
    let mut parser = Parser {
        chars: text.chars().peekable(),
    };
    parser.parse_entries()
}
