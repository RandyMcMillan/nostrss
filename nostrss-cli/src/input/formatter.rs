pub struct InputFormatter {}

impl InputFormatter {
    pub fn input_to_vec(value: String) -> Vec<String> {
        value.split(',').map(|e| e.trim().to_string()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::InputFormatter;

    #[test]
    fn input_to_vec_test() {
        let value = "a,b,c".to_string();

        let result = InputFormatter::input_to_vec(value);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], "a".to_string());
    }
}
