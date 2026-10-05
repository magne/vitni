//! How `vitni import` shows a bulk import's plan and reviews its possible matches (ADR 0040 §4).
//!
//! - `--plan` prints the plan, as text or `--json`, and writes nothing.
//! - On a terminal the plan is shown on stderr and each possible match is put to the operator, who
//!   answers it, treats a whole probable group as the same, or decides the rest later.
//! - Otherwise, or with `--defer-matches`, every possible match is left for later.

use std::io::{BufRead, Write};

use serde::Serialize;
use tokio::sync::mpsc;
use vitni_app::{
    IdentityDecision, MatchQuestion, PairAnswer, PlanStep, PlanSummary, PlannedChange, Provenance, ReviewReply,
};
use vitni_plugin_host::ReviewRequest;

use crate::i18n::Localizer;

/// How an import's plan is shown and its possible matches decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewMode {
    /// Print the plan and write nothing.
    Print(PlanFormat),
    /// Show the plan and ask about each possible match.
    Interactive,
    /// Commit, leaving every possible match for later.
    Defer,
}

/// How a printed plan is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanFormat {
    /// Localized lines.
    Text,
    /// One JSON object.
    Json,
}

/// What `vitni import` was asked about its plan.
#[derive(Debug, Clone, Copy, Default)]
pub struct PlanFlags {
    /// `--plan`.
    pub plan: bool,
    /// `--json`.
    pub json: bool,
    /// `--defer-matches`.
    pub defer_matches: bool,
}

/// The mode `flags` ask for, with stdin a `terminal` or not.
#[must_use]
pub fn review_mode(flags: PlanFlags, terminal: bool) -> ReviewMode {
    match (flags.plan, flags.json) {
        (true, true) => ReviewMode::Print(PlanFormat::Json),
        (true, false) => ReviewMode::Print(PlanFormat::Text),
        (false, _) if terminal && !flags.defer_matches => ReviewMode::Interactive,
        (false, _) => ReviewMode::Defer,
    }
}

/// How the review of an import ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Reviewed {
    /// No plan was shown: the file held nothing to write, or the import ended before planning.
    #[default]
    NotPlanned,
    /// The plan was committed.
    Committed,
    /// The plan was printed; nothing was written.
    Printed,
    /// The operator cancelled the review; nothing was written.
    Cancelled,
}

/// Where a review reads answers and writes its lines.
pub struct Terminal<I, E, O> {
    /// The operator's answers.
    pub input: I,
    /// The prompts and the plan an interactive review shows.
    pub prompts: E,
    /// A printed plan.
    pub output: O,
}

/// Answers each request the import's reviewer puts, as `mode` says, until the import ends.
pub async fn answer<I: BufRead, E: Write, O: Write>(
    mut requests: mpsc::Receiver<ReviewRequest>,
    localizer: &Localizer,
    (mode, source): (ReviewMode, &str),
    terminal: &mut Terminal<I, E, O>,
) -> Reviewed {
    let mut reviewed = Reviewed::NotPlanned;
    while let Some(request) = requests.recv().await {
        match request {
            ReviewRequest::Plan { summary, reply } => {
                let step = show_plan(localizer, mode, source, &summary, terminal);
                reviewed = match step {
                    PlanStep::Discard => Reviewed::Printed,
                    PlanStep::Review | PlanStep::DeferMatches => Reviewed::Committed,
                };
                // A dropped receiver means the import is over; the answer is then moot.
                let _ = reply.send(step);
            }
            ReviewRequest::Match { question, reply } => {
                let answer = ask(localizer, &question, &mut terminal.input, &mut terminal.prompts);
                if answer == ReviewReply::Cancel {
                    reviewed = Reviewed::Cancelled;
                }
                let _ = reply.send(answer);
            }
            ReviewRequest::Confirm { summary, reply } => {
                let mut lines = vec![localizer.import_plan_reviewed_heading(source)];
                lines.extend(plan_body(localizer, &summary));
                write_lines(&mut terminal.prompts, &lines);
                let commit = confirm(localizer, &mut terminal.input, &mut terminal.prompts);
                if !commit {
                    reviewed = Reviewed::Cancelled;
                }
                let _ = reply.send(commit);
            }
        }
    }
    reviewed
}

/// Shows `summary` as `mode` says and returns what to do with the plan.
fn show_plan<I, E: Write, O: Write>(
    localizer: &Localizer,
    mode: ReviewMode,
    source: &str,
    summary: &PlanSummary,
    terminal: &mut Terminal<I, E, O>,
) -> PlanStep {
    match mode {
        ReviewMode::Print(PlanFormat::Text) => {
            write_lines(&mut terminal.output, &plan_lines(localizer, source, summary, true));
            PlanStep::Discard
        }
        ReviewMode::Print(PlanFormat::Json) => {
            write_lines(&mut terminal.output, &[plan_json(source, summary)]);
            PlanStep::Discard
        }
        ReviewMode::Interactive => {
            write_lines(&mut terminal.prompts, &plan_lines(localizer, source, summary, false));
            PlanStep::Review
        }
        ReviewMode::Defer => PlanStep::DeferMatches,
    }
}

/// The plan of importing `source` as lines: a heading, one line per kind followed by what it changes
/// on each record, and what it comes to. A `printed` plan with possible matches ends saying how to
/// review them.
#[must_use]
pub fn plan_lines(localizer: &Localizer, source: &str, summary: &PlanSummary, printed: bool) -> Vec<String> {
    let mut lines = vec![localizer.import_plan_heading(source)];
    lines.extend(plan_body(localizer, summary));
    let writes = summary.kinds.iter().any(|row| {
        let counts = row.counts;
        counts.new + counts.updated + counts.linked + counts.candidates > 0
    });
    if summary.kinds.is_empty() {
        lines.push(localizer.import_plan_empty());
    } else if !writes {
        lines.push(localizer.import_plan_all_unchanged());
    } else if printed && summary.candidates > 0 {
        lines.push(localizer.import_plan_review_hint(summary.candidates));
    }
    lines
}

/// One line per kind, each followed by what the plan changes on that kind's records.
fn plan_body(localizer: &Localizer, summary: &PlanSummary) -> Vec<String> {
    let mut lines = Vec::new();
    for row in &summary.kinds {
        lines.push(format!("  {}", localizer.import_plan_row(row)));
        for record in summary.records.iter().filter(|record| record.kind == row.kind) {
            lines.push(format!("    {}", localizer.import_plan_record(record)));
        }
    }
    lines
}

/// The plan of importing `source` as one JSON object, for scripts and agents: its kinds' counts under
/// their stable names, and each record it changes with the field keys its writes assert.
#[must_use]
pub fn plan_json(source: &str, summary: &PlanSummary) -> String {
    #[derive(Serialize)]
    struct Plan<'a> {
        source: &'a str,
        kinds: Vec<Row>,
        candidates: u32,
        records: Vec<Record<'a>>,
    }
    #[derive(Serialize)]
    struct Record<'a> {
        kind: &'static str,
        label: &'a str,
        human_id: &'a str,
        change: &'static str,
        fields: &'a [String],
    }
    #[derive(Serialize)]
    struct Row {
        kind: &'static str,
        new: u32,
        unchanged: u32,
        updated: u32,
        linked: u32,
        candidates: u32,
        withheld: u32,
    }
    let kinds = summary
        .kinds
        .iter()
        .map(|row| Row {
            kind: row.kind.as_str(),
            new: row.counts.new,
            unchanged: row.counts.unchanged,
            updated: row.counts.updated,
            linked: row.counts.linked,
            candidates: row.counts.candidates,
            withheld: row.counts.withheld,
        })
        .collect();
    let records = summary
        .records
        .iter()
        .map(|record| Record {
            kind: record.kind.as_str(),
            label: &record.label,
            human_id: &record.human_id,
            change: match record.change {
                PlannedChange::Updates => "updates",
                PlannedChange::Reuses => "reuses",
            },
            fields: &record.keys,
        })
        .collect();
    let plan = Plan {
        source,
        kinds,
        candidates: summary.candidates,
        records,
    };
    // A struct of strings and integers always serializes.
    serde_json::to_string(&plan).unwrap_or_default()
}

/// Puts `question` to the operator and reads answers until one is valid. The end of input cancels the
/// import, so nothing is written on an answer never given.
pub fn ask(
    localizer: &Localizer,
    question: &MatchQuestion,
    input: &mut impl BufRead,
    prompts: &mut impl Write,
) -> ReviewReply {
    let mut lines = localizer.import_review_question(question).into_iter();
    let heading: Vec<String> = lines.next().into_iter().collect();
    write_lines(prompts, &heading);
    write_lines(prompts, &lines.map(|line| format!("  {line}")).collect::<Vec<_>>());
    let prompt = localizer.import_review_prompt(question);
    loop {
        // A prompt that cannot be written still waits for its answer, as `confirm` does.
        drop(write!(prompts, "{prompt} "));
        drop(prompts.flush());
        let mut answer = String::new();
        match input.read_line(&mut answer) {
            Ok(0) | Err(_) => return ReviewReply::Cancel,
            Ok(_) => {}
        }
        if let Some(reply) = parse_answer(answer.trim(), question) {
            return reply;
        }
    }
}

/// Asks the operator whether to import the reviewed plan, until they answer. The end of input declines,
/// so nothing is written on an answer never given.
fn confirm(localizer: &Localizer, input: &mut impl BufRead, prompts: &mut impl Write) -> bool {
    let prompt = localizer.import_plan_confirm();
    loop {
        // A prompt that cannot be written still waits for its answer, as `ask` does.
        drop(write!(prompts, "{prompt} "));
        drop(prompts.flush());
        let mut answer = String::new();
        match input.read_line(&mut answer) {
            Ok(0) | Err(_) => return false,
            Ok(_) => {}
        }
        match answer.trim().to_lowercase().as_str() {
            "y" | "j" => return true,
            "n" => return false,
            _ => {}
        }
    }
}

/// The reply `answer` gives to `question`, if it is one of the prompt's letters. Each decision carries
/// the assessment the operator was shown.
fn parse_answer(answer: &str, question: &MatchQuestion) -> Option<ReviewReply> {
    let decision = || IdentityDecision {
        provenance: Provenance::default(),
        assessment: Some(question.assessment.evidence()),
    };
    let pair = |answer: PairAnswer| Some(ReviewReply::Pair(Box::new(answer)));
    match answer.to_lowercase().as_str() {
        "y" | "j" => pair(PairAnswer::Same(decision())),
        "n" => pair(PairAnswer::Distinct(decision())),
        "l" => pair(PairAnswer::Later),
        "a" if question.group.is_some() => Some(ReviewReply::SameForGroup(Box::new(decision()))),
        "r" => Some(ReviewReply::DeferRest),
        "c" => Some(ReviewReply::Cancel),
        _ => None,
    }
}

/// Writes each of `lines` to `out`; a line that cannot be written is dropped, as a failed print is.
fn write_lines(out: &mut impl Write, lines: &[String]) {
    for line in lines {
        drop(writeln!(out, "{line}"));
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use tokio::sync::{mpsc, oneshot};
    use vitni_app::{
        AggRef, ENGINE_VERSION, KindCounts, MatchAssessment, MatchBand, MatchGroup, MatchQuestion, MatchableKind,
        PairAnswer, PlanCounts, PlanSummary, PlannedChange, PlannedField, PlannedRecord, ReviewReply,
    };
    use vitni_core::enums::FactType;
    use vitni_plugin_host::ReviewRequest;

    use super::{PlanFlags, PlanFormat, ReviewMode, Terminal, answer, ask, plan_json, plan_lines, review_mode};
    use crate::i18n::Localizer;

    fn question(group: Option<MatchGroup>) -> MatchQuestion {
        MatchQuestion {
            kind: MatchableKind::Person,
            incoming_label: "Ole Hansen".to_owned(),
            incoming_origin: None,
            candidate: AggRef {
                human_id: "I0001".to_owned(),
                id: "01".to_owned(),
            },
            candidate_label: "Ole Hansen".to_owned(),
            candidate_origin: None,
            assessment: MatchAssessment {
                score: 0.97,
                band: MatchBand::Probable,
                features: Vec::new(),
                cultures: Vec::new(),
                parts: Vec::new(),
                engine: ENGINE_VERSION,
            },
            position: 1,
            total: 3,
            group,
        }
    }

    fn answered(input: &str, group: Option<MatchGroup>) -> (ReviewReply, String) {
        let mut prompts = Vec::new();
        let reply = ask(
            &Localizer::english(),
            &question(group),
            &mut Cursor::new(input),
            &mut prompts,
        );
        (reply, String::from_utf8_lossy(&prompts).into_owned())
    }

    fn summary(counts: PlanCounts) -> PlanSummary {
        PlanSummary {
            kinds: vec![KindCounts {
                kind: MatchableKind::Person,
                counts,
            }],
            candidates: counts.candidates,
            records: Vec::new(),
        }
    }

    /// A plan that updates Ole Hansen's occupation and a family's partner, and reuses a census that
    /// lacks an author.
    fn changes() -> PlanSummary {
        let row = |kind, counts| KindCounts { kind, counts };
        let updated = PlanCounts {
            updated: 1,
            ..PlanCounts::default()
        };
        let linked = PlanCounts {
            linked: 1,
            ..PlanCounts::default()
        };
        let record = |kind, label: &str, human_id: &str, change, fields, keys: &[&str]| PlannedRecord {
            kind,
            label: label.to_owned(),
            human_id: human_id.to_owned(),
            change,
            fields,
            keys: keys.iter().map(|key| (*key).to_owned()).collect(),
        };
        PlanSummary {
            kinds: vec![
                row(MatchableKind::Source, linked),
                row(MatchableKind::Person, updated),
                row(MatchableKind::Family, updated),
            ],
            candidates: 0,
            records: vec![
                record(
                    MatchableKind::Source,
                    "1900 census",
                    "S0001",
                    PlannedChange::Reuses,
                    vec![PlannedField::Author, PlannedField::Publication],
                    &["source.AuthorSet", "source.PubInfoSet"],
                ),
                record(
                    MatchableKind::Person,
                    "Ole Hansen",
                    "I0001",
                    PlannedChange::Updates,
                    vec![PlannedField::Fact(FactType::Occupation)],
                    &["person.FactAsserted.Occupation"],
                ),
                record(
                    MatchableKind::Family,
                    "",
                    "F0001",
                    PlannedChange::Updates,
                    vec![PlannedField::Partner],
                    &["family.PartnerAdded"],
                ),
            ],
        }
    }

    #[test]
    fn a_plan_lists_what_it_changes_on_each_record_under_its_kind() {
        assert_eq!(
            plan_lines(&Localizer::english(), "tree.ged", &changes(), true),
            vec![
                "Plan for tree.ged:".to_owned(),
                "  sources: 1 already in the tree".to_owned(),
                "    1900 census (S0001): reused · adds author, publication".to_owned(),
                "  persons: 1 updated".to_owned(),
                "    Ole Hansen (I0001): updates occupation".to_owned(),
                "  families: 1 updated".to_owned(),
                "    F0001: updates partner".to_owned(),
            ]
        );
    }

    #[test]
    fn a_json_plan_lists_each_changed_record_with_its_field_keys() {
        let json = plan_json("tree.ged", &changes());
        assert!(
            json.contains(
                r#""records":[{"kind":"source","label":"1900 census","human_id":"S0001","change":"reuses","fields":["source.AuthorSet","source.PubInfoSet"]},"#
            ),
            "{json}"
        );
    }

    /// What an interactive review answers to the reviewed plan `changes()` given `input`, and what it
    /// wrote to the operator.
    async fn confirmed(input: &str) -> (Option<bool>, String) {
        let (requests, received) = mpsc::channel(1);
        let (reply, answered) = oneshot::channel();
        requests
            .send(ReviewRequest::Confirm {
                summary: changes(),
                reply,
            })
            .await
            .expect("send");
        drop(requests);
        let mut terminal = Terminal {
            input: Cursor::new(input.to_owned()),
            prompts: Vec::new(),
            output: Vec::new(),
        };
        answer(
            received,
            &Localizer::english(),
            (ReviewMode::Interactive, "tree.ged"),
            &mut terminal,
        )
        .await;
        let prompts = String::from_utf8_lossy(&terminal.prompts).into_owned();
        (answered.await.ok(), prompts)
    }

    #[tokio::test]
    async fn a_reviewed_plan_is_shown_and_committed_only_when_the_operator_says_so() {
        let (commit, prompts) = confirmed("x\ny\n").await;
        assert_eq!(commit, Some(true), "asked again until answered");
        assert!(prompts.contains("Plan for tree.ged, as reviewed:"), "{prompts}");
        assert!(prompts.contains("Ole Hansen (I0001): updates occupation"), "{prompts}");
        assert!(prompts.contains("Import this plan? [y] yes, [n] no:"), "{prompts}");
        assert_eq!(confirmed("n\n").await.0, Some(false));
        assert_eq!(confirmed("").await.0, Some(false), "no answer writes nothing");
    }

    #[test]
    fn the_mode_follows_the_flags_and_the_terminal() {
        let plan = PlanFlags {
            plan: true,
            ..PlanFlags::default()
        };
        let json = PlanFlags { json: true, ..plan };
        let defer = PlanFlags {
            defer_matches: true,
            ..PlanFlags::default()
        };
        assert_eq!(review_mode(plan, true), ReviewMode::Print(PlanFormat::Text));
        assert_eq!(review_mode(json, false), ReviewMode::Print(PlanFormat::Json));
        assert_eq!(review_mode(PlanFlags::default(), true), ReviewMode::Interactive);
        assert_eq!(review_mode(defer, true), ReviewMode::Defer);
        assert_eq!(review_mode(PlanFlags::default(), false), ReviewMode::Defer);
    }

    #[test]
    fn each_letter_answers_the_pair() {
        let (same, prompts) = answered("y\n", None);
        assert!(
            matches!(same, ReviewReply::Pair(answer) if matches!(*answer, PairAnswer::Same(ref decision) if decision.assessment.is_some()))
        );
        assert!(
            prompts.contains("Possible match 1 of 3: person, probable match (97%)"),
            "{prompts}"
        );
        assert!(prompts.contains("In your tree: Ole Hansen (I0001)"), "{prompts}");
        assert!(
            matches!(answered("n\n", None).0, ReviewReply::Pair(answer) if matches!(*answer, PairAnswer::Distinct(_)))
        );
        assert!(matches!(answered("l\n", None).0, ReviewReply::Pair(answer) if *answer == PairAnswer::Later));
        assert_eq!(answered("r\n", None).0, ReviewReply::DeferRest);
        assert_eq!(answered("c\n", None).0, ReviewReply::Cancel);
    }

    #[test]
    fn the_group_answer_is_offered_only_with_a_probable_group() {
        let group = Some(MatchGroup {
            band: MatchBand::Probable,
            remaining: 12,
        });
        let (reply, prompts) = answered("a\n", group);
        assert!(matches!(reply, ReviewReply::SameForGroup(_)));
        assert!(
            prompts.contains("treat all 12 probable person matches as the same"),
            "{prompts}"
        );

        let (reply, prompts) = answered("a\nl\n", None);
        assert!(
            matches!(reply, ReviewReply::Pair(answer) if *answer == PairAnswer::Later),
            "asked again"
        );
        assert!(!prompts.contains("treat all"), "{prompts}");
    }

    #[test]
    fn an_unanswered_question_cancels_the_import() {
        assert_eq!(answered("x\n", None).0, ReviewReply::Cancel);
    }

    #[test]
    fn a_plan_lists_each_kind_and_says_when_it_writes_nothing() {
        let localizer = Localizer::english();
        let unchanged = summary(PlanCounts {
            unchanged: 2,
            ..PlanCounts::default()
        });
        assert_eq!(
            plan_lines(&localizer, "tree.ged", &unchanged, true),
            vec![
                "Plan for tree.ged:".to_owned(),
                "  persons: 2 unchanged".to_owned(),
                "Every record is already on record: importing this file writes nothing.".to_owned(),
            ]
        );
        let matched = summary(PlanCounts {
            new: 1,
            candidates: 2,
            ..PlanCounts::default()
        });
        let lines = plan_lines(&localizer, "tree.ged", &matched, true);
        assert_eq!(lines[1], "  persons: 1 new, 2 with possible matches");
        assert!(lines[2].contains("--defer-matches"), "{lines:?}");
        assert_eq!(
            plan_lines(&localizer, "tree.ged", &matched, false).len(),
            2,
            "no hint when reviewing"
        );
    }

    #[test]
    fn a_json_plan_names_each_kind_and_count() {
        let json = plan_json(
            "tree.ged",
            &summary(PlanCounts {
                unchanged: 2,
                ..PlanCounts::default()
            }),
        );
        assert_eq!(
            json,
            r#"{"source":"tree.ged","kinds":[{"kind":"person","new":0,"unchanged":2,"updated":0,"linked":0,"candidates":0,"withheld":0}],"candidates":0,"records":[]}"#
        );
    }
}
