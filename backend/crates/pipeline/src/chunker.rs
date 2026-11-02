//! Clause chunking: streamed LLM text in, speakable clauses out, so TTS can start on the
//! first clause while the LLM is still writing the rest (research 01 §5).
//!
//! - The first clause is short: it ends at the first sentence end, at a comma, semicolon or
//!   colon after at least four words, or after twelve words.
//! - Later clauses end at sentence ends, or after twenty-five words.
//! - A boundary counts only once whitespace follows it, so "10.5" or "e.g." mid-stream are not
//!   cut; [`ClauseChunker::finish`] flushes whatever is left.

const FIRST_CLAUSE_MAX_WORDS: usize = 12;
const CLAUSE_MAX_WORDS: usize = 25;
const FIRST_CLAUSE_MIN_WORDS_AT_COMMA: usize = 4;

#[derive(Debug, Default)]
pub(crate) struct ClauseChunker {
    buffer: String,
    emitted: usize,
}

impl ClauseChunker {
    /// Adds a piece of streamed text; returns the clauses it completed, in order.
    pub(crate) fn push(&mut self, text: &str) -> Vec<String> {
        self.buffer.push_str(text);
        let mut clauses = Vec::new();
        while let Some(cut) = self.cut() {
            let rest = self.buffer.split_off(cut);
            let clause = std::mem::replace(&mut self.buffer, rest);
            let clause = clause.trim();
            if !clause.is_empty() {
                clauses.push(clause.to_owned());
                self.emitted += 1;
            }
        }
        clauses
    }

    /// The last clause, once the text is complete.
    pub(crate) fn finish(&mut self) -> Option<String> {
        let clause = std::mem::take(&mut self.buffer);
        let clause = clause.trim();
        (!clause.is_empty()).then(|| {
            self.emitted += 1;
            clause.to_owned()
        })
    }

    /// Where the next clause ends in the buffer, if it is complete.
    fn cut(&self) -> Option<usize> {
        let first = self.emitted == 0;
        let max_words = if first {
            FIRST_CLAUSE_MAX_WORDS
        } else {
            CLAUSE_MAX_WORDS
        };
        let mut words = 0;
        let mut in_word = false;
        let mut chars = self.buffer.char_indices().peekable();
        while let Some((at, c)) = chars.next() {
            if c.is_whitespace() {
                if in_word {
                    in_word = false;
                    if words >= max_words {
                        return Some(at);
                    }
                }
                continue;
            }
            if !in_word {
                in_word = true;
                words += 1;
            }
            let followed_by_space = chars.peek().is_some_and(|(_, next)| next.is_whitespace());
            if !followed_by_space {
                continue;
            }
            let end = at + c.len_utf8();
            if matches!(c, '.' | '!' | '?') {
                return Some(end);
            }
            if first && matches!(c, ',' | ';' | ':') && words >= FIRST_CLAUSE_MIN_WORDS_AT_COMMA {
                return Some(end);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(pieces: &[&str]) -> Vec<String> {
        let mut chunker = ClauseChunker::default();
        let mut clauses: Vec<String> = pieces
            .iter()
            .flat_map(|piece| chunker.push(piece))
            .collect();
        clauses.extend(chunker.finish());
        clauses
    }

    #[test]
    fn sentences_become_clauses_once_whitespace_follows() {
        let mut chunker = ClauseChunker::default();
        assert_eq!(
            chunker.push("Sure, I can help with that."),
            Vec::<String>::new()
        );
        assert_eq!(
            chunker.push(" How many people?"),
            ["Sure, I can help with that."]
        );
        assert_eq!(chunker.finish().as_deref(), Some("How many people?"));
        assert_eq!(chunker.finish(), None);
    }

    #[test]
    fn the_first_clause_ends_early_at_a_comma_after_four_words() {
        assert_eq!(
            chunk(&["I'm sorry to hear that, can I offer you something else?"]),
            ["I'm sorry to hear that,", "can I offer you something else?"]
        );
        assert_eq!(
            chunk(&["Sure, for how many people?"]),
            ["Sure, for how many people?"]
        );
        assert_eq!(
            chunk(&["Yes, hi! How can I help you today?"]),
            ["Yes, hi!", "How can I help you today?"]
        );
    }

    #[test]
    fn later_commas_do_not_cut_and_long_runs_are_split() {
        assert_eq!(
            chunk(&["Hello there. We have pasta, pizza, salads and soup tonight."]),
            [
                "Hello there.",
                "We have pasta, pizza, salads and soup tonight."
            ]
        );
        let long =
            "one two three four five six seven eight nine ten eleven twelve thirteen fourteen";
        assert_eq!(
            chunk(&[long]),
            [
                "one two three four five six seven eight nine ten eleven twelve",
                "thirteen fourteen"
            ]
        );
    }

    #[test]
    fn numbers_and_streamed_tokens_do_not_break_clauses() {
        assert_eq!(
            chunk(&[
                "Yes", ",", " we", " are", " open", " until", " 10", ".", "5", " PM", ". ", "OK?"
            ]),
            ["Yes, we are open until 10.5 PM.", "OK?"]
        );
        assert_eq!(chunk(&["  ", " "]), Vec::<String>::new());
    }
}
