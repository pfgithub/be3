#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CValidatedIdentifierName(String);

impl CValidatedIdentifierName {
    pub fn new(name: String) -> Option<Self> {
        validate_c_name(&name).then_some(Self(name))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub fn validate_c_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}
