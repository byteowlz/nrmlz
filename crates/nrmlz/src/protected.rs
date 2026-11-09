//! Collision-free opaque words for final lexicon replacements and language spans. Tokens use uncased
//! letters so number/letter/symbol rules leave them alone. Restore once, never recursively.

use std::collections::HashSet;

pub(crate) struct ProtectedText {
    sentinel: String,
    values: Vec<String>,
}

impl ProtectedText {
    pub fn new(input: &str) -> Self {
        let used: HashSet<char> = input.chars().collect();
        // Frame and ID alphabets are disjoint. If all candidate letters occur, lengthen the frame
        // until it is absent; even adversarial text cannot collide with a generated token.
        let mut sentinel = (0xA000..=0xA48C)
            .chain(0x3400..=0x4DBF)
            .filter_map(char::from_u32)
            .find(|c| !used.contains(c))
            .unwrap_or('\u{A000}')
            .to_string();
        while input.contains(&sentinel) {
            sentinel.push('\u{A000}');
        }
        Self {
            sentinel,
            values: Vec::new(),
        }
    }

    pub fn insert(&mut self, value: String) -> String {
        let mut n = self.values.len();
        self.values.push(value);
        let mut code = String::new();
        loop {
            code.push(char::from_u32(0x5000 + (n % 4096) as u32).unwrap());
            n /= 4096;
            if n == 0 {
                break;
            }
        }
        format!("{}{code}{}", self.sentinel, self.sentinel)
    }

    pub fn restore(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(start) = rest.find(&self.sentinel) {
            out.push_str(&rest[..start]);
            rest = &rest[start + self.sentinel.len()..];
            let Some(end) = rest.find(&self.sentinel) else {
                out.push_str(&self.sentinel);
                out.push_str(rest);
                return out;
            };
            let id = rest[..end].chars().rev().try_fold(0usize, |n, c| {
                let d = (c as u32).checked_sub(0x5000)?;
                if d >= 4096 {
                    return None;
                }
                n.checked_mul(4096)?.checked_add(d as usize)
            });
            if let Some(value) = id.and_then(|i| self.values.get(i)) {
                out.push_str(value);
            } else {
                out.push_str(&self.sentinel);
                out.push_str(&rest[..end]);
                out.push_str(&self.sentinel);
            }
            rest = &rest[end + self.sentinel.len()..];
        }
        out.push_str(rest);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn collisions_and_single_restore() {
        let mut p = ProtectedText::new("ꀀ 漢字");
        let token = p.insert("Okay 7 PDF".into());
        assert_ne!(p.sentinel, "ꀀ");
        let second = p.insert(token.clone());
        assert_eq!(p.restore(&second), token);
        assert_eq!(p.restore(&format!("ꀀ {token}")), "ꀀ Okay 7 PDF");
    }
    #[test]
    fn many_replacements_and_exhausted_frame_alphabet() {
        let input: String = (0xA000..=0xA48C)
            .chain(0x3400..=0x4DBF)
            .filter_map(char::from_u32)
            .collect();
        let mut p = ProtectedText::new(&input);
        for n in 0..5000 {
            let token = p.insert(n.to_string());
            assert_eq!(p.restore(&token), n.to_string());
        }
    }
}
