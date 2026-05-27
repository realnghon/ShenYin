const LINE_WIDTH: usize = 76;

#[derive(Debug)]
pub struct TextError;

pub fn encode_wrapped(raw_bytes: &[u8]) -> Result<String, TextError> {
    let encoded = base91::encode(raw_bytes);
    let encoded = std::str::from_utf8(&encoded).map_err(|_| TextError)?;
    Ok(wrap_lines(encoded))
}

pub fn decode_relaxed(text: &str) -> Result<Vec<u8>, TextError> {
    let normalized = text
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    Ok(base91::decode(normalized.as_bytes()))
}

fn wrap_lines(encoded: &str) -> String {
    if encoded.len() <= LINE_WIDTH {
        return encoded.to_owned();
    }

    encoded
        .as_bytes()
        .chunks(LINE_WIDTH)
        .map(|chunk| std::str::from_utf8(chunk).expect("basE91 output is ASCII"))
        .collect::<Vec<_>>()
        .join("\n")
}
