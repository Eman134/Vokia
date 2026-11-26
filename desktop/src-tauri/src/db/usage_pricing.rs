pub const PRICING_VERSION: &str = "2026-04-01";

const TOKENS_PER_MILLION: f64 = 1_000_000.0;
const DEFAULT_LONG_CONTEXT_THRESHOLD: u64 = 200_000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TokenCounts {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub cache_write_tokens: u64,
    pub output_tokens: u64,
    pub thought_tokens: u64,
}

impl TokenCounts {
    pub fn total_context_input(self) -> u64 {
        self.input_tokens
            .saturating_add(self.cached_input_tokens)
            .saturating_add(self.cache_write_tokens)
    }

    pub fn effective_output(self) -> u64 {
        self.output_tokens.saturating_add(self.thought_tokens)
    }

    pub fn total(self) -> u64 {
        self.total_context_input()
            .saturating_add(self.effective_output())
    }
}

#[derive(Clone, Copy, Debug)]
struct TierRates {
    input_per_mtok: f64,
    cache_read_per_mtok: f64,
    cache_write_per_mtok: f64,
    output_per_mtok: f64,
}

impl TierRates {
    fn compute_cost(self, tokens: TokenCounts) -> f64 {
        let input = (tokens.input_tokens as f64 / TOKENS_PER_MILLION) * self.input_per_mtok;
        let cache_read =
            (tokens.cached_input_tokens as f64 / TOKENS_PER_MILLION) * self.cache_read_per_mtok;
        let cache_write =
            (tokens.cache_write_tokens as f64 / TOKENS_PER_MILLION) * self.cache_write_per_mtok;
        // thought_tokens são contabilizados junto à saída, nunca como entrada.
        let output =
            (tokens.effective_output() as f64 / TOKENS_PER_MILLION) * self.output_per_mtok;
        input + cache_read + cache_write + output
    }

    fn cache_savings(self, tokens: TokenCounts) -> f64 {
        if tokens.cached_input_tokens == 0 {
            return 0.0;
        }
        let diff = (self.input_per_mtok - self.cache_read_per_mtok).max(0.0);
        (tokens.cached_input_tokens as f64 / TOKENS_PER_MILLION) * diff
    }
}

#[derive(Clone, Copy, Debug)]
struct ModelPriceRule {
    pattern: &'static str,
    standard: TierRates,
    long_context: Option<(u64, TierRates)>,
    // Referência de modelo padrão da mesma família para estimar economia ao usar tier leve (haiku, flash, mini, nano).
    baseline_standard: Option<TierRates>,
}

const CLAUDE_SONNET_STANDARD: TierRates = TierRates {
    input_per_mtok: 3.00,
    cache_read_per_mtok: 0.30,
    cache_write_per_mtok: 3.75,
    output_per_mtok: 15.00,
};

const CLAUDE_SONNET_LONG: TierRates = TierRates {
    input_per_mtok: 6.00,
    cache_read_per_mtok: 0.60,
    cache_write_per_mtok: 7.50,
    output_per_mtok: 22.50,
};

const CLAUDE_OPUS_4_5_STANDARD: TierRates = TierRates {
    input_per_mtok: 5.00,
    cache_read_per_mtok: 0.50,
    cache_write_per_mtok: 6.25,
    output_per_mtok: 25.00,
};

const CLAUDE_OPUS_4_5_LONG: TierRates = TierRates {
    input_per_mtok: 10.00,
    cache_read_per_mtok: 1.00,
    cache_write_per_mtok: 12.50,
    output_per_mtok: 37.50,
};

const CLAUDE_OPUS_LEGACY_STANDARD: TierRates = TierRates {
    input_per_mtok: 15.00,
    cache_read_per_mtok: 1.50,
    cache_write_per_mtok: 18.75,
    output_per_mtok: 75.00,
};

const CLAUDE_HAIKU_4_5_STANDARD: TierRates = TierRates {
    input_per_mtok: 1.00,
    cache_read_per_mtok: 0.10,
    cache_write_per_mtok: 1.25,
    output_per_mtok: 5.00,
};

const CLAUDE_HAIKU_3_5_STANDARD: TierRates = TierRates {
    input_per_mtok: 0.80,
    cache_read_per_mtok: 0.08,
    cache_write_per_mtok: 1.00,
    output_per_mtok: 4.00,
};

const OPENAI_GPT5_STANDARD: TierRates = TierRates {
    input_per_mtok: 1.25,
    cache_read_per_mtok: 0.125,
    cache_write_per_mtok: 1.25,
    output_per_mtok: 10.00,
};

const OPENAI_GPT5_CODEX_STANDARD: TierRates = TierRates {
    input_per_mtok: 1.50,
    cache_read_per_mtok: 0.15,
    cache_write_per_mtok: 1.50,
    output_per_mtok: 10.00,
};

const OPENAI_GPT5_MINI_STANDARD: TierRates = TierRates {
    input_per_mtok: 0.25,
    cache_read_per_mtok: 0.025,
    cache_write_per_mtok: 0.25,
    output_per_mtok: 2.00,
};

const OPENAI_GPT5_NANO_STANDARD: TierRates = TierRates {
    input_per_mtok: 0.05,
    cache_read_per_mtok: 0.005,
    cache_write_per_mtok: 0.05,
    output_per_mtok: 0.40,
};

const OPENAI_GPT41_STANDARD: TierRates = TierRates {
    input_per_mtok: 2.00,
    cache_read_per_mtok: 0.50,
    cache_write_per_mtok: 2.00,
    output_per_mtok: 8.00,
};

const OPENAI_GPT41_MINI_STANDARD: TierRates = TierRates {
    input_per_mtok: 0.40,
    cache_read_per_mtok: 0.10,
    cache_write_per_mtok: 0.40,
    output_per_mtok: 1.60,
};

const OPENAI_GPT41_NANO_STANDARD: TierRates = TierRates {
    input_per_mtok: 0.10,
    cache_read_per_mtok: 0.025,
    cache_write_per_mtok: 0.10,
    output_per_mtok: 0.40,
};

const OPENAI_GPT4O_STANDARD: TierRates = TierRates {
    input_per_mtok: 2.50,
    cache_read_per_mtok: 1.25,
    cache_write_per_mtok: 2.50,
    output_per_mtok: 10.00,
};

const OPENAI_GPT4O_MINI_STANDARD: TierRates = TierRates {
    input_per_mtok: 0.15,
    cache_read_per_mtok: 0.075,
    cache_write_per_mtok: 0.15,
    output_per_mtok: 0.60,
};

const OPENAI_O3_STANDARD: TierRates = TierRates {
    input_per_mtok: 2.00,
    cache_read_per_mtok: 0.50,
    cache_write_per_mtok: 2.00,
    output_per_mtok: 8.00,
};

const OPENAI_O4_MINI_STANDARD: TierRates = TierRates {
    input_per_mtok: 1.10,
    cache_read_per_mtok: 0.275,
    cache_write_per_mtok: 1.10,
    output_per_mtok: 4.40,
};

const OPENAI_CODEX_MINI_STANDARD: TierRates = TierRates {
    input_per_mtok: 1.50,
    cache_read_per_mtok: 0.375,
    cache_write_per_mtok: 1.50,
    output_per_mtok: 6.00,
};

const GEMINI_PRO_STANDARD: TierRates = TierRates {
    input_per_mtok: 1.25,
    cache_read_per_mtok: 0.3125,
    cache_write_per_mtok: 1.25,
    output_per_mtok: 10.00,
};

const GEMINI_PRO_LONG: TierRates = TierRates {
    input_per_mtok: 2.50,
    cache_read_per_mtok: 0.625,
    cache_write_per_mtok: 2.50,
    output_per_mtok: 15.00,
};

const GEMINI_25_FLASH_STANDARD: TierRates = TierRates {
    input_per_mtok: 0.30,
    cache_read_per_mtok: 0.075,
    cache_write_per_mtok: 0.30,
    output_per_mtok: 2.50,
};

const GEMINI_FLASH_LITE_STANDARD: TierRates = TierRates {
    input_per_mtok: 0.10,
    cache_read_per_mtok: 0.025,
    cache_write_per_mtok: 0.10,
    output_per_mtok: 0.40,
};

const GEMINI_15_PRO_STANDARD: TierRates = TierRates {
    input_per_mtok: 1.25,
    cache_read_per_mtok: 0.3125,
    cache_write_per_mtok: 1.25,
    output_per_mtok: 5.00,
};

const GEMINI_15_PRO_LONG: TierRates = TierRates {
    input_per_mtok: 2.50,
    cache_read_per_mtok: 0.625,
    cache_write_per_mtok: 2.50,
    output_per_mtok: 10.00,
};

const GEMINI_15_FLASH_STANDARD: TierRates = TierRates {
    input_per_mtok: 0.075,
    cache_read_per_mtok: 0.01875,
    cache_write_per_mtok: 0.075,
    output_per_mtok: 0.30,
};

const GEMINI_15_FLASH_LONG: TierRates = TierRates {
    input_per_mtok: 0.15,
    cache_read_per_mtok: 0.0375,
    cache_write_per_mtok: 0.15,
    output_per_mtok: 0.60,
};

const DEEPSEEK_CHAT_STANDARD: TierRates = TierRates {
    input_per_mtok: 0.27,
    cache_read_per_mtok: 0.07,
    cache_write_per_mtok: 0.27,
    output_per_mtok: 1.10,
};

const DEEPSEEK_REASONER_STANDARD: TierRates = TierRates {
    input_per_mtok: 0.55,
    cache_read_per_mtok: 0.14,
    cache_write_per_mtok: 0.55,
    output_per_mtok: 2.19,
};

// Ordem do mais específico ao mais genérico para evitar que "gpt-4o" capture "gpt-4o-mini" ou "flash" capture "flash-lite".
const PRICING_RULES: &[ModelPriceRule] = &[
    // Anthropic / Claude
    ModelPriceRule {
        pattern: "claude-opus-4-6",
        standard: CLAUDE_OPUS_4_5_STANDARD,
        long_context: Some((DEFAULT_LONG_CONTEXT_THRESHOLD, CLAUDE_OPUS_4_5_LONG)),
        baseline_standard: None,
    },
    ModelPriceRule {
        pattern: "claude-opus-4-5",
        standard: CLAUDE_OPUS_4_5_STANDARD,
        long_context: Some((DEFAULT_LONG_CONTEXT_THRESHOLD, CLAUDE_OPUS_4_5_LONG)),
        baseline_standard: None,
    },
    ModelPriceRule {
        pattern: "claude-opus-4",
        standard: CLAUDE_OPUS_LEGACY_STANDARD,
        long_context: None,
        baseline_standard: None,
    },
    ModelPriceRule {
        pattern: "claude-3-opus",
        standard: CLAUDE_OPUS_LEGACY_STANDARD,
        long_context: None,
        baseline_standard: None,
    },
    ModelPriceRule {
        pattern: "claude-sonnet-4",
        standard: CLAUDE_SONNET_STANDARD,
        long_context: Some((DEFAULT_LONG_CONTEXT_THRESHOLD, CLAUDE_SONNET_LONG)),
        baseline_standard: None,
    },
    ModelPriceRule {
        pattern: "claude-3-7-sonnet",
        standard: CLAUDE_SONNET_STANDARD,
        long_context: Some((DEFAULT_LONG_CONTEXT_THRESHOLD, CLAUDE_SONNET_LONG)),
        baseline_standard: None,
    },
    ModelPriceRule {
        pattern: "claude-3-5-sonnet",
        standard: CLAUDE_SONNET_STANDARD,
        long_context: None,
        baseline_standard: None,
    },
    ModelPriceRule {
        pattern: "claude-haiku-4",
        standard: CLAUDE_HAIKU_4_5_STANDARD,
        long_context: None,
        baseline_standard: Some(CLAUDE_SONNET_STANDARD),
    },
    ModelPriceRule {
        pattern: "claude-3-5-haiku",
        standard: CLAUDE_HAIKU_3_5_STANDARD,
        long_context: None,
        baseline_standard: Some(CLAUDE_SONNET_STANDARD),
    },
    ModelPriceRule {
        pattern: "opus",
        standard: CLAUDE_OPUS_4_5_STANDARD,
        long_context: Some((DEFAULT_LONG_CONTEXT_THRESHOLD, CLAUDE_OPUS_4_5_LONG)),
        baseline_standard: None,
    },
    ModelPriceRule {
        pattern: "sonnet",
        standard: CLAUDE_SONNET_STANDARD,
        long_context: Some((DEFAULT_LONG_CONTEXT_THRESHOLD, CLAUDE_SONNET_LONG)),
        baseline_standard: None,
    },
    ModelPriceRule {
        pattern: "haiku",
        standard: CLAUDE_HAIKU_4_5_STANDARD,
        long_context: None,
        baseline_standard: Some(CLAUDE_SONNET_STANDARD),
    },
    // OpenAI / Codex
    ModelPriceRule {
        pattern: "gpt-5-nano",
        standard: OPENAI_GPT5_NANO_STANDARD,
        long_context: None,
        baseline_standard: Some(OPENAI_GPT5_STANDARD),
    },
    ModelPriceRule {
        pattern: "gpt-5-mini",
        standard: OPENAI_GPT5_MINI_STANDARD,
        long_context: None,
        baseline_standard: Some(OPENAI_GPT5_STANDARD),
    },
    ModelPriceRule {
        pattern: "gpt-5",
        standard: OPENAI_GPT5_CODEX_STANDARD,
        long_context: None,
        baseline_standard: None,
    },
    ModelPriceRule {
        pattern: "codex-mini",
        standard: OPENAI_CODEX_MINI_STANDARD,
        long_context: None,
        baseline_standard: Some(OPENAI_GPT5_CODEX_STANDARD),
    },
    ModelPriceRule {
        pattern: "gpt-4.1-nano",
        standard: OPENAI_GPT41_NANO_STANDARD,
        long_context: None,
        baseline_standard: Some(OPENAI_GPT41_STANDARD),
    },
    ModelPriceRule {
        pattern: "gpt-4.1-mini",
        standard: OPENAI_GPT41_MINI_STANDARD,
        long_context: None,
        baseline_standard: Some(OPENAI_GPT41_STANDARD),
    },
    ModelPriceRule {
        pattern: "gpt-4.1",
        standard: OPENAI_GPT41_STANDARD,
        long_context: None,
        baseline_standard: None,
    },
    ModelPriceRule {
        pattern: "gpt-4o-mini",
        standard: OPENAI_GPT4O_MINI_STANDARD,
        long_context: None,
        baseline_standard: Some(OPENAI_GPT4O_STANDARD),
    },
    ModelPriceRule {
        pattern: "gpt-4o",
        standard: OPENAI_GPT4O_STANDARD,
        long_context: None,
        baseline_standard: None,
    },
    ModelPriceRule {
        pattern: "o4-mini",
        standard: OPENAI_O4_MINI_STANDARD,
        long_context: None,
        baseline_standard: Some(OPENAI_O3_STANDARD),
    },
    ModelPriceRule {
        pattern: "o3-mini",
        standard: OPENAI_O4_MINI_STANDARD,
        long_context: None,
        baseline_standard: Some(OPENAI_O3_STANDARD),
    },
    ModelPriceRule {
        pattern: "o3",
        standard: OPENAI_O3_STANDARD,
        long_context: None,
        baseline_standard: None,
    },
    // Google / Gemini / Antigravity
    ModelPriceRule {
        pattern: "gemini-4-hydrogen",
        standard: GEMINI_PRO_STANDARD,
        long_context: Some((DEFAULT_LONG_CONTEXT_THRESHOLD, GEMINI_PRO_LONG)),
        baseline_standard: None,
    },
    ModelPriceRule {
        pattern: "gemini-4-neon",
        standard: GEMINI_PRO_STANDARD,
        long_context: Some((DEFAULT_LONG_CONTEXT_THRESHOLD, GEMINI_PRO_LONG)),
        baseline_standard: None,
    },
    ModelPriceRule {
        pattern: "gemini-4-argon",
        standard: GEMINI_PRO_STANDARD,
        long_context: Some((DEFAULT_LONG_CONTEXT_THRESHOLD, GEMINI_PRO_LONG)),
        baseline_standard: None,
    },
    ModelPriceRule {
        pattern: "gemini-4-carbon",
        standard: GEMINI_PRO_STANDARD,
        long_context: Some((DEFAULT_LONG_CONTEXT_THRESHOLD, GEMINI_PRO_LONG)),
        baseline_standard: None,
    },
    ModelPriceRule {
        pattern: "gemini-3.8-flash",
        standard: GEMINI_25_FLASH_STANDARD,
        long_context: None,
        baseline_standard: Some(GEMINI_PRO_STANDARD),
    },
    ModelPriceRule {
        pattern: "gemini-3-flash",
        standard: GEMINI_25_FLASH_STANDARD,
        long_context: None,
        baseline_standard: Some(GEMINI_PRO_STANDARD),
    },
    ModelPriceRule {
        pattern: "gemini-3.1-pro",
        standard: GEMINI_PRO_STANDARD,
        long_context: Some((DEFAULT_LONG_CONTEXT_THRESHOLD, GEMINI_PRO_LONG)),
        baseline_standard: None,
    },
    ModelPriceRule {
        pattern: "gemini-3-pro",
        standard: GEMINI_PRO_STANDARD,
        long_context: Some((DEFAULT_LONG_CONTEXT_THRESHOLD, GEMINI_PRO_LONG)),
        baseline_standard: None,
    },
    ModelPriceRule {
        pattern: "gemini-2.5-flash-lite",
        standard: GEMINI_FLASH_LITE_STANDARD,
        long_context: None,
        baseline_standard: Some(GEMINI_PRO_STANDARD),
    },
    ModelPriceRule {
        pattern: "gemini-2.5-flash",
        standard: GEMINI_25_FLASH_STANDARD,
        long_context: None,
        baseline_standard: Some(GEMINI_PRO_STANDARD),
    },
    ModelPriceRule {
        pattern: "gemini-2.5-pro",
        standard: GEMINI_PRO_STANDARD,
        long_context: Some((DEFAULT_LONG_CONTEXT_THRESHOLD, GEMINI_PRO_LONG)),
        baseline_standard: None,
    },
    ModelPriceRule {
        pattern: "gemini-2.0-flash",
        standard: GEMINI_FLASH_LITE_STANDARD,
        long_context: None,
        baseline_standard: Some(GEMINI_PRO_STANDARD),
    },
    ModelPriceRule {
        pattern: "gemini-1.5-pro",
        standard: GEMINI_15_PRO_STANDARD,
        long_context: Some((128_000, GEMINI_15_PRO_LONG)),
        baseline_standard: None,
    },
    ModelPriceRule {
        pattern: "gemini-1.5-flash",
        standard: GEMINI_15_FLASH_STANDARD,
        long_context: Some((128_000, GEMINI_15_FLASH_LONG)),
        baseline_standard: Some(GEMINI_15_PRO_STANDARD),
    },
    // DeepSeek
    ModelPriceRule {
        pattern: "deepseek-reasoner",
        standard: DEEPSEEK_REASONER_STANDARD,
        long_context: None,
        baseline_standard: None,
    },
    ModelPriceRule {
        pattern: "deepseek-r1",
        standard: DEEPSEEK_REASONER_STANDARD,
        long_context: None,
        baseline_standard: None,
    },
    ModelPriceRule {
        pattern: "deepseek-chat",
        standard: DEEPSEEK_CHAT_STANDARD,
        long_context: None,
        baseline_standard: Some(DEEPSEEK_REASONER_STANDARD),
    },
    ModelPriceRule {
        pattern: "deepseek-v3",
        standard: DEEPSEEK_CHAT_STANDARD,
        long_context: None,
        baseline_standard: Some(DEEPSEEK_REASONER_STANDARD),
    },
];

#[derive(Clone, Debug, PartialEq)]
pub struct PriceEstimate {
    pub cost_usd: f64,
    pub price_basis: String,
    pub is_long_context: bool,
    pub cache_savings_usd: f64,
    pub light_tier_savings_usd: f64,
}

fn normalize_model_name(model: &str) -> String {
    let trimmed = model.trim();
    // Remove prefixos como "anthropic/", "openai/", "google/", "openrouter/anthropic/".
    let bare = trimmed.rsplit('/').next().unwrap_or(trimmed);
    bare.to_ascii_lowercase()
}

fn find_rule(model: &str) -> Option<&'static ModelPriceRule> {
    let normalized = normalize_model_name(model);
    if normalized.is_empty() {
        return None;
    }
    PRICING_RULES.iter().find(|rule| {
        if rule.pattern == "opus" || rule.pattern == "sonnet" || rule.pattern == "haiku" {
            normalized == rule.pattern
                || normalized.starts_with(&format!("{}-", rule.pattern))
                || normalized.contains(&format!("-{}", rule.pattern))
        } else if rule.pattern == "o3" {
            normalized == "o3" || normalized.starts_with("o3-")
        } else {
            normalized == rule.pattern
                || normalized.starts_with(&format!("{}-", rule.pattern))
                || normalized.starts_with(&format!("{}.", rule.pattern))
                || normalized.starts_with(&format!("{}@", rule.pattern))
                || normalized.contains(rule.pattern)
        }
    })
}

/// Calcula estimativa USD equivalente à API usando a tabela local versionada.
/// Modelos desconhecidos retornam `None` (nunca `Some(0.0)`).
pub fn estimate_usage_price(model: &str, tokens: TokenCounts) -> Option<PriceEstimate> {
    let rule = find_rule(model)?;
    let context_input = tokens.total_context_input();
    let (rates, is_long_context) = match rule.long_context {
        Some((threshold, long_rates)) if context_input > threshold => (long_rates, true),
        _ => (rule.standard, false),
    };
    let cost_usd = rates.compute_cost(tokens);
    let cache_savings_usd = rates.cache_savings(tokens);
    let light_tier_savings_usd = rule
        .baseline_standard
        .map(|baseline| (baseline.compute_cost(tokens) - cost_usd).max(0.0))
        .unwrap_or(0.0);
    let tier = if is_long_context {
        "long-context"
    } else {
        "standard"
    };
    Some(PriceEstimate {
        cost_usd,
        price_basis: format!("api-table@{PRICING_VERSION}:{tier}"),
        is_long_context,
        cache_savings_usd,
        light_tier_savings_usd,
    })
}
