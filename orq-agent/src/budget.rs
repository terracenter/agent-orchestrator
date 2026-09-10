//! Presupuesto real en dinero para gating de `exec`/`delegate` (issue #183).
//!
//! El unico control de gasto previo (`approval_required_model_patterns` en
//! `policy.json`) dependia de substrings en el nombre del modelo
//! (`sonnet`/`opus`). Cualquier modelo caro futuro que no contenga esos
//! substrings pasaba sin aprobacion. Este modulo agrega un segundo gate
//! ortogonal, basado en `cost_hint` (USD estimados por ejecucion) del
//! catalogo de modelos contra un techo diario/mensual configurado, en vez de
//! en el nombre del modelo. La politica de patrones existente se mantiene
//! como fallback: si el modelo no tiene `cost_hint` configurado, este gate
//! no bloquea nada y `approval_required_model_patterns` sigue siendo la
//! unica autoridad.

use color_eyre::eyre::{eyre, Result, WrapErr};
use serde::{Deserialize, Serialize};
use std::path::Path;

const SUPPORTED_SCHEMA_VERSION: u8 = 1;
pub const BUILTIN_BUDGET_JSON: &str = include_str!("../config/budget.json");

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct BudgetConfig {
    pub schema_version: u8,
    pub currency: String,
    #[serde(default)]
    pub daily_limit_usd: Option<f64>,
    #[serde(default)]
    pub monthly_limit_usd: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LoadedBudget {
    pub config: BudgetConfig,
    pub source: String,
    pub path: String,
}

#[derive(Debug, Serialize)]
pub struct BudgetDecision {
    pub allowed: bool,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_cost_usd: Option<f64>,
    pub spent_today_usd: f64,
    pub spent_month_usd: f64,
}

pub fn default_config() -> Result<BudgetConfig> {
    parse_config(BUILTIN_BUDGET_JSON)
}

pub fn default_loaded_budget() -> Result<LoadedBudget> {
    Ok(LoadedBudget {
        config: default_config()?,
        source: "builtin".to_string(),
        path: "builtin".to_string(),
    })
}

/// Carga la config de presupuesto. Igual que `policy::load_config`, un
/// `path` explicito (flag CLI) es la unica forma de sobreescribir el
/// builtin: no se consulta ninguna variable de entorno, para que el techo de
/// gasto no pueda cambiarse silenciosamente desde el shell.
pub async fn load_config(path: Option<&Path>) -> Result<LoadedBudget> {
    match path {
        None => default_loaded_budget(),
        Some(path) => {
            let content = tokio::fs::read_to_string(path)
                .await
                .wrap_err_with(|| format!("reading budget config {}", path.display()))?;
            let config = parse_config(&content)?;
            Ok(LoadedBudget {
                config,
                source: "override".to_string(),
                path: path.display().to_string(),
            })
        }
    }
}

pub fn parse_config(content: &str) -> Result<BudgetConfig> {
    let config: BudgetConfig =
        serde_json::from_str(content).wrap_err("parsing budget config json")?;
    validate_config(&config)?;
    Ok(config)
}

fn validate_config(config: &BudgetConfig) -> Result<()> {
    if config.schema_version != SUPPORTED_SCHEMA_VERSION {
        return Err(eyre!(
            "unsupported budget schema_version {}; expected {}",
            config.schema_version,
            SUPPORTED_SCHEMA_VERSION
        ));
    }
    if config.currency.trim().is_empty() {
        return Err(eyre!("budget config must define a non-empty currency"));
    }
    if let Some(limit) = config.daily_limit_usd {
        if limit <= 0.0 || limit.is_nan() {
            return Err(eyre!("daily_limit_usd must be greater than 0 when set"));
        }
    }
    if let Some(limit) = config.monthly_limit_usd {
        if limit <= 0.0 || limit.is_nan() {
            return Err(eyre!("monthly_limit_usd must be greater than 0 when set"));
        }
    }
    Ok(())
}

/// Evalua si una ejecucion cabe dentro del presupuesto configurado.
///
/// `model_cost_hint_usd` es el `cost_hint` numerico del catalogo de modelos
/// (`ModelCandidate::cost_hint`) para el agente/modelo objetivo, interpretado
/// como el costo estimado en `config.currency` de una ejecucion. Es una
/// estimacion, no facturacion exacta (issue #183, alcance de vertical
/// slice): no hay reconciliacion contra costo real facturado todavia.
///
/// Si el modelo no tiene `cost_hint` configurado, o si ningun techo esta
/// configurado, el gate no bloquea: `approval_required_model_patterns`
/// (policy.rs) sigue siendo la unica autoridad para ese caso.
pub fn evaluate(
    model_cost_hint_usd: Option<f64>,
    config: &BudgetConfig,
    spent_today_usd: f64,
    spent_month_usd: f64,
) -> BudgetDecision {
    let Some(cost) = model_cost_hint_usd else {
        return BudgetDecision {
            allowed: true,
            reason: "no cost_hint configured for model; budget gating skipped, \
                approval_required_model_patterns sigue siendo la autoridad"
                .to_string(),
            estimated_cost_usd: None,
            spent_today_usd,
            spent_month_usd,
        };
    };

    if let Some(daily_limit) = config.daily_limit_usd {
        let projected = spent_today_usd + cost;
        if projected > daily_limit {
            return BudgetDecision {
                allowed: false,
                reason: format!(
                    "estimated cost {:.4} {currency} would bring today's spend to {:.4}, \
                        exceeding daily_limit_usd {:.4}",
                    cost,
                    projected,
                    daily_limit,
                    currency = config.currency
                ),
                estimated_cost_usd: Some(cost),
                spent_today_usd,
                spent_month_usd,
            };
        }
    }

    if let Some(monthly_limit) = config.monthly_limit_usd {
        let projected = spent_month_usd + cost;
        if projected > monthly_limit {
            return BudgetDecision {
                allowed: false,
                reason: format!(
                    "estimated cost {:.4} {currency} would bring this month's spend to {:.4}, \
                        exceeding monthly_limit_usd {:.4}",
                    cost,
                    projected,
                    monthly_limit,
                    currency = config.currency
                ),
                estimated_cost_usd: Some(cost),
                spent_today_usd,
                spent_month_usd,
            };
        }
    }

    BudgetDecision {
        allowed: true,
        reason: "within configured budget".to_string(),
        estimated_cost_usd: Some(cost),
        spent_today_usd,
        spent_month_usd,
    }
}

#[cfg(test)]
mod tests {
    use super::{default_config, default_loaded_budget, evaluate, load_config, parse_config};

    #[test]
    fn default_budget_has_no_limits() {
        let config = default_config().unwrap();
        assert_eq!(config.daily_limit_usd, None);
        assert_eq!(config.monthly_limit_usd, None);
        assert_eq!(config.currency, "USD");
    }

    #[tokio::test]
    async fn default_loaded_budget_is_builtin() {
        let loaded = default_loaded_budget().unwrap();
        assert_eq!(loaded.source, "builtin");
        assert_eq!(loaded.path, "builtin");
    }

    #[test]
    fn expensive_model_without_sonnet_or_opus_in_name_exceeds_daily_budget() {
        let config = super::BudgetConfig {
            schema_version: 1,
            currency: "USD".to_string(),
            daily_limit_usd: Some(1.0),
            monthly_limit_usd: Some(20.0),
        };
        // "kimi-k2.5-ultra-max" no contiene "sonnet" ni "opus": el patron
        // legacy nunca lo hubiera bloqueado.
        let decision = evaluate(Some(5.0), &config, 0.0, 0.0);
        assert!(!decision.allowed);
        assert!(decision.reason.contains("daily_limit_usd"));
        assert_eq!(decision.estimated_cost_usd, Some(5.0));
    }

    #[test]
    fn cheap_model_under_budget_is_allowed() {
        let config = super::BudgetConfig {
            schema_version: 1,
            currency: "USD".to_string(),
            daily_limit_usd: Some(1.0),
            monthly_limit_usd: Some(20.0),
        };
        let decision = evaluate(Some(0.01), &config, 0.0, 0.0);
        assert!(decision.allowed);
        assert_eq!(decision.estimated_cost_usd, Some(0.01));
    }

    #[test]
    fn accumulated_spend_pushes_a_call_over_the_daily_limit() {
        let config = super::BudgetConfig {
            schema_version: 1,
            currency: "USD".to_string(),
            daily_limit_usd: Some(1.0),
            monthly_limit_usd: None,
        };
        // Ya se gastaron 0.95; una llamada de 0.10 mas excede el techo
        // aunque el costo individual sea bajo.
        let decision = evaluate(Some(0.10), &config, 0.95, 0.0);
        assert!(!decision.allowed);
    }

    #[test]
    fn monthly_limit_blocks_even_when_daily_limit_is_not_configured() {
        let config = super::BudgetConfig {
            schema_version: 1,
            currency: "USD".to_string(),
            daily_limit_usd: None,
            monthly_limit_usd: Some(10.0),
        };
        let decision = evaluate(Some(11.0), &config, 0.0, 0.0);
        assert!(!decision.allowed);
        assert!(decision.reason.contains("monthly_limit_usd"));
    }

    #[test]
    fn model_without_cost_hint_is_never_blocked_by_budget() {
        let config = super::BudgetConfig {
            schema_version: 1,
            currency: "USD".to_string(),
            daily_limit_usd: Some(0.0001),
            monthly_limit_usd: Some(0.0001),
        };
        let decision = evaluate(None, &config, 999.0, 999.0);
        assert!(decision.allowed);
        assert_eq!(decision.estimated_cost_usd, None);
    }

    #[test]
    fn no_limits_configured_never_blocks_regardless_of_cost() {
        let config = default_config().unwrap();
        let decision = evaluate(Some(999.0), &config, 0.0, 0.0);
        assert!(decision.allowed);
    }

    #[test]
    fn rejects_unsupported_schema_version() {
        let err = parse_config(r#"{"schema_version":2,"currency":"USD"}"#).unwrap_err();
        assert!(err.to_string().contains("schema_version"));
    }

    #[test]
    fn rejects_empty_currency() {
        let err = parse_config(r#"{"schema_version":1,"currency":""}"#).unwrap_err();
        assert!(err.to_string().contains("currency"));
    }

    #[test]
    fn rejects_zero_or_negative_limits() {
        let err = parse_config(
            r#"{"schema_version":1,"currency":"USD","daily_limit_usd":0,"monthly_limit_usd":null}"#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("daily_limit_usd"));

        let err = parse_config(
            r#"{"schema_version":1,"currency":"USD","daily_limit_usd":null,"monthly_limit_usd":-5.0}"#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("monthly_limit_usd"));
    }

    #[tokio::test]
    async fn load_config_none_uses_builtin() {
        let loaded = load_config(None).await.unwrap();
        assert_eq!(loaded.source, "builtin");
        assert_eq!(loaded.path, "builtin");
    }

    #[tokio::test]
    async fn load_config_with_valid_override() {
        use std::io::Write;
        let mut temp = tempfile::NamedTempFile::new().unwrap();
        writeln!(
            temp,
            r#"{{"schema_version":1,"currency":"USD","daily_limit_usd":2.5,"monthly_limit_usd":50.0}}"#
        )
        .unwrap();

        let loaded = load_config(Some(temp.path())).await.unwrap();
        assert_eq!(loaded.source, "override");
        assert_eq!(loaded.path, temp.path().display().to_string());
        assert_eq!(loaded.config.daily_limit_usd, Some(2.5));
        assert_eq!(loaded.config.monthly_limit_usd, Some(50.0));
    }
}
