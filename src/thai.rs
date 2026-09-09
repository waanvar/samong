//! Custom tantivy tokenizer for mixed Thai/non-Thai text.
//!
//! Thai has no spaces between words, so the default tokenizer treats a whole
//! sentence as one term and mid-sentence search never matches. This tokenizer
//! scans the text for runs of Thai characters and segments those with the
//! newmm dictionary algorithm (nlpo3); everything else is split on
//! non-alphanumeric boundaries like tantivy's SimpleTokenizer, and then again
//! inside each word at case and letter/digit boundaries.
//!
//! # Why a vault gets a say in the dictionary
//!
//! The bundled dictionary knows Thai, not your employer. A team's own words —
//! `LHVendor`, `สมองกล`, the project codename nobody spells out — are exactly
//! the ones people search for, and exactly the ones a general dictionary has
//! never heard of. `[search] words` in `samong.toml` adds them, and travels with
//! the vault because `samong pack` copies that file.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use nlpo3::tokenizer::newmm::NewmmTokenizer;
use nlpo3::tokenizer::tokenizer_trait::Tokenizer as ThaiSegmenter;
use tantivy::tokenizer::{Token, TokenStream, Tokenizer};

/// PyThaiNLP's newmm dictionary (62k words), embedded so the binary works
/// offline. Source: https://github.com/PyThaiNLP/pythainlp (Apache-2.0).
const DICT: &str = include_str!("../assets/words_th.txt");

/// Identity of a word list, so an index can tell whether it was built with the
/// same one that is now being used to query it.
///
/// This is the whole reason the hash exists rather than just the words: change
/// the dictionary and every document already in the index was segmented by the
/// old one. Queries would be segmented by the new one, and search would return
/// wrong results while reporting nothing at all — the failure this project has
/// been bitten by before. [`crate::indexer`] compares this against the value
/// stored with the index and rebuilds when they differ.
pub fn dictionary_hash(words: &[String]) -> u64 {
    let mut hasher = blake3::Hasher::new();
    // Sorted and deduplicated first: reordering the list in samong.toml changes
    // nothing about how text is segmented, so it must not force a rebuild.
    let mut normalized: Vec<&str> = words.iter().map(String::as_str).collect();
    normalized.sort_unstable();
    normalized.dedup();
    for word in normalized {
        hasher.update(word.as_bytes());
        hasher.update(b"\n");
    }
    // 64 bits of blake3, because the value is stored in the same redb table as
    // the index version and that table holds u64. A collision would mean an
    // index silently kept across a dictionary change; at 2^-64 per pair that is
    // not the risk worth adding a table for.
    let digest = hasher.finalize();
    u64::from_le_bytes(
        digest.as_bytes()[..8]
            .try_into()
            .expect("blake3 is 32 bytes"),
    )
}

fn base_words() -> &'static Vec<String> {
    static INSTANCE: OnceLock<Vec<String>> = OnceLock::new();
    INSTANCE.get_or_init(|| {
        DICT.lines()
            .map(str::trim)
            .filter(|w| !w.is_empty())
            .map(str::to_string)
            .collect()
    })
}

/// Segmenters built so far, keyed by [`dictionary_hash`] of the extra words.
///
/// Building one means handing nlpo3 62k words and letting it construct its
/// trie, which is far too slow to repeat per query — and search opens an index
/// on every call. Keyed by hash rather than by vault because two vaults that
/// declare the same words can share one.
fn segmenter_for(words: &[String]) -> Arc<NewmmTokenizer> {
    static CACHE: OnceLock<Mutex<HashMap<u64, Arc<NewmmTokenizer>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = dictionary_hash(words);

    let mut guard = cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(existing) = guard.get(&key) {
        return Arc::clone(existing);
    }
    let mut all = base_words().clone();
    all.extend(words.iter().cloned());
    let built = Arc::new(NewmmTokenizer::from_word_list(all));
    guard.insert(key, Arc::clone(&built));
    built
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

/// Where a run of letters and digits should also be cut into smaller words.
///
/// `LHVendor` is one token to any tokenizer that only splits on punctuation, so
/// searching `vendor` does not find the note titled `LHVendor` — measured on a
/// real vault, where the irrelevant note that happened to spell "vendor" out
/// ranked above it. Team vocabulary is full of these: `IfxDriver`, `getUserById`,
/// `LH2291`.
///
/// The cuts are the conservative ones, the same shape Lucene's word-delimiter
/// filter uses: lower-to-upper (`getUser` -> `get`, `User`), the last capital of
/// a run before a capitalised word (`JDBCDriver` -> `JDBC`, `Driver`), and either
/// direction across a letter/digit boundary (`LH2291` -> `LH`, `2291`).
fn sub_words(text: &str) -> Vec<(usize, &str)> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    if chars.len() < 2 {
        return Vec::new();
    }
    let mut cuts = Vec::new();
    for window in chars.windows(2) {
        let (_, previous) = window[0];
        let (index, current) = window[1];
        let boundary = (previous.is_lowercase() && current.is_uppercase())
            || (previous.is_numeric() != current.is_numeric());
        if boundary {
            cuts.push(index);
        }
    }
    // ACRONYMFollowed by a word: cut before the last capital, not after it, so
    // `JDBCDriver` yields `JDBC` and `Driver` rather than `JDBCD` and `river`.
    for window in chars.windows(3) {
        let (_, first) = window[0];
        let (index, second) = window[1];
        let (_, third) = window[2];
        if first.is_uppercase() && second.is_uppercase() && third.is_lowercase() {
            cuts.push(index);
        }
    }
    cuts.sort_unstable();
    cuts.dedup();
    if cuts.is_empty() {
        return Vec::new();
    }

    let mut pieces = Vec::new();
    let mut start = 0;
    for cut in cuts {
        if cut > start {
            pieces.push((start, &text[start..cut]));
        }
        start = cut;
    }
    if start < text.len() {
        pieces.push((start, &text[start..]));
    }
    // One piece means the cuts found nothing the whole word did not already say.
    if pieces.len() < 2 {
        return Vec::new();
    }
    pieces
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

/// The whole word, then the smaller words inside it — all at one position.
///
/// The original is kept because searching `LHVendor` must still find it, and it
/// comes first because it is the term the writer actually typed. The pieces share
/// the original's position rather than taking their own: they are alternatives at
/// one place in the sentence, not extra words in it, and giving them positions of
/// their own would make phrase queries across the surrounding text silently stop
/// matching.
fn push_word(tokens: &mut Vec<Token>, position: &mut usize, from: usize, text: &str) {
    let at = *position;
    push_token(tokens, position, from, text);
    for (offset, piece) in sub_words(text) {
        tokens.push(Token {
            offset_from: from + offset,
            offset_to: from + offset + piece.len(),
            position: at,
            text: piece.to_string(),
            position_length: 1,
        });
    }
}

fn tokenize(text: &str, segmenter: &NewmmTokenizer) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut position = 0;
    for (run_start, run_text, run_is_thai) in runs(text) {
        if run_is_thai {
            // newmm partitions the run: segments concatenate back to the
            // original slice, so byte offsets accumulate exactly.
            let mut offset = run_start;
            for segment in segmenter.segment_to_string(run_text, true, false) {
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
                        push_word(
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
                push_word(
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

/// The tokenizer, carrying the word list its vault asked for.
///
/// Not a unit struct any more: two vaults can declare different words, and a
/// process that searches both must segment each with its own. Cloning is cheap —
/// the segmenter behind the `Arc` is shared and built once per distinct word
/// list.
#[derive(Clone)]
pub struct ThaiTokenizer {
    segmenter: Arc<NewmmTokenizer>,
}

impl ThaiTokenizer {
    /// A tokenizer that knows the bundled dictionary plus `words`.
    pub fn with_words(words: &[String]) -> Self {
        Self {
            segmenter: segmenter_for(words),
        }
    }
}

impl Default for ThaiTokenizer {
    fn default() -> Self {
        Self::with_words(&[])
    }
}

pub struct PrecomputedTokenStream {
    tokens: Vec<Token>,
    /// 1-based index of the current token (0 = before the first).
    idx: usize,
}

impl Tokenizer for ThaiTokenizer {
    type TokenStream<'a> = PrecomputedTokenStream;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> PrecomputedTokenStream {
        PrecomputedTokenStream {
            tokens: tokenize(text, &self.segmenter),
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

    fn tokens_of(text: &str) -> Vec<Token> {
        tokenize(text, &segmenter_for(&[]))
    }

    fn token_texts(text: &str) -> Vec<String> {
        tokens_of(text).into_iter().map(|t| t.text).collect()
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
        for token in tokens_of(text) {
            assert_eq!(
                &text[token.offset_from..token.offset_to],
                token.text,
                "offset mismatch for {token:?}"
            );
        }
    }

    /// Positions may repeat now — the pieces of a split word share the whole
    /// word's position — but they must never go backwards, or tantivy's phrase
    /// queries read the sentence out of order.
    #[test]
    fn positions_never_go_backwards() {
        let positions: Vec<usize> = tokens_of("แมวกินปลา cat eats fish LHVendor")
            .into_iter()
            .map(|t| t.position)
            .collect();
        assert!(
            positions.windows(2).all(|w| w[0] <= w[1]),
            "positions went backwards: {positions:?}"
        );
        // Separate words still advance, so a phrase query still means something.
        let plain: Vec<usize> = tokens_of("cat eats fish")
            .into_iter()
            .map(|t| t.position)
            .collect();
        assert_eq!(plain, vec![0, 1, 2]);
    }

    /// The measured failure this splitting exists for: a note titled `LHVendor`
    /// was not found by searching `vendor`, and an unrelated note that spelled
    /// the word out ranked above it.
    #[test]
    fn a_word_stuck_to_a_prefix_is_findable_by_either_half() {
        let tokens = token_texts("LHVendor");
        assert!(tokens.contains(&"LHVendor".to_string()), "{tokens:?}");
        assert!(tokens.contains(&"Vendor".to_string()), "{tokens:?}");
        assert!(tokens.contains(&"LH".to_string()), "{tokens:?}");
    }

    #[test]
    fn splits_the_shapes_team_vocabulary_actually_takes() {
        // An acronym followed by a word cuts before the last capital.
        let jdbc = token_texts("JDBCDriver");
        assert!(jdbc.contains(&"JDBC".to_string()), "{jdbc:?}");
        assert!(jdbc.contains(&"Driver".to_string()), "{jdbc:?}");

        // camelCase.
        let camel = token_texts("getUserById");
        assert!(camel.contains(&"get".to_string()), "{camel:?}");
        assert!(camel.contains(&"User".to_string()), "{camel:?}");
        assert!(camel.contains(&"By".to_string()), "{camel:?}");

        // Letters against digits, in both directions.
        let asset = token_texts("LH2291");
        assert!(asset.contains(&"LH".to_string()), "{asset:?}");
        assert!(asset.contains(&"2291".to_string()), "{asset:?}");
    }

    /// A word with nothing to split must not be emitted twice: duplicate terms
    /// inflate the index and skew the relevance scoring that ranks on frequency.
    #[test]
    fn an_ordinary_word_is_emitted_once() {
        assert_eq!(token_texts("vendor"), vec!["vendor".to_string()]);
        assert_eq!(token_texts("Informix"), vec!["Informix".to_string()]);
        assert_eq!(token_texts("JNDI"), vec!["JNDI".to_string()]);
    }

    /// Offsets still slice back to the source, for the pieces as well as the
    /// whole — the snippet highlighter reads them, and a wrong one highlights
    /// the wrong characters or panics on a char boundary.
    #[test]
    fn split_pieces_keep_offsets_into_the_original_text() {
        let text = "แก้ที่ IfxDriver ของ LH2291 แล้ว";
        for token in tokens_of(text) {
            assert_eq!(
                &text[token.offset_from..token.offset_to],
                token.text,
                "offset mismatch for {token:?}"
            );
        }
    }

    /// The point of a vault dictionary: a word the bundled 62k has never heard
    /// of is one word, not the several the segmenter would guess at.
    #[test]
    fn a_vault_word_is_segmented_as_one_word() {
        let unknown = "สมองกลอัจฉริยะ";
        let without = tokenize(unknown, &segmenter_for(&[]))
            .into_iter()
            .map(|t| t.text)
            .collect::<Vec<_>>();
        let with = tokenize(unknown, &segmenter_for(&[unknown.to_string()]))
            .into_iter()
            .map(|t| t.text)
            .collect::<Vec<_>>();

        assert_eq!(
            with,
            vec![unknown.to_string()],
            "declared word was still cut up"
        );
        assert_ne!(without, with, "the dictionary made no difference");
    }

    /// Reordering the list must not force a reindex; changing it must.
    #[test]
    fn the_hash_tracks_the_words_not_their_order() {
        let a = vec!["LHVendor".to_string(), "Informix".to_string()];
        let reordered = vec!["Informix".to_string(), "LHVendor".to_string()];
        let with_duplicate = vec![
            "Informix".to_string(),
            "LHVendor".to_string(),
            "Informix".to_string(),
        ];
        let extended = vec![
            "LHVendor".to_string(),
            "Informix".to_string(),
            "IfxDriver".to_string(),
        ];

        assert_eq!(dictionary_hash(&a), dictionary_hash(&reordered));
        assert_eq!(dictionary_hash(&a), dictionary_hash(&with_duplicate));
        assert_ne!(dictionary_hash(&a), dictionary_hash(&extended));
        assert_ne!(dictionary_hash(&a), dictionary_hash(&[]));
    }

    #[test]
    fn whitespace_and_punctuation_produce_no_tokens() {
        assert!(tokens_of("  ... !!! ").is_empty());
        assert!(tokens_of("").is_empty());
    }
}
