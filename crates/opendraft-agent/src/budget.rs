//! What a task may spend. A runaway loop spends the user's own key, so these
//! are the control: ceilings in code, with defaults that are deliberately low.
//! Reaching one ends the task with a named reason and keeps the conversation.

use std::time::{Duration, Instant};

use crate::types::Usage;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BudgetKind {
    ModelCalls,
    ToolCalls,
    Time,
    ResultBytes,
    Tokens,
}

impl BudgetKind {
    /// For the user: what ran out.
    pub fn describe(self) -> &'static str {
        match self {
            Self::ModelCalls => "the limit on questions to the model",
            Self::ToolCalls => "the limit on tool calls",
            Self::Time => "the time limit",
            Self::ResultBytes => "the limit on drawing data read",
            Self::Tokens => "the limit on model tokens",
        }
    }
}

/// What one task may use.
#[derive(Clone, Debug)]
pub struct Budgets {
    pub model_calls: u32,
    pub tool_calls: u32,
    pub elapsed: Duration,
    /// How long to wait for one answer from the model before giving up.
    pub model_call: Duration,
    /// Bytes of tool results kept in the conversation, in all.
    pub result_bytes: usize,
    /// The most any one result may keep; the rest is cut, visibly.
    pub result_bytes_each: usize,
    pub pictures: u32,
    /// Input plus output tokens, counted only where the provider reports them.
    pub tokens: Option<u64>,
    pub max_output_tokens: u32,
}

impl Default for Budgets {
    fn default() -> Self {
        Self {
            model_calls: 20,
            tool_calls: 60,
            elapsed: Duration::from_secs(10 * 60),
            model_call: Duration::from_secs(120),
            result_bytes: 400_000,
            result_bytes_each: 30_000,
            pictures: 10,
            tokens: None,
            max_output_tokens: 4096,
        }
    }
}

/// What a task has used so far.
#[derive(Debug)]
pub struct Meter {
    budgets: Budgets,
    started: Instant,
    model_calls: u32,
    tool_calls: u32,
    result_bytes: usize,
    pictures: u32,
    tokens: u64,
}

impl Meter {
    pub fn new(budgets: Budgets) -> Self {
        Self {
            budgets,
            started: Instant::now(),
            model_calls: 0,
            tool_calls: 0,
            result_bytes: 0,
            pictures: 0,
            tokens: 0,
        }
    }

    pub fn budgets(&self) -> &Budgets {
        &self.budgets
    }

    /// Time left, or the budget if it is spent.
    pub fn time_left(&self) -> Result<Duration, BudgetKind> {
        self.budgets
            .elapsed
            .checked_sub(self.started.elapsed())
            .filter(|left| !left.is_zero())
            .ok_or(BudgetKind::Time)
    }

    /// Before asking the model: count the call, or say what has run out.
    pub fn charge_model_call(&mut self) -> Result<(), BudgetKind> {
        self.time_left()?;
        if let Some(limit) = self.budgets.tokens {
            if self.tokens >= limit {
                return Err(BudgetKind::Tokens);
            }
        }
        if self.model_calls >= self.budgets.model_calls {
            return Err(BudgetKind::ModelCalls);
        }
        self.model_calls += 1;
        Ok(())
    }

    /// Before running a tool: count the call, or say what has run out.
    pub fn charge_tool_call(&mut self) -> Result<(), BudgetKind> {
        self.time_left()?;
        if self.tool_calls >= self.budgets.tool_calls {
            return Err(BudgetKind::ToolCalls);
        }
        self.tool_calls += 1;
        Ok(())
    }

    /// Keep `bytes` of a result in the conversation, or say it is full.
    pub fn charge_result_bytes(&mut self, bytes: usize) -> Result<(), BudgetKind> {
        let total = self.result_bytes.saturating_add(bytes);
        if total > self.budgets.result_bytes {
            return Err(BudgetKind::ResultBytes);
        }
        self.result_bytes = total;
        Ok(())
    }

    /// Show a picture, if the task has any left.
    pub fn take_picture(&mut self) -> bool {
        if self.pictures >= self.budgets.pictures {
            return false;
        }
        self.pictures += 1;
        true
    }

    pub fn add_usage(&mut self, usage: Usage) {
        self.tokens = self
            .tokens
            .saturating_add(usage.input.unwrap_or(0))
            .saturating_add(usage.output.unwrap_or(0));
    }

    pub fn model_calls(&self) -> u32 {
        self.model_calls
    }

    pub fn tool_calls(&self) -> u32 {
        self.tool_calls
    }

    pub fn tokens(&self) -> u64 {
        self.tokens
    }
}
