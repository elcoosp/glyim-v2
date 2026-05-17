pub struct TokenBudget { pub max_tokens: usize, pub used_tokens: usize }
impl TokenBudget { pub fn new(max: usize) -> Self { Self { max_tokens: max, used_tokens: 0 } } }
