//! Measuring whether a change to search actually helped.
//!
//! The roadmap wants a similarity floor for semantic search, and says the
//! threshold "has to be measured against real vaults, not guessed". This is the
//! measuring. Without it, every future change to ranking is judged by trying a
//! few queries and forming an impression — which is exactly how a search that
//! feels better and scores worse gets shipped.
//!
//! # What a question set is, and what it is not
//!
//! A list of questions somebody actually asked, each paired with the notes that
//! answer it:
//!
//! ```toml
//! [[question]]
//! ask = "ทำไม tomcat ต่อ db ไม่ได้"
//! answers = ["ops/tomcat-jdbc.md"]
//!
//! [[question]]
//! ask = "how do we rotate the signing key"
//! answers = []   # nothing in this vault answers it
//! ```
//!
//! The questions have to be real and the vault has to be yours. A set written by
//! reading the notes and inventing questions that obviously match them measures
//! nothing: it scores the questions, not the search. The awkward ones — the
//! half-remembered phrase, the English question about a Thai note, the acronym
//! nobody spells out — are the ones worth writing down.
//!
//! `answers = []` is not padding. A vault that has no answer should return
//! nothing convincing, and a ranking change that improves every other number
//! while making the system confidently answer questions it cannot answer has
//! made things worse, not better. That is the failure that misleads an agent,
//! so it gets counted separately rather than averaged away.

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

/// One question and the notes that answer it.
#[derive(Debug, Deserialize)]
pub struct Question {
    /// The question, as it was actually asked.
    pub ask: String,
    /// Vault-relative keys of the notes that answer it. Empty means the vault
    /// has no answer and the right behaviour is to return nothing.
    #[serde(default)]
    pub answers: Vec<String>,
}

/// A whole question set, as parsed from TOML.
#[derive(Debug, Deserialize)]
pub struct QuestionSet {
    #[serde(default, rename = "question")]
    pub questions: Vec<Question>,
}

impl QuestionSet {
    /// Read a question set from a TOML file.
    pub fn load(path: &Path) -> Result<Self> {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let set: Self =
            toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        if set.questions.is_empty() {
            bail!(
                "{} has no [[question]] entries — an empty set would score 100% \
                 and mean nothing",
                path.display()
            );
        }
        Ok(set)
    }

    /// Fail on answer keys that name no note in the vault.
    ///
    /// A typo in an answer key would otherwise look exactly like a search that
    /// cannot find the note: the question scores as a miss, the report says the
    /// ranking got worse, and the ranking is fine. A measurement that can be
    /// wrong in a direction nobody notices is worse than no measurement, so this
    /// is an error naming every bad key, not a warning.
    pub fn check_against(&self, notes: &BTreeSet<String>) -> Result<()> {
        let mut unknown: Vec<String> = Vec::new();
        for question in &self.questions {
            for answer in &question.answers {
                if !notes.contains(answer) {
                    unknown.push(format!("{answer:?} (for {:?})", question.ask));
                }
            }
        }
        if !unknown.is_empty() {
            bail!(
                "these answer keys name no note in the vault:\n  {}\n\
                 Answer keys are vault-relative paths, as printed by `samong list`.",
                unknown.join("\n  ")
            );
        }
        Ok(())
    }
}

/// What the search returned for one question.
#[derive(Debug)]
pub struct Outcome {
    pub ask: String,
    /// Empty when the question is one the vault cannot answer.
    pub expected: Vec<String>,
    /// Keys the search returned, best first.
    pub returned: Vec<String>,
}

impl Outcome {
    /// Whether the vault is supposed to be able to answer this at all.
    pub fn answerable(&self) -> bool {
        !self.expected.is_empty()
    }

    /// 1-based position of the first correct answer, if one came back.
    pub fn rank_of_first_answer(&self) -> Option<usize> {
        self.returned
            .iter()
            .position(|key| self.expected.contains(key))
            .map(|index| index + 1)
    }

    /// Reciprocal rank: 1.0 for a correct first hit, 0.5 for second, 0 for a
    /// miss. Averaged across questions this is MRR, which rewards moving the
    /// right answer *up*, not merely into the list.
    pub fn reciprocal_rank(&self) -> f32 {
        match self.rank_of_first_answer() {
            Some(rank) => 1.0 / rank as f32,
            None => 0.0,
        }
    }
}

/// The numbers, and the questions behind them.
#[derive(Debug)]
pub struct Report {
    pub outcomes: Vec<Outcome>,
    /// The `k` in hit@k, as asked for on the command line.
    pub at: usize,
}

impl Report {
    pub fn answerable(&self) -> usize {
        self.outcomes.iter().filter(|o| o.answerable()).count()
    }

    /// Questions whose correct answer came back at position `k` or better.
    pub fn hits_at(&self, k: usize) -> usize {
        self.outcomes
            .iter()
            .filter(|o| o.answerable())
            .filter(|o| o.rank_of_first_answer().is_some_and(|rank| rank <= k))
            .count()
    }

    /// Mean reciprocal rank over the answerable questions.
    ///
    /// Over the answerable ones only: a question with no answer has no rank to
    /// take the reciprocal of, and folding it in as a zero would make a vault
    /// look worse for containing honest gaps.
    pub fn mrr(&self) -> f32 {
        let answerable = self.answerable();
        if answerable == 0 {
            return 0.0;
        }
        let total: f32 = self
            .outcomes
            .iter()
            .filter(|o| o.answerable())
            .map(Outcome::reciprocal_rank)
            .sum();
        total / answerable as f32
    }

    /// Questions the vault cannot answer where search returned something anyway.
    ///
    /// The number to watch when adding a similarity floor: a floor that is too
    /// low leaves this high, and a floor that is too high shows up as hit@k
    /// falling. Neither number alone can be read as progress.
    pub fn noise(&self) -> usize {
        self.outcomes
            .iter()
            .filter(|o| !o.answerable())
            .filter(|o| !o.returned.is_empty())
            .count()
    }

    pub fn unanswerable(&self) -> usize {
        self.outcomes.len() - self.answerable()
    }
}

/// Ask every question of one vault and record what came back.
///
/// Goes through [`crate::ops::search_vault`], the same path the CLI, the HTTP
/// API and the MCP server use, so what is measured is what those return —
/// including the semantic half when the binary was built with it.
pub fn run(vault: &Path, set: &QuestionSet, at: usize) -> Result<Report> {
    let notes: BTreeSet<String> = crate::vault::list_notes(vault)?
        .into_iter()
        .map(|note| note.key)
        .collect();
    set.check_against(&notes)?;

    let options = crate::search::SearchOptions::with_limit(at);
    let mut outcomes = Vec::with_capacity(set.questions.len());
    for question in &set.questions {
        let hits = crate::ops::search_vault(vault, &question.ask, &options)?;
        outcomes.push(Outcome {
            ask: question.ask.clone(),
            expected: question.answers.clone(),
            returned: hits.into_iter().map(|hit| hit.key).collect(),
        });
    }
    Ok(Report { outcomes, at })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(expected: &[&str], returned: &[&str]) -> Outcome {
        Outcome {
            ask: "q".to_string(),
            expected: expected.iter().map(|s| s.to_string()).collect(),
            returned: returned.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn rank_is_one_based_so_the_first_hit_scores_one() {
        assert_eq!(outcome(&["a.md"], &["a.md"]).reciprocal_rank(), 1.0);
        assert_eq!(outcome(&["a.md"], &["b.md", "a.md"]).reciprocal_rank(), 0.5);
        assert_eq!(outcome(&["a.md"], &["b.md", "c.md"]).reciprocal_rank(), 0.0);
    }

    /// Any one of several acceptable answers counts, and the best-placed one is
    /// the one that scores: a question can have two notes that answer it, and
    /// finding either is a success, not half of one.
    #[test]
    fn several_acceptable_answers_score_as_the_best_placed_one() {
        let outcome = outcome(&["a.md", "b.md"], &["c.md", "b.md", "a.md"]);
        assert_eq!(outcome.rank_of_first_answer(), Some(2));
    }

    /// The failure this whole module exists to make visible: a vault that cannot
    /// answer a question must not answer it anyway.
    #[test]
    fn a_question_with_no_answer_counts_as_noise_only_when_something_came_back() {
        let report = Report {
            at: 5,
            outcomes: vec![
                outcome(&[], &[]),       // right: nothing to say, said nothing
                outcome(&[], &["x.md"]), // wrong: answered anyway
                outcome(&["a.md"], &[]), // an ordinary miss, not noise
            ],
        };
        assert_eq!(report.noise(), 1);
        assert_eq!(report.unanswerable(), 2);
        assert_eq!(report.answerable(), 1);
    }

    /// MRR over answerable questions only. Were unanswerable ones folded in as
    /// zeroes, a vault would score worse the more honest gaps its question set
    /// admitted to — and the set would quietly stop admitting to them.
    #[test]
    fn unanswerable_questions_do_not_drag_the_mean_down() {
        let with_gaps = Report {
            at: 5,
            outcomes: vec![
                outcome(&["a.md"], &["a.md"]),
                outcome(&[], &[]),
                outcome(&[], &[]),
            ],
        };
        assert_eq!(with_gaps.mrr(), 1.0);
        assert_eq!(with_gaps.hits_at(5), 1);
    }

    #[test]
    fn hits_at_k_counts_position_not_presence() {
        let report = Report {
            at: 5,
            outcomes: vec![outcome(&["a.md"], &["x.md", "y.md", "a.md"])],
        };
        assert_eq!(report.hits_at(1), 0);
        assert_eq!(report.hits_at(3), 1);
        assert_eq!(report.hits_at(5), 1);
    }

    /// A typo in an answer key is a broken question set, and looks exactly like a
    /// search failure unless it is caught here.
    #[test]
    fn an_answer_key_naming_no_note_is_an_error_not_a_miss() {
        let set: QuestionSet = toml::from_str(
            r#"
            [[question]]
            ask = "ทำไม tomcat ต่อ db ไม่ได้"
            answers = ["ops/tomcat-jbdc.md"]
            "#,
        )
        .unwrap();
        let notes: BTreeSet<String> = ["ops/tomcat-jdbc.md".to_string()].into_iter().collect();

        let error = set.check_against(&notes).unwrap_err().to_string();
        assert!(error.contains("ops/tomcat-jbdc.md"), "{error}");
        assert!(error.contains("ทำไม tomcat"), "names the question: {error}");
    }

    #[test]
    fn a_question_set_with_no_questions_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("questions.toml");
        std::fs::write(&path, "# nothing here yet\n").unwrap();
        let error = QuestionSet::load(&path).unwrap_err().to_string();
        assert!(error.contains("no [[question]] entries"), "{error}");
    }

    #[test]
    fn a_question_may_omit_answers_entirely() {
        let set: QuestionSet = toml::from_str("[[question]]\nask = \"anything?\"\n").unwrap();
        assert!(set.questions[0].answers.is_empty());
        assert_eq!(set.questions.len(), 1);
    }
}
