//! The rows the index is made of.

/// What a symbol row is: something defined, something referenced, or a string literal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    Definition,
    Reference,
    Literal,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Definition => "definition",
            Role::Reference => "reference",
            Role::Literal => "literal",
        }
    }

    pub fn parse(text: &str) -> Option<Role> {
        match text {
            "definition" => Some(Role::Definition),
            "reference" => Some(Role::Reference),
            "literal" => Some(Role::Literal),
            _ => None,
        }
    }
}

/// One declaration, reference or literal inside a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub name: String,
    pub role: Role,
    pub kind: String,
    pub line: u32,
    /// The first and last line of the definition node the grammar captured (annotations and doc
    /// comments the grammar puts in it included); `line` for everything else.
    pub start_line: u32,
    pub end_line: u32,
    pub flags: Vec<String>,
    pub supertypes: Vec<String>,
}

/// A symbol located in a repository: the unit every query returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub repo: String,
    pub path: String,
    pub scope: String,
    pub language: String,
    pub symbol: Symbol,
}
