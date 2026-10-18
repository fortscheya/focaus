//! Deterministic registered algorithms. No model code, eval, URL or GPU context.
use crate::storage::{self, check_revision, decode, sql_error, Result, Store};
use focaus_contracts::*;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    sync::Mutex,
    time::{Duration, Instant},
};

pub const ALGORITHM_VERSION: &str = "1.0.0";
fn invalid(message: &str) -> SafeError {
    error(ErrorCode::InvalidArgument, message)
}
fn finite(x: f64) -> Result<f64> {
    if x.is_finite() {
        Ok(x)
    } else {
        Err(invalid("Non-finite computation result"))
    }
}
fn check_budget(start: Instant) -> Result<()> {
    if start.elapsed() > Duration::from_millis(100) {
        Err(error(
            ErrorCode::DeadlineExceeded,
            "Interactive computation budget exceeded",
        ))
    } else {
        Ok(())
    }
}
pub fn evaluate_template(
    template: &str,
    input: &VisualizationEvaluateInput,
) -> Result<EvaluationResult> {
    if input.algorithm_version != ALGORITHM_VERSION {
        return Err(error(
            ErrorCode::UnsupportedVersion,
            "Unsupported algorithm version",
        ));
    }
    let parameter_type = match template {
        "function_curve_2d" => "CurveParameters",
        "conv2d_spatial" => "Conv2dParameters",
        _ => {
            return Err(error(
                ErrorCode::UnsupportedCapability,
                "Unregistered visualization template",
            ))
        }
    };
    if serde_json::to_vec(&input.parameters)
        .map_err(|_| invalid("Invalid parameter JSON"))?
        .len()
        > 2_000_000
    {
        return Err(invalid("Parameter input budget exceeded"));
    }
    validate(parameter_type, &input.parameters)?;
    let start = Instant::now();
    let (shape, values, steps) = match template {
        "function_curve_2d" => {
            let p: CurveParameters = serde_json::from_value(input.parameters.clone())
                .map_err(|_| invalid("Invalid curve parameters"))?;
            for x in [p.slope, p.intercept, p.x_min, p.x_max] {
                finite(x)?;
            }
            if p.x_min >= p.x_max
                || p.samples < 2
                || p.samples > 4096
                || p.formula_id != "affine_v1"
            {
                return Err(invalid("Invalid curve domain or sample budget"));
            }
            let mut points = Vec::with_capacity(p.samples as usize * 2);
            for i in 0..p.samples {
                // Endpoint-inclusive interpolation does not accumulate a delta.
                let t = i as f64 / (p.samples - 1) as f64;
                let x = finite(p.x_min * (1. - t) + p.x_max * t)?;
                let y = finite(p.slope * x + p.intercept)?;
                points.extend([x, y]);
            }
            (vec![p.samples, 2], points, Vec::new())
        }
        "conv2d_spatial" => {
            let p: Conv2dParameters = serde_json::from_value(input.parameters.clone())
                .map_err(|_| invalid("Invalid convolution parameters"))?;
            if p.axis_order != "CHW"
                || p.operator_dimension != 2
                || p.scene_dimension != 3
                || p.stride.len() != 2
                || p.padding.len() != 4
                || p.dilation != [1, 1]
                || p.stride.contains(&0)
            {
                return Err(invalid(
                    "Unsupported convolution dimensions, stride or dilation",
                ));
            }
            let channels = p.input.len();
            let h = p.input.first().map_or(0, Vec::len);
            let w = p.input.first().and_then(|c| c.first()).map_or(0, Vec::len);
            let kh = p.kernel.first().map_or(0, Vec::len);
            let kw = p.kernel.first().and_then(|c| c.first()).map_or(0, Vec::len);
            if channels == 0 || h == 0 || w == 0 || kh == 0 || kw == 0 || p.kernel.len() != channels
            {
                return Err(invalid("Empty or mismatched channel shape"));
            }
            for (tensor, rows, cols) in [(&p.input, h, w), (&p.kernel, kh, kw)] {
                if tensor.iter().any(|c| {
                    c.len() != rows
                        || c.iter()
                            .any(|r| r.len() != cols || r.iter().any(|v| !v.is_finite()))
                }) {
                    return Err(invalid("Ragged or non-finite tensor"));
                }
            }
            let elements = channels
                .checked_mul(
                    h.checked_mul(w)
                        .ok_or_else(|| invalid("Tensor shape overflow"))?,
                )
                .and_then(|x| x.checked_add(channels.checked_mul(kh)?.checked_mul(kw)?))
                .ok_or_else(|| invalid("Tensor shape overflow"))?;
            if elements > 65536 {
                return Err(invalid("Tensor element budget exceeded"));
            }
            let ph = (h as u64)
                .checked_add(p.padding[0])
                .and_then(|x| x.checked_add(p.padding[1]))
                .ok_or_else(|| invalid("Padding overflow"))?;
            let pw = (w as u64)
                .checked_add(p.padding[2])
                .and_then(|x| x.checked_add(p.padding[3]))
                .ok_or_else(|| invalid("Padding overflow"))?;
            if ph < kh as u64 || pw < kw as u64 {
                return Err(invalid("Kernel exceeds padded input"));
            }
            let oh = (ph - kh as u64) / p.stride[0] + 1;
            let ow = (pw - kw as u64) / p.stride[1] + 1;
            let operations = oh
                .checked_mul(ow)
                .and_then(|x| x.checked_mul(channels as u64))
                .and_then(|x| x.checked_mul(kh as u64))
                .and_then(|x| x.checked_mul(kw as u64))
                .ok_or_else(|| invalid("Convolution operation overflow"))?;
            if operations > 1_000_000
                || oh.checked_mul(ow).is_none_or(|x| x > 65536)
                || p.padding.iter().any(|v| *v > 65536)
            {
                return Err(invalid("Convolution computation budget exceeded"));
            }
            let bias = finite(p.bias.unwrap_or(0.))?;
            let flipped = p.operator_semantics == Conv2dParametersOperatorSemantics::Convolution;
            let mut values = Vec::with_capacity((oh * ow) as usize);
            let mut steps = Vec::with_capacity((oh * ow) as usize);
            for y in 0..oh {
                for x in 0..ow {
                    check_budget(start)?;
                    let mut step = EvaluationStep {
                        output_index: vec![y, x],
                        input_indices: Vec::new(),
                        input_values: Vec::new(),
                        kernel_values: Vec::new(),
                        products: Vec::new(),
                        bias,
                        sum: bias,
                    };
                    for c in 0..channels {
                        for ky in 0..kh {
                            for kx in 0..kw {
                                let iy = (y * p.stride[0]) as i64 + ky as i64 - p.padding[0] as i64;
                                let ix = (x * p.stride[1]) as i64 + kx as i64 - p.padding[2] as i64;
                                let input_value =
                                    if iy >= 0 && ix >= 0 && iy < (h as i64) && ix < (w as i64) {
                                        p.input[c][iy as usize][ix as usize]
                                    } else {
                                        0.
                                    };
                                let kernel = p.kernel[c][if flipped { kh - 1 - ky } else { ky }]
                                    [if flipped { kw - 1 - kx } else { kx }];
                                let product = finite(input_value * kernel)?;
                                step.sum = finite(step.sum + product)?;
                                step.input_indices.push(vec![c as i64, iy, ix]);
                                step.input_values.push(input_value);
                                step.kernel_values.push(kernel);
                                step.products.push(product);
                            }
                        }
                    }
                    values.push(step.sum);
                    steps.push(step);
                }
            }
            (vec![oh, ow], values, steps)
        }
        _ => {
            return Err(error(
                ErrorCode::UnsupportedCapability,
                "Unregistered visualization template",
            ))
        }
    };
    check_budget(start)?;
    Ok(EvaluationResult {
        object_id: input.object_id.clone(),
        revision_id: input.revision_id.clone(),
        interaction_seq: input.interaction_seq,
        parameters_hash: canonical_hash(&input.parameters),
        algorithm_version: ALGORITHM_VERSION.into(),
        shape,
        values,
        steps,
        validation: ValidationReport {
            checks: ValidationChecks {
                schema: CheckState::Pass,
                source: CheckState::Unknown,
                semantic: CheckState::Unknown,
                math: CheckState::Pass,
            },
            validator_version: ALGORITHM_VERSION.into(),
            parameters_hash: canonical_hash(&input.parameters),
            validated_revision: input.revision_id.clone(),
            diagnostics: Vec::new(),
        },
    })
}

pub struct Visualizations {
    store: Store,
    latest: Mutex<BTreeMap<(String, String), u64>>,
    receipts: Mutex<BTreeMap<(String, String, String), EvaluationResult>>,
}
impl Visualizations {
    pub fn new(store: Store) -> Self {
        Self {
            store,
            latest: Mutex::new(BTreeMap::new()),
            receipts: Mutex::new(BTreeMap::new()),
        }
    }
    pub fn evaluate(&self, input: VisualizationEvaluateInput) -> Result<EvaluationResult> {
        let content: VisualizationContent = self.store.read(|c| {
            check_revision(c, &input.object_id, &input.revision_id)?;
            let s: String = c
                .query_row(
                    "SELECT description_json FROM visualizations WHERE object_id=?1",
                    [&input.object_id],
                    |r| r.get(0),
                )
                .map_err(sql_error)?;
            decode(&s)
        })?;
        let key = (input.object_id.clone(), input.revision_id.clone());
        {
            let mut seq = self
                .latest
                .lock()
                .map_err(|_| invalid("Computation state unavailable"))?;
            if seq.get(&key).is_some_and(|s| *s > input.interaction_seq) {
                return Err(error(
                    ErrorCode::StaleInput,
                    "Newer parameter interaction exists",
                ));
            }
            seq.insert(key.clone(), input.interaction_seq);
        }
        let result = evaluate_template(&content.template_id, &input)?;
        self.store
            .read(|c| check_revision(c, &input.object_id, &input.revision_id))?;
        if self
            .latest
            .lock()
            .map_err(|_| invalid("Computation state unavailable"))?
            .get(&key)
            != Some(&input.interaction_seq)
        {
            return Err(error(ErrorCode::StaleInput, "Computation superseded"));
        }
        let mut receipts = self
            .receipts
            .lock()
            .map_err(|_| invalid("Computation state unavailable"))?;
        // Bounded proof cache; eviction requires a fresh evaluation, never trust F.
        if receipts.len() >= 128 {
            receipts.clear();
        }
        receipts.insert(
            (
                input.object_id,
                input.revision_id,
                result.parameters_hash.clone(),
            ),
            result.clone(),
        );
        Ok(result)
    }
    pub fn validate(&self, input: VisualizationValidateInput) -> Result<ValidationReport> {
        let content = input.candidate;
        validate("VisualizationContent", &storage::value(&content)?)?;
        if content.registry_revision
            != focaus_contracts::schema()["x-registry"]["registry_revision"]
                .as_str()
                .unwrap_or("")
        {
            return Err(error(
                ErrorCode::UnsupportedVersion,
                "Visualization registry changed",
            ));
        }
        let evaluation = evaluate_template(
            &content.template_id,
            &VisualizationEvaluateInput {
                object_id: content.object_id.clone(),
                revision_id: content.revision_id.clone(),
                parameters: content.parameters.clone(),
                interaction_seq: 0,
                algorithm_version: content.computation.algorithm_version.clone(),
            },
        )?;
        if evaluation.shape != content.computation.output_shape {
            return Err(error(
                ErrorCode::InvalidResult,
                "Declared output shape differs from deterministic result",
            ));
        }
        let algorithm = focaus_contracts::schema()["x-registry"]["templates"][&content.template_id]
            ["algorithm_id"]
            .as_str()
            .unwrap_or("")
            .to_owned();
        if algorithm != content.computation.algorithm_id {
            return Err(error(
                ErrorCode::UnsupportedCapability,
                "Algorithm/template mismatch",
            ));
        }
        self.store.read(|c| {
            for source in &content.source_refs {
                crate::library::check_source(c, source)?;
            }
            Ok(())
        })?;
        Ok(ValidationReport {
            checks: ValidationChecks {
                schema: CheckState::Pass,
                source: if content.source_refs.is_empty() {
                    CheckState::Unknown
                } else {
                    CheckState::Pass
                },
                semantic: input.semantic_check,
                math: CheckState::Pass,
            },
            validator_version: ALGORITHM_VERSION.into(),
            parameters_hash: evaluation.parameters_hash,
            validated_revision: content.revision_id,
            diagnostics: Vec::new(),
        })
    }
    pub(crate) fn require_receipt(
        &self,
        object: &str,
        revision: &str,
        parameters: &Value,
        claimed: Option<&str>,
    ) -> Result<()> {
        let hash = canonical_hash(parameters);
        if claimed != Some(hash.as_str())
            || !self
                .receipts
                .lock()
                .map_err(|_| invalid("Computation state unavailable"))?
                .contains_key(&(object.into(), revision.into(), hash))
        {
            return Err(error(ErrorCode::InvalidResult, "validation_required"));
        }
        Ok(())
    }
}
