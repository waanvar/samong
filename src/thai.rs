//! Custom tantivy tokenizer for mixed Thai/non-Thai text.
//!
//! Thai has no spaces between words, so the default tokenizer treats a whole
//! sentence as one term and mid-sentence search never matches. This tokenizer
//! scans the text for runs of Thai characters and segments those with the
//! newmm dictionary algorithm (nlpo3); everything else is split on
//! non-alphanumeric boundaries like tantivy's SimpleTokenizer.

use std::sync::OnceLock;

use nlpo3::tokenizer::newmm::NewmmTokenizer;
use nlpo3::tokenizer::tokenizer_trait::Tokenizer as ThaiSegmenter;
use tantivy::tokenizer::{Token, TokenStream, Tokenizer};

/// PyThaiNLP's newmm dictionary (62k words), embedded so the binary works
/// offline. Source: https://github.com/PyThaiNLP/pythainlp (Apache-2.0).
const DICT: &str = include_str!("../assets/words_th.txt");

fn newmm() -> &'static NewmmTokenizer {
    static INSTANCE: OnceLock<NewmmTokenizer> = OnceLock::new();
    INSTANCE.get_or_init(|| {
        let words: Vec<String> = DICT
            .lines()
            .map(str::trim)
            .filter(|w| !w.is_empty())
            .map(str::to_string)
            .collect();
        NewmmTokenizer::from_word_list(words)
    })
}

fn is_thai(c: char) -> bool {
    ('\u{0E00}'..='\u{0E7F}').contains(&c)
}

/// Split `text` into maximal runs of Thai / non-Thai characters,
/// returning `(byte_offset, run_slice, run_is_thai)`.
fn runs(text: &str) -> Vec<(usize, &str, bool)> {
    let mut out = Vec::new();
    let mut run_start = 0;
    let mut run_is_thai = None;
    for (idx, c) in text.char_indices() {
        let thai = is_thai(c);
        match run_is_thai {
            Some(current) if current == thai => {}
            Some(current) => {
                out.push((run_start, &text[run_start..idx], current));
                run_start = idx;
                run_is_thai = Some(thai);
            }
            None => run_is_thai = Some(thai),
        }
    }
    if let Some(current) = run_is_thai {
        out.push((run_start, &text[run_start..], current));
    }
    out
}

fn push_token(tokens: &mut Vec<Token>, position: &mut usize, from: usize, text: &str) {
    tokens.push(Token {
        offset_from: from,
        offset_to: from + text.len(),
        position: *position,
        text: text.to_string(),
        position_length: 1,
    });
    *position += 1;
}

fn tokenize(text: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut position = 0;
    for (run_start, run_text, run_is_thai) in runs(text) {
        if run_is_thai {
            // newmm partitions the run: segments concatenate back to the
            // original slice, so byte offsets accumulate exactly.
            let mut offset = run_start;
            for segment in newmm().segment_to_string(run_text, true, false) {
                if segment.chars().any(char::is_alphanumeric) {
                    push_token(&mut tokens, &mut position, offset, &segment);
                }
                offset += segment.len();
            }
        } else {
            // SimpleTokenizer-style: maximal alphanumeric words.
            let mut word_start = None;
            for (idx, c) in run_text.char_indices() {
                match (c.is_alphanumeric(), word_start) {
                    (true, None) => word_start = Some(idx),
                    (false, Some(start)) => {
                        push_token(
                            &mut tokens,
                            &mut position,
                            run_start + start,
                            &run_text[start..idx],
                        );
                        word_start = None;
                    }
                    _ => {}
                }
            }
            if let Some(start) = word_start {
                push_token(
                    &mut tokens,
                    &mut position,
                    run_start + start,
                    &run_text[start..],
                );
            }
        }
    }
    tokens
}

#[derive(Clone, Default)]
pub struct ThaiTokenizer;

pub struct PrecomputedTokenStream {
    tokens: Vec<Token>,
    /// 1-based index of the current token (0 = before the first).
    idx: usize,
}

impl Tokenizer for ThaiTokenizer {
    type TokenStream<'a> = PrecomputedTokenStream;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> PrecomputedTokenStream {
        PrecomputedTokenStream {
            tokens: tokenize(text),
            idx: 0,
        }
    }
}

impl TokenStream for PrecomputedTokenStream {
    fn advance(&mut self) -> bool {
        if self.idx < self.tokens.len() {
            self.idx += 1;
            true
        } else {
            false
        }
    }

    fn token(&self) -> &Token {
        &self.tokens[self.idx - 1]
    }

    fn token_mut(&mut self) -> &mut Token {
        &mut self.tokens[self.idx - 1]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token_texts(text: &str) -> Vec<String> {
        tokenize(text).into_iter().map(|t| t.text).collect()
    }

    #[test]
    fn segments_thai_sentence_into_dictionary_words() {
        let tokens = token_texts("ตลาดหลักทรัพย์แห่งประเทศไทย");
        assert!(
            tokens.contains(&"ตลาดหลักทรัพย์".to_string()),
            "expected dictionary word in {tokens:?}"
        );
        assert!(tokens.contains(&"ประเทศ".to_string()), "{tokens:?}");
        assert!(tokens.contains(&"ไทย".to_string()), "{tokens:?}");
    }

    #[test]
    fn handles_mixed_thai_and_english() {
        let tokens = token_texts("Rust คือภาษาเขียนโปรแกรม for systems!");
        assert!(tokens.contains(&"Rust".to_string()), "{tokens:?}");
        assert!(tokens.contains(&"คือ".to_string()), "{tokens:?}");
        assert!(tokens.contains(&"โปรแกรม".to_string()), "{tokens:?}");
        assert!(tokens.contains(&"systems".to_string()), "{tokens:?}");
    }

    #[test]
    fn offsets_slice_back_to_original_text() {
        let text = "โน้ต Samong รองรับภาษาไทย 100%";
        for token in tokenize(text) {
            assert_eq!(
                &text[token.offset_from..token.offset_to],
                token.text,
                "offset mismatch for {token:?}"
            );
        }
    }

    #[test]
    fn positions_strictly_increase() {
        let positions: Vec<usize> = tokenize("แมวกินปลา cat eats fish")
            .into_iter()
            .map(|t| t.position)
            .collect();
        let mut sorted = positions.clone();
        sorted.dedup();
        assert_eq!(positions, sorted, "duplicate positions");
        assert!(positions.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn whitespace_and_punctuation_produce_no_tokens() {
        assert!(tokenize("  ... !!! ").is_empty());
        assert!(tokenize("").is_empty());
    }
}
