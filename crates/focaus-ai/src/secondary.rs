//! Secondary transcript, correction and language-variant proposal helpers.
//!
//! The secondary layer is a source-preserving cleanup.  It may remove verbal
//! filler and repair punctuation, but it cannot paraphrase the teacher into a
//! third-person summary.  Critical semantic corrections remain suggestions
//! for user review.

use crate::{invalid, invalid_argument, models, PROMPT_SECONDARY_V1};
use focaus_contracts::{
    CorrectionSuggestion, CorrectionSuggestionSemanticRisk, CorrectionSuggestionState,
    Definition, EntityId, InputSnapshot, LanguageVariant, LanguageVariantSyncStatus, ModelId,
    Selector, SafeError, SecondarySegment, SourceRef, SymbolBinding, TaskKind, UserPatch,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;

const FILLERS: &[&str] = &["嗯", "呃", "啊", "这个", "那个", "you know", "um", "uh"];
const THIRD_PERSON_MARKERS: &[&str] = &["老师说", "老师认为", "他说", "她说", "他说道", "the teacher says", "he says", "she says"];
const NEGATION_MARKERS: &[&str] = &["不", "无", "非", "未", "没有", "不能", "不会", "not", "never", "without"];
const CONDITION_MARKERS: &[&str] = &["如果", "若", "只有", "当", "则", "否则", "if", "unless", "only if"];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawTextUnit {
    pub utterance_id: EntityId,
    pub recording_id: EntityId,
    pub text: String,
    pub source_ref: SourceRef,
    pub span_id: EntityId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SecondaryDraft {
    pub recording_id: EntityId,
    pub raw_refs: Vec<SourceRef>,
    pub ordered_spans: Vec<EntityId>,
    pub text: String,
    pub suggestions: Vec<CorrectionSuggestion>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrimmedText {
    pub text: String,
    pub removed_fillers: Vec<String>,
}

pub fn validate_secondary_snapshot(snapshot: &InputSnapshot) -> Result<(), SafeError> {
    if snapshot.purpose != TaskKind::Secondary
        || snapshot.model_id != Some(ModelId::Qwen38Flash)
        || snapshot.region.as_deref() != Some(models::REGION)
        || snapshot.prompt_version.as_deref() != Some(PROMPT_SECONDARY_V1)
    {
        return Err(invalid_argument("secondary_snapshot_binding_mismatch"));
    }
    Ok(())
}

/// Deterministic cleanup used for a local fallback and as a semantic guard on
/// model output.  It keeps the original order and does not invent wording.
pub fn trim_direct_speech(text: &str) -> TrimmedText {
    let mut result = text.to_string();
    let mut removed = Vec::new();
    for filler in FILLERS {
        if result.contains(filler) {
            removed.push((*filler).to_string());
            result = result.replace(filler, "");
        }
    }
    // Repeated punctuation is a presentation cleanup, not a semantic edit.
    while result.contains("。。") {
        result = result.replace("。。", "。");
    }
    while result.contains("，，") {
        result = result.replace("，，", "，");
    }
    TrimmedText { text: result.split_whitespace().collect::<Vec<_>>().join(" ").trim().to_string(), removed_fillers: removed }
}

pub fn validate_secondary_draft(
    draft: &SecondaryDraft,
    raw_units: &[RawTextUnit],
) -> Result<(), SafeError> {
    if draft.recording_id.is_empty() || draft.raw_refs.is_empty() || draft.text.trim().is_empty() {
        return Err(invalid("secondary_draft_missing_identity_or_source"));
    }
    let raw_text = raw_units
        .iter()
        .filter(|unit| unit.recording_id == draft.recording_id)
        .map(|unit| unit.text.as_str())
        .collect::<String>();
    if raw_text.is_empty() {
        return Err(invalid("secondary_raw_source_unavailable"));
    }
    let source_normalized = strip_non_content(&raw_text);
    let draft_normalized = strip_non_content(&draft.text);
    if !is_subsequence(&draft_normalized, &source_normalized) {
        return Err(invalid("secondary_output_is_paraphrase_or_reordered"));
    }
    for marker in THIRD_PERSON_MARKERS {
        if draft.text.to_lowercase().contains(&marker.to_lowercase())
            && !raw_text.to_lowercase().contains(&marker.to_lowercase())
        {
            return Err(invalid("secondary_third_person_rewrite"));
        }
    }
    for suggestion in &draft.suggestions {
        if suggestion.original_text.trim().is_empty() || suggestion.suggested_text.trim().is_empty() {
            return Err(invalid("secondary_correction_text_missing"));
        }
        let expected = classify_semantic_risk(&suggestion.original_text, &suggestion.suggested_text);
        if suggestion.semantic_risk != expected {
            return Err(invalid("secondary_correction_risk_mismatch"));
        }
        if suggestion.state != CorrectionSuggestionState::Pending {
            return Err(invalid("secondary_draft_contains_applied_correction"));
        }
    }
    Ok(())
}

fn strip_non_content(text: &str) -> Vec<char> {
    text.chars().filter(|ch| !ch.is_whitespace() && !"，。！？；：,.!?;:".contains(*ch)).collect()
}

fn is_subsequence(candidate: &[char], source: &[char]) -> bool {
    if candidate.is_empty() {
        return false;
    }
    let mut index = 0;
    for ch in candidate {
        let Some(found) = source[index..].iter().position(|source_ch| source_ch == ch) else { return false };
        index += found + 1;
    }
    true
}

pub fn classify_semantic_risk(original: &str, suggested: &str) -> CorrectionSuggestionSemanticRisk {
    if number_tokens(original) != number_tokens(suggested)
        || polarity_tokens(original) != polarity_tokens(suggested)
        || condition_tokens(original) != condition_tokens(suggested)
        || formula_shape(original) != formula_shape(suggested)
    {
        CorrectionSuggestionSemanticRisk::Critical
    } else {
        CorrectionSuggestionSemanticRisk::Ordinary
    }
}

fn number_tokens(text: &str) -> Vec<String> {
    let mut output = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_ascii_digit() || ch == '.' || ch == '%' {
            current.push(ch);
        } else if !current.is_empty() {
            output.push(current.clone());
            current.clear();
        }
    }
    if !current.is_empty() { output.push(current); }
    output
}

fn polarity_tokens(text: &str) -> Vec<&'static str> {
    NEGATION_MARKERS.iter().copied().filter(|marker| text.to_lowercase().contains(&marker.to_lowercase())).collect()
}

fn condition_tokens(text: &str) -> Vec<&'static str> {
    CONDITION_MARKERS.iter().copied().filter(|marker| text.to_lowercase().contains(&marker.to_lowercase())).collect()
}

fn formula_shape(text: &str) -> String {
    text.chars().filter(|ch| "=+-*/^()<>".contains(*ch)).collect()
}

pub fn build_secondary_segment(
    secondary_id: impl Into<String>,
    generation_revision: impl Into<String>,
    revision_id: impl Into<String>,
    units: &[RawTextUnit],
) -> Result<SecondarySegment, SafeError> {
    if units.is_empty() {
        return Err(invalid_argument("secondary_units_required"));
    }
    let recording_id = units[0].recording_id.clone();
    if units.iter().any(|unit| unit.recording_id != recording_id) {
        return Err(invalid("secondary_mixes_recordings"));
    }
    let mut text_parts = Vec::new();
    let mut refs = Vec::new();
    let mut spans = Vec::new();
    for unit in units {
        let trimmed = trim_direct_speech(&unit.text);
        if !trimmed.text.is_empty() {
            text_parts.push(trimmed.text);
        }
        refs.push(unit.source_ref.clone());
        spans.push(unit.span_id.clone());
    }
    let draft = SecondaryDraft { recording_id: recording_id.clone(), raw_refs: refs.clone(), ordered_spans: spans.clone(), text: text_parts.join(" "), suggestions: vec![] };
    validate_secondary_draft(&draft, units)?;
    Ok(SecondarySegment { secondary_id: secondary_id.into(), recording_id, raw_refs: refs, ordered_spans: spans, text: draft.text, suggestions: vec![], generation_revision: generation_revision.into(), revision_id: revision_id.into() })
}

pub fn make_correction(
    suggestion_id: impl Into<String>,
    secondary_id: impl Into<String>,
    source_revision: impl Into<String>,
    anchor: focaus_contracts::TextAnchor,
    original_text: impl Into<String>,
    suggested_text: impl Into<String>,
    reason: impl Into<String>,
) -> CorrectionSuggestion {
    let original_text = original_text.into();
    let suggested_text = suggested_text.into();
    CorrectionSuggestion { suggestion_id: suggestion_id.into(), secondary_id: secondary_id.into(), source_revision: source_revision.into(), anchor, original_text: original_text.clone(), suggested_text: suggested_text.clone(), reason: reason.into(), semantic_risk: classify_semantic_risk(&original_text, &suggested_text), state: CorrectionSuggestionState::Pending }
}

pub fn validate_five_tier_variants(
    note_id: &str,
    fact_set_revision: &str,
    fact_ids: &[EntityId],
    variants: &[LanguageVariant],
) -> Result<(), SafeError> {
    if variants.len() != 5 {
        return Err(invalid("language_variants_must_have_five_tiers"));
    }
    let expected: BTreeSet<String> = fact_ids.iter().cloned().collect();
    let mut tiers = BTreeSet::new();
    for variant in variants {
        if variant.note_id != note_id || variant.fact_set_revision != fact_set_revision || variant.fact_ids.iter().cloned().collect::<BTreeSet<_>>() != expected || !(1..=5).contains(&variant.tier) {
            return Err(invalid("language_variant_fact_set_mismatch"));
        }
        if !tiers.insert(variant.tier) {
            return Err(invalid("duplicate_language_tier"));
        }
    }
    Ok(())
}

/// The host supplies exact selectors and user patches.  AI treats an exact or
/// overlapping protected selector as review-required; it never silently
/// rewrites an active composition or a remembered delete intent.
pub fn proposal_intersects_user_patch(
    target_id: &str,
    selector: &Selector,
    patches: &[UserPatch],
) -> bool {
    patches.iter().any(|patch| {
        patch.target_id == target_id && selectors_intersect(selector, &patch.selector)
    })
}

fn selectors_intersect(left: &Selector, right: &Selector) -> bool {
    let left_json = serde_json::to_value(left).ok();
    let right_json = serde_json::to_value(right).ok();
    if left_json == right_json {
        return true;
    }
    match (left, right) {
        (Selector::V0(a), Selector::V0(b)) => {
            a.recording_id == b.recording_id
                && a.audio_span_id == b.audio_span_id
                && a.track_id == b.track_id
                && a.start_ms < b.end_ms
                && b.start_ms < a.end_ms
        }
        (Selector::V1(a), Selector::V1(b)) => {
            a.block_id == b.block_id
                && a.anchor.scalar_range.start < b.anchor.scalar_range.end
                && b.anchor.scalar_range.start < a.anchor.scalar_range.end
        }
        (Selector::V2(a), Selector::V2(b)) => {
            a.material_id == b.material_id
                && a.source_version_id == b.source_version_id
                && a.parse_version_id == b.parse_version_id
                && a.page_id == b.page_id
                && a.region_id == b.region_id
        }
        (Selector::V3(a), Selector::V3(b)) => a.object_id == b.object_id,
        (Selector::V4(_), Selector::V4(_)) => true,
        _ => false,
    }
}

pub fn validate_symbol_binding_scope(
    binding: &SymbolBinding,
    active_scope_id: &str,
) -> Result<(), SafeError> {
    if binding.binding_id.is_empty()
        || binding.symbol.trim().is_empty()
        || binding.scope_id != active_scope_id
        || binding.definition_id.is_empty()
        || binding.valid_from_anchor.entity_id.is_empty()
    {
        return Err(invalid("symbol_binding_scope_mismatch"));
    }
    if binding.value.is_none() && binding.meaning.trim().is_empty() && binding.expression.trim().is_empty() {
        return Err(invalid("symbol_without_value_or_meaning"));
    }
    Ok(())
}

/// Resolve a definition only within the host-filtered lesson/scope.  The
/// caller may label a returned definition outside the requested window; this
/// function never imports a cross-course definition as if it were current.
pub fn choose_scoped_definition<'a>(
    definitions: &'a [Definition],
    lesson_id: &str,
    scope_id: &str,
    concept_id: &str,
) -> Option<&'a Definition> {
    definitions
        .iter()
        .filter(|definition| definition.lesson_id == lesson_id && definition.concept_id == concept_id)
        .find(|definition| definition.scope_id == scope_id)
        .or_else(|| definitions.iter().find(|definition| definition.lesson_id == lesson_id && definition.concept_id == concept_id))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArrangementProposal {
    pub kind: ArrangementKind,
    pub text: String,
    pub source_refs: Vec<SourceRef>,
    pub relative_date_status: RelativeDateStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArrangementKind {
    TeacherAssignment,
    TeacherDeadline,
    TeacherExam,
    TeacherEmphasis,
    AiInference,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelativeDateStatus {
    Resolved,
    Unknown,
    NotApplicable,
}

pub fn validate_arrangement(proposal: &ArrangementProposal) -> Result<(), SafeError> {
    if proposal.text.trim().is_empty() {
        return Err(invalid("arrangement_text_missing"));
    }
    if !matches!(proposal.kind, ArrangementKind::AiInference) && proposal.source_refs.is_empty() {
        return Err(invalid("teacher_arrangement_source_missing"));
    }
    if matches!(proposal.kind, ArrangementKind::TeacherDeadline) && proposal.relative_date_status == RelativeDateStatus::Resolved && proposal.text.contains("?") {
        return Err(invalid("resolved_deadline_contains_uncertainty"));
    }
    Ok(())
}

pub fn make_language_variant(
    variant_id: impl Into<String>,
    note_id: impl Into<String>,
    tier: u64,
    fact_set_revision: impl Into<String>,
    fact_ids: Vec<EntityId>,
    presentation_revision: impl Into<String>,
) -> Result<LanguageVariant, SafeError> {
    if !(1..=5).contains(&tier) {
        return Err(invalid_argument("language_tier_out_of_range"));
    }
    Ok(LanguageVariant { variant_id: variant_id.into(), note_id: note_id.into(), tier, fact_set_revision: fact_set_revision.into(), fact_ids, presentation_revision: presentation_revision.into(), sync_status: LanguageVariantSyncStatus::Generating })
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QaPart {
    pub class: QaClass,
    pub text: String,
    pub source_refs: Vec<SourceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QaClass {
    Classroom,
    Material,
    AiBackground,
}

pub fn validate_qa_parts(parts: &[QaPart]) -> Result<(), SafeError> {
    if parts.is_empty() {
        return Err(invalid("qa_answer_empty"));
    }
    for part in parts {
        if part.text.trim().is_empty() {
            return Err(invalid("qa_part_empty"));
        }
        if matches!(part.class, QaClass::Classroom | QaClass::Material) && part.source_refs.is_empty() {
            return Err(invalid("sourced_qa_part_missing_source"));
        }
    }
    Ok(())
}

pub fn build_secondary_request(snapshot: &InputSnapshot, raw_text: &[RawTextUnit]) -> Result<Value, SafeError> {
    validate_secondary_snapshot(snapshot)?;
    if raw_text.is_empty() {
        return Err(invalid_argument("secondary_raw_text_required"));
    }
    Ok(json!({
        "model": models::MULTIMODAL,
        "prompt_version": PROMPT_SECONDARY_V1,
        "snapshot_id": snapshot.snapshot_id,
        "raw_units": raw_text,
        "output_contract": "direct_speech_cleanup_with_review_suggestions",
        "constraints": ["no_third_person_rewrite", "preserve_order", "critical_changes_are_suggestions"]
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use focaus_contracts::{Selector, WholeSelector};

    fn source() -> SourceRef {
        SourceRef { entity_id: "u".into(), revision_id: "r".into(), content_hash: "h".into(), selector: Selector::V4(WholeSelector { kind: "whole".into() }) }
    }

    fn unit(text: &str) -> RawTextUnit {
        RawTextUnit { utterance_id: "u".into(), recording_id: "rec".into(), text: text.into(), source_ref: source(), span_id: "span".into() }
    }

    #[test]
    fn trimming_preserves_direct_speech_and_removes_fillers() {
        let result = build_secondary_segment("sec", "g", "r", &[unit("嗯 这个 theorem。。 holds")]).unwrap();
        assert_eq!(result.text, "theorem。 holds");
        assert!(result.text.contains("theorem"));
    }

    #[test]
    fn paraphrase_and_unbacked_third_person_are_rejected() {
        let bad = SecondaryDraft { recording_id: "rec".into(), raw_refs: vec![source()], ordered_spans: vec!["span".into()], text: "the teacher says it works".into(), suggestions: vec![] };
        assert!(validate_secondary_draft(&bad, &[unit("it works")]).is_err());
        let reordered = SecondaryDraft { recording_id: "rec".into(), raw_refs: vec![source()], ordered_spans: vec!["span".into()], text: "works it".into(), suggestions: vec![] };
        assert!(validate_secondary_draft(&reordered, &[unit("it works")]).is_err());
    }

    #[test]
    fn critical_changes_are_redline_suggestions() {
        let ordinary = classify_semantic_risk("the value is 2", "the value is 2");
        assert_eq!(ordinary, CorrectionSuggestionSemanticRisk::Ordinary);
        assert_eq!(classify_semantic_risk("x is 2", "x is 3"), CorrectionSuggestionSemanticRisk::Critical);
        assert_eq!(classify_semantic_risk("if x", "x"), CorrectionSuggestionSemanticRisk::Critical);
    }

    #[test]
    fn five_tiers_share_exact_fact_set() {
        let variants = (1..=5).map(|tier| make_language_variant(format!("v{tier}"), "note", tier, "facts", vec!["f1".into(), "f2".into()], format!("p{tier}")).unwrap()).collect::<Vec<_>>();
        validate_five_tier_variants("note", "facts", &["f1".into(), "f2".into()], &variants).unwrap();
        let mut bad = variants;
        bad[4].fact_ids.push("extra".into());
        assert!(validate_five_tier_variants("note", "facts", &["f1".into(), "f2".into()], &bad).is_err());
    }

    #[test]
    fn qa_keeps_classroom_and_background_separate() {
        validate_qa_parts(&[QaPart { class: QaClass::Classroom, text: "teacher said this".into(), source_refs: vec![source()] }, QaPart { class: QaClass::AiBackground, text: "additional context".into(), source_refs: vec![] }]).unwrap();
        assert!(validate_qa_parts(&[QaPart { class: QaClass::Classroom, text: "unsourced".into(), source_refs: vec![] }]).is_err());
    }

    #[test]
    fn user_patches_and_scoped_symbols_are_protected() {
        let selector = Selector::V1(focaus_contracts::TextSelector { kind: "text".into(), block_id: "00000000-0000-4000-8000-000000000020".into(), anchor: focaus_contracts::TextAnchor { left_element_id: None, right_element_id: None, element_ids: vec![], scalar_range: focaus_contracts::ScalarRange { start: 0, end: 5 } } });
        let patch = UserPatch { patch_id: "00000000-0000-4000-8000-000000000021".into(), target_id: "00000000-0000-4000-8000-000000000022".into(), base_revision: "00000000-0000-4000-8000-000000000023".into(), operation: focaus_contracts::UserPatchOperation::TextReplace, selector: selector.clone(), before_hash: "0000000000000000000000000000000000000000000000000000000000000000".into(), after_value: json!("user"), author: "user".into(), intent: "edit".into(), undo_group_id: "00000000-0000-4000-8000-000000000024".into(), created_at: "2026-01-01T00:00:00Z".into() };
        let target_id = patch.target_id.clone();
        assert!(proposal_intersects_user_patch(&target_id, &selector, &[patch]));
    }
}
