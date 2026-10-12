//! Crockford base32 room codes — compatible with lolPing `roomCode.ts`.

pub const ROOM_ALPHABET: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const PREFIX: &str = "P1NG";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedRoomCode {
    Ok { code: String },
    Err { error: &'static str },
}

pub fn check_char(nine: &str) -> char {
    let mut factor = 2i32;
    let mut sum = 0i32;
    for ch in nine.chars().rev() {
        let idx = ROOM_ALPHABET
            .iter()
            .position(|&c| c == ch as u8)
            .unwrap_or(0) as i32;
        let mut addend = factor * idx;
        factor = if factor == 2 { 1 } else { 2 };
        addend = addend / 32 + (addend % 32);
        sum += addend;
    }
    ROOM_ALPHABET[((32 - (sum % 32)) % 32) as usize] as char
}

pub fn make_room_code(random9: &[u8; 9]) -> String {
    let mut nine = String::new();
    for b in random9 {
        nine.push(ROOM_ALPHABET[(*b as usize) & 31] as char);
    }
    let check = check_char(&nine);
    nine.push(check);
    nine
}

pub fn format_room_code(canonical: &str) -> String {
    format!("PING-{}-{}", &canonical[..5], &canonical[5..])
}

fn map_lookalikes(t: &str) -> String {
    t.to_uppercase()
        .chars()
        .map(|c| match c {
            'O' => '0',
            'I' | 'L' => '1',
            x => x,
        })
        .collect()
}

fn is_code_shaped(c: &str) -> bool {
    c.len() == 10 && c.chars().all(|ch| ROOM_ALPHABET.contains(&(ch as u8)))
}

pub fn parse_room_code(text: &str) -> ParsedRoomCode {
    let cleaned = map_lookalikes(text)
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>();

    // PING / P1NG prefix
    for prefix in ["PING", "P1NG"] {
        if let Some(rest) = cleaned.strip_prefix(prefix) {
            if is_code_shaped(rest) {
                if check_char(&rest[..9]) == rest.chars().nth(9).unwrap() {
                    return ParsedRoomCode::Ok {
                        code: rest.to_string(),
                    };
                }
                return ParsedRoomCode::Err { error: "mistyped" };
            }
        }
    }

    if is_code_shaped(&cleaned) {
        if check_char(&cleaned[..9]) == cleaned.chars().nth(9).unwrap() {
            return ParsedRoomCode::Ok { code: cleaned };
        }
        return ParsedRoomCode::Err { error: "mistyped" };
    }

    // Display form already stripped of dashes in cleaned — try last 10 of PING...
    if cleaned.len() >= 10 {
        let rest = &cleaned[cleaned.len() - 10..];
        if is_code_shaped(rest) && check_char(&rest[..9]) == rest.chars().nth(9).unwrap() {
            return ParsedRoomCode::Ok {
                code: rest.to_string(),
            };
        }
    }

    let _ = PREFIX;
    ParsedRoomCode::Err { error: "invalid" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn make_and_parse() {
        let code = make_room_code(&[1, 2, 3, 4, 5, 6, 7, 8, 9]);
        assert_eq!(code.len(), 10);
        let disp = format_room_code(&code);
        assert!(disp.starts_with("PING-"));
        match parse_room_code(&disp) {
            ParsedRoomCode::Ok { code: c } => assert_eq!(c, code),
            ParsedRoomCode::Err { error } => panic!("{error}"),
        }
    }
}
