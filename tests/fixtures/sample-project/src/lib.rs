/// Add two numbers.
pub fn add(a: i64, b: i64) -> i64 {
    a + b
}

/// Subtract b from a.
pub fn subtract(a: i64, b: i64) -> i64 {
    a - b
}

/// Multiply two numbers.
/// TODO: This is a stub — auto-dev agent should implement this.
pub fn multiply(_a: i64, _b: i64) -> i64 {
    todo!("implement multiply")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 3), 5);
        assert_eq!(add(-1, 1), 0);
        assert_eq!(add(0, 0), 0);
    }

    #[test]
    fn test_subtract() {
        assert_eq!(subtract(5, 3), 2);
        assert_eq!(subtract(0, 0), 0);
        assert_eq!(subtract(-1, -1), 0);
    }
}
