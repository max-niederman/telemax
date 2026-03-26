use input_linux::Key;

/// Map JavaScript `KeyboardEvent.key` names to Linux `input_linux::Key` codes.
///
/// Covers letters (a-z, case insensitive), digits 0-9, F1-F12, arrow keys,
/// common navigation, modifiers, punctuation, and whitespace keys.
pub fn js_key_to_linux(key: &str) -> Option<Key> {
    // Case-insensitive single-character letters
    match key {
        // Letters (both cases map to same Key)
        "a" | "A" => Some(Key::A),
        "b" | "B" => Some(Key::B),
        "c" | "C" => Some(Key::C),
        "d" | "D" => Some(Key::D),
        "e" | "E" => Some(Key::E),
        "f" | "F" => Some(Key::F),
        "g" | "G" => Some(Key::G),
        "h" | "H" => Some(Key::H),
        "i" | "I" => Some(Key::I),
        "j" | "J" => Some(Key::J),
        "k" | "K" => Some(Key::K),
        "l" | "L" => Some(Key::L),
        "m" | "M" => Some(Key::M),
        "n" | "N" => Some(Key::N),
        "o" | "O" => Some(Key::O),
        "p" | "P" => Some(Key::P),
        "q" | "Q" => Some(Key::Q),
        "r" | "R" => Some(Key::R),
        "s" | "S" => Some(Key::S),
        "t" | "T" => Some(Key::T),
        "u" | "U" => Some(Key::U),
        "v" | "V" => Some(Key::V),
        "w" | "W" => Some(Key::W),
        "x" | "X" => Some(Key::X),
        "y" | "Y" => Some(Key::Y),
        "z" | "Z" => Some(Key::Z),

        // Digits
        "0" => Some(Key::Num0),
        "1" => Some(Key::Num1),
        "2" => Some(Key::Num2),
        "3" => Some(Key::Num3),
        "4" => Some(Key::Num4),
        "5" => Some(Key::Num5),
        "6" => Some(Key::Num6),
        "7" => Some(Key::Num7),
        "8" => Some(Key::Num8),
        "9" => Some(Key::Num9),

        // Function keys
        "F1" => Some(Key::F1),
        "F2" => Some(Key::F2),
        "F3" => Some(Key::F3),
        "F4" => Some(Key::F4),
        "F5" => Some(Key::F5),
        "F6" => Some(Key::F6),
        "F7" => Some(Key::F7),
        "F8" => Some(Key::F8),
        "F9" => Some(Key::F9),
        "F10" => Some(Key::F10),
        "F11" => Some(Key::F11),
        "F12" => Some(Key::F12),

        // Arrow keys
        "ArrowUp" => Some(Key::Up),
        "ArrowDown" => Some(Key::Down),
        "ArrowLeft" => Some(Key::Left),
        "ArrowRight" => Some(Key::Right),

        // Common keys
        "Enter" | "Return" => Some(Key::Enter),
        "Escape" => Some(Key::Esc),
        "Backspace" | "BackSpace" => Some(Key::Backspace),
        "Tab" => Some(Key::Tab),
        " " | "space" | "Space" => Some(Key::Space),
        "Unidentified" => None,

        // Punctuation
        "-" => Some(Key::Minus),
        "=" => Some(Key::Equal),
        "[" => Some(Key::LeftBrace),
        "]" => Some(Key::RightBrace),
        "\\" => Some(Key::Backslash),
        ";" => Some(Key::Semicolon),
        "'" => Some(Key::Apostrophe),
        "`" => Some(Key::Grave),
        "," => Some(Key::Comma),
        "." => Some(Key::Dot),
        "/" => Some(Key::Slash),

        // Navigation
        "Home" => Some(Key::Home),
        "End" => Some(Key::End),
        "PageUp" => Some(Key::PageUp),
        "PageDown" => Some(Key::PageDown),
        "Insert" => Some(Key::Insert),
        "Delete" => Some(Key::Delete),

        // Modifiers
        "Control" => Some(Key::LeftCtrl),
        "Shift" => Some(Key::LeftShift),
        "Alt" => Some(Key::LeftAlt),
        "Meta" => Some(Key::LeftMeta),

        _ => None,
    }
}

/// Returns the key and whether shift is required for a given character
/// when typing text. This handles shifted characters like uppercase letters
/// and symbols that require the shift key on a US keyboard layout.
pub fn char_to_key_shifted(ch: char) -> Option<(Key, bool)> {
    match ch {
        'a'..='z' => {
            let key = match ch {
                'a' => Key::A,
                'b' => Key::B,
                'c' => Key::C,
                'd' => Key::D,
                'e' => Key::E,
                'f' => Key::F,
                'g' => Key::G,
                'h' => Key::H,
                'i' => Key::I,
                'j' => Key::J,
                'k' => Key::K,
                'l' => Key::L,
                'm' => Key::M,
                'n' => Key::N,
                'o' => Key::O,
                'p' => Key::P,
                'q' => Key::Q,
                'r' => Key::R,
                's' => Key::S,
                't' => Key::T,
                'u' => Key::U,
                'v' => Key::V,
                'w' => Key::W,
                'x' => Key::X,
                'y' => Key::Y,
                'z' => Key::Z,
                _ => unreachable!(),
            };
            Some((key, false))
        }
        'A'..='Z' => {
            let key = match ch {
                'A' => Key::A,
                'B' => Key::B,
                'C' => Key::C,
                'D' => Key::D,
                'E' => Key::E,
                'F' => Key::F,
                'G' => Key::G,
                'H' => Key::H,
                'I' => Key::I,
                'J' => Key::J,
                'K' => Key::K,
                'L' => Key::L,
                'M' => Key::M,
                'N' => Key::N,
                'O' => Key::O,
                'P' => Key::P,
                'Q' => Key::Q,
                'R' => Key::R,
                'S' => Key::S,
                'T' => Key::T,
                'U' => Key::U,
                'V' => Key::V,
                'W' => Key::W,
                'X' => Key::X,
                'Y' => Key::Y,
                'Z' => Key::Z,
                _ => unreachable!(),
            };
            Some((key, true))
        }
        '0' => Some((Key::Num0, false)),
        '1' => Some((Key::Num1, false)),
        '2' => Some((Key::Num2, false)),
        '3' => Some((Key::Num3, false)),
        '4' => Some((Key::Num4, false)),
        '5' => Some((Key::Num5, false)),
        '6' => Some((Key::Num6, false)),
        '7' => Some((Key::Num7, false)),
        '8' => Some((Key::Num8, false)),
        '9' => Some((Key::Num9, false)),

        // Unshifted punctuation
        '-' => Some((Key::Minus, false)),
        '=' => Some((Key::Equal, false)),
        '[' => Some((Key::LeftBrace, false)),
        ']' => Some((Key::RightBrace, false)),
        '\\' => Some((Key::Backslash, false)),
        ';' => Some((Key::Semicolon, false)),
        '\'' => Some((Key::Apostrophe, false)),
        '`' => Some((Key::Grave, false)),
        ',' => Some((Key::Comma, false)),
        '.' => Some((Key::Dot, false)),
        '/' => Some((Key::Slash, false)),
        ' ' => Some((Key::Space, false)),
        '\n' => Some((Key::Enter, false)),
        '\t' => Some((Key::Tab, false)),

        // Shifted punctuation (US keyboard layout)
        '!' => Some((Key::Num1, true)),
        '@' => Some((Key::Num2, true)),
        '#' => Some((Key::Num3, true)),
        '$' => Some((Key::Num4, true)),
        '%' => Some((Key::Num5, true)),
        '^' => Some((Key::Num6, true)),
        '&' => Some((Key::Num7, true)),
        '*' => Some((Key::Num8, true)),
        '(' => Some((Key::Num9, true)),
        ')' => Some((Key::Num0, true)),
        '_' => Some((Key::Minus, true)),
        '+' => Some((Key::Equal, true)),
        '{' => Some((Key::LeftBrace, true)),
        '}' => Some((Key::RightBrace, true)),
        '|' => Some((Key::Backslash, true)),
        ':' => Some((Key::Semicolon, true)),
        '"' => Some((Key::Apostrophe, true)),
        '~' => Some((Key::Grave, true)),
        '<' => Some((Key::Comma, true)),
        '>' => Some((Key::Dot, true)),
        '?' => Some((Key::Slash, true)),

        _ => None,
    }
}
