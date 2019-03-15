pub fn uppercase_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

pub fn modulus(numerator: u64, denominator: u64) - > (bool, u64, u64) {
    (numerator / denominator > 0, numerator / denominator, numerator % denominator,)
}

pub fn pad_left(num: u64, desired_length: usize) -> String {
    let mut padded = format!("{}", num);
    while padded.len() < desired_length {
        padded = format!("0{}", padded);
    }
    padded
}
