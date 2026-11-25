//! Test-guarda de ausencia de literales de id de modelo/agente (issue #264).
//!
//! # Por qué existe
//!
//! Los ids de modelo de la matriz de routing SON dinámicos (decisión Freddy
//! 2026-11-21): la matriz cambia con el catálogo vivo (CA-0) y cada test que
//! fija un id como literal se rompe con el siguiente cambio (evidencia:
//! issue #262, CI roja en `route.rs:910/1067` y `exec_cli.rs:851`).
//!
//! # Alcance del guarda
//!
//! Solo escanea los tests que dependen de la matriz REAL:
//!
//! - `src/route.rs`: los `#[test]` del bloque `#[cfg(test)]` cuyo cuerpo usa
//!   `load_default_config()` (la matriz resuelta por `resolve_path`).
//! - `tests/exec_cli.rs`: los `#[test]` cuyo cuerpo referencia la matriz de
//!   producción (`config/routing-matrix.json` via `CARGO_MANIFEST_DIR`).
//!
//! # Heurística (documentada por decisión de diseño)
//!
//! Un literal de cadena dentro de un test escaneado FALLA si:
//!
//! 1. Coincide EXACTO con un id de agente o modelo extraído de la propia
//!    matriz (`orq-agent/config/routing-matrix.json`, fuente única, leída con
//!    `serde_json`): `default_agent`, `default_model`, agentes/modelos en
//!    `cheap_sufficient`, `escalate_to` y `avoid` (parte `agent` y `model` de
//!    `agente/modelo`) y `approval_required_model_patterns`. PROHIBIDO
//!    duplicar esa lista como constante aquí: se deriva en runtime.
//! 2. Contiene una palabra clave de vendor conocida (`deepseek`, `glm`,
//!    `claude`, `gemini`, `gpt`, `qwen`, `codex`, `fable`, `opus`, `sonnet`):
//!    atrapa ids muertos que ya no están en la matriz (p. ej. `deepseek-*`,
//!    eliminado por CA-0) reintroducidos como literales.
//!
//! # Exención explícita (dominios cerrados de prueba)
//!
//! Los fixtures inline que construyen matrices PROPIAS son datos arbitrarios
//! de escenario, NO dependencias de la matriz real, y quedan fuera del
//! guarda: quedan exentos por construcción porque el guarda solo escanea los
//! tests que cargan la matriz real o el default. Ejemplos exentos: la
//! constante `LEGACY_ROUTE_CONFIG` (`tests/fixtures/routing-matrix-hermes-agy.json`)
//! en `exec_cli.rs`, los fixtures `parse_config(r#"{...}"#)` de `route.rs`
//! con `deepseek-flash`/`gemini-3.7-flash-high`, y los fake runners con
//! `deepseek-v4-pro`. Nombres de binario de escenario (`test-runner`), task
//! kinds (`documentation`) y campos de receipt (`corr-1`, `run`, `allowed`)
//! no son ids y no disparan ninguna regla.

use serde_json::Value;
use std::path::Path;

const MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");
const MATRIX_REL: &str = "config/routing-matrix.json";
const ROUTE_RS: &str = "src/route.rs";
const EXEC_CLI_RS: &str = "tests/exec_cli.rs";

/// Palabras clave de vendor para atrapar ids muertos (regla 2 del doc-comment).
const VENDOR_KEYWORDS: [&str; 10] = [
    "deepseek", "glm", "claude", "gemini", "gpt", "qwen", "codex", "fable", "opus", "sonnet",
];

/// Extrae los ids válidos de la matriz del repo (fuente única, PROHIBIDO
/// duplicar la lista como constante: se deriva con serde_json en runtime).
fn matrix_ids() -> Vec<String> {
    let path = Path::new(MANIFEST_DIR).join(MATRIX_REL);
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("el guarda necesita la matriz {}: {}", path.display(), err));
    let matrix: Value =
        serde_json::from_str(&raw).unwrap_or_else(|err| panic!("matriz ilegible: {}", err));
    let mut ids = Vec::new();
    for pattern in matrix["approval_required_model_patterns"]
        .as_array()
        .unwrap_or(&vec![])
    {
        if let Some(p) = pattern.as_str() {
            ids.push(p.to_string());
        }
    }
    for route in matrix["routes"].as_array().unwrap_or(&vec![]) {
        for field in [
            "default_agent",
            "default_model",
            "cheap_sufficient",
            "escalate_to",
        ] {
            if let Some(value) = route[field].as_str() {
                if let Some((agent, model)) = value.split_once('/') {
                    ids.push(agent.to_string());
                    ids.push(model.to_string());
                } else {
                    ids.push(value.to_string());
                }
            }
        }
        for avoid in route["avoid"].as_array().unwrap_or(&vec![]) {
            if let Some(value) = avoid.as_str() {
                if let Some((agent, model)) = value.split_once('/') {
                    ids.push(agent.to_string());
                    ids.push(model.to_string());
                }
            }
        }
    }
    assert!(
        !ids.is_empty(),
        "el guarda no pudo derivar ids de la matriz (fuente única vacía): revisar {}",
        MATRIX_REL
    );
    ids.sort();
    ids.dedup();
    ids
}

/// Lexea código Rust saltando comentarios y literales de cadena (normales y
/// raw strings) para delimitar bloques por llaves de forma fiable.
fn skip_non_code(bytes: &[u8], mut i: usize) -> usize {
    let n = bytes.len();
    while i < n {
        match bytes[i] {
            b'/' if i + 1 < n && bytes[i + 1] == b'/' => {
                while i < n && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if i + 1 < n && bytes[i + 1] == b'*' => {
                i += 2;
                while i + 1 < n && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                i = (i + 2).min(n);
            }
            b'"' => {
                i += 1;
                while i < n {
                    match bytes[i] {
                        b'\\' => i += 2,
                        b'"' => {
                            i += 1;
                            break;
                        }
                        _ => i += 1,
                    }
                }
            }
            b'r' if i + 1 < n && (bytes[i + 1] == b'"' || bytes[i + 1] == b'#') => {
                let mut hashes = 0usize;
                let mut j = i + 1;
                while j < n && bytes[j] == b'#' {
                    hashes += 1;
                    j += 1;
                }
                if j < n && bytes[j] == b'"' {
                    j += 1;
                    let mut closing = Vec::with_capacity(hashes + 1);
                    closing.push(b'"');
                    closing.extend(std::iter::repeat_n(b'#', hashes));
                    'outer: while j < n {
                        if bytes[j] == b'"' {
                            let mut k = 0usize;
                            while k < closing.len() && j + k < n && bytes[j + k] == closing[k] {
                                k += 1;
                            }
                            if k == closing.len() {
                                j += closing.len();
                                break 'outer;
                            }
                        }
                        j += 1;
                    }
                    i = j;
                } else {
                    // `r` seguido de otra cosa: identificador normal.
                    i += 1;
                }
            }
            _ => break,
        }
    }
    i
}

/// Cuerpo de cada `#[test] fn nombre() { ... }` de nivel superior del archivo
/// (delimitado por llaves de código, ignorando llaves dentro de strings).
fn test_bodies(source: &str) -> Vec<(String, String)> {
    let bytes = source.as_bytes();
    let mut tests = Vec::new();
    let mut i = 0usize;
    let n = bytes.len();
    while i < n {
        i = skip_non_code(bytes, i);
        if i >= n {
            break;
        }
        if source[i..].starts_with("#[test]") {
            // Buscar `fn nombre(` tras el atributo (puede haber atributos previos ya consumidos).
            let mut j = i + "#[test]".len();
            while j < n {
                j = skip_non_code(bytes, j);
                if source[j..].starts_with("fn ") {
                    j += 3;
                    while j < n && (bytes[j].is_ascii_whitespace() || bytes[j] == b'(') {
                        j += 1;
                    }
                    let name_start = j;
                    while j < n && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
                        j += 1;
                    }
                    let name = source[name_start..j].to_string();
                    // Saltar hasta la llave de apertura del cuerpo.
                    while j < n && bytes[j] != b'{' {
                        j = skip_non_code(bytes, j);
                        if j < n && bytes[j] != b'{' {
                            j += 1;
                        }
                    }
                    let mut depth = 0i32;
                    let body_start = j;
                    while j < n {
                        j = skip_non_code(bytes, j);
                        match bytes[j] {
                            b'{' => {
                                depth += 1;
                                j += 1;
                            }
                            b'}' => {
                                depth -= 1;
                                j += 1;
                                if depth == 0 {
                                    tests.push((name.clone(), source[body_start..j].to_string()));
                                    break;
                                }
                            }
                            _ => j += 1,
                        }
                    }
                    break;
                }
                if source[j..].starts_with("#[") {
                    // Otro atributo entre #[test] y fn: saltarlo completo.
                    while j < n && bytes[j] != b']' {
                        j += 1;
                    }
                    j += 1;
                } else {
                    j += 1;
                }
            }
            i = j.max(i + 1);
        } else {
            i += 1;
        }
    }
    tests
}

/// Literales de cadena dentro de un bloque de código Rust. Lexer propio:
/// captura strings normales y raw strings, y salta comentarios y char
/// literals (con soporte mínimo de lifetimes para no confundirlos).
fn string_literals(code: &str) -> Vec<String> {
    let bytes = code.as_bytes();
    let mut literals = Vec::new();
    let mut i = 0usize;
    let n = bytes.len();
    while i < n {
        match bytes[i] {
            b'"' => {
                let start = i;
                i += 1;
                while i < n {
                    match bytes[i] {
                        b'\\' => i += 2,
                        b'"' => {
                            i += 1;
                            break;
                        }
                        _ => i += 1,
                    }
                }
                literals.push(code[start..i.min(n)].to_string());
            }
            b'r' if i + 1 < n && (bytes[i + 1] == b'"' || bytes[i + 1] == b'#') => {
                let start = i;
                let end = skip_non_code(bytes, i);
                literals.push(code[start..end].to_string());
                i = end;
            }
            b'/' if i + 1 < n && (bytes[i + 1] == b'/' || bytes[i + 1] == b'*') => {
                i = skip_non_code(bytes, i);
            }
            b'\''
                if i + 2 < n
                    && (bytes[i + 2] == b'\''
                        || (bytes[i + 1] == b'\\' && i + 3 < n && bytes[i + 3] == b'\'')) =>
            {
                // char literal: saltarlo para no confundirlo con string.
                i += if bytes[i + 1] == b'\\' { 4 } else { 3 };
            }
            _ => i += 1,
        }
    }
    literals
}

fn violates(literal: &str, matrix_ids: &[String]) -> Option<String> {
    let bare = literal.trim_matches(|c| c == '"' || c == '#');
    if matrix_ids.iter().any(|id| id == bare) {
        return Some(format!(
            "id exacto de la matriz usado como literal: {}",
            literal
        ));
    }
    let lower = bare.to_lowercase();
    for keyword in VENDOR_KEYWORDS {
        if lower.contains(keyword) {
            return Some(format!(
                "id de vendor no derivado de la matriz (contiene '{}'): {}",
                keyword, literal
            ));
        }
    }
    None
}

#[test]
fn no_model_agent_literals_in_matrix_dependent_tests() {
    let ids = matrix_ids();

    let mut violations = Vec::new();

    // src/route.rs: solo los #[test] que cargan la matriz real (load_default_config).
    let route_rs = std::fs::read_to_string(Path::new(MANIFEST_DIR).join(ROUTE_RS))
        .unwrap_or_else(|err| panic!("leyendo {}: {}", ROUTE_RS, err));
    for (name, body) in test_bodies(&route_rs) {
        if !body.contains("load_default_config") {
            continue; // fixture inline con matriz propia: exento (ver doc-comment).
        }
        for literal in string_literals(&body) {
            if let Some(reason) = violates(&literal, &ids) {
                violations.push(format!("{} :: {} :: {}", ROUTE_RS, name, reason));
            }
        }
    }

    // tests/exec_cli.rs: solo los #[test] que referencian la matriz de producción.
    let exec_cli = std::fs::read_to_string(Path::new(MANIFEST_DIR).join(EXEC_CLI_RS))
        .unwrap_or_else(|err| panic!("leyendo {}: {}", EXEC_CLI_RS, err));
    for (name, body) in test_bodies(&exec_cli) {
        if !body.contains("config/routing-matrix.json") {
            continue; // fixture propio (LEGACY_ROUTE_CONFIG, provider cache, etc.): exento.
        }
        for literal in string_literals(&body) {
            if let Some(reason) = violates(&literal, &ids) {
                violations.push(format!("{} :: {} :: {}", EXEC_CLI_RS, name, reason));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "literales de id de modelo/agente en tests que dependen de la matriz real\n\
         (deben derivarse de {} — ver issue #264):\n{}",
        MATRIX_REL,
        violations.join("\n")
    );
}
