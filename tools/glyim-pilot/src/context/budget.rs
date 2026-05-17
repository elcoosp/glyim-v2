pub struct TokenBudget {
    pub max_tokens: usize,
    pub used_tokens: usize,
}
impl TokenBudget {
    pub fn new(max_tokens: usize) -> Self {
        Self {
            max_tokens,
            used_tokens: 0,
        }
    }
    pub fn remaining(&self) -> usize {
        self.max_tokens.saturating_sub(self.used_tokens)
    }
    pub fn try_allocate(&mut self, tokens: usize) -> bool {
        if self.used_tokens + tokens <= self.max_tokens {
            self.used_tokens += tokens;
            true
        } else {
            false
        }
    }
    pub fn force_allocate(&mut self, tokens: usize) {
        self.used_tokens += tokens;
    }
    pub fn estimate_tokens(text: &str) -> usize {
        (text.len() * 11 + 27) / 40
    }
}
